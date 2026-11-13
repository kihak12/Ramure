//! References and history: branches, tags, stashes, merge, rebase, cherry-pick, reset,
//! operations stopped by a conflict, and interactive rebase.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::work::{Diff, Status, check_sha, status, to_diff};
use super::{CommandLog, Done, Failed, Opts, blocking, check_name, report, run, step, step_with, text};

/// git's messages in English, for the few outputs that Ramure has to interpret.
const ENGLISH: [(&str, &str); 1] = [("LC_ALL", "C")];

// --- Reading the references ---------------------------------------------------------------

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The remote branch it tracks no longer exists.
    pub gone: bool,
    pub current: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedRef {
    pub name: String,
    /// Commit it points to (for an annotated tag, the commit and not the tag object).
    pub sha: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stash {
    pub index: u32,
    pub sha: String,
    pub message: String,
    pub time: i64,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refs {
    pub branches: Vec<Branch>,
    pub remotes: Vec<NamedRef>,
    pub tags: Vec<NamedRef>,
    pub stashes: Vec<Stash>,
    pub remote_names: Vec<String>,
}

/// `[ahead 2, behind 1]`, `[ahead 2]`, `[gone]` or empty → (ahead, behind, gone).
fn parse_track(track: &str) -> (u32, u32, bool) {
    let inner = track.trim_matches(['[', ']']);
    let (mut ahead, mut behind, mut gone) = (0, 0, false);
    for part in inner.split(", ") {
        if part == "gone" {
            gone = true;
        } else if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        }
    }
    (ahead, behind, gone)
}

/// Parses the output of `for-each-ref` in the format of [`refs`].
fn parse_refs(raw: &str, refs: &mut Refs) {
    for line in raw.lines() {
        let fields: Vec<&str> = line.split('\x1f').collect();
        let [full, object, peeled, upstream, track, head] = fields[..] else { continue };
        // Annotated tag: `*objectname` is the commit it points to.
        let sha = if peeled.is_empty() { object } else { peeled }.to_string();
        if let Some(name) = full.strip_prefix("refs/heads/") {
            let (ahead, behind, gone) = parse_track(track);
            refs.branches.push(Branch {
                name: name.into(),
                sha,
                upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
                ahead,
                behind,
                gone,
                current: head == "*",
            });
        } else if let Some(name) = full.strip_prefix("refs/remotes/") {
            if !name.ends_with("/HEAD") {
                refs.remotes.push(NamedRef { name: name.into(), sha });
            }
        } else if let Some(name) = full.strip_prefix("refs/tags/") {
            refs.tags.push(NamedRef { name: name.into(), sha });
        }
    }
}

fn parse_stashes(raw: &str) -> Vec<Stash> {
    raw.lines()
        .filter_map(|line| {
            let mut fields = line.split('\x1f');
            let index = fields.next()?.strip_prefix("stash@{")?.strip_suffix('}')?.parse().ok()?;
            Some(Stash {
                index,
                sha: fields.next()?.to_string(),
                message: fields.next()?.to_string(),
                time: fields.next()?.parse().unwrap_or(0),
            })
        })
        .collect()
}

pub fn refs(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<Refs, String> {
    let mut refs = Refs::default();
    let format = "--format=%(refname)%1f%(objectname)%1f%(*objectname)%1f%(upstream:short)%1f%(upstream:track)%1f%(HEAD)";
    // git translates `upstream:track`: English is forced to read it.
    let opts = Opts { envs: &ENGLISH, ..Opts::default() };
    let raw = step_with(journal, repo, &["for-each-ref", format, "refs/heads", "refs/remotes", "refs/tags"], opts)?;
    parse_refs(&String::from_utf8_lossy(&raw.stdout), &mut refs);
    let stashes = step(journal, repo, &["stash", "list", "--format=%gd%x1f%H%x1f%gs%x1f%ct"])?;
    refs.stashes = parse_stashes(&String::from_utf8_lossy(&stashes));
    refs.remote_names = text(&step(journal, repo, &["remote"])?).lines().map(str::to_string).collect();
    Ok(refs)
}

/// Default remote: `origin` if it exists, otherwise the first one.
pub fn default_remote(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<String, String> {
    let remotes = text(&step(journal, repo, &["remote"])?);
    remotes
        .lines()
        .find(|r| *r == "origin")
        .or_else(|| remotes.lines().next())
        .map(str::to_string)
        .ok_or_else(|| "This repository has no remote. Add one with git remote add.".to_string())
}

// --- Branches, tags and stashes -----------------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum RefOp {
    Switch { name: String },
    /// Switches to the local branch that tracks `name` (`origin/x`), creating it if needed.
    SwitchRemote { name: String },
    Detach { sha: String },
    BranchCreate { name: String, start: String, switch: bool },
    BranchRename { name: String, new_name: String },
    BranchDelete { name: String, force: bool },
    RemoteBranchDelete { name: String },
    TagCreate { name: String, sha: String, message: String },
    TagDelete { name: String },
    TagPush { name: String },
    TagDeleteRemote { name: String },
    StashPush { message: String },
    StashPop { index: u32 },
    StashApply { index: u32 },
    StashDrop { index: u32 },
}

/// `origin/feat/x` → (`origin`, `feat/x`).
fn split_remote(name: &str) -> Result<(&str, &str), String> {
    name.split_once('/')
        .filter(|(remote, branch)| !remote.is_empty() && !branch.is_empty())
        .ok_or_else(|| format!("Invalid remote branch: “{name}”. Name a remote and a branch, as in “origin/main”."))
}

/// Position of the local branch that bears the name of a remote branch, relative to it.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    /// Name of the local branch; `None` if it does not exist yet.
    pub local: Option<String>,
    /// Commits of the local branch missing from the remote branch.
    pub ahead: u32,
    /// Commits of the remote branch missing from the local branch.
    pub behind: u32,
}

fn local_exists(repo: &Path, local: &str) -> bool {
    run(repo, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{local}")]).is_ok()
}

/// Compares the remote branch `remote` (`origin/main`) with the local branch of the same name.
pub fn relation(journal: &mut Vec<CommandLog>, repo: &Path, remote: &str) -> Result<Relation, String> {
    check_name(remote)?;
    let (_, local) = split_remote(remote)?;
    if !local_exists(repo, local) {
        return Ok(Relation { local: None, ahead: 0, behind: 0 });
    }
    let range = format!("refs/heads/{local}...refs/remotes/{remote}");
    let out = text(&step(journal, repo, &["rev-list", "--left-right", "--count", &range])?);
    let mut counts = out.split_whitespace().map(|n| n.parse::<u32>().unwrap_or(0));
    let (ahead, behind) = (counts.next().unwrap_or(0), counts.next().unwrap_or(0));
    Ok(Relation { local: Some(local.to_string()), ahead, behind })
}

/// Changed files that would prevent a switch to a branch: those that the working tree or the
/// index has touched, and that differ between HEAD and that branch. git then refuses the switch,
/// so as not to overwrite these changes. `remote`: `name` is a remote branch.
pub fn switch_blockers(journal: &mut Vec<CommandLog>, repo: &Path, name: &str, remote: bool) -> Result<Vec<String>, String> {
    check_name(name)?;
    let target = format!("refs/{}/{name}", if remote { "remotes" } else { "heads" });
    let touched = text(&step(journal, repo, &["diff", "--name-only", "-z", "HEAD"])?);
    if touched.is_empty() {
        return Ok(Vec::new());
    }
    let differ = text(&step(journal, repo, &["diff", "--name-only", "-z", "HEAD", &target])?);
    let differ: Vec<&str> = differ.split('\0').collect();
    Ok(touched.split('\0').filter(|path| !path.is_empty() && differ.contains(path)).map(str::to_string).collect())
}

pub fn ref_op(journal: &mut Vec<CommandLog>, repo: &Path, op: &RefOp) -> Result<String, String> {
    let git = |journal: &mut Vec<CommandLog>, args: &[&str]| -> Result<String, String> {
        step_with(journal, repo, args, Opts::default()).map(|out| report(&out))
    };
    match op {
        RefOp::Switch { name } => {
            check_name(name)?;
            git(journal, &["switch", name])
        }
        RefOp::SwitchRemote { name } => {
            check_name(name)?;
            let (_, local) = split_remote(name)?;
            if !local_exists(repo, local) {
                return git(journal, &["switch", "--track", name]);
            }
            let switched = git(journal, &["switch", local])?;
            // The local branch is behind the remote branch, with no commit of its own: move it
            // forward. If it has diverged, leave it alone: that is for the user to decide.
            let behind = relation(journal, repo, name)?;
            if behind.ahead == 0 && behind.behind > 0 {
                let target = format!("refs/remotes/{name}");
                return git(journal, &["merge", "--ff-only", &target]);
            }
            Ok(switched)
        }
        RefOp::Detach { sha } => {
            check_sha(sha)?;
            git(journal, &["switch", "--detach", sha])
        }
        RefOp::BranchCreate { name, start, switch } => {
            check_name(name)?;
            check_name(start)?;
            step(journal, repo, &["check-ref-format", "--branch", name])
                .map_err(|_| format!("“{name}” is not a valid branch name."))?;
            if *switch {
                git(journal, &["switch", "-c", name, start])
            } else {
                git(journal, &["branch", name, start])
            }
        }
        RefOp::BranchRename { name, new_name } => {
            check_name(name)?;
            check_name(new_name)?;
            git(journal, &["branch", "-m", name, new_name])
        }
        RefOp::BranchDelete { name, force } => {
            check_name(name)?;
            git(journal, &["branch", if *force { "-D" } else { "-d" }, name])
        }
        RefOp::RemoteBranchDelete { name } => {
            check_name(name)?;
            let (remote, branch) = split_remote(name)?;
            git(journal, &["push", remote, "--delete", branch])
        }
        RefOp::TagCreate { name, sha, message } => {
            check_name(name)?;
            check_sha(sha)?;
            step(journal, repo, &["check-ref-format", &format!("refs/tags/{name}")])
                .map_err(|_| format!("“{name}” is not a valid tag name."))?;
            let message = message.trim();
            if message.is_empty() {
                git(journal, &["tag", name, sha])
            } else {
                // Annotated tag: the message goes through the standard input.
                let opts = Opts { input: Some(message.as_bytes()), ..Opts::default() };
                step_with(journal, repo, &["tag", "-a", name, sha, "-F", "-"], opts).map(|out| report(&out))
            }
        }
        RefOp::TagDelete { name } => {
            check_name(name)?;
            git(journal, &["tag", "-d", name])
        }
        RefOp::TagPush { name } => {
            check_name(name)?;
            let remote = default_remote(journal, repo)?;
            git(journal, &["push", &remote, &format!("refs/tags/{name}")])
        }
        RefOp::TagDeleteRemote { name } => {
            check_name(name)?;
            let remote = default_remote(journal, repo)?;
            git(journal, &["push", &remote, "--delete", &format!("refs/tags/{name}")])
        }
        RefOp::StashPush { message } => {
            let message = message.trim();
            if message.is_empty() {
                git(journal, &["stash", "push", "-u"])
            } else {
                git(journal, &["stash", "push", "-u", "-m", message])
            }
        }
        RefOp::StashPop { index } => git(journal, &["stash", "pop", &format!("stash@{{{index}}}")]),
        RefOp::StashApply { index } => git(journal, &["stash", "apply", &format!("stash@{{{index}}}")]),
        RefOp::StashDrop { index } => git(journal, &["stash", "drop", &format!("stash@{{{index}}}")]),
    }
}

/// Content of a stash, untracked files included when git allows it (2.32 or later).
pub fn stash_show(journal: &mut Vec<CommandLog>, repo: &Path, index: u32) -> Result<Diff, String> {
    let name = format!("stash@{{{index}}}");
    let common = ["stash", "show", "-p", "--no-color", "--no-ext-diff"];
    let mut full = common.to_vec();
    full.extend(["--include-untracked", &name]);
    let raw = match step(journal, repo, &full) {
        Ok(raw) => raw,
        Err(_) => {
            let mut plain = common.to_vec();
            plain.push(&name);
            step(journal, repo, &plain)?
        }
    };
    Ok(to_diff(raw))
}

// --- History operations -------------------------------------------------------------------

/// Result of an operation that can stop on a conflict.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// What git wrote.
    pub output: String,
    /// Operation left stopped (`merge`, `rebase`, `cherry-pick`, `revert`): the conflicts must
    /// be resolved, then it continues, or it is aborted.
    pub stopped: Option<&'static str>,
}

pub fn git_dir(repo: &Path) -> Option<PathBuf> {
    run(repo, &["rev-parse", "--absolute-git-dir"]).ok().map(|out| PathBuf::from(text(&out)))
}

/// Operation in progress in the repository, from the files that git leaves in `.git`.
pub fn operation(repo: &Path) -> Option<&'static str> {
    let dir = git_dir(repo)?;
    // A rebase stopped on a commit also leaves CHERRY_PICK_HEAD: it is tested first.
    if dir.join("rebase-merge").is_dir() || dir.join("rebase-apply").is_dir() {
        Some("rebase")
    } else if dir.join("CHERRY_PICK_HEAD").exists() {
        Some("cherry-pick")
    } else if dir.join("REVERT_HEAD").exists() {
        Some("revert")
    } else if dir.join("MERGE_HEAD").exists() {
        Some("merge")
    } else {
        None
    }
}

/// A failure that leaves an operation stopped is not an error: it is a conflict.
pub fn guarded(repo: &Path, result: Result<Output, String>) -> Result<Outcome, String> {
    match result {
        Ok(out) => Ok(Outcome { output: report(&out), stopped: operation(repo) }),
        Err(message) => match operation(repo) {
            Some(op) => Ok(Outcome { output: message, stopped: Some(op) }),
            None => Err(message),
        },
    }
}

/// `YYYYMMDD-HHMMSS` in universal time, for an instant given in Unix seconds.
fn stamp_at(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days → civil date conversion (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", rem / 3_600, rem % 3_600 / 60, rem % 60)
}

/// Before rewriting the history, keeps the position of the branch in
/// `refs/ramure/backup/<branch>-<date>`: `git reset --hard <that reference>` goes back to it.
fn backup(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<(), String> {
    let Ok(branch) = run(repo, &["symbolic-ref", "--short", "--quiet", "HEAD"]) else {
        return Ok(()); // detached HEAD: no branch moves
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let name = format!("refs/ramure/backup/{}-{}", text(&branch), stamp_at(now));
    step(journal, repo, &["update-ref", &name, "HEAD"]).map(drop)
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum HistoryOp {
    /// Merges `target` into the current branch.
    Merge { target: String },
    /// Replays the current branch onto `onto`.
    Rebase { onto: String },
    /// `mainline`: the commit is a merge, its changes are taken relative to the first parent.
    CherryPick { sha: String, mainline: bool },
    Revert { sha: String, mainline: bool },
    /// `mode`: `soft`, `mixed` or `hard`.
    Reset { sha: String, mode: String },
}

pub fn history(journal: &mut Vec<CommandLog>, repo: &Path, op: &HistoryOp) -> Result<Outcome, String> {
    let args: Vec<&str> = match op {
        HistoryOp::Merge { target } => {
            check_name(target)?;
            vec!["merge", target]
        }
        HistoryOp::Rebase { onto } => {
            check_name(onto)?;
            backup(journal, repo)?;
            vec!["rebase", onto]
        }
        HistoryOp::CherryPick { sha, mainline } => {
            check_sha(sha)?;
            if *mainline { vec!["cherry-pick", "-m", "1", sha] } else { vec!["cherry-pick", sha] }
        }
        HistoryOp::Revert { sha, mainline } => {
            check_sha(sha)?;
            if *mainline { vec!["revert", "--no-edit", "-m", "1", sha] } else { vec!["revert", "--no-edit", sha] }
        }
        HistoryOp::Reset { sha, mode } => {
            check_sha(sha)?;
            let flag = match mode.as_str() {
                "soft" => "--soft",
                "mixed" => "--mixed",
                "hard" => "--hard",
                other => return Err(format!("Unknown reset mode: “{other}”. Use soft, mixed or hard.")),
            };
            backup(journal, repo)?;
            vec!["reset", flag, sha]
        }
    };
    let result = step_with(journal, repo, &args, Opts::default());
    guarded(repo, result)
}

/// Continues, skips or aborts the operation in progress. `action`: `continue`, `skip` or `abort`.
pub fn operate(journal: &mut Vec<CommandLog>, repo: &Path, action: &str) -> Result<Outcome, String> {
    let op = operation(repo).ok_or("No operation is in progress in this repository.")?;
    let flag = match action {
        "continue" => "--continue",
        "skip" => "--skip",
        "abort" => "--abort",
        other => return Err(format!("Unknown action: “{other}”.")),
    };
    let args: Vec<&str> = match (op, action) {
        // `git merge --continue` only exists since git 2.12; `git commit` does the same thing.
        ("merge", "continue") => vec!["commit", "--no-edit"],
        ("merge", "skip") => return Err("A merge cannot be skipped. Continue it or abort it.".into()),
        (op, _) => vec![op, flag],
    };
    let result = step_with(journal, repo, &args, Opts::default());
    guarded(repo, result)
}

/// Resolves a file in conflict by keeping a single side. `side`: `ours` or `theirs`.
pub fn resolve(journal: &mut Vec<CommandLog>, repo: &Path, file: &str, side: &str) -> Result<(), String> {
    let flag = match side {
        "ours" => "--ours",
        "theirs" => "--theirs",
        other => return Err(format!("Unknown side: “{other}”. Use ours or theirs.")),
    };
    let opts = Opts { envs: &ENGLISH, ..Opts::default() };
    match step_with(journal, repo, &["checkout", flag, "--", file], opts) {
        Ok(_) => step(journal, repo, &["add", "--", file]).map(drop),
        // This side deleted the file: keeping it means deleting the file.
        Err(message) if message.contains("does not have") => step(journal, repo, &["rm", "-q", "--", file]).map(drop),
        Err(message) => Err(message),
    }
}

// --- Interactive rebase -------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCommit {
    pub sha: String,
    pub subject: String,
    /// Whole message, to reword it without losing its description.
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    /// Commit to replay onto; absent if the range starts at the very first commit.
    pub base: Option<String>,
    /// Commits to replay, from the oldest to the most recent.
    pub commits: Vec<PlanCommit>,
    /// At least one commit of the range exists on a remote: a force push will be needed.
    pub pushed: bool,
}

/// Prepares an interactive rebase from `sha` (included) up to HEAD.
pub fn rebase_plan(journal: &mut Vec<CommandLog>, repo: &Path, sha: &str) -> Result<RebasePlan, String> {
    check_sha(sha)?;
    step(journal, repo, &["merge-base", "--is-ancestor", sha, "HEAD"])
        .map_err(|_| "This commit is not on the current branch. Switch to a branch that contains it.".to_string())?;
    let parents = text(&step(journal, repo, &["rev-list", "--parents", "-n", "1", sha])?);
    let base = parents.split_whitespace().nth(1).map(str::to_string);
    let range = match &base {
        Some(base) => format!("{base}..HEAD"),
        None => "HEAD".to_string(),
    };
    let raw = step(journal, repo, &["log", "--reverse", "--format=%H%x1f%P%x1f%B%x1e", &range])?;
    let mut commits = Vec::new();
    for record in String::from_utf8_lossy(&raw).split('\x1e') {
        let mut fields = record.trim_start_matches(['\n', '\r']).splitn(3, '\x1f');
        let (Some(sha), Some(parents), Some(message)) = (fields.next(), fields.next(), fields.next()) else { continue };
        if parents.split_whitespace().count() > 1 {
            return Err("The range contains a merge commit, which an interactive rebase would flatten. Choose a more recent commit.".into());
        }
        let message = message.trim();
        let subject = message.lines().next().unwrap_or("").to_string();
        commits.push(PlanCommit { sha: sha.into(), subject, message: message.into() });
    }
    let on_remote = step(journal, repo, &["for-each-ref", "--count=1", "--contains", sha, "refs/remotes"])?;
    Ok(RebasePlan { base, commits, pushed: !on_remote.is_empty() })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    /// `pick`, `reword`, `squash`, `fixup` or `drop`.
    pub action: String,
    pub sha: String,
    /// New message, for `reword`.
    pub message: Option<String>,
}

/// Path in single quotes for git's shell, with forward slashes on Windows too.
fn sh_quote(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Writes the list of actions and the reworded messages into `<.git>/ramure/`.
fn write_todo(dir: &Path, todo: &[TodoItem]) -> Result<PathBuf, String> {
    let io = |e: std::io::Error| format!("Could not prepare the rebase: {e}");
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir).map_err(io)?;

    let mut lines = String::new();
    let mut first = true;
    for (i, item) in todo.iter().enumerate() {
        check_sha(&item.sha)?;
        match item.action.as_str() {
            "drop" => continue,
            "pick" | "reword" => lines.push_str(&format!("pick {}\n", item.sha)),
            "squash" | "fixup" if first => {
                return Err("The first commit kept cannot be squashed or fixed up: there is no commit above it.".into());
            }
            "squash" | "fixup" => lines.push_str(&format!("{} {}\n", item.action, item.sha)),
            other => return Err(format!("Unknown rebase action: “{other}”.")),
        }
        first = false;
        if item.action == "reword" {
            let message = item.message.as_deref().map(str::trim).unwrap_or("");
            if message.is_empty() {
                return Err("A commit to reword has an empty message.".into());
            }
            let file = dir.join(format!("message-{i}.txt"));
            fs::write(&file, message).map_err(io)?;
            // The message is replaced right after the commit has been replayed.
            lines.push_str(&format!("exec git commit --amend -q -F {}\n", sh_quote(&file)));
        }
    }
    if first {
        return Err("Every commit would be dropped: use a reset instead.".into());
    }
    let file = dir.join("todo");
    fs::write(&file, lines).map_err(io)?;
    Ok(file)
}

/// Runs `git rebase -i` with a list of actions written by Ramure.
///
/// git asks the “sequence editor” to edit its list; `editor` is a command that replaces it with
/// ours. By default, it is Ramure itself, run again with `--sequence-editor` (see `main.rs`): no
/// outside tool is required, on any system.
pub fn rebase_interactive(
    journal: &mut Vec<CommandLog>,
    repo: &Path,
    base: Option<&str>,
    todo: &[TodoItem],
    editor: Option<&str>,
) -> Result<Outcome, String> {
    if let Some(base) = base {
        check_sha(base)?;
    }
    let dir = git_dir(repo).ok_or("The .git folder of this repository was not found.")?.join("ramure");
    let file = write_todo(&dir, todo)?;
    let editor = match editor {
        Some(editor) => format!("{editor} {}", sh_quote(&file)),
        None => {
            let exe = std::env::current_exe().map_err(|e| format!("The Ramure executable was not found: {e}"))?;
            format!("{} --sequence-editor {}", sh_quote(&exe), sh_quote(&file))
        }
    };
    backup(journal, repo)?;
    let mut args = vec!["rebase", "-i", "--no-autosquash"];
    args.push(base.unwrap_or("--root"));
    let envs = [("GIT_SEQUENCE_EDITOR", editor.as_str())];
    let result = step_with(journal, repo, &args, Opts { envs: &envs, ..Opts::default() });
    guarded(repo, result)
}

// --- Commands exposed to the interface ----------------------------------------------------

type Reply<T> = Result<Done<T>, Failed>;

#[tauri::command]
pub async fn git_refs(repo: String) -> Reply<Refs> {
    blocking(move |j| refs(j, Path::new(&repo))).await
}

#[tauri::command]
pub async fn git_remote_relation(repo: String, remote: String) -> Reply<Relation> {
    blocking(move |j| relation(j, Path::new(&repo), &remote)).await
}

#[tauri::command]
pub async fn git_switch_blockers(repo: String, name: String, remote: bool) -> Reply<Vec<String>> {
    blocking(move |j| switch_blockers(j, Path::new(&repo), &name, remote)).await
}

#[tauri::command]
pub async fn git_ref_op(repo: String, op: RefOp) -> Reply<String> {
    blocking(move |j| ref_op(j, Path::new(&repo), &op)).await
}

#[tauri::command]
pub async fn git_stash_show(repo: String, index: u32) -> Reply<Diff> {
    blocking(move |j| stash_show(j, Path::new(&repo), index)).await
}

#[tauri::command]
pub async fn git_history(repo: String, op: HistoryOp) -> Reply<Outcome> {
    blocking(move |j| history(j, Path::new(&repo), &op)).await
}

#[tauri::command]
pub async fn git_operate(repo: String, action: String) -> Reply<Outcome> {
    blocking(move |j| operate(j, Path::new(&repo), &action)).await
}

#[tauri::command]
pub async fn git_resolve(repo: String, path: String, side: String) -> Reply<Status> {
    blocking(move |j| {
        let repo = Path::new(&repo);
        resolve(j, repo, &path, &side)?;
        status(j, repo)
    })
    .await
}

#[tauri::command]
pub async fn git_rebase_plan(repo: String, sha: String) -> Reply<RebasePlan> {
    blocking(move |j| rebase_plan(j, Path::new(&repo), &sha)).await
}

#[tauri::command]
pub async fn git_rebase_interactive(repo: String, base: Option<String>, todo: Vec<TodoItem>) -> Reply<Outcome> {
    blocking(move |j| rebase_interactive(j, Path::new(&repo), base.as_deref(), &todo, None)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::work::tests::Scratch;
    use crate::git::work::{commit, stage};

    #[test]
    fn track_is_parsed() {
        assert_eq!(parse_track("[ahead 2, behind 1]"), (2, 1, false));
        assert_eq!(parse_track("[behind 7]"), (0, 7, false));
        assert_eq!(parse_track("[gone]"), (0, 0, true));
        assert_eq!(parse_track(""), (0, 0, false));
    }

    #[test]
    fn for_each_ref_is_parsed() {
        let raw = "refs/heads/main\x1faaa\x1f\x1forigin/main\x1f[ahead 1]\x1f*\n\
refs/heads/topic\x1fbbb\x1f\x1f\x1f\x1f \n\
refs/remotes/origin/HEAD\x1faaa\x1f\x1f\x1f\x1f \n\
refs/remotes/origin/main\x1fccc\x1f\x1f\x1f\x1f \n\
refs/tags/v1\x1fddd\x1feee\x1f\x1f\x1f \n";
        let mut refs = Refs::default();
        parse_refs(raw, &mut refs);
        assert_eq!(refs.branches.len(), 2);
        assert!(refs.branches[0].current && refs.branches[0].ahead == 1);
        assert_eq!(refs.branches[0].upstream.as_deref(), Some("origin/main"));
        assert!(!refs.branches[1].current && refs.branches[1].upstream.is_none());
        assert_eq!(refs.remotes, vec![NamedRef { name: "origin/main".into(), sha: "ccc".into() }]);
        // Annotated tag: what counts is the commit it points to.
        assert_eq!(refs.tags, vec![NamedRef { name: "v1".into(), sha: "eee".into() }]);
    }

    #[test]
    fn stamps_are_utc_dates() {
        assert_eq!(stamp_at(0), "19700101-000000");
        assert_eq!(stamp_at(1_700_000_000), "20231114-221320");
        assert_eq!(stamp_at(951_782_400), "20000229-000000");
    }

    /// Repository with three commits on `main`: a.txt = "1\n", then "1\n2\n", then "1\n2\n3\n".
    fn three_commits(label: &str) -> Scratch {
        let repo = Scratch::new(label);
        let mut j = Vec::new();
        for (content, message) in [("1\n", "one"), ("1\n2\n", "two"), ("1\n2\n3\n", "three")] {
            repo.write("a.txt", content);
            stage(&mut j, &repo.0, &["a.txt".into()]).unwrap();
            commit(&mut j, &repo.0, message, false).unwrap();
        }
        repo
    }

    #[test]
    fn local_changes_that_block_a_switch_are_listed() {
        let repo = three_commits("blockers");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        repo.git(&["branch", "before", "HEAD~1"]);
        repo.git(&["branch", "here"]);
        repo.write("b.txt", "other\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "four", false).unwrap();

        // Nothing changed: nothing prevents a switch.
        assert!(switch_blockers(&mut j, dir, "before", false).unwrap().is_empty());

        // “b.txt” is changed, and it differs between HEAD and both branches: it blocks.
        repo.write("b.txt", "in progress\n");
        assert_eq!(switch_blockers(&mut j, dir, "here", false).unwrap(), vec!["b.txt".to_string()]);
        assert!(ref_op(&mut j, dir, &RefOp::Switch { name: "here".into() }).is_err());

        // “a.txt” is changed, but the same between HEAD and “here”: git carries it along.
        repo.git(&["checkout", "--", "b.txt"]);
        repo.write("a.txt", "1\n2\n3\nin progress\n");
        assert!(switch_blockers(&mut j, dir, "here", false).unwrap().is_empty());
        assert_eq!(switch_blockers(&mut j, dir, "before", false).unwrap(), vec!["a.txt".to_string()]);
        assert!(ref_op(&mut j, dir, &RefOp::Switch { name: "here".into() }).is_ok());
    }

    #[test]
    fn switching_to_a_remote_branch_updates_the_local_one() {
        let repo = three_commits("tracking");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        let third = repo.git(&["rev-parse", "HEAD"]);
        let second = repo.git(&["rev-parse", "HEAD~1"]);
        let origin = |sha: &str| repo.git(&["update-ref", "refs/remotes/origin/main", sha]);
        let switch = RefOp::SwitchRemote { name: "origin/main".into() };

        // No local branch of that name: nothing to compare.
        repo.git(&["update-ref", "refs/remotes/origin/other", &third]);
        assert_eq!(relation(&mut j, dir, "origin/other").unwrap(), Relation { local: None, ahead: 0, behind: 0 });

        // Local branch one commit behind: the switch moves it forward to the remote branch.
        origin(&third);
        repo.git(&["reset", "-q", "--hard", &second]);
        repo.git(&["switch", "-q", "-c", "elsewhere"]);
        let late = Relation { local: Some("main".into()), ahead: 0, behind: 1 };
        assert_eq!(relation(&mut j, dir, "origin/main").unwrap(), late);
        ref_op(&mut j, dir, &switch).unwrap();
        assert_eq!(repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]), "main");
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);

        // Local branch ahead: the switch leaves it alone.
        origin(&second);
        assert_eq!(relation(&mut j, dir, "origin/main").unwrap(), Relation { local: Some("main".into()), ahead: 1, behind: 0 });
        ref_op(&mut j, dir, &switch).unwrap();
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);

        // Diverged branches: the switch happens, but the user chooses how to bring them together.
        repo.git(&["switch", "-q", "--detach", &second]);
        repo.write("b.txt", "remote\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "remote", false).unwrap();
        origin(&repo.git(&["rev-parse", "HEAD"]));
        assert_eq!(relation(&mut j, dir, "origin/main").unwrap(), Relation { local: Some("main".into()), ahead: 1, behind: 1 });
        ref_op(&mut j, dir, &switch).unwrap();
        assert_eq!(repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]), "main");
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);
    }

    #[test]
    fn branches_tags_and_stashes() {
        let repo = three_commits("refs");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        let head = repo.git(&["rev-parse", "HEAD"]);

        let create = RefOp::BranchCreate { name: "topic".into(), start: head.clone(), switch: true };
        ref_op(&mut j, dir, &create).unwrap();
        ref_op(&mut j, dir, &RefOp::BranchRename { name: "topic".into(), new_name: "topic/2".into() }).unwrap();
        let bad = RefOp::BranchCreate { name: "a..b".into(), start: head.clone(), switch: false };
        assert!(ref_op(&mut j, dir, &bad).is_err());
        assert!(ref_op(&mut j, dir, &RefOp::Switch { name: "--orphan".into() }).is_err());

        ref_op(&mut j, dir, &RefOp::TagCreate { name: "v1".into(), sha: head.clone(), message: String::new() }).unwrap();
        let annotated = RefOp::TagCreate { name: "v2".into(), sha: head.clone(), message: "version 2".into() };
        ref_op(&mut j, dir, &annotated).unwrap();

        repo.write("a.txt", "changed\n");
        repo.write("new.txt", "x\n");
        ref_op(&mut j, dir, &RefOp::StashPush { message: "in progress".into() }).unwrap();
        assert!(status(&mut j, dir).unwrap().unstaged.is_empty());

        let all = refs(&mut j, dir).unwrap();
        let names: Vec<_> = all.branches.iter().map(|b| (b.name.as_str(), b.current)).collect();
        assert_eq!(names, vec![("main", false), ("topic/2", true)]);
        assert_eq!(all.tags.len(), 2);
        assert!(all.tags.iter().all(|t| t.sha == head));
        assert_eq!(all.stashes.len(), 1);
        assert!(all.stashes[0].message.ends_with("in progress"));
        assert!(stash_show(&mut j, dir, 0).unwrap().text.contains("+changed"));

        ref_op(&mut j, dir, &RefOp::StashPop { index: 0 }).unwrap();
        assert_eq!(status(&mut j, dir).unwrap().unstaged.len(), 2);
        ref_op(&mut j, dir, &RefOp::StashPush { message: String::new() }).unwrap();
        ref_op(&mut j, dir, &RefOp::StashDrop { index: 0 }).unwrap();

        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();
        ref_op(&mut j, dir, &RefOp::BranchDelete { name: "topic/2".into(), force: false }).unwrap();
        ref_op(&mut j, dir, &RefOp::TagDelete { name: "v1".into() }).unwrap();
        let all = refs(&mut j, dir).unwrap();
        assert_eq!(all.branches.len(), 1);
        assert_eq!(all.tags.len(), 1);
        assert!(all.stashes.is_empty());
    }

    #[test]
    fn conflicting_merge_is_resolved_or_aborted() {
        let repo = three_commits("merge");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        let base = repo.git(&["rev-parse", "HEAD~1"]);

        // `topic` starts from “two” and changes the same line as “three”.
        ref_op(&mut j, dir, &RefOp::BranchCreate { name: "topic".into(), start: base, switch: true }).unwrap();
        repo.write("a.txt", "1\n2\nTHREE\n");
        stage(&mut j, dir, &["a.txt".into()]).unwrap();
        commit(&mut j, dir, "three, differently", false).unwrap();
        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();

        let merge = HistoryOp::Merge { target: "topic".into() };
        let outcome = history(&mut j, dir, &merge).unwrap();
        assert_eq!(outcome.stopped, Some("merge"));
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.operation, Some("merge"));
        assert_eq!(s.unstaged[0].kind, "conflict");

        // Continuing without resolving: git refuses, the operation stays stopped.
        assert_eq!(operate(&mut j, dir, "continue").unwrap().stopped, Some("merge"));
        assert!(operate(&mut j, dir, "skip").is_err());
        assert_eq!(operate(&mut j, dir, "abort").unwrap().stopped, None);
        assert_eq!(status(&mut j, dir).unwrap().operation, None);

        // Second try: keep the version of `topic`, then finish.
        history(&mut j, dir, &merge).unwrap();
        resolve(&mut j, dir, "a.txt", "theirs").unwrap();
        assert_eq!(operate(&mut j, dir, "continue").unwrap().stopped, None);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n2\nTHREE\n");
        assert_eq!(repo.git(&["rev-list", "--parents", "-n", "1", "HEAD"]).split(' ').count(), 3);
        assert!(operate(&mut j, dir, "continue").is_err());
    }

    #[test]
    fn rebase_cherry_pick_revert_and_reset() {
        let repo = three_commits("history");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        let first = repo.git(&["rev-parse", "HEAD~2"]);
        let third = repo.git(&["rev-parse", "HEAD"]);

        // Branch started from the first commit, with a file of its own.
        ref_op(&mut j, dir, &RefOp::BranchCreate { name: "topic".into(), start: first.clone(), switch: true }).unwrap();
        repo.write("b.txt", "b\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "add b", false).unwrap();
        let on_topic = repo.git(&["rev-parse", "HEAD"]);

        // Rebase without conflict: the branch starts again from `main`, and a backup exists.
        assert_eq!(history(&mut j, dir, &HistoryOp::Rebase { onto: "main".into() }).unwrap().stopped, None);
        assert_eq!(repo.git(&["rev-parse", "HEAD~1"]), third);
        let backups = repo.git(&["for-each-ref", "--format=%(refname) %(objectname)", "refs/ramure/backup"]);
        assert!(backups.starts_with("refs/ramure/backup/topic-") && backups.ends_with(&on_topic));

        // Cherry-pick onto main, then revert.
        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();
        let pick = HistoryOp::CherryPick { sha: on_topic.clone(), mainline: false };
        assert_eq!(history(&mut j, dir, &pick).unwrap().stopped, None);
        assert!(dir.join("b.txt").exists());
        let picked = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(history(&mut j, dir, &HistoryOp::Revert { sha: picked, mainline: false }).unwrap().stopped, None);
        assert!(!dir.join("b.txt").exists());

        // Reset: soft keeps the changes staged, hard throws them away.
        history(&mut j, dir, &HistoryOp::Reset { sha: third.clone(), mode: "soft".into() }).unwrap();
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);
        history(&mut j, dir, &HistoryOp::Reset { sha: first.clone(), mode: "hard".into() }).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n");
        assert!(history(&mut j, dir, &HistoryOp::Reset { sha: first, mode: "harder".into() }).is_err());

        // The backups must not clutter the graph.
        let log = crate::git::log::load(dir, None).unwrap();
        assert!(log.commits.iter().all(|c| c.subject != "revert" && !c.refs.iter().any(|r| r.name.contains("ramure"))));
    }

    #[test]
    fn interactive_rebase_rewrites_history() {
        let repo = three_commits("interactive");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        repo.write("c.txt", "c\n");
        stage(&mut j, dir, &["c.txt".into()]).unwrap();
        commit(&mut j, dir, "four", false).unwrap();

        let second = repo.git(&["rev-parse", "HEAD~2"]);
        let plan = rebase_plan(&mut j, dir, &second).unwrap();
        let subjects: Vec<_> = plan.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["two", "three", "four"]);
        assert!(plan.base.is_some() && !plan.pushed);

        // “four” goes first and gets a new message, “three” is melded into “two”.
        let item = |action: &str, n: usize, message: Option<&str>| TodoItem {
            action: action.into(),
            sha: plan.commits[n].sha.clone(),
            message: message.map(str::to_string),
        };
        let todo = [item("reword", 2, Some("FOUR")), item("pick", 0, None), item("fixup", 1, None)];
        let outcome = rebase_interactive(&mut j, dir, plan.base.as_deref(), &todo, Some("cp")).unwrap();
        assert_eq!(outcome.stopped, None, "{}", outcome.output);
        assert_eq!(repo.git(&["log", "--format=%s"]), "two\nFOUR\none");
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n2\n3\n");

        // Safeguards.
        let plan = rebase_plan(&mut j, dir, &repo.git(&["rev-parse", "HEAD~2"])).unwrap();
        assert!(plan.base.is_none(), "the range starts at the first commit");
        let squash_first = [TodoItem { action: "squash".into(), sha: plan.commits[0].sha.clone(), message: None }];
        assert!(rebase_interactive(&mut j, dir, None, &squash_first, Some("cp")).is_err());
        let drop_all = [TodoItem { action: "drop".into(), sha: plan.commits[0].sha.clone(), message: None }];
        assert!(rebase_interactive(&mut j, dir, None, &drop_all, Some("cp")).is_err());
    }
}
