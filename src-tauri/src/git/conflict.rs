//! Fichier en conflit : lecture de ses versions pour l'éditeur à trois voies, puis écriture
//! du résultat.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::work::{Status, status};
use super::{CommandLog, Done, Failed, Opts, blocking, exec, step};

/// Au-delà, le fichier n'est pas ouvert dans l'éditeur : il se résout en gardant un côté.
const MAX_BYTES: usize = 2_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    /// Ancêtre commun ; absent si les deux côtés ont créé le fichier.
    pub base: Option<String>,
    /// Version « ours » ; absente si ce côté a supprimé le fichier.
    pub ours: Option<String>,
    /// Version « theirs » ; absente si ce côté a supprimé le fichier.
    pub theirs: Option<String>,
    /// Fichier du répertoire de travail, avec les marqueurs de conflit de git.
    pub merged: Option<String>,
    /// Vrai si une version n'est pas du texte UTF-8, ou est trop volumineuse.
    pub binary: bool,
}

/// Un chemin de fichier du dépôt : relatif, sans remontée (`..`).
fn inside(repo: &Path, file: &str) -> Result<PathBuf, String> {
    let relative = Path::new(file);
    let plain = relative.components().all(|c| matches!(c, Component::Normal(_)));
    if file.is_empty() || !plain {
        return Err(format!("chemin invalide : « {file} »"));
    }
    Ok(repo.join(relative))
}

/// Contenu d'une version indexée (`1` ancêtre, `2` ours, `3` theirs), si elle existe.
fn stage(repo: &Path, number: u8, file: &str) -> Option<Vec<u8>> {
    let spec = format!(":{number}:{file}");
    let out = exec(repo, &["show", &spec], Opts::default()).ok()?;
    out.status.success().then_some(out.stdout)
}

pub fn read(journal: &mut Vec<CommandLog>, repo: &Path, file: &str) -> Result<ConflictFile, String> {
    let path = inside(repo, file)?;
    // Garde-fou : seul un fichier que git considère en conflit s'ouvre ici.
    if step(journal, repo, &["ls-files", "--unmerged", "--", file])?.is_empty() {
        return Err(format!("« {file} » n'est pas en conflit."));
    }
    let mut binary = false;
    let mut decode = |bytes: Option<Vec<u8>>| -> Option<String> {
        let bytes = bytes?;
        if bytes.len() > MAX_BYTES {
            binary = true;
            return None;
        }
        match String::from_utf8(bytes) {
            Ok(text) => Some(text),
            Err(_) => {
                binary = true;
                None
            }
        }
    };
    let base = decode(stage(repo, 1, file));
    let ours = decode(stage(repo, 2, file));
    let theirs = decode(stage(repo, 3, file));
    let merged = decode(fs::read(&path).ok());
    Ok(ConflictFile { base, ours, theirs, merged, binary })
}

/// Écrit le résultat de la résolution, puis marque le fichier comme résolu.
pub fn write(journal: &mut Vec<CommandLog>, repo: &Path, file: &str, content: &str) -> Result<(), String> {
    let path = inside(repo, file)?;
    if step(journal, repo, &["ls-files", "--unmerged", "--", file])?.is_empty() {
        return Err(format!("« {file} » n'est pas en conflit."));
    }
    // Un lien symbolique ferait écrire ailleurs que dans le dépôt.
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(format!("« {file} » est un lien symbolique : résous-le en gardant un côté."));
    }
    fs::write(&path, content).map_err(|e| format!("impossible d'écrire « {file} » : {e}"))?;
    step(journal, repo, &["add", "--", file]).map(drop)
}

type Reply<T> = Result<Done<T>, Failed>;

#[tauri::command]
pub async fn git_conflict_read(repo: String, path: String) -> Reply<ConflictFile> {
    blocking(move |j| read(j, Path::new(&repo), &path)).await
}

#[tauri::command]
pub async fn git_conflict_write(repo: String, path: String, content: String) -> Reply<Status> {
    blocking(move |j| {
        let repo = Path::new(&repo);
        write(j, repo, &path, &content)?;
        status(j, repo)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::ops::{HistoryOp, RefOp, history, operate, ref_op};
    use crate::git::work::tests::Scratch;
    use crate::git::work::{commit, stage as stage_files};

    #[test]
    fn paths_must_stay_inside_the_repository() {
        let repo = Path::new("/depot");
        assert!(inside(repo, "src/a.rs").is_ok());
        assert!(inside(repo, "../ailleurs").is_err());
        assert!(inside(repo, "/etc/passwd").is_err());
        assert!(inside(repo, "a/../../b").is_err());
        assert!(inside(repo, "").is_err());
    }

    #[test]
    fn a_conflict_is_read_then_resolved_by_hand() {
        let repo = Scratch::new("conflict");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        let save = |content: &str, message: &str, j: &mut Vec<CommandLog>| {
            repo.write("a.txt", content);
            stage_files(j, dir, &["a.txt".into()]).unwrap();
            commit(j, dir, message, false).unwrap();
        };
        save("un\ndeux\ntrois\n", "base", &mut j);
        let base = repo.git(&["rev-parse", "HEAD"]);
        save("un\nDEUX\ntrois\n", "le mien", &mut j);
        ref_op(&mut j, dir, &RefOp::BranchCreate { name: "autre".into(), start: base, switch: true }).unwrap();
        save("un\ndeux bis\ntrois\n", "le leur", &mut j);
        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();

        // Hors conflit, rien ne se lit ni ne s'écrit.
        assert!(read(&mut j, dir, "a.txt").is_err());
        assert!(write(&mut j, dir, "a.txt", "x").is_err());

        assert_eq!(history(&mut j, dir, &HistoryOp::Merge { target: "autre".into() }).unwrap().stopped, Some("merge"));
        let file = read(&mut j, dir, "a.txt").unwrap();
        assert!(!file.binary);
        assert_eq!(file.base.as_deref(), Some("un\ndeux\ntrois\n"));
        assert_eq!(file.ours.as_deref(), Some("un\nDEUX\ntrois\n"));
        assert_eq!(file.theirs.as_deref(), Some("un\ndeux bis\ntrois\n"));
        let merged = file.merged.unwrap();
        assert!(merged.contains("<<<<<<< ") && merged.contains(">>>>>>> autre"));

        assert!(write(&mut j, dir, "../dehors.txt", "x").is_err());
        write(&mut j, dir, "a.txt", "un\nDEUX bis\ntrois\n").unwrap();
        let s = status(&mut j, dir).unwrap();
        assert!(s.unstaged.is_empty() && s.staged.len() == 1);
        assert_eq!(operate(&mut j, dir, "continue").unwrap().stopped, None);
        assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "un\nDEUX bis\ntrois");
    }
}
