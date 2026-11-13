//! Local terminals: the shell of the user in a pseudo-terminal, on their own machine.
//!
//! The same contract as SSH sessions: the interface writes and resizes through commands
//! (`local_write`, `local_resize`, `local_close`), receives the output as raw bytes on a Tauri
//! channel, and learns that the shell ended through the `local-closed` event. The pseudo-terminal
//! is ConPTY on Windows and openpty elsewhere (crate `portable-pty`).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};

/// What is kept of an open terminal to talk to it.
struct Local {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    /// The process of the shell: used to know whether a program has taken the foreground.
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
    /// The shell started, as it is shown: `bash`, `zsh`, `powershell`.
    shell: String,
    /// The starting folder.
    cwd: String,
    /// The log asked for could not be opened: why. The terminal opens all the same.
    journal_failed: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Closed {
    id: u32,
    exit_status: Option<u32>,
    /// The signal that stopped the shell, if it did not end by itself.
    signal: Option<String>,
}

/// The shell of the user: PowerShell on Windows, `$SHELL` elsewhere, `/bin/sh` for want of anything better.
fn default_shell() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("powershell.exe")
    } else {
        std::env::var_os("SHELL").map(PathBuf::from).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("/bin/sh"))
    }
}

/// The home folder of the user.
fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_dir())
}

/// The folder to open the terminal in: `wanted` if it exists, otherwise the home folder.
fn start_dir(wanted: Option<&str>) -> PathBuf {
    wanted
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// A folder as it is shown: the home folder is written `~`.
fn shown(dir: &std::path::Path) -> String {
    match home_dir().and_then(|home| dir.strip_prefix(&home).ok().map(|rest| rest.to_path_buf())) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => dir.display().to_string(),
    }
}

/// The short name of a shell: `/usr/bin/zsh` gives `zsh`, `powershell.exe` gives `powershell`.
fn shell_name(shell: &std::path::Path) -> String {
    shell.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| shell.display().to_string())
}

fn with_terminal<T>(state: &LocalState, id: u32, f: impl FnOnce(&mut Local) -> Result<T, String>) -> Result<T, String> {
    let mut terminals = state.terminals.lock().map_err(|_| "The terminals are unavailable. Restart Ramure.".to_string())?;
    let terminal = terminals.get_mut(&id).ok_or_else(|| "This terminal is closed.".to_string())?;
    f(terminal)
}

/// Opens a local terminal of `cols` × `rows`, in `cwd` or the home folder.
#[tauri::command]
pub fn local_open(
    app: AppHandle,
    state: State<'_, LocalState>,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
    journal: Option<crate::journal::JournalOpen>,
    on_data: Channel,
) -> Result<Opened, String> {
    let size = PtySize { rows: rows.max(2), cols: cols.max(2), pixel_width: 0, pixel_height: 0 };
    let pair = native_pty_system().openpty(size).map_err(|e| format!("The pseudo-terminal could not be opened: {e}."))?;

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
        .map_err(|e| format!("{} could not start: {e}.", shell.display()))?;
    // The slave side now belongs to the shell: keeping it open would delay the end of reading.
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let killer = child.clone_killer();
    let shell_pid = child.process_id();

    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    state
        .terminals
        .lock()
        .map_err(|_| "The terminals are unavailable. Restart Ramure.".to_string())?
        .insert(id, Local { writer, master: pair.master, killer, shell_pid });

    // The log of every session of the host opens before the first read.
    let key = format!("local:{id}");
    let journal_failed = journal.as_ref().and_then(|open| crate::journal::start(&key, open).err());

    // Reading blocks: a thread of its own passes the output on to the interface, and to the log.
    std::thread::spawn(move || {
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    crate::journal::feed(&key, &buffer[..n]);
                    if on_data.send(InvokeResponseBody::Raw(buffer[..n].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
        crate::journal::stop(&key);
    });

    // A second thread waits for the shell to end, puts the terminal away, and tells the interface.
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

    Ok(Opened { id, shell: shell_name(&shell), cwd: shown(&dir), journal_failed })
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

/// Closes the terminal: the shell is stopped, and `local-closed` follows.
#[tauri::command]
pub fn local_close(state: State<'_, LocalState>, id: u32) -> Result<(), String> {
    with_terminal(&state, id, |t| t.killer.kill().map_err(|e| e.to_string()))
}

/// Is a program other than the shell itself running in the foreground of the terminal?
///
/// On Unix, the pseudo-terminal says which process group is in the foreground: that of the shell
/// while it waits at its prompt, that of the program while it runs. `None` when there is no way
/// to know: ConPTY, on Windows, has no equivalent.
#[cfg(unix)]
fn foreground_is_program(master: &dyn MasterPty, shell_pid: Option<u32>) -> Option<bool> {
    let leader = master.process_group_leader()?;
    Some(i64::from(leader) != i64::from(shell_pid?))
}

#[cfg(not(unix))]
fn foreground_is_program(_master: &dyn MasterPty, _shell_pid: Option<u32>) -> Option<bool> {
    None
}

/// For each terminal of `ids`, in order: `true` if a program runs in it, `false` if the shell
/// waits at its prompt or the terminal is closed, `null` if there is no way to know. The interface
/// uses it before closing a tab or the window (guidelines, section 9.20).
#[tauri::command]
pub fn local_busy(state: State<'_, LocalState>, ids: Vec<u32>) -> Result<Vec<Option<bool>>, String> {
    let terminals = state.terminals.lock().map_err(|_| "The terminals are unavailable. Restart Ramure.".to_string())?;
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
        assert_eq!(start_dir(Some("/this/folder/does/not/exist")), home);
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
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        assert!(child.wait().expect("exit").success());
        assert!(output.contains("ramure-42"), "output: {output:?}");
    }

    /// Waits for `condition` to become true, two seconds at most.
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
        // An interactive shell: it is the one that puts each program in its own group.
        let mut command = CommandBuilder::new("/bin/sh");
        command.arg("-i");
        let mut child = pair.slave.spawn_command(command).expect("shell");
        drop(pair.slave);
        let shell_pid = child.process_id();
        let mut reader = pair.master.try_clone_reader().expect("reader");
        // The output must be read, otherwise the shell blocks while writing its prompt.
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            while matches!(reader.read(&mut buffer), Ok(n) if n > 0) {}
        });
        let mut writer = pair.master.take_writer().expect("writer");
        let master = pair.master.as_ref();

        assert!(eventually(|| foreground_is_program(master, shell_pid) == Some(false)), "at the prompt, nothing runs");
        writer.write_all(b"sleep 30\n").expect("command");
        assert!(eventually(|| foreground_is_program(master, shell_pid) == Some(true)), "sleep runs in the foreground");
        // Ctrl + C stops the program: the shell takes the foreground back.
        writer.write_all(&[3]).expect("interrupt");
        assert!(eventually(|| foreground_is_program(master, shell_pid) == Some(false)), "the shell has taken over again");

        writer.write_all(b"exit\n").expect("exit");
        let _ = child.wait();
    }
}
