//! A file in conflict: reading its versions for the three-way editor, then writing the
//! result.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::work::{Status, status};
use super::{CommandLog, Done, Failed, Opts, blocking, exec, step};

/// Beyond this size, the file is not opened in the editor: it is resolved by keeping one side.
const MAX_BYTES: usize = 2_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    /// Common ancestor; absent if both sides created the file.
    pub base: Option<String>,
    /// The “ours” version; absent if that side deleted the file.
    pub ours: Option<String>,
    /// The “theirs” version; absent if that side deleted the file.
    pub theirs: Option<String>,
    /// The file in the working tree, with the conflict markers of git.
    pub merged: Option<String>,
    /// True if a version is not UTF-8 text, or is too large.
    pub binary: bool,
}

/// A file path in the repository: relative, never going up (`..`).
fn inside(repo: &Path, file: &str) -> Result<PathBuf, String> {
    let relative = Path::new(file);
    let plain = relative.components().all(|c| matches!(c, Component::Normal(_)));
    if file.is_empty() || !plain {
        return Err(format!("Invalid path: “{file}”. The path must stay inside the repository."));
    }
    Ok(repo.join(relative))
}

/// Content of a version in the index (`1` ancestor, `2` ours, `3` theirs), if it exists.
fn stage(repo: &Path, number: u8, file: &str) -> Option<Vec<u8>> {
    let spec = format!(":{number}:{file}");
    let out = exec(repo, &["show", &spec], Opts::default()).ok()?;
    out.status.success().then_some(out.stdout)
}

pub fn read(commands: &mut Vec<CommandLog>, repo: &Path, file: &str) -> Result<ConflictFile, String> {
    let path = inside(repo, file)?;
    // Safeguard: only a file that git considers in conflict opens here.
    if step(commands, repo, &["ls-files", "--unmerged", "--", file])?.is_empty() {
        return Err(format!("The file “{file}” is not in conflict."));
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

/// Writes the result of the resolution, then marks the file as resolved.
pub fn write(commands: &mut Vec<CommandLog>, repo: &Path, file: &str, content: &str) -> Result<(), String> {
    let path = inside(repo, file)?;
    if step(commands, repo, &["ls-files", "--unmerged", "--", file])?.is_empty() {
        return Err(format!("The file “{file}” is not in conflict."));
    }
    // A symbolic link would make the write land outside the repository.
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(format!("The file “{file}” is a symbolic link. Resolve it by keeping one side."));
    }
    fs::write(&path, content).map_err(|e| format!("Could not write the file “{file}”: {e}"))?;
    step(commands, repo, &["add", "--", file]).map(drop)
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
        let repo = Path::new("/repo");
        assert!(inside(repo, "src/a.rs").is_ok());
        assert!(inside(repo, "../elsewhere").is_err());
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
        save("one\ntwo\nthree\n", "base", &mut j);
        let base = repo.git(&["rev-parse", "HEAD"]);
        save("one\nTWO\nthree\n", "mine", &mut j);
        ref_op(&mut j, dir, &RefOp::BranchCreate { name: "other".into(), start: base, switch: true }).unwrap();
        save("one\ntwo, too\nthree\n", "theirs", &mut j);
        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();

        // Outside a conflict, nothing is read or written.
        assert!(read(&mut j, dir, "a.txt").is_err());
        assert!(write(&mut j, dir, "a.txt", "x").is_err());

        assert_eq!(history(&mut j, dir, &HistoryOp::Merge { target: "other".into() }).unwrap().stopped, Some("merge"));
        let file = read(&mut j, dir, "a.txt").unwrap();
        assert!(!file.binary);
        assert_eq!(file.base.as_deref(), Some("one\ntwo\nthree\n"));
        assert_eq!(file.ours.as_deref(), Some("one\nTWO\nthree\n"));
        assert_eq!(file.theirs.as_deref(), Some("one\ntwo, too\nthree\n"));
        let merged = file.merged.unwrap();
        assert!(merged.contains("<<<<<<< ") && merged.contains(">>>>>>> other"));

        assert!(write(&mut j, dir, "../outside.txt", "x").is_err());
        write(&mut j, dir, "a.txt", "one\nTWO, too\nthree\n").unwrap();
        let s = status(&mut j, dir).unwrap();
        assert!(s.unstaged.is_empty() && s.staged.len() == 1);
        assert_eq!(operate(&mut j, dir, "continue").unwrap().stopped, None);
        assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "one\nTWO, too\nthree");
    }
}
