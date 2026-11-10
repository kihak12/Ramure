//! Pilotage du binaire `git` installé sur la machine.
//! Ramure ne réimplémente pas Git : il lance les commandes officielles et parse leur sortie.

pub mod clone;
pub mod conflict;
pub mod graph;
pub mod log;
pub mod ops;
pub mod patch;
pub mod work;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use serde::Serialize;

/// Une commande git lancée par Ramure, telle qu'elle apparaît dans le journal.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLog {
    pub command: String,
    pub ms: f64,
    pub ok: bool,
}

/// Résultat d'une opération réussie, avec les commandes qu'elle a lancées.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Done<T> {
    pub data: T,
    pub commands: Vec<CommandLog>,
}

/// Échec d'une opération : le message de git, et les commandes lancées jusque-là.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failed {
    pub message: String,
    pub commands: Vec<CommandLog>,
}

/// Options d'une commande git. Les valeurs par défaut conviennent à la plupart des appels.
#[derive(Default, Clone, Copy)]
pub struct Opts<'a> {
    /// Écrit sur l'entrée standard de git.
    pub input: Option<&'a [u8]>,
    /// Codes de retour à considérer comme des réussites, en plus de zéro.
    pub accepted: &'a [i32],
    /// Variables d'environnement ajoutées à celles de Ramure.
    pub envs: &'a [(&'a str, &'a str)],
}

pub fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

/// Nettoie une sortie destinée à un terminal : git y réécrit sa ligne d'avancement avec des
/// retours chariot et des séquences d'effacement. On ne garde que l'état final de chaque ligne.
pub fn plain(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            // Séquence `ESC [ … lettre` : couleur ou effacement de ligne.
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out.lines()
        .filter_map(|line| line.rsplit('\r').find(|part| !part.trim().is_empty()))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Commande `git` de base : sans fenêtre de console, sans question, sans éditeur.
pub fn command() -> Command {
    let mut cmd = Command::new("git");
    // Jamais de question posée dans un terminal invisible.
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        // Ni d'éditeur : `:` est l'éditeur « ne rien faire » de git, il garde le texte proposé.
        .env("GIT_EDITOR", ":")
        .env("GIT_SEQUENCE_EDITOR", ":");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Lance `git` dans `dir` et renvoie sa sortie complète, quel que soit le code de retour.
pub fn exec(dir: &Path, args: &[&str], opts: Opts) -> Result<Output, String> {
    let mut cmd = command();
    cmd.arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args(args)
        .envs(opts.envs.iter().copied())
        .stdin(if opts.input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("impossible de lancer git : {e}"))?;
    if let (Some(input), Some(mut stdin)) = (opts.input, child.stdin.take()) {
        // Si git s'arrête avant d'avoir tout lu, son message d'erreur est plus utile que le nôtre.
        let _ = stdin.write_all(input);
    }
    child
        .wait_with_output()
        .map_err(|e| format!("impossible de lancer git : {e}"))
}

fn failure(out: &Output) -> String {
    let stderr = plain(&out.stderr);
    let stdout = plain(&out.stdout);
    // Une fusion en conflit s'explique sur la sortie standard, un rebase sur la sortie d'erreur :
    // on garde les deux.
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!("git s'est arrêté avec le code {:?}", out.status.code()),
        (true, false) => stderr,
        (false, true) => stdout,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

/// Ce que git a écrit, sortie standard puis sortie d'erreur : l'avancement d'un push ou
/// d'un fetch passe par la seconde, même en cas de succès.
pub fn report(out: &Output) -> String {
    let mut report = plain(&out.stdout);
    let stderr = plain(&out.stderr);
    if !stderr.is_empty() {
        if !report.is_empty() {
            report.push('\n');
        }
        report.push_str(&stderr);
    }
    report
}

/// Lance `git` sans le consigner : pour les lectures internes qui encombreraient le journal.
pub fn run(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = exec(dir, args, Opts::default())?;
    if out.status.success() { Ok(out.stdout) } else { Err(failure(&out)) }
}

/// Lance git avec des options et consigne la commande dans le journal.
pub fn step_with(
    journal: &mut Vec<CommandLog>,
    dir: &Path,
    args: &[&str],
    opts: Opts,
) -> Result<Output, String> {
    let start = Instant::now();
    let result = exec(dir, args, opts).and_then(|out| {
        let fine = out.status.success() || out.status.code().is_some_and(|c| opts.accepted.contains(&c));
        if fine { Ok(out) } else { Err(failure(&out)) }
    });
    journal.push(CommandLog {
        command: format!("git {}", args.join(" ")),
        ms: ms(start),
        ok: result.is_ok(),
    });
    result
}

/// Lance git, consigne la commande dans le journal et renvoie la sortie standard.
pub fn step(journal: &mut Vec<CommandLog>, dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    step_with(journal, dir, args, Opts::default()).map(|out| out.stdout)
}

/// Un nom fourni par l'utilisateur (branche, tag, commit) ne doit jamais pouvoir passer pour
/// une option de git.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_control) {
        Err(format!("nom invalide : « {name} »"))
    } else {
        Ok(())
    }
}

/// Exécute une opération git hors du fil de l'interface et lui attache son journal.
pub async fn blocking<T, F>(op: F) -> Result<Done<T>, Failed>
where
    T: Send + 'static,
    F: FnOnce(&mut Vec<CommandLog>) -> Result<T, String> + Send + 'static,
{
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut commands = Vec::new();
        let result = op(&mut commands);
        (result, commands)
    })
    .await;
    match joined {
        Ok((Ok(data), commands)) => Ok(Done { data, commands }),
        Ok((Err(message), commands)) => Err(Failed { message, commands }),
        Err(e) => Err(Failed { message: e.to_string(), commands: Vec::new() }),
    }
}

/// Charge l'historique d'un dépôt, déjà mis en page pour le graphe.
#[tauri::command]
pub async fn git_log(repo: String, limit: Option<u32>) -> Result<log::LogResult, String> {
    tauri::async_runtime::spawn_blocking(move || log::load(Path::new(&repo), limit))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_are_reduced_to_their_final_state() {
        let raw = b"Rebasing (1/3)\rRebasing (2/3)\rRebasing (3/3)\r\x1b[KSuccessfully rebased.\nremote: \x1b[32mok\x1b[0m\r\n\n";
        assert_eq!(plain(raw), "Successfully rebased.\nremote: ok");
        assert_eq!(plain(b""), "");
    }

    #[test]
    fn option_like_names_are_refused() {
        assert!(check_name("feat/x").is_ok());
        assert!(check_name("--force").is_err());
        assert!(check_name("").is_err());
        assert!(check_name("a\nb").is_err());
    }
}
