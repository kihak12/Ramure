//! Répertoire de travail et index : état, diff, indexation, commit, synchronisation.

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

use super::ops::{Outcome, default_remote, guarded, operation};
use super::{CommandLog, Done, Failed, Opts, blocking, patch, step, step_with, text};

/// Au-delà, le diff est coupé : l'interface n'afficherait pas plus de toute façon.
const MAX_DIFF_BYTES: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// Ancien chemin, pour un renommage ou une copie.
    pub old_path: Option<String>,
    /// `added`, `modified`, `deleted`, `renamed`, `copied`, `typechange`, `untracked` ou `conflict`.
    pub kind: &'static str,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Commit courant, absent tant que le dépôt n'a aucun commit.
    pub head: Option<String>,
    /// Branche courante, absente si HEAD est détaché.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    /// Opération en suspens : `merge`, `rebase`, `cherry-pick` ou `revert`.
    pub operation: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub text: String,
    pub truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    /// Message complet, sujet compris.
    pub message: String,
    /// Fichiers modifiés par rapport au premier parent.
    pub files: Vec<FileChange>,
}

fn kind_of(code: u8) -> &'static str {
    match code {
        b'A' => "added",
        b'D' => "deleted",
        b'R' => "renamed",
        b'C' => "copied",
        b'T' => "typechange",
        _ => "modified",
    }
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Découpe `champ champ … chemin` : `fields` champs séparés par des espaces, puis le chemin,
/// qui peut lui-même contenir des espaces.
fn split_entry(entry: &[u8], fields: usize) -> Option<(Vec<&[u8]>, &[u8])> {
    let mut parts = entry.splitn(fields + 1, |b| *b == b' ');
    let head: Vec<&[u8]> = parts.by_ref().take(fields).collect();
    let path = parts.next()?;
    (head.len() == fields).then_some((head, path))
}

/// Parse la sortie de `git status --porcelain=v2 --branch -z`.
fn parse_status(raw: &[u8]) -> Status {
    let mut status = Status::default();
    let mut entries = raw.split(|b| *b == 0);
    while let Some(entry) = entries.next() {
        match entry.first() {
            Some(b'#') => {
                let line = utf8(entry);
                if let Some(oid) = line.strip_prefix("# branch.oid ") {
                    status.head = (oid != "(initial)").then(|| oid.to_string());
                } else if let Some(head) = line.strip_prefix("# branch.head ") {
                    status.branch = (head != "(detached)").then(|| head.to_string());
                } else if let Some(upstream) = line.strip_prefix("# branch.upstream ") {
                    status.upstream = Some(upstream.to_string());
                } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
                    let mut counts = ab.split(' ').map(|n| n[1..].parse().unwrap_or(0));
                    status.ahead = counts.next().unwrap_or(0);
                    status.behind = counts.next().unwrap_or(0);
                }
            }
            Some(b'1') | Some(b'2') => {
                let renamed = entry[0] == b'2';
                let Some((head, path)) = split_entry(entry, if renamed { 9 } else { 8 }) else { continue };
                // Pour un renommage, l'ancien chemin occupe l'entrée suivante.
                let old_path = if renamed { entries.next().map(utf8) } else { None };
                let xy = head[1];
                if xy.len() != 2 {
                    continue;
                }
                let path = utf8(path);
                if xy[0] != b'.' {
                    let moved = matches!(xy[0], b'R' | b'C');
                    status.staged.push(FileChange {
                        path: path.clone(),
                        old_path: if moved { old_path.clone() } else { None },
                        kind: kind_of(xy[0]),
                    });
                }
                if xy[1] != b'.' {
                    let moved = matches!(xy[1], b'R' | b'C');
                    status.unstaged.push(FileChange {
                        path,
                        old_path: if moved { old_path } else { None },
                        kind: kind_of(xy[1]),
                    });
                }
            }
            Some(b'u') => {
                if let Some((_, path)) = split_entry(entry, 10) {
                    status.unstaged.push(FileChange { path: utf8(path), old_path: None, kind: "conflict" });
                }
            }
            Some(b'?') => {
                if let Some(path) = entry.get(2..) {
                    status.unstaged.push(FileChange { path: utf8(path), old_path: None, kind: "untracked" });
                }
            }
            _ => {}
        }
    }
    status
}

/// Parse la sortie de `git diff --name-status -z` : `M\0chemin\0` ou `R100\0ancien\0nouveau\0`.
fn parse_name_status(raw: &[u8]) -> Vec<FileChange> {
    let mut files = Vec::new();
    let mut entries = raw.split(|b| *b == 0).filter(|e| !e.is_empty());
    while let Some(code) = entries.next() {
        let code = code[0];
        let Some(first) = entries.next() else { break };
        if matches!(code, b'R' | b'C') {
            let Some(new) = entries.next() else { break };
            files.push(FileChange { path: utf8(new), old_path: Some(utf8(first)), kind: kind_of(code) });
        } else {
            files.push(FileChange { path: utf8(first), old_path: None, kind: kind_of(code) });
        }
    }
    files
}

/// Un identifiant de commit ne contient que des chiffres hexadécimaux : rien qui puisse
/// passer pour une option de git.
pub fn check_sha(sha: &str) -> Result<(), String> {
    if (4..=64).contains(&sha.len()) && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("identifiant de commit invalide : {sha}"))
    }
}

fn with_paths<'a>(args: &[&'a str], paths: &'a [String]) -> Vec<&'a str> {
    let mut all = args.to_vec();
    all.push("--");
    all.extend(paths.iter().map(String::as_str));
    all
}

pub fn to_diff(mut raw: Vec<u8>) -> Diff {
    let truncated = raw.len() > MAX_DIFF_BYTES;
    if truncated {
        raw.truncate(MAX_DIFF_BYTES);
    }
    Diff { text: utf8(&raw), truncated }
}

pub fn status(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<Status, String> {
    let raw = step(
        journal,
        repo,
        &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"],
    )?;
    let mut status = parse_status(&raw);
    status.operation = operation(repo);
    Ok(status)
}

/// Diff d'un fichier du répertoire de travail (`staged = false`) ou de l'index (`staged = true`).
pub fn diff_file(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    file: &str,
    old_path: Option<&str>,
    staged: bool,
    untracked: bool,
) -> Result<Diff, String> {
    const COMMON: [&str; 4] = ["diff", "--no-color", "--no-ext-diff", "-M"];
    let out = if untracked {
        // Fichier inconnu de git : on le compare au vide. Le code 1 signale « différences ».
        let args = ["diff", "--no-color", "--no-ext-diff", "--no-index", "--", "/dev/null", file];
        step_with(journal, repo, &args, Opts { accepted: &[1], ..Opts::default() })?.stdout
    } else {
        let mut args = COMMON.to_vec();
        if staged {
            args.push("--cached");
        }
        args.push("--");
        // Les deux chemins sont nécessaires pour que git reconnaisse le renommage.
        args.extend(old_path);
        args.push(file);
        step(journal, repo, &args)?
    };
    Ok(to_diff(out))
}

pub fn commit_detail(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    sha: &str,
    parent: Option<&str>,
) -> Result<CommitDetail, String> {
    check_sha(sha)?;
    let message = text(&step(journal, repo, &["show", "-s", "--format=%B", sha])?);
    let raw = match parent {
        Some(parent) => {
            check_sha(parent)?;
            step(journal, repo, &["diff", "--name-status", "-z", "-M", parent, sha])?
        }
        // Premier commit : tout son contenu est un ajout.
        None => step(
            journal,
            repo,
            &["diff-tree", "--root", "--no-commit-id", "-r", "--name-status", "-z", "-M", sha],
        )?,
    };
    Ok(CommitDetail { message, files: parse_name_status(&raw) })
}

pub fn diff_commit(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    sha: &str,
    parent: Option<&str>,
    file: &str,
    old_path: Option<&str>,
) -> Result<Diff, String> {
    check_sha(sha)?;
    let mut args = match parent {
        Some(parent) => {
            check_sha(parent)?;
            vec!["diff", "--no-color", "--no-ext-diff", "-M", parent, sha, "--"]
        }
        None => vec!["diff-tree", "--root", "--no-commit-id", "-p", "--no-color", "--no-ext-diff", "-M", sha, "--"],
    };
    args.extend(old_path);
    args.push(file);
    Ok(to_diff(step(journal, repo, &args)?))
}

pub fn stage(journal: &mut Vec<CommandLog>, repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    step(journal, repo, &with_paths(&["add", "-A"], paths)).map(drop)
}

pub fn unstage(journal: &mut Vec<CommandLog>, repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    // `git reset` fonctionne aussi dans un dépôt sans commit, contrairement à `git restore --staged`.
    step(journal, repo, &with_paths(&["reset", "-q"], paths)).map(drop)
}

/// Abandonne les modifications non indexées : restaure les fichiers suivis, supprime les nouveaux.
pub fn discard(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    tracked: &[String],
    untracked: &[String],
) -> Result<(), String> {
    if !tracked.is_empty() {
        step(journal, repo, &with_paths(&["restore", "--worktree"], tracked))?;
    }
    if !untracked.is_empty() {
        step(journal, repo, &with_paths(&["clean", "-f", "-q"], untracked))?;
    }
    Ok(())
}

/// Indexe (`staged = false`) ou retire de l'index (`staged = true`) les lignes choisies
/// dans le diff d'un fichier. `lines` : indices dans `diff.split('\n')`.
pub fn apply_lines(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    diff: &str,
    lines: &[usize],
    staged: bool,
) -> Result<(), String> {
    let selected: HashSet<usize> = lines.iter().copied().collect();
    let patch = patch::partial_patch(diff, &selected, staged)?;
    let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
    if staged {
        args.push("--reverse");
    }
    args.push("-");
    step_with(journal, repo, &args, Opts { input: Some(patch.as_bytes()), ..Opts::default() }).map(drop)
}

/// Crée un commit, ou remplace le dernier si `amend` est vrai.
pub fn commit(journal: &mut Vec<CommandLog>, repo: &Path, message: &str, amend: bool) -> Result<(), String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Le message de commit est vide.".into());
    }
    let mut args = vec!["commit", "--cleanup=strip"];
    if amend {
        args.push("--amend");
    }
    // Le message passe par l'entrée standard : aucune limite de longueur ni d'échappement.
    args.extend(["-F", "-"]);
    step_with(journal, repo, &args, Opts { input: Some(message.as_bytes()), ..Opts::default() }).map(drop)
}

/// Message complet du dernier commit, pour préremplir un amend.
pub fn head_message(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<String, String> {
    step(journal, repo, &["log", "-1", "--format=%B"]).map(|out| text(&out))
}

/// `fetch`, `pull`, `push` ou `push-force`. `pull_mode` : `ff-only` (par défaut), `rebase` ou `merge`.
pub fn sync(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    action: &str,
    pull_mode: Option<&str>,
) -> Result<Outcome, String> {
    let plain = Opts::default();
    let result = match action {
        "fetch" => step_with(journal, repo, &["fetch", "--all", "--prune"], plain),
        "pull" => {
            let flag = match pull_mode.unwrap_or("ff-only") {
                "ff-only" => "--ff-only",
                "rebase" => "--rebase",
                "merge" => "--no-rebase",
                other => return Err(format!("stratégie de pull inconnue : {other}")),
            };
            step_with(journal, repo, &["pull", flag], plain)
        }
        "push" | "push-force" => {
            let current = status(journal, repo)?;
            if current.branch.is_none() {
                return Err("HEAD est détaché : il n'y a pas de branche à pousser.".into());
            }
            // `--force-with-lease` refuse d'écraser un travail distant qu'on n'a pas encore récupéré.
            let force: &[&str] = if action == "push-force" { &["--force-with-lease"] } else { &[] };
            let mut args = vec!["push"];
            args.extend(force);
            let remote;
            if current.upstream.is_none() {
                // Première publication de la branche : on la relie à son homologue distante.
                remote = default_remote(journal, repo)?;
                args.extend(["-u", &remote, "HEAD"]);
            }
            step_with(journal, repo, &args, plain)
        }
        other => return Err(format!("action inconnue : {other}")),
    };
    // Un pull en mode rebase ou fusion peut s'arrêter sur un conflit.
    guarded(repo, result)
}

// --- Commandes exposées à l'interface -----------------------------------------------------

type Reply<T> = Result<Done<T>, Failed>;

#[tauri::command]
pub async fn git_status(repo: String) -> Reply<Status> {
    blocking(move |j| status(j, Path::new(&repo))).await
}

#[tauri::command]
pub async fn git_diff_file(
    repo: String,
    path: String,
    old_path: Option<String>,
    staged: bool,
    untracked: bool,
) -> Reply<Diff> {
    blocking(move |j| diff_file(j, Path::new(&repo), &path, old_path.as_deref(), staged, untracked)).await
}

#[tauri::command]
pub async fn git_commit_detail(repo: String, sha: String, parent: Option<String>) -> Reply<CommitDetail> {
    blocking(move |j| commit_detail(j, Path::new(&repo), &sha, parent.as_deref())).await
}

#[tauri::command]
pub async fn git_diff_commit(
    repo: String,
    sha: String,
    parent: Option<String>,
    path: String,
    old_path: Option<String>,
) -> Reply<Diff> {
    blocking(move |j| {
        diff_commit(j, Path::new(&repo), &sha, parent.as_deref(), &path, old_path.as_deref())
    })
    .await
}

/// Les commandes qui modifient l'index renvoient le nouvel état : un seul aller-retour.
#[tauri::command]
pub async fn git_stage(repo: String, paths: Vec<String>) -> Reply<Status> {
    blocking(move |j| {
        let repo = Path::new(&repo);
        stage(j, repo, &paths)?;
        status(j, repo)
    })
    .await
}

#[tauri::command]
pub async fn git_unstage(repo: String, paths: Vec<String>) -> Reply<Status> {
    blocking(move |j| {
        let repo = Path::new(&repo);
        unstage(j, repo, &paths)?;
        status(j, repo)
    })
    .await
}

#[tauri::command]
pub async fn git_discard(repo: String, tracked: Vec<String>, untracked: Vec<String>) -> Reply<Status> {
    blocking(move |j| {
        let repo = Path::new(&repo);
        discard(j, repo, &tracked, &untracked)?;
        status(j, repo)
    })
    .await
}

#[tauri::command]
pub async fn git_apply_lines(repo: String, diff: String, lines: Vec<usize>, staged: bool) -> Reply<Status> {
    blocking(move |j| {
        let repo = Path::new(&repo);
        apply_lines(j, repo, &diff, &lines, staged)?;
        status(j, repo)
    })
    .await
}

#[tauri::command]
pub async fn git_commit(repo: String, message: String, amend: bool) -> Reply<()> {
    blocking(move |j| commit(j, Path::new(&repo), &message, amend)).await
}

#[tauri::command]
pub async fn git_head_message(repo: String) -> Reply<String> {
    blocking(move |j| head_message(j, Path::new(&repo))).await
}

#[tauri::command]
pub async fn git_sync(repo: String, action: String, pull_mode: Option<String>) -> Reply<Outcome> {
    blocking(move |j| sync(j, Path::new(&repo), &action, pull_mode.as_deref())).await
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn status_v2_is_parsed() {
        let raw = b"# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 aaa bbb src/a b.rs\0\
1 MM N... 100644 100644 100644 aaa bbb both.rs\0\
2 R. N... 100644 100644 100644 aaa bbb R100 new name.rs\0old name.rs\0\
u UU N... 100644 100644 100644 100644 aaa bbb ccc clash.rs\0\
? notes/todo.md\0";
        let s = parse_status(raw);
        assert_eq!(s.head.as_deref(), Some("abc"));
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (2, 1));

        let staged: Vec<_> = s.staged.iter().map(|f| (f.path.as_str(), f.kind)).collect();
        assert_eq!(staged, vec![("both.rs", "modified"), ("new name.rs", "renamed")]);
        assert_eq!(s.staged[1].old_path.as_deref(), Some("old name.rs"));

        let unstaged: Vec<_> = s.unstaged.iter().map(|f| (f.path.as_str(), f.kind)).collect();
        assert_eq!(
            unstaged,
            vec![
                ("src/a b.rs", "modified"),
                ("both.rs", "modified"),
                ("clash.rs", "conflict"),
                ("notes/todo.md", "untracked"),
            ]
        );
    }

    #[test]
    fn detached_head_has_no_branch() {
        let s = parse_status(b"# branch.oid (initial)\0# branch.head (detached)\0");
        assert_eq!(s, Status::default());
    }

    #[test]
    fn name_status_is_parsed() {
        let files = parse_name_status(b"M\0a.rs\0R087\0old.rs\0new.rs\0D\0gone.rs\0");
        assert_eq!(files.len(), 3);
        assert_eq!(files[1].kind, "renamed");
        assert_eq!(files[1].path, "new.rs");
        assert_eq!(files[1].old_path.as_deref(), Some("old.rs"));
        assert_eq!(files[2].kind, "deleted");
    }

    #[test]
    fn sha_must_be_hexadecimal() {
        assert!(check_sha("959c37f").is_ok());
        assert!(check_sha("--output=x").is_err());
        assert!(check_sha("").is_err());
    }

    /// Dépôt jetable dans le dossier temporaire, supprimé à la fin du test.
    pub struct Scratch(pub PathBuf);

    impl Scratch {
        pub fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ramure-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            let scratch = Scratch(dir);
            scratch.git(&["init", "-q", "-b", "main"]);
            scratch.git(&["config", "user.name", "Test"]);
            scratch.git(&["config", "user.email", "test@example.org"]);
            scratch.git(&["config", "commit.gpgsign", "false"]);
            scratch
        }
        pub fn git(&self, args: &[&str]) -> String {
            text(&super::super::run(&self.0, args).unwrap())
        }
        pub fn write(&self, name: &str, content: &str) {
            fs::write(self.0.join(name), content).unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stage_commit_and_inspect() {
        let repo = Scratch::new("commit");
        let dir = repo.0.as_path();
        let mut j = Vec::new();

        // Dépôt sans commit : indexer puis désindexer doit fonctionner.
        repo.write("a.txt", "un\n");
        repo.write("b c.txt", "deux\n");
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.unstaged.len(), 2);
        assert!(s.unstaged.iter().all(|f| f.kind == "untracked"));
        let untracked = diff_file(&mut j, dir, "b c.txt", None, false, true).unwrap();
        assert!(untracked.text.contains("+deux"));

        stage(&mut j, dir, &["a.txt".into(), "b c.txt".into()]).unwrap();
        unstage(&mut j, dir, &["b c.txt".into()]).unwrap();
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.staged.len(), 1);
        assert_eq!(s.staged[0].kind, "added");
        assert_eq!(s.unstaged.len(), 1);

        assert!(commit(&mut j, dir, "   ", false).is_err());
        commit(&mut j, dir, "premier commit\n\ncorps du message", false).unwrap();
        let first = repo.git(&["rev-parse", "HEAD"]);
        let detail = commit_detail(&mut j, dir, &first, None).unwrap();
        assert_eq!(detail.message, "premier commit\n\ncorps du message");
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].kind, "added");

        // Modification, renommage et second commit.
        repo.write("a.txt", "un\nbis\n");
        let d = diff_file(&mut j, dir, "a.txt", None, false, false).unwrap();
        assert!(d.text.contains("+bis") && !d.truncated);
        stage(&mut j, dir, &["a.txt".into(), "b c.txt".into()]).unwrap();
        let d = diff_file(&mut j, dir, "a.txt", None, true, false).unwrap();
        assert!(d.text.contains("+bis"));
        commit(&mut j, dir, "second", false).unwrap();
        let second = repo.git(&["rev-parse", "HEAD"]);
        let detail = commit_detail(&mut j, dir, &second, Some(&first)).unwrap();
        assert_eq!(detail.files.len(), 2);
        let d = diff_commit(&mut j, dir, &second, Some(&first), "a.txt", None).unwrap();
        assert!(d.text.contains("+bis"));
        let d = diff_commit(&mut j, dir, &first, None, "a.txt", None).unwrap();
        assert!(d.text.contains("+un"));

        // Abandon : le fichier suivi retrouve son contenu, le nouveau disparaît.
        repo.write("a.txt", "cassé\n");
        repo.write("neuf.txt", "x\n");
        discard(&mut j, dir, &["a.txt".into()], &["neuf.txt".into()]).unwrap();
        let s = status(&mut j, dir).unwrap();
        assert!(s.staged.is_empty() && s.unstaged.is_empty());
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "un\nbis\n");
        assert!(j.iter().all(|c| c.ok || c.command.starts_with("git commit")));
    }

    #[test]
    fn push_pull_and_fetch_between_clones() {
        let origin = Scratch::new("origin");
        origin.git(&["config", "receive.denyCurrentBranch", "ignore"]);
        origin.write("a.txt", "un\n");
        origin.git(&["add", "-A"]);
        origin.git(&["commit", "-q", "-m", "init"]);

        let clone = Scratch::new("clone");
        let dir = clone.0.as_path();
        clone.git(&["remote", "add", "origin", origin.0.to_str().unwrap()]);
        clone.git(&["pull", "-q", "origin", "main"]);
        let mut j = Vec::new();

        // Branche sans amont : le premier push la publie et la relie.
        clone.git(&["checkout", "-q", "-b", "sujet"]);
        clone.write("b.txt", "deux\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "ajout de b", false).unwrap();
        assert!(status(&mut j, dir).unwrap().upstream.is_none());
        sync(&mut j, dir, "push", None).unwrap();
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.upstream.as_deref(), Some("origin/sujet"));
        assert_eq!((s.ahead, s.behind), (0, 0));
        assert_eq!(origin.git(&["log", "-1", "--format=%s", "sujet"]), "ajout de b");

        // Un commit arrive côté distant : fetch le signale, pull l'intègre.
        origin.git(&["checkout", "-q", "sujet"]);
        origin.write("c.txt", "trois\n");
        origin.git(&["add", "-A"]);
        origin.git(&["commit", "-q", "-m", "distant"]);
        sync(&mut j, dir, "fetch", None).unwrap();
        assert_eq!(status(&mut j, dir).unwrap().behind, 1);
        sync(&mut j, dir, "pull", None).unwrap();
        assert_eq!(status(&mut j, dir).unwrap().behind, 0);
        assert!(dir.join("c.txt").exists());

        assert!(sync(&mut j, dir, "rebase", None).is_err());

        // Amend puis push forcé : le push simple est refusé, le forcé passe.
        assert_eq!(head_message(&mut j, dir).unwrap(), "distant");
        clone.write("c.txt", "trois, corrigé\n");
        stage(&mut j, dir, &["c.txt".into()]).unwrap();
        commit(&mut j, dir, "distant, corrigé", true).unwrap();
        assert!(sync(&mut j, dir, "push", None).is_err());
        sync(&mut j, dir, "push-force", None).unwrap();
        assert_eq!(origin.git(&["log", "-1", "--format=%s", "sujet"]), "distant, corrigé");
    }

    #[test]
    fn single_lines_are_staged_and_unstaged() {
        let repo = Scratch::new("lines");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        repo.write("f.txt", "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n");
        stage(&mut j, dir, &["f.txt".into()]).unwrap();
        commit(&mut j, dir, "init", false).unwrap();

        // Deux modifications éloignées : « b » remplacé, « k » supprimé.
        repo.write("f.txt", "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nl\n");
        let diff = diff_file(&mut j, dir, "f.txt", None, false, false).unwrap().text;
        let index_of = |needle: &str| diff.split('\n').position(|l| l == needle).unwrap();

        // On n'indexe que la suppression de « k ».
        apply_lines(&mut j, dir, &diff, &[index_of("-k")], false).unwrap();
        assert_eq!(repo.git(&["show", ":f.txt"]), "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nl");
        let s = status(&mut j, dir).unwrap();
        assert_eq!((s.staged.len(), s.unstaged.len()), (1, 1));

        // Puis le remplacement de « b », et on retire la suppression de « k » de l'index.
        let diff = diff_file(&mut j, dir, "f.txt", None, false, false).unwrap().text;
        let at = |needle: &str| diff.split('\n').position(|l| l == needle).unwrap();
        apply_lines(&mut j, dir, &diff, &[at("-b"), at("+B")], false).unwrap();
        let cached = diff_file(&mut j, dir, "f.txt", None, true, false).unwrap().text;
        let at = |needle: &str| cached.split('\n').position(|l| l == needle).unwrap();
        apply_lines(&mut j, dir, &cached, &[at("-k")], true).unwrap();
        assert_eq!(repo.git(&["show", ":f.txt"]), "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl");

        assert!(apply_lines(&mut j, dir, &cached, &[], true).is_err());
    }
}
