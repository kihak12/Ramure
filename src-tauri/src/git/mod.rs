//! Pilotage du binaire `git` installé sur la machine.
//! Ramure ne réimplémente pas Git : il lance les commandes officielles et parse leur sortie.

pub mod graph;
pub mod log;
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

pub fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

/// Lance `git` dans `dir` et renvoie sa sortie complète, quel que soit le code de retour.
/// `input`, s'il est fourni, est écrit sur l'entrée standard de git.
pub fn exec(dir: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Output, String> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args(args)
        // Jamais de question posée dans un terminal invisible.
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("impossible de lancer git : {e}"))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        // Si git s'arrête avant d'avoir tout lu, son message d'erreur est plus utile que le nôtre.
        let _ = stdin.write_all(input);
    }
    child
        .wait_with_output()
        .map_err(|e| format!("impossible de lancer git : {e}"))
}

fn failure(out: &Output) -> String {
    let stderr = text(&out.stderr);
    if !stderr.is_empty() {
        return stderr;
    }
    // Certaines commandes (`git commit` sans rien d'indexé) s'expliquent sur la sortie standard.
    let stdout = text(&out.stdout);
    if !stdout.is_empty() {
        return stdout;
    }
    format!("git s'est arrêté avec le code {:?}", out.status.code())
}

/// Lance `git` dans `dir` et renvoie sa sortie standard brute.
/// En cas d'échec, renvoie le message d'erreur de git.
#[cfg(test)]
pub fn run(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = exec(dir, args, None)?;
    if out.status.success() { Ok(out.stdout) } else { Err(failure(&out)) }
}

/// Comme [`exec`], en consignant la commande dans le journal.
/// Les codes de retour listés dans `accepted` comptent comme des réussites.
pub fn step_full(
    journal: &mut Vec<CommandLog>,
    dir: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    accepted: &[i32],
) -> Result<Output, String> {
    let start = Instant::now();
    let result = exec(dir, args, input).and_then(|out| {
        let fine = out.status.success() || out.status.code().is_some_and(|c| accepted.contains(&c));
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
    step_full(journal, dir, args, None, &[]).map(|out| out.stdout)
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
