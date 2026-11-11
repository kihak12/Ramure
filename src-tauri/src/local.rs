//! Terminaux locaux : le shell de l'utilisateur dans un pseudo-terminal, sur sa propre machine.
//!
//! Même contrat que les sessions SSH : l'interface écrit et redimensionne par des commandes
//! (`local_write`, `local_resize`, `local_close`), reçoit la sortie en octets bruts sur un canal
//! Tauri, et apprend la fin du shell par l'évènement `local-closed`. Le pseudo-terminal est
//! ConPTY sous Windows et openpty ailleurs (crate `portable-pty`).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};

/// Ce que l'interface garde d'un terminal ouvert pour lui parler.
struct Local {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    /// Le processus du shell : sert à savoir si un programme a pris le premier plan.
    shell_pid: Option<u32>,
}

#[derive(Default)]
pub struct LocalState {
    terminals: Mutex<HashMap<u32, Local>>,
    next_id: AtomicU32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    id: u32,
    /// Le shell lancé, tel qu'on le montre : `bash`, `zsh`, `powershell`.
    shell: String,
    /// Le dossier de départ.
    cwd: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Closed {
    id: u32,
    exit_status: Option<u32>,
    /// Le signal qui a arrêté le shell, s'il n'a pas fini de lui-même.
    signal: Option<String>,
}

/// Le shell de l'utilisateur : PowerShell sous Windows, `$SHELL` ailleurs, `/bin/sh` faute de mieux.
fn default_shell() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("powershell.exe")
    } else {
        std::env::var_os("SHELL").map(PathBuf::from).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("/bin/sh"))
    }
}

/// Le dossier personnel de l'utilisateur.
fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_dir())
}

/// Le dossier où ouvrir le terminal : `wanted` s'il existe, sinon le dossier personnel.
fn start_dir(wanted: Option<&str>) -> PathBuf {
    wanted
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Un dossier tel qu'on le montre : le dossier personnel s'écrit `~`.
fn shown(dir: &std::path::Path) -> String {
    match home_dir().and_then(|home| dir.strip_prefix(&home).ok().map(|rest| rest.to_path_buf())) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => dir.display().to_string(),
    }
}

/// Le nom court d'un shell : `/usr/bin/zsh` donne `zsh`, `powershell.exe` donne `powershell`.
fn shell_name(shell: &std::path::Path) -> String {
    shell.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| shell.display().to_string())
}

fn with_terminal<T>(state: &LocalState, id: u32, f: impl FnOnce(&mut Local) -> Result<T, String>) -> Result<T, String> {
    let mut terminals = state.terminals.lock().map_err(|_| "état des terminaux indisponible".to_string())?;
    let terminal = terminals.get_mut(&id).ok_or_else(|| "ce terminal est fermé".to_string())?;
    f(terminal)
}

/// Ouvre un terminal local de `cols` × `rows`, dans `cwd` ou le dossier personnel.
#[tauri::command]
pub fn local_open(
    app: AppHandle,
    state: State<'_, LocalState>,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
    on_data: Channel,
) -> Result<Opened, String> {
    let size = PtySize { rows: rows.max(2), cols: cols.max(2), pixel_width: 0, pixel_height: 0 };
    let pair = native_pty_system().openpty(size).map_err(|e| format!("Pseudo-terminal indisponible\u{a0}: {e}."))?;

    let shell = default_shell();
    let dir = start_dir(cwd.as_deref());
    let mut command = CommandBuilder::new(&shell);
    command.cwd(&dir);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    if cfg!(windows) {
        command.arg("-NoLogo");
    }
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| format!("{} n’a pas pu être lancé\u{a0}: {e}.", shell.display()))?;
    // Le côté esclave appartient désormais au shell : le garder ouvert retarderait la fin de lecture.
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let killer = child.clone_killer();
    let shell_pid = child.process_id();

    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    state
        .terminals
        .lock()
        .map_err(|_| "état des terminaux indisponible".to_string())?
        .insert(id, Local { writer, master: pair.master, killer, shell_pid });

    // La lecture est bloquante : un fil à part relaie la sortie vers l'interface.
    std::thread::spawn(move || {
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if on_data.send(InvokeResponseBody::Raw(buffer[..n].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Un second fil attend la fin du shell, range le terminal, et le dit à l'interface.
    std::thread::spawn(move || {
        let status = child.wait().ok();
        if let Ok(mut terminals) = app.state::<LocalState>().terminals.lock() {
            terminals.remove(&id);
        }
        let closed = Closed {
            id,
            exit_status: status.as_ref().map(|status| status.exit_code()),
            signal: status.as_ref().and_then(|status| status.signal().map(str::to_string)),
        };
        let _ = app.emit("local-closed", closed);
    });

    Ok(Opened { id, shell: shell_name(&shell), cwd: shown(&dir) })
}

#[tauri::command]
pub fn local_write(state: State<'_, LocalState>, id: u32, data: String) -> Result<(), String> {
    with_terminal(&state, id, |t| {
        t.writer.write_all(data.as_bytes()).and_then(|_| t.writer.flush()).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn local_resize(state: State<'_, LocalState>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    with_terminal(&state, id, |t| {
        t.master
            .resize(PtySize { rows: rows.max(2), cols: cols.max(2), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())
    })
}

/// Ferme le terminal : le shell est arrêté, et `local-closed` suit.
#[tauri::command]
pub fn local_close(state: State<'_, LocalState>, id: u32) -> Result<(), String> {
    with_terminal(&state, id, |t| t.killer.kill().map_err(|e| e.to_string()))
}

/// Un programme tourne-t-il au premier plan du terminal, autre que le shell lui-même ?
///
/// Sous Unix, le pseudo-terminal dit quel groupe de processus est au premier plan : c'est celui du
/// shell tant qu'il attend à son invite, celui du programme pendant qu'il tourne. `None` quand on
/// ne peut pas le savoir : ConPTY, sous Windows, n'a pas d'équivalent.
#[cfg(unix)]
fn foreground_is_program(master: &dyn MasterPty, shell_pid: Option<u32>) -> Option<bool> {
    let leader = master.process_group_leader()?;
    Some(i64::from(leader) != i64::from(shell_pid?))
}

#[cfg(not(unix))]
fn foreground_is_program(_master: &dyn MasterPty, _shell_pid: Option<u32>) -> Option<bool> {
    None
}

/// Pour chaque terminal de `ids`, dans l'ordre : `true` si un programme y tourne, `false` si le
/// shell attend à son invite ou si le terminal est fermé, `null` si on ne peut pas le savoir.
/// L'interface s'en sert avant de fermer un onglet ou la fenêtre (charte, section 9.20).
#[tauri::command]
pub fn local_busy(state: State<'_, LocalState>, ids: Vec<u32>) -> Result<Vec<Option<bool>>, String> {
    let terminals = state.terminals.lock().map_err(|_| "état des terminaux indisponible".to_string())?;
    Ok(ids
        .iter()
        .map(|id| match terminals.get(id) {
            Some(terminal) => foreground_is_program(terminal.master.as_ref(), terminal.shell_pid),
            None => Some(false),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_is_named_by_its_file_name() {
        assert_eq!(shell_name(std::path::Path::new("/usr/bin/zsh")), "zsh");
        assert_eq!(shell_name(std::path::Path::new("powershell.exe")), "powershell");
    }

    #[test]
    fn a_missing_folder_falls_back_to_the_home_folder() {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        assert_eq!(start_dir(Some("/ce/dossier/n/existe/pas")), home);
        assert_eq!(start_dir(None), home);
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_runs_in_a_pseudo_terminal_and_its_output_comes_back() {
        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .expect("pseudo-terminal");
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "echo ramure-$((40 + 2))"]);
        let mut child = pair.slave.spawn_command(command).expect("shell");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("lecture");
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        assert!(child.wait().expect("fin").success());
        assert!(output.contains("ramure-42"), "sortie : {output:?}");
    }

    /// Attend que `condition` devienne vraie, deux secondes au plus.
    #[cfg(unix)]
    fn eventually(mut condition: impl FnMut() -> bool) -> bool {
        for _ in 0..100 {
            if condition() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }

    #[cfg(unix)]
    #[test]
    fn a_running_program_takes_the_foreground_and_gives_it_back() {
        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .expect("pseudo-terminal");
        // Un shell interactif : c'est lui qui met chaque programme dans son propre groupe.
        let mut command = CommandBuilder::new("/bin/sh");
        command.arg("-i");
        let mut child = pair.slave.spawn_command(command).expect("shell");
        drop(pair.slave);
        let shell_pid = child.process_id();
        let mut reader = pair.master.try_clone_reader().expect("lecture");
        // La sortie doit être lue, sinon le shell se bloque en écrivant son invite.
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            while matches!(reader.read(&mut buffer), Ok(n) if n > 0) {}
        });
        let mut writer = pair.master.take_writer().expect("écriture");
        let master = pair.master.as_ref();

        assert!(eventually(|| foreground_is_program(master, shell_pid) == Some(false)), "à l'invite, rien ne tourne");
        writer.write_all(b"sleep 30\n").expect("commande");
        assert!(eventually(|| foreground_is_program(master, shell_pid) == Some(true)), "sleep tourne au premier plan");
        // Ctrl + C arrête le programme : le shell reprend le premier plan.
        writer.write_all(&[3]).expect("interruption");
        assert!(eventually(|| foreground_is_program(master, shell_pid) == Some(false)), "le shell a repris la main");

        writer.write_all(b"exit\n").expect("fin");
        let _ = child.wait();
    }
}
