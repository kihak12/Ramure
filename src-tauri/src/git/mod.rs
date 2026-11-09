//! Pilotage du binaire `git` installé sur la machine.
//! Ramure ne réimplémente pas Git : il lance les commandes officielles et parse leur sortie.

pub mod graph;
pub mod log;

use std::path::Path;
use std::process::Command;

/// Lance `git` dans `dir` et renvoie sa sortie standard brute.
/// En cas d'échec, renvoie le message d'erreur de git.
pub fn run(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args(args)
        // Jamais de question posée dans un terminal invisible.
        .env("GIT_TERMINAL_PROMPT", "0");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let out = cmd
        .output()
        .map_err(|e| format!("impossible de lancer git : {e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git s'est arrêté avec le code {:?}", out.status.code())
        } else {
            stderr
        });
    }
    Ok(out.stdout)
}

/// Charge l'historique d'un dépôt, déjà mis en page pour le graphe.
#[tauri::command]
pub async fn git_log(repo: String, limit: Option<u32>) -> Result<log::LogResult, String> {
    tauri::async_runtime::spawn_blocking(move || log::load(Path::new(&repo), limit))
        .await
        .map_err(|e| e.to_string())?
}
