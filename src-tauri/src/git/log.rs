//! Reading the history: `git log` → parsed commits → rows of the graph.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use super::{CommandLog, graph, ms, step, text};

/// Fields separated by 0x1f, commits ended by 0x1e: neither of these bytes appears in a subject.
const FORMAT: &str = "--format=%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefLabel {
    pub name: String,
    /// `head` (current branch), `local`, `remote`, `tag` or `detached`.
    pub kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    /// Author date, in Unix seconds.
    pub time: i64,
    pub refs: Vec<RefLabel>,
    pub subject: String,
    pub lane: u16,
    pub edges: Vec<[u16; 3]>,
    /// The “work in progress” row, put at the top when the working tree is not clean.
    pub wip: bool,
    /// True if no remote branch can reach the commit: it has not been pushed.
    /// Always false when the repository tracks no remote branch.
    pub local_only: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    pub git_ms: f64,
    pub parse_ms: f64,
    pub layout_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogResult {
    /// Root of the repository.
    pub repo: String,
    pub name: String,
    pub branch: Option<String>,
    pub commits: Vec<CommitRow>,
    pub max_lanes: u16,
    /// True if the history goes beyond the limit asked for.
    pub truncated: bool,
    pub wip_files: usize,
    pub timings: Timings,
    /// git commands run, for the command log.
    pub commands: Vec<CommandLog>,
}

#[derive(Debug, PartialEq)]
struct Meta {
    sha: String,
    parents: Vec<String>,
    author: String,
    time: i64,
    refs: Vec<RefLabel>,
    subject: String,
}

/// Decorations in the `--decorate=full` format:
/// `HEAD -> refs/heads/main, tag: refs/tags/v1, refs/remotes/origin/main`.
fn parse_refs(decorations: &str) -> Vec<RefLabel> {
    let mut refs = Vec::new();
    for item in decorations.split(", ") {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (full, current) = match item.strip_prefix("HEAD -> ") {
            Some(rest) => (rest, true),
            None => (item, false),
        };
        if full == "HEAD" {
            refs.push(RefLabel { name: "HEAD".into(), kind: "detached" });
        } else if let Some(tag) = full.strip_prefix("tag: ") {
            let name = tag.strip_prefix("refs/tags/").unwrap_or(tag);
            refs.push(RefLabel { name: name.into(), kind: "tag" });
        } else if let Some(name) = full.strip_prefix("refs/heads/") {
            refs.push(RefLabel { name: name.into(), kind: if current { "head" } else { "local" } });
        } else if let Some(name) = full.strip_prefix("refs/remotes/") {
            // `origin/HEAD` adds nothing to the display.
            if !name.ends_with("/HEAD") {
                refs.push(RefLabel { name: name.into(), kind: "remote" });
            }
        }
    }
    refs
}

fn parse_log(raw: &str) -> Vec<Meta> {
    let mut commits = Vec::new();
    for record in raw.split('\x1e') {
        let record = record.trim_start_matches(['\n', '\r']);
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(6, '\x1f');
        let (Some(sha), Some(parents), Some(author), Some(time), Some(decorations), Some(subject)) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            continue;
        };
        commits.push(Meta {
            sha: sha.to_string(),
            parents: parents.split_whitespace().map(str::to_string).collect(),
            author: author.to_string(),
            time: time.parse().unwrap_or(0),
            refs: parse_refs(decorations),
            subject: subject.to_string(),
        });
    }
    commits
}

/// Finds the commits that no remote branch contains.
/// `commits` is in topological order: each commit comes before its parents.
fn find_local_only(commits: &[Meta]) -> Vec<bool> {
    let is_remote = |meta: &Meta| meta.refs.iter().any(|r| r.kind == "remote");
    if !commits.iter().any(is_remote) {
        // No remote branch known: the distinction has no meaning.
        return vec![false; commits.len()];
    }
    let mut pushed: HashSet<&str> = HashSet::new();
    commits
        .iter()
        .map(|meta| {
            let on_remote = is_remote(meta) || pushed.contains(meta.sha.as_str());
            if on_remote {
                // Every ancestor of a pushed commit is pushed too.
                pushed.extend(meta.parents.iter().map(String::as_str));
            }
            !on_remote
        })
        .collect()
}

/// Counts the changed files in the output of `git status --porcelain -z`.
fn count_changes(status: &[u8]) -> usize {
    let mut count = 0;
    let mut skip_next = false;
    for entry in status.split(|b| *b == 0) {
        if entry.is_empty() {
            continue;
        }
        if skip_next {
            // Old path of a rename: it belongs to the previous entry.
            skip_next = false;
            continue;
        }
        count += 1;
        if entry.len() > 2 && matches!(entry[0], b'R' | b'C') {
            skip_next = true;
        }
    }
    count
}

pub fn load(dir: &Path, limit: Option<u32>) -> Result<LogResult, String> {
    let mut commands = Vec::new();

    let top = step(&mut commands, dir, &["rev-parse", "--show-toplevel"])
        .map_err(|e| format!("This folder is not a Git repository. {e}"))?;
    let top = text(&top);
    let root = Path::new(&top);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| top.clone());

    let head = step(&mut commands, root, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .ok()
        .map(|b| text(&b))
        .filter(|s| !s.is_empty());
    let branch = step(&mut commands, root, &["symbolic-ref", "--short", "--quiet", "HEAD"])
        .ok()
        .map(|b| text(&b))
        .filter(|s| !s.is_empty());

    // One commit more than the limit: that is what reveals a truncated history.
    let limit = limit.filter(|n| *n > 0).map(|n| n as usize);
    let max_count = limit.map(|n| format!("--max-count={}", n + 1));
    let mut args = vec!["log", "--topo-order", "--decorate=full", FORMAT];
    if let Some(max_count) = &max_count {
        args.push(max_count);
    }
    // Neither the stash nor the backups Ramure creates before it rewrites history.
    args.extend(["--exclude=refs/stash", "--exclude=refs/ramure/*", "--all"]);
    if head.is_some() {
        args.push("HEAD");
    }

    let start = Instant::now();
    let raw = match step(&mut commands, root, &args) {
        Ok(raw) => raw,
        // A repository with no commit at all: git log fails, and the history is simply empty.
        Err(_) if head.is_none() => Vec::new(),
        Err(e) => return Err(e),
    };
    let git_ms = ms(start);

    let start = Instant::now();
    let mut metas = parse_log(&String::from_utf8_lossy(&raw));
    let truncated = limit.is_some_and(|n| metas.len() > n);
    if let Some(n) = limit {
        metas.truncate(n);
    }
    let local_only = find_local_only(&metas);
    let parse_ms = ms(start);

    let wip_files = step(&mut commands, root, &["status", "--porcelain", "-z", "--untracked-files=all"])
        .map(|out| count_changes(&out))
        .unwrap_or(0);
    let wip_parent = head.as_deref().filter(|_| wip_files > 0);

    let start = Instant::now();
    let mut input: Vec<(&str, Vec<&str>)> = Vec::with_capacity(metas.len() + 1);
    if let Some(parent) = wip_parent {
        input.push(("WIP", vec![parent]));
    }
    for meta in &metas {
        input.push((meta.sha.as_str(), meta.parents.iter().map(String::as_str).collect()));
    }
    let (layout, max_lanes) = graph::assign_lanes(&input);
    let layout_ms = ms(start);
    drop(input);

    let mut layout = layout.into_iter();
    let mut commits = Vec::with_capacity(metas.len() + 1);
    if let Some(parent) = wip_parent {
        if let Some(row) = layout.next() {
            commits.push(CommitRow {
                sha: "WIP".into(),
                parents: vec![parent.to_string()],
                author: String::new(),
                time: 0,
                refs: Vec::new(),
                subject: String::new(),
                lane: row.lane,
                edges: row.edges,
                wip: true,
                local_only: false,
            });
        }
    }
    for ((meta, row), local_only) in metas.into_iter().zip(layout).zip(local_only) {
        commits.push(CommitRow {
            sha: meta.sha,
            parents: meta.parents,
            author: meta.author,
            time: meta.time,
            refs: meta.refs,
            subject: meta.subject,
            lane: row.lane,
            edges: row.edges,
            wip: false,
            local_only,
        });
    }

    Ok(LogResult {
        repo: top,
        name,
        branch,
        commits,
        max_lanes,
        truncated,
        wip_files,
        timings: Timings { git_ms, parse_ms, layout_ms },
        commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_are_classified() {
        let refs = parse_refs(
            "HEAD -> refs/heads/main, tag: refs/tags/v0.2.0, refs/remotes/origin/main, refs/remotes/origin/HEAD, refs/heads/fix/known-hosts",
        );
        let pairs: Vec<(&str, &str)> = refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
        assert_eq!(
            pairs,
            vec![
                ("main", "head"),
                ("v0.2.0", "tag"),
                ("origin/main", "remote"),
                ("fix/known-hosts", "local"),
            ]
        );
    }

    #[test]
    fn detached_head_is_reported() {
        let refs = parse_refs("HEAD, refs/heads/main");
        assert_eq!(refs[0], RefLabel { name: "HEAD".into(), kind: "detached" });
        assert_eq!(refs[1].kind, "local");
    }

    #[test]
    fn log_records_are_parsed() {
        let raw = "aaa\x1fbbb ccc\x1fJane\x1f1730000000\x1fHEAD -> refs/heads/main\x1fMerge: subject, with a comma\x1e\nbbb\x1f\x1fPaul\x1f1720000000\x1f\x1finit\x1e\n";
        let commits = parse_log(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, vec!["bbb", "ccc"]);
        assert_eq!(commits[0].subject, "Merge: subject, with a comma");
        assert_eq!(commits[0].refs[0].kind, "head");
        assert!(commits[1].parents.is_empty());
        assert_eq!(commits[1].time, 1_720_000_000);
    }

    #[test]
    fn unpushed_commits_are_local_only() {
        // d (main, local) → c (origin/main) → a; e (topic, local) → a.
        let raw = "d\x1fc\x1fA\x1f4\x1fHEAD -> refs/heads/main\x1fd\x1e\
e\x1fa\x1fA\x1f3\x1frefs/heads/topic\x1fe\x1e\
c\x1fa\x1fA\x1f2\x1frefs/remotes/origin/main\x1fc\x1e\
a\x1f\x1fA\x1f1\x1f\x1fa\x1e";
        let commits = parse_log(raw);
        assert_eq!(find_local_only(&commits), vec![true, true, false, false]);
    }

    #[test]
    fn nothing_is_local_only_without_a_remote() {
        let commits = parse_log("b\x1fa\x1fA\x1f2\x1fHEAD -> refs/heads/main\x1fb\x1e\na\x1f\x1fA\x1f1\x1f\x1fa\x1e");
        assert_eq!(find_local_only(&commits), vec![false, false]);
    }

    #[test]
    fn renames_count_once() {
        assert_eq!(count_changes(b" M a.rs\0R  new.rs\0old.rs\0?? c.md\0"), 3);
        assert_eq!(count_changes(b""), 0);
    }

    /// Measures a real repository: `RAMURE_TEST_REPO=/path cargo test real_repo -- --nocapture`.
    #[test]
    fn real_repo_loads() {
        let Ok(path) = std::env::var("RAMURE_TEST_REPO") else { return };
        let limit = std::env::var("RAMURE_TEST_LIMIT").ok().and_then(|v| v.parse().ok());
        let start = Instant::now();
        let result = load(Path::new(&path), limit).expect("loading the repository");
        println!(
            "{}: {} commits, {} lanes, truncated = {}, WIP = {} files",
            result.name,
            result.commits.len(),
            result.max_lanes,
            result.truncated,
            result.wip_files
        );
        println!(
            "git {:.0} ms, parse {:.0} ms, lanes {:.0} ms, total {:.0} ms",
            result.timings.git_ms,
            result.timings.parse_ms,
            result.timings.layout_ms,
            ms(start)
        );
        let json = serde_json::to_string(&result).unwrap();
        println!("IPC payload: {:.1} MB", json.len() as f64 / 1_048_576.0);
        // `RAMURE_TEST_DUMP=file.json`: writes the reply, to test the interface outside Tauri.
        if let Ok(dump) = std::env::var("RAMURE_TEST_DUMP") {
            std::fs::write(dump, &json).expect("writing the test file");
        }
        assert!(!result.commits.is_empty());
    }
}
