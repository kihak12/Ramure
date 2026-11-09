//! Lecture de l'historique : `git log` → commits parsés → lignes du graphe.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use super::{CommandLog, graph, ms, step, text};

/// Champs séparés par 0x1f, commits terminés par 0x1e : aucun de ces octets n'apparaît dans un sujet.
const FORMAT: &str = "--format=%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefLabel {
    pub name: String,
    /// `head` (branche courante), `local`, `remote`, `tag` ou `detached`.
    pub kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    /// Date de l'auteur, en secondes Unix.
    pub time: i64,
    pub refs: Vec<RefLabel>,
    pub subject: String,
    pub lane: u16,
    pub edges: Vec<[u16; 3]>,
    /// Ligne « modifications en cours », placée en tête quand le répertoire de travail n'est pas propre.
    pub wip: bool,
    /// Vrai si le commit n'est atteignable depuis aucune branche distante : il n'a pas été poussé.
    /// Toujours faux quand le dépôt ne suit aucune branche distante.
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
    /// Racine du dépôt.
    pub repo: String,
    pub name: String,
    pub branch: Option<String>,
    pub commits: Vec<CommitRow>,
    pub max_lanes: u16,
    /// Vrai si l'historique dépasse la limite demandée.
    pub truncated: bool,
    pub wip_files: usize,
    pub timings: Timings,
    /// Commandes git lancées, pour le journal.
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

/// Décorations au format `--decorate=full` :
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
            // `origin/HEAD` n'apporte rien à l'affichage.
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

/// Repère les commits absents de toute branche distante.
/// `commits` est en ordre topologique : chaque commit précède ses parents.
fn find_local_only(commits: &[Meta]) -> Vec<bool> {
    let is_remote = |meta: &Meta| meta.refs.iter().any(|r| r.kind == "remote");
    if !commits.iter().any(is_remote) {
        // Aucune branche distante connue : la distinction n'a pas de sens.
        return vec![false; commits.len()];
    }
    let mut pushed: HashSet<&str> = HashSet::new();
    commits
        .iter()
        .map(|meta| {
            let on_remote = is_remote(meta) || pushed.contains(meta.sha.as_str());
            if on_remote {
                // Tout ancêtre d'un commit poussé l'est aussi.
                pushed.extend(meta.parents.iter().map(String::as_str));
            }
            !on_remote
        })
        .collect()
}

/// Compte les fichiers modifiés dans la sortie de `git status --porcelain -z`.
fn count_changes(status: &[u8]) -> usize {
    let mut count = 0;
    let mut skip_next = false;
    for entry in status.split(|b| *b == 0) {
        if entry.is_empty() {
            continue;
        }
        if skip_next {
            // Ancien chemin d'un renommage : il appartient à l'entrée précédente.
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
    let mut journal = Vec::new();

    let top = step(&mut journal, dir, &["rev-parse", "--show-toplevel"])
        .map_err(|e| format!("Ce dossier n'est pas un dépôt Git. {e}"))?;
    let top = text(&top);
    let root = Path::new(&top);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| top.clone());

    let head = step(&mut journal, root, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .ok()
        .map(|b| text(&b))
        .filter(|s| !s.is_empty());
    let branch = step(&mut journal, root, &["symbolic-ref", "--short", "--quiet", "HEAD"])
        .ok()
        .map(|b| text(&b))
        .filter(|s| !s.is_empty());

    // Un commit de plus que la limite : c'est ce qui révèle un historique tronqué.
    let limit = limit.filter(|n| *n > 0).map(|n| n as usize);
    let max_count = limit.map(|n| format!("--max-count={}", n + 1));
    let mut args = vec!["log", "--topo-order", "--decorate=full", FORMAT];
    if let Some(max_count) = &max_count {
        args.push(max_count);
    }
    args.extend(["--exclude=refs/stash", "--all"]);
    if head.is_some() {
        args.push("HEAD");
    }

    let start = Instant::now();
    let raw = match step(&mut journal, root, &args) {
        Ok(raw) => raw,
        // Dépôt sans aucun commit : git log échoue, l'historique est simplement vide.
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

    let wip_files = step(&mut journal, root, &["status", "--porcelain", "-z", "--untracked-files=all"])
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
        commands: journal,
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
        let raw = "aaa\x1fbbb ccc\x1fJeanne\x1f1730000000\x1fHEAD -> refs/heads/main\x1fMerge: sujet, avec virgule\x1e\nbbb\x1f\x1fPaul\x1f1720000000\x1f\x1finit\x1e\n";
        let commits = parse_log(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, vec!["bbb", "ccc"]);
        assert_eq!(commits[0].subject, "Merge: sujet, avec virgule");
        assert_eq!(commits[0].refs[0].kind, "head");
        assert!(commits[1].parents.is_empty());
        assert_eq!(commits[1].time, 1_720_000_000);
    }

    #[test]
    fn unpushed_commits_are_local_only() {
        // d (main, local) → c (origin/main) → a ; e (sujet, local) → a.
        let raw = "d\x1fc\x1fA\x1f4\x1fHEAD -> refs/heads/main\x1fd\x1e\
e\x1fa\x1fA\x1f3\x1frefs/heads/sujet\x1fe\x1e\
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

    /// Mesure sur un vrai dépôt : `RAMURE_TEST_REPO=/chemin cargo test real_repo -- --nocapture`.
    #[test]
    fn real_repo_loads() {
        let Ok(path) = std::env::var("RAMURE_TEST_REPO") else { return };
        let limit = std::env::var("RAMURE_TEST_LIMIT").ok().and_then(|v| v.parse().ok());
        let start = Instant::now();
        let result = load(Path::new(&path), limit).expect("chargement du dépôt");
        println!(
            "{} : {} commits, {} lanes, tronqué = {}, WIP = {} fichiers",
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
        println!("charge utile IPC : {:.1} Mo", json.len() as f64 / 1_048_576.0);
        // `RAMURE_TEST_DUMP=fichier.json` : écrit la réponse, pour tester l'interface hors de Tauri.
        if let Ok(dump) = std::env::var("RAMURE_TEST_DUMP") {
            std::fs::write(dump, &json).expect("écriture du fichier de test");
        }
        assert!(!result.commits.is_empty());
    }
}
