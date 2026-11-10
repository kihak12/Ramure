//! Références et historique : branches, tags, remises, fusion, rebase, cherry-pick, reset,
//! opérations interrompues par un conflit et rebase interactif.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::work::{Diff, Status, check_sha, status, to_diff};
use super::{CommandLog, Done, Failed, Opts, blocking, check_name, report, run, step, step_with, text};

/// Messages de git en anglais, pour les rares sorties que Ramure doit interpréter.
const ENGLISH: [(&str, &str); 1] = [("LC_ALL", "C")];

// --- Lecture des références ---------------------------------------------------------------

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// La branche distante suivie n'existe plus.
    pub gone: bool,
    pub current: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedRef {
    pub name: String,
    /// Commit désigné (pour un tag annoté, le commit et non l'objet tag).
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

/// `[ahead 2, behind 1]`, `[ahead 2]`, `[gone]` ou vide → (avance, retard, disparue).
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

/// Parse la sortie de `for-each-ref` au format de [`refs`].
fn parse_refs(raw: &str, refs: &mut Refs) {
    for line in raw.lines() {
        let fields: Vec<&str> = line.split('\x1f').collect();
        let [full, object, peeled, upstream, track, head] = fields[..] else { continue };
        // Tag annoté : `*objectname` est le commit visé.
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
    // `upstream:track` est traduit par git : on impose l'anglais pour le lire.
    let opts = Opts { envs: &ENGLISH, ..Opts::default() };
    let raw = step_with(journal, repo, &["for-each-ref", format, "refs/heads", "refs/remotes", "refs/tags"], opts)?;
    parse_refs(&String::from_utf8_lossy(&raw.stdout), &mut refs);
    let stashes = step(journal, repo, &["stash", "list", "--format=%gd%x1f%H%x1f%gs%x1f%ct"])?;
    refs.stashes = parse_stashes(&String::from_utf8_lossy(&stashes));
    refs.remote_names = text(&step(journal, repo, &["remote"])?).lines().map(str::to_string).collect();
    Ok(refs)
}

/// Dépôt distant par défaut : `origin` s'il existe, sinon le premier.
pub fn default_remote(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<String, String> {
    let remotes = text(&step(journal, repo, &["remote"])?);
    remotes
        .lines()
        .find(|r| *r == "origin")
        .or_else(|| remotes.lines().next())
        .map(str::to_string)
        .ok_or_else(|| "Ce dépôt n'a aucun dépôt distant configuré.".to_string())
}

// --- Branches, tags et remises ------------------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum RefOp {
    Switch { name: String },
    /// Bascule sur la branche locale qui suit `name` (`origin/x`), en la créant au besoin.
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
        .ok_or_else(|| format!("branche distante invalide : « {name} »"))
}

/// Position de la branche locale qui porte le nom d'une branche distante, par rapport à elle.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    /// Nom de la branche locale ; `None` si elle n'existe pas encore.
    pub local: Option<String>,
    /// Commits de la branche locale absents de la branche distante.
    pub ahead: u32,
    /// Commits de la branche distante absents de la branche locale.
    pub behind: u32,
}

fn local_exists(repo: &Path, local: &str) -> bool {
    run(repo, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{local}")]).is_ok()
}

/// Compare la branche distante `remote` (`origin/main`) à la branche locale de même nom.
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
            // La branche locale est en retard sur la branche distante, sans commit à elle : on
            // l'avance. Si elle a divergé, on n'y touche pas : c'est à l'utilisateur de décider.
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
                .map_err(|_| format!("« {name} » n'est pas un nom de branche valide."))?;
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
                .map_err(|_| format!("« {name} » n'est pas un nom de tag valide."))?;
            let message = message.trim();
            if message.is_empty() {
                git(journal, &["tag", name, sha])
            } else {
                // Tag annoté : le message passe par l'entrée standard.
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

/// Contenu d'une remise, fichiers non suivis compris quand git le permet (2.32 ou plus récent).
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

// --- Opérations d'historique --------------------------------------------------------------

/// Résultat d'une opération qui peut s'arrêter sur un conflit.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// Ce que git a écrit.
    pub output: String,
    /// Opération restée en suspens (`merge`, `rebase`, `cherry-pick`, `revert`) : il faut
    /// résoudre les conflits puis continuer, ou abandonner.
    pub stopped: Option<&'static str>,
}

pub fn git_dir(repo: &Path) -> Option<PathBuf> {
    run(repo, &["rev-parse", "--absolute-git-dir"]).ok().map(|out| PathBuf::from(text(&out)))
}

/// Opération en cours dans le dépôt, d'après les fichiers que git laisse dans `.git`.
pub fn operation(repo: &Path) -> Option<&'static str> {
    let dir = git_dir(repo)?;
    // Un rebase arrêté sur un commit laisse aussi CHERRY_PICK_HEAD : on le teste en premier.
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

/// Un échec qui laisse une opération en suspens n'est pas une erreur : c'est un conflit.
pub fn guarded(repo: &Path, result: Result<Output, String>) -> Result<Outcome, String> {
    match result {
        Ok(out) => Ok(Outcome { output: report(&out), stopped: operation(repo) }),
        Err(message) => match operation(repo) {
            Some(op) => Ok(Outcome { output: message, stopped: Some(op) }),
            None => Err(message),
        },
    }
}

/// `AAAAMMJJ-HHMMSS` en temps universel, pour un instant donné en secondes Unix.
fn stamp_at(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Conversion jours → date civile (algorithme de Howard Hinnant).
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

/// Avant de réécrire l'historique, garde la position de la branche dans
/// `refs/ramure/backup/<branche>-<date>` : `git reset --hard <cette référence>` y revient.
fn backup(journal: &mut Vec<CommandLog>, repo: &Path) -> Result<(), String> {
    let Ok(branch) = run(repo, &["symbolic-ref", "--short", "--quiet", "HEAD"]) else {
        return Ok(()); // HEAD détaché : aucune branche ne bouge
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let name = format!("refs/ramure/backup/{}-{}", text(&branch), stamp_at(now));
    step(journal, repo, &["update-ref", &name, "HEAD"]).map(drop)
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum HistoryOp {
    /// Fusionne `target` dans la branche courante.
    Merge { target: String },
    /// Rejoue la branche courante sur `onto`.
    Rebase { onto: String },
    /// `mainline` : le commit est une fusion, on reprend ses changements par rapport au premier parent.
    CherryPick { sha: String, mainline: bool },
    Revert { sha: String, mainline: bool },
    /// `mode` : `soft`, `mixed` ou `hard`.
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
                other => return Err(format!("mode de reset inconnu : {other}")),
            };
            backup(journal, repo)?;
            vec!["reset", flag, sha]
        }
    };
    let result = step_with(journal, repo, &args, Opts::default());
    guarded(repo, result)
}

/// Continue, passe ou abandonne l'opération en cours. `action` : `continue`, `skip` ou `abort`.
pub fn operate(journal: &mut Vec<CommandLog>, repo: &Path, action: &str) -> Result<Outcome, String> {
    let op = operation(repo).ok_or("Aucune opération n'est en cours dans ce dépôt.")?;
    let flag = match action {
        "continue" => "--continue",
        "skip" => "--skip",
        "abort" => "--abort",
        other => return Err(format!("action inconnue : {other}")),
    };
    let args: Vec<&str> = match (op, action) {
        // `git merge --continue` n'existe que depuis git 2.12 ; `git commit` fait la même chose.
        ("merge", "continue") => vec!["commit", "--no-edit"],
        ("merge", "skip") => return Err("Une fusion ne se passe pas : il faut la terminer ou l'abandonner.".into()),
        (op, _) => vec![op, flag],
    };
    let result = step_with(journal, repo, &args, Opts::default());
    guarded(repo, result)
}

/// Résout un fichier en conflit en gardant un seul côté. `side` : `ours` ou `theirs`.
pub fn resolve(journal: &mut Vec<CommandLog>, repo: &Path, file: &str, side: &str) -> Result<(), String> {
    let flag = match side {
        "ours" => "--ours",
        "theirs" => "--theirs",
        other => return Err(format!("côté inconnu : {other}")),
    };
    let opts = Opts { envs: &ENGLISH, ..Opts::default() };
    match step_with(journal, repo, &["checkout", flag, "--", file], opts) {
        Ok(_) => step(journal, repo, &["add", "--", file]).map(drop),
        // Ce côté a supprimé le fichier : le garder, c'est supprimer le fichier.
        Err(message) if message.contains("does not have") => step(journal, repo, &["rm", "-q", "--", file]).map(drop),
        Err(message) => Err(message),
    }
}

// --- Rebase interactif --------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCommit {
    pub sha: String,
    pub subject: String,
    /// Message complet, pour le reformuler sans perdre sa description.
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    /// Commit sur lequel rejouer ; absent si la plage commence au tout premier commit.
    pub base: Option<String>,
    /// Commits à rejouer, du plus ancien au plus récent.
    pub commits: Vec<PlanCommit>,
    /// Au moins un commit de la plage existe sur un dépôt distant : il faudra un push forcé.
    pub pushed: bool,
}

/// Prépare un rebase interactif de `sha` (inclus) jusqu'à HEAD.
pub fn rebase_plan(journal: &mut Vec<CommandLog>, repo: &Path, sha: &str) -> Result<RebasePlan, String> {
    check_sha(sha)?;
    step(journal, repo, &["merge-base", "--is-ancestor", sha, "HEAD"])
        .map_err(|_| "Ce commit ne fait pas partie de la branche courante.".to_string())?;
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
            return Err("La plage contient un commit de fusion : le rebase interactif l'aplatirait. Choisis un commit plus récent.".into());
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
    /// `pick`, `reword`, `squash`, `fixup` ou `drop`.
    pub action: String,
    pub sha: String,
    /// Nouveau message, pour `reword`.
    pub message: Option<String>,
}

/// Chemin entre apostrophes pour le shell de git, barres obliques comprises sous Windows.
fn sh_quote(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Écrit la liste des actions et les messages modifiés dans `<.git>/ramure/`.
fn write_todo(dir: &Path, todo: &[TodoItem]) -> Result<PathBuf, String> {
    let io = |e: std::io::Error| format!("impossible de préparer le rebase : {e}");
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
                return Err("Le premier commit conservé ne peut pas être fusionné dans un précédent.".into());
            }
            "squash" | "fixup" => lines.push_str(&format!("{} {}\n", item.action, item.sha)),
            other => return Err(format!("action de rebase inconnue : {other}")),
        }
        first = false;
        if item.action == "reword" {
            let message = item.message.as_deref().map(str::trim).unwrap_or("");
            if message.is_empty() {
                return Err("Un commit à reformuler a un message vide.".into());
            }
            let file = dir.join(format!("message-{i}.txt"));
            fs::write(&file, message).map_err(io)?;
            // Le message est remplacé juste après que le commit a été rejoué.
            lines.push_str(&format!("exec git commit --amend -q -F {}\n", sh_quote(&file)));
        }
    }
    if first {
        return Err("Tous les commits seraient supprimés : utilise plutôt un reset.".into());
    }
    let file = dir.join("todo");
    fs::write(&file, lines).map_err(io)?;
    Ok(file)
}

/// Lance `git rebase -i` avec une liste d'actions écrite par Ramure.
///
/// git demande à « l'éditeur de séquence » de modifier sa liste ; `editor` est une commande
/// qui la remplace par la nôtre. Par défaut, c'est Ramure lui-même, relancé avec
/// `--sequence-editor` (voir `main.rs`) : aucun outil extérieur n'est requis, sur aucun système.
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
    let dir = git_dir(repo).ok_or("dossier .git introuvable")?.join("ramure");
    let file = write_todo(&dir, todo)?;
    let editor = match editor {
        Some(editor) => format!("{editor} {}", sh_quote(&file)),
        None => {
            let exe = std::env::current_exe().map_err(|e| format!("exécutable de Ramure introuvable : {e}"))?;
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

// --- Commandes exposées à l'interface -----------------------------------------------------

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
refs/heads/sujet\x1fbbb\x1f\x1f\x1f\x1f \n\
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
        // Tag annoté : c'est le commit visé qui compte.
        assert_eq!(refs.tags, vec![NamedRef { name: "v1".into(), sha: "eee".into() }]);
    }

    #[test]
    fn stamps_are_utc_dates() {
        assert_eq!(stamp_at(0), "19700101-000000");
        assert_eq!(stamp_at(1_700_000_000), "20231114-221320");
        assert_eq!(stamp_at(951_782_400), "20000229-000000");
    }

    /// Dépôt avec trois commits sur `main` : a.txt = "1\n", puis "1\n2\n", puis "1\n2\n3\n".
    fn three_commits(label: &str) -> Scratch {
        let repo = Scratch::new(label);
        let mut j = Vec::new();
        for (content, message) in [("1\n", "un"), ("1\n2\n", "deux"), ("1\n2\n3\n", "trois")] {
            repo.write("a.txt", content);
            stage(&mut j, &repo.0, &["a.txt".into()]).unwrap();
            commit(&mut j, &repo.0, message, false).unwrap();
        }
        repo
    }

    #[test]
    fn switching_to_a_remote_branch_updates_the_local_one() {
        let repo = three_commits("suivi");
        let dir = repo.0.as_path();
        let mut j = Vec::new();
        let third = repo.git(&["rev-parse", "HEAD"]);
        let second = repo.git(&["rev-parse", "HEAD~1"]);
        let origin = |sha: &str| repo.git(&["update-ref", "refs/remotes/origin/main", sha]);
        let switch = RefOp::SwitchRemote { name: "origin/main".into() };

        // Pas de branche locale de ce nom : rien à comparer.
        repo.git(&["update-ref", "refs/remotes/origin/autre", &third]);
        assert_eq!(relation(&mut j, dir, "origin/autre").unwrap(), Relation { local: None, ahead: 0, behind: 0 });

        // Branche locale en retard d'un commit : basculer l'avance jusqu'à la branche distante.
        origin(&third);
        repo.git(&["reset", "-q", "--hard", &second]);
        repo.git(&["switch", "-q", "-c", "ailleurs"]);
        let late = Relation { local: Some("main".into()), ahead: 0, behind: 1 };
        assert_eq!(relation(&mut j, dir, "origin/main").unwrap(), late);
        ref_op(&mut j, dir, &switch).unwrap();
        assert_eq!(repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]), "main");
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);

        // Branche locale en avance : on bascule sans y toucher.
        origin(&second);
        assert_eq!(relation(&mut j, dir, "origin/main").unwrap(), Relation { local: Some("main".into()), ahead: 1, behind: 0 });
        ref_op(&mut j, dir, &switch).unwrap();
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);

        // Branches divergées : on bascule, mais c'est à l'utilisateur de choisir comment les réunir.
        repo.git(&["switch", "-q", "--detach", &second]);
        repo.write("b.txt", "distant\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "distant", false).unwrap();
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

        let create = RefOp::BranchCreate { name: "sujet".into(), start: head.clone(), switch: true };
        ref_op(&mut j, dir, &create).unwrap();
        ref_op(&mut j, dir, &RefOp::BranchRename { name: "sujet".into(), new_name: "sujet/2".into() }).unwrap();
        let bad = RefOp::BranchCreate { name: "a..b".into(), start: head.clone(), switch: false };
        assert!(ref_op(&mut j, dir, &bad).is_err());
        assert!(ref_op(&mut j, dir, &RefOp::Switch { name: "--orphan".into() }).is_err());

        ref_op(&mut j, dir, &RefOp::TagCreate { name: "v1".into(), sha: head.clone(), message: String::new() }).unwrap();
        let annotated = RefOp::TagCreate { name: "v2".into(), sha: head.clone(), message: "version 2".into() };
        ref_op(&mut j, dir, &annotated).unwrap();

        repo.write("a.txt", "modifié\n");
        repo.write("neuf.txt", "x\n");
        ref_op(&mut j, dir, &RefOp::StashPush { message: "en cours".into() }).unwrap();
        assert!(status(&mut j, dir).unwrap().unstaged.is_empty());

        let all = refs(&mut j, dir).unwrap();
        let names: Vec<_> = all.branches.iter().map(|b| (b.name.as_str(), b.current)).collect();
        assert_eq!(names, vec![("main", false), ("sujet/2", true)]);
        assert_eq!(all.tags.len(), 2);
        assert!(all.tags.iter().all(|t| t.sha == head));
        assert_eq!(all.stashes.len(), 1);
        assert!(all.stashes[0].message.ends_with("en cours"));
        assert!(stash_show(&mut j, dir, 0).unwrap().text.contains("+modifié"));

        ref_op(&mut j, dir, &RefOp::StashPop { index: 0 }).unwrap();
        assert_eq!(status(&mut j, dir).unwrap().unstaged.len(), 2);
        ref_op(&mut j, dir, &RefOp::StashPush { message: String::new() }).unwrap();
        ref_op(&mut j, dir, &RefOp::StashDrop { index: 0 }).unwrap();

        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();
        ref_op(&mut j, dir, &RefOp::BranchDelete { name: "sujet/2".into(), force: false }).unwrap();
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

        // `sujet` part de « deux » et modifie la même ligne que « trois ».
        ref_op(&mut j, dir, &RefOp::BranchCreate { name: "sujet".into(), start: base, switch: true }).unwrap();
        repo.write("a.txt", "1\n2\nTROIS\n");
        stage(&mut j, dir, &["a.txt".into()]).unwrap();
        commit(&mut j, dir, "trois, autrement", false).unwrap();
        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();

        let merge = HistoryOp::Merge { target: "sujet".into() };
        let outcome = history(&mut j, dir, &merge).unwrap();
        assert_eq!(outcome.stopped, Some("merge"));
        let s = status(&mut j, dir).unwrap();
        assert_eq!(s.operation, Some("merge"));
        assert_eq!(s.unstaged[0].kind, "conflict");

        // Continuer sans résoudre : git refuse, l'opération reste en suspens.
        assert_eq!(operate(&mut j, dir, "continue").unwrap().stopped, Some("merge"));
        assert!(operate(&mut j, dir, "skip").is_err());
        assert_eq!(operate(&mut j, dir, "abort").unwrap().stopped, None);
        assert_eq!(status(&mut j, dir).unwrap().operation, None);

        // Seconde tentative : on garde la version de `sujet`, puis on termine.
        history(&mut j, dir, &merge).unwrap();
        resolve(&mut j, dir, "a.txt", "theirs").unwrap();
        assert_eq!(operate(&mut j, dir, "continue").unwrap().stopped, None);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n2\nTROIS\n");
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

        // Branche partie du premier commit, avec un fichier à elle.
        ref_op(&mut j, dir, &RefOp::BranchCreate { name: "sujet".into(), start: first.clone(), switch: true }).unwrap();
        repo.write("b.txt", "b\n");
        stage(&mut j, dir, &["b.txt".into()]).unwrap();
        commit(&mut j, dir, "ajout de b", false).unwrap();
        let on_topic = repo.git(&["rev-parse", "HEAD"]);

        // Rebase sans conflit : la branche repart de `main`, et une sauvegarde existe.
        assert_eq!(history(&mut j, dir, &HistoryOp::Rebase { onto: "main".into() }).unwrap().stopped, None);
        assert_eq!(repo.git(&["rev-parse", "HEAD~1"]), third);
        let backups = repo.git(&["for-each-ref", "--format=%(refname) %(objectname)", "refs/ramure/backup"]);
        assert!(backups.starts_with("refs/ramure/backup/sujet-") && backups.ends_with(&on_topic));

        // Cherry-pick sur main, puis revert.
        ref_op(&mut j, dir, &RefOp::Switch { name: "main".into() }).unwrap();
        let pick = HistoryOp::CherryPick { sha: on_topic.clone(), mainline: false };
        assert_eq!(history(&mut j, dir, &pick).unwrap().stopped, None);
        assert!(dir.join("b.txt").exists());
        let picked = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(history(&mut j, dir, &HistoryOp::Revert { sha: picked, mainline: false }).unwrap().stopped, None);
        assert!(!dir.join("b.txt").exists());

        // Reset : soft garde les changements indexés, hard les jette.
        history(&mut j, dir, &HistoryOp::Reset { sha: third.clone(), mode: "soft".into() }).unwrap();
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), third);
        history(&mut j, dir, &HistoryOp::Reset { sha: first.clone(), mode: "hard".into() }).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n");
        assert!(history(&mut j, dir, &HistoryOp::Reset { sha: first, mode: "dur".into() }).is_err());

        // Les sauvegardes ne doivent pas polluer le graphe.
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
        commit(&mut j, dir, "quatre", false).unwrap();

        let second = repo.git(&["rev-parse", "HEAD~2"]);
        let plan = rebase_plan(&mut j, dir, &second).unwrap();
        let subjects: Vec<_> = plan.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["deux", "trois", "quatre"]);
        assert!(plan.base.is_some() && !plan.pushed);

        // « quatre » passe en premier et change de message, « trois » est fusionné dans « deux ».
        let item = |action: &str, n: usize, message: Option<&str>| TodoItem {
            action: action.into(),
            sha: plan.commits[n].sha.clone(),
            message: message.map(str::to_string),
        };
        let todo = [item("reword", 2, Some("QUATRE")), item("pick", 0, None), item("fixup", 1, None)];
        let outcome = rebase_interactive(&mut j, dir, plan.base.as_deref(), &todo, Some("cp")).unwrap();
        assert_eq!(outcome.stopped, None, "{}", outcome.output);
        assert_eq!(repo.git(&["log", "--format=%s"]), "deux\nQUATRE\nun");
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n2\n3\n");

        // Garde-fous.
        let plan = rebase_plan(&mut j, dir, &repo.git(&["rev-parse", "HEAD~2"])).unwrap();
        assert!(plan.base.is_none(), "la plage part du premier commit");
        let squash_first = [TodoItem { action: "squash".into(), sha: plan.commits[0].sha.clone(), message: None }];
        assert!(rebase_interactive(&mut j, dir, None, &squash_first, Some("cp")).is_err());
        let drop_all = [TodoItem { action: "drop".into(), sha: plan.commits[0].sha.clone(), message: None }];
        assert!(rebase_interactive(&mut j, dir, None, &drop_all, Some("cp")).is_err());
    }
}
