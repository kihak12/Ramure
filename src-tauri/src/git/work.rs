//! Working tree and index: status, diff, staging, commit, synchronization.

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

use super::ops::{Outcome, default_remote, guarded, operation};
use super::{CommandLog, Done, Failed, Opts, blocking, patch, step, step_with, text};

/// Beyond this, the diff is cut: the interface would not show more anyway.
const MAX_DIFF_BYTES: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// Old path, for a rename or a copy.
    pub old_path: Option<String>,
    /// `added`, `modified`, `deleted`, `renamed`, `copied`, `typechange`, `untracked` or `conflict`.
    pub kind: &'static str,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Current commit, absent as long as the repository has no commit.
    pub head: Option<String>,
    /// Current branch, absent if HEAD is detached.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    /// Operation that stopped: `merge`, `rebase`, `cherry-pick` or `revert`.
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
    /// Whole message, subject included.
    pub message: String,
    /// Files changed relative to the first parent.
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

/// Splits `field field … path`: `fields` fields separated by spaces, then the path, which may
/// itself contain spaces.
fn split_entry(entry: &[u8], fields: usize) -> Option<(Vec<&[u8]>, &[u8])> {
    let mut parts = entry.splitn(fields + 1, |b| *b == b' ');
    let head: Vec<&[u8]> = parts.by_ref().take(fields).collect();
    let path = parts.next()?;
    (head.len() == fields).then_some((head, path))
}

/// Parses the output of `git status --porcelain=v2 --branch -z`.
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
                // For a rename, the old path takes the next entry.
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

/// Parses the output of `git diff --name-status -z`: `M\0path\0` or `R100\0old\0new\0`.
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

/// A commit hash holds only hexadecimal digits: nothing that could pass for an option of git.
pub fn check_sha(sha: &str) -> Result<(), String> {
    if (4..=64).contains(&sha.len()) && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("Invalid commit SHA: “{sha}”. A SHA holds 4 to 64 hexadecimal digits."))
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

/// Diff of a file of the working tree (`staged = false`) or of the index (`staged = true`).
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
        // File unknown to git: it is compared with nothing. Code 1 means “differences”.
        let args = ["diff", "--no-color", "--no-ext-diff", "--no-index", "--", "/dev/null", file];
        step_with(journal, repo, &args, Opts { accepted: &[1], ..Opts::default() })?.stdout
    } else {
        let mut args = COMMON.to_vec();
        if staged {
            args.push("--cached");
        }
        args.push("--");
        // Both paths are needed for git to recognize the rename.
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
        // First commit: all of its content is an addition.
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
    // `git reset` also works in a repository without commits, unlike `git restore --staged`.
    step(journal, repo, &with_paths(&["reset", "-q"], paths)).map(drop)
}

/// Discards the unstaged changes: restores the tracked files, deletes the new ones.
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

/// Stages (`staged = false`) or unstages (`staged = true`) the lines chosen in the diff of a
/// file. `lines`: indices in `diff.split('\n')`.
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

/// Creates a commit, or replaces the last one if `amend` is true.
pub fn commit(journal: &mut Vec<CommandLog>, repo: &Path, message: &str, amend: bool) -> Result<(), String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("The commit message is empty. Write one, then commit again.".into());
    }
    let mut args = vec!["commit", "--cleanup=strip"];
    if amend {
        args.push("--amend");
    }
    // The message goes through the standard input: no length limit and no escaping.
    args.extend(["-F", "-"]);
    step_with(journal, repo, &args, Opts { input: Some(message.as_bytes()), ..Opts::default() }).map(drop)
}

/// Whole message of the last commit, to prefill an amend.
pub fn head_message(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<String, String> {
    step(journal, repo, &["log", "-1", "--format=%B"]).map(|out| text(&out))
}

/// `fetch`, `pull`, `push` or `push-force`. `pull_mode`: `ff-only` (the default), `rebase` or `merge`.
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
                other => return Err(format!("Unknown pull mode: “{other}”. Use ff-only, rebase or merge.")),
            };
            step_with(journal, repo, &["pull", flag], plain)
        }
        "push" | "push-force" => {
            let current = status(journal, repo)?;
            if current.branch.is_none() {
                return Err("HEAD is detached: there is no branch to push. Switch to a branch first.".into());
            }
            // `--force-with-lease` refuses to overwrite remote work that has not been fetched yet.
            let force: &[&str] = if action == "push-force" { &["--force-with-lease"] } else { &[] };
            let mut args = vec!["push"];
            args.extend(force);
            let remote;
            if current.upstream.is_none() {
                // First publication of the branch: it is linked to its remote counterpart.
                remote = default_remote(journal, repo)?;
                args.extend(["-u", &remote, "HEAD"]);
            }
            step_with(journal, repo, &args, plain)
        }
        other => return Err(format!("Unknown action: “{other}”.")),
    };
    // A pull in rebase or merge mode can stop on a conflict.
    guarded(repo, result)
}

// --- Commands exposed to the interface ----------------------------------------------------

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

/// The commands that change the index return the new status: a single round trip.
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

    /// Throwaway repository in the temporary folder, deleted at the end of the test.
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

        // Repository without commits: staging then unstaging must work.
        repo.write("a.txt", "one\n");
        repo.write("b c.txt", "two\n");
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.unstaged.len(), 2);
        assert!(s.unstaged.iter().all(|f| f.kind == "untracked"));
        let untracked = diff_file(&mut j, dir, "b c.txt", None, false, true).unwrap();
        assert!(untracked.text.contains("+two"));

        stage(&mut j, dir, &["a.txt".into(), "b c.txt".into()]).unwrap();
        unstage(&mut j, dir, &["b c.txt".into()]).unwrap();
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.staged.len(), 1);
        assert_eq!(s.staged[0].kind, "added");
        assert_eq!(s.unstaged.len(), 1);

        assert!(commit(&mut j, dir, "   ", false).is_err());
        commit(&mut j, dir, "first commit\n\nbody of the message", false).unwrap();
        let first = repo.git(&["rev-parse", "HEAD"]);
        let detail = commit_detail(&mut j, dir, &first, None).unwrap();
        assert_eq!(detail.message, "first commit\n\nbody of the message");
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].kind, "added");

        // Change, rename and second commit.
        repo.write("a.txt", "one\nmore\n");
        let d = diff_file(&mut j, dir, "a.txt", None, false, false).unwrap();
        assert!(d.text.contains("+more") && !d.truncated);
        stage(&mut j, dir, &["a.txt".into(), "b c.txt".into()]).unwrap();
        let d = diff_file(&mut j, dir, "a.txt", None, true, false).unwrap();
        assert!(d.text.contains("+more"));
        commit(&mut j, dir, "second", false).unwrap();
        let second = repo.git(&["rev-parse", "HEAD"]);
        let detail = commit_detail(&mut j, dir, &second, Some(&first)).unwrap();
        assert_eq!(detail.files.len(), 2);
        let d = diff_commit(&mut j, dir, &second, Some(&first), "a.txt", None).unwrap();
        assert!(d.text.contains("+more"));
        let d = diff_commit(&mut j, dir, &first, None, "a.txt", None).unwrap();
        assert!(d.text.contains("+one"));

        // Discard: the tracked file gets its content back, the new one disappears.
        repo.write("a.txt", "broken\n");
        repo.write("new.txt", "x\n");
        discard(&mut j, dir, &["a.txt".into()], &["new.txt".into()]).unwrap();
        let s = status(&mut j, dir).unwrap();
        assert!(s.staged.is_empty() && s.unstaged.is_empty());
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "one\nmore\n");
        assert!(j.iter().all(|c| c.ok || c.command.starts_with("git commit")));
    }

    #[test]
    fn push_pull_and_fetch_between_clones() {
        let origin = Scratch::new("origin");
        origin.git(&["config", "receive.denyCurrentBranch", "ignore"]);
        origin.write("a.txt", "one\n");
        origin.git(&["add", "-A"]);
        origin.git(&["commit", "-q", "-m", "init"]);

        let clone = Scratch::new("clone");
        let dir = clone.0.as_path();
        clone.git(&["remote", "add", "origin", origin.0.to_str().unwrap()]);
        clone.git(&["pull", "-q", "origin", "main"]);
        let mut j = Vec::new();

        // Branch without an upstream: the first push publishes it and links it.
        clone.git(&["checkout", "-q", "-b", "topic"]);
        clone.write("b.txt", "two\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "add b", false).unwrap();
        assert!(status(&mut j, dir).unwrap().upstream.is_none());
        sync(&mut j, dir, "push", None).unwrap();
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.upstream.as_deref(), Some("origin/topic"));
        assert_eq!((s.ahead, s.behind), (0, 0));
        assert_eq!(origin.git(&["log", "-1", "--format=%s", "topic"]), "add b");

        // A commit arrives on the remote side: fetch reports it, pull brings it in.
        origin.git(&["checkout", "-q", "topic"]);
        origin.write("c.txt", "three\n");
        origin.git(&["add", "-A"]);
        origin.git(&["commit", "-q", "-m", "remote"]);
        sync(&mut j, dir, "fetch", None).unwrap();
        assert_eq!(status(&mut j, dir).unwrap().behind, 1);
        sync(&mut j, dir, "pull", None).unwrap();
        assert_eq!(status(&mut j, dir).unwrap().behind, 0);
        assert!(dir.join("c.txt").exists());

        assert!(sync(&mut j, dir, "rebase", None).is_err());

        // Amend then force push: the plain push is refused, the forced one goes through.
        assert_eq!(head_message(&mut j, dir).unwrap(), "remote");
        clone.write("c.txt", "three, fixed\n");
        stage(&mut j, dir, &["c.txt".into()]).unwrap();
        commit(&mut j, dir, "remote, fixed", true).unwrap();
        assert!(sync(&mut j, dir, "push", None).is_err());
        sync(&mut j, dir, "push-force", None).unwrap();
        assert_eq!(origin.git(&["log", "-1", "--format=%s", "topic"]), "remote, fixed");
    }

    #[test]
    fn single_lines_are_staged_and_unstaged() {
        let repo = Scratch::new("lines");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        repo.write("f.txt", "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n");
        stage(&mut j, dir, &["f.txt".into()]).unwrap();
        commit(&mut j, dir, "init", false).unwrap();

        // Two changes far apart: “b” replaced, “k” deleted.
        repo.write("f.txt", "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nl\n");
        let diff = diff_file(&mut j, dir, "f.txt", None, false, false).unwrap().text;
        let index_of = |needle: &str| diff.split('\n').position(|l| l == needle).unwrap();

        // Only the deletion of “k” is staged.
        apply_lines(&mut j, dir, &diff, &[index_of("-k")], false).unwrap();
        assert_eq!(repo.git(&["show", ":f.txt"]), "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nl");
        let s = status(&mut j, dir).unwrap();
        assert_eq!((s.staged.len(), s.unstaged.len()), (1, 1));

        // Then the replacement of “b”, and the deletion of “k” is unstaged.
        let diff = diff_file(&mut j, dir, "f.txt", None, false, false).unwrap().text;
        let at = |needle: &str| diff.split('\n').position(|l| l == needle).unwrap();
        apply_lines(&mut j, dir, &diff, &[at("-b"), at("+B")], false).unwrap();
        let cached = diff_file(&mut j, dir, "f.txt", None, true, false).unwrap().text;
        let at = |needle: &str| cached.split('\n').position(|l| l == needle).unwrap();
        apply_lines(&mut j, dir, &cached, &[at("-k")], true).unwrap();
        assert_eq!(repo.git(&["show", ":f.txt"]), "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl");

        assert!(apply_lines(&mut j, dir, &cached, &[], true).is_err());
    }

    /// Staging only half of a replaced line: the index keeps the old line and the new one, or
    /// loses the line without its replacement. That is what the interface warns about.
    #[test]
    fn staging_half_of_a_replaced_line_keeps_or_drops_it() {
        let repo = Scratch::new("half");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        repo.write("f.txt", "a\nb\nc\n");
        stage(&mut j, dir, &["f.txt".into()]).unwrap();
        commit(&mut j, dir, "init", false).unwrap();
        repo.write("f.txt", "a\nB\nc\n");

        // Only the addition: the old line stays beside the new one.
        let diff = diff_file(&mut j, dir, "f.txt", None, false, false).unwrap().text;
        let at = |needle: &str| diff.split('\n').position(|l| l == needle).unwrap();
        apply_lines(&mut j, dir, &diff, &[at("+B")], false).unwrap();
        assert_eq!(repo.git(&["show", ":f.txt"]), "a\nb\nB\nc");

        // Only the deletion: the line disappears from the index, without its replacement.
        repo.git(&["reset", "-q"]);
        apply_lines(&mut j, dir, &diff, &[at("-b")], false).unwrap();
        assert_eq!(repo.git(&["show", ":f.txt"]), "a\nc");
    }
}
