//! Creating a local repository: a clone, with progress and cancellation, or an init.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::State;
use tauri::ipc::Channel;

use super::{CommandLog, Done, Failed, check_name, command, ms, step, text};

/// The clone in progress, if there is one. One at a time.
#[derive(Default)]
pub struct CloneJob {
    child: Mutex<Option<Child>>,
    cancelled: AtomicBool,
}

#[derive(Default)]
pub struct CloneState(pub Arc<CloneJob>);

/// Hides the user name and the password of a URL (`https://me:secret@host/…`), so that the
/// command log never keeps them.
pub fn redact(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    match rest[..authority_end].rfind('@') {
        Some(at) => format!("{scheme}://***@{}", &rest[at + 1..]),
        None => url.to_string(),
    }
}

/// Folder name suggested for a URL: `https://host/me/project.git` → `project`.
/// The interface applies the same rule (`folderName` in `CloneDialog.svelte`).
#[cfg(test)]
pub fn folder_name(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches(['/', '\\']);
    let last = trimmed.rsplit(['/', '\\', ':']).next().unwrap_or("");
    last.strip_suffix(".git").unwrap_or(last).to_string()
}

/// Clones `url` into `dest`. `progress` receives each progress line of git.
pub fn clone_repo(job: &CloneJob, url: &str, dest: &Path, progress: impl Fn(String)) -> Result<(), String> {
    let url = url.trim();
    check_name(url)?;
    let parent = dest.parent().filter(|p| p.is_dir()).ok_or("The parent folder does not exist. Choose an existing folder.")?;
    let existed = dest.exists();
    if existed && fs::read_dir(dest).map(|mut entries| entries.next().is_some()).unwrap_or(true) {
        return Err(format!(
            "The folder “{}” already exists and is not empty. Choose another folder name.",
            dest.display()
        ));
    }

    let mut slot = job.child.lock().map_err(|_| "The clone state is unavailable. Restart Ramure.")?;
    if slot.is_some() {
        return Err("A clone is already in progress. Wait for it to finish.".into());
    }
    let mut cmd = command();
    cmd.current_dir(parent)
        // `--`: the URL and the folder cannot be taken for options.
        .args(["clone", "--progress", "--", url])
        .arg(dest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Could not start git: {e}. Check that git is installed."))?;
    let mut stderr = child.stderr.take().ok_or("The output of git is unavailable. Try again.")?;
    job.cancelled.store(false, Ordering::SeqCst);
    *slot = Some(child);
    drop(slot);

    // git rewrites its progress line with carriage returns: each fragment is a state.
    let mut tail: Vec<String> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut emit = |bytes: &[u8]| {
        let line = text(bytes);
        if line.is_empty() {
            return;
        }
        // Progress lines (“Receiving objects: 42%”) are not used as the error message.
        if !line.contains('%') {
            tail.push(line.clone());
            if tail.len() > 12 {
                tail.remove(0);
            }
        }
        progress(line);
    };
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                for &byte in &chunk[..n] {
                    if byte == b'\r' || byte == b'\n' {
                        emit(&pending);
                        pending.clear();
                    } else {
                        pending.push(byte);
                    }
                }
            }
        }
    }
    emit(&pending);

    let child = job.child.lock().map_err(|_| "The clone state is unavailable. Restart Ramure.")?.take();
    let success = child.and_then(|mut c| c.wait().ok()).is_some_and(|status| status.success());
    if success {
        return Ok(());
    }
    // Only what this clone created is removed.
    if !existed {
        let _ = fs::remove_dir_all(dest);
    }
    if job.cancelled.load(Ordering::SeqCst) {
        Err("Clone canceled.".into())
    } else if tail.is_empty() {
        Err("git clone failed.".into())
    } else {
        Err(tail.join("\n"))
    }
}

/// Stops the clone in progress. No effect when there is none.
pub fn cancel(job: &CloneJob) {
    if let Ok(mut slot) = job.child.lock() {
        if let Some(child) = slot.as_mut() {
            job.cancelled.store(true, Ordering::SeqCst);
            let _ = child.kill();
        }
    }
}

#[tauri::command]
pub async fn git_clone(
    state: State<'_, CloneState>,
    url: String,
    dest: String,
    on_progress: Channel<String>,
) -> Result<Done<String>, Failed> {
    let job = state.0.clone();
    let command_line = format!("git clone --progress -- {} {dest}", redact(&url));
    let start = Instant::now();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let result = clone_repo(&job, &url, Path::new(&dest), |line| {
            let _ = on_progress.send(line);
        });
        (result, dest)
    })
    .await;
    let (result, dest) = match joined {
        Ok(pair) => pair,
        Err(e) => return Err(Failed { message: e.to_string(), commands: Vec::new() }),
    };
    let commands = vec![CommandLog { command: command_line, ms: ms(start), ok: result.is_ok() }];
    match result {
        Ok(()) => Ok(Done { data: dest, commands }),
        Err(message) => Err(Failed { message, commands }),
    }
}

#[tauri::command]
pub fn git_clone_cancel(state: State<'_, CloneState>) {
    cancel(&state.0);
}

/// Initializes a repository in an existing folder. Safe on a repository already initialized.
#[tauri::command]
pub async fn git_init(path: String) -> Result<Done<String>, Failed> {
    super::blocking(move |j| {
        let dir = Path::new(&path);
        if !dir.is_dir() {
            return Err(format!("The folder “{path}” does not exist."));
        }
        step(j, dir, &["init"])?;
        Ok(path)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::work::tests::Scratch;
    use std::sync::Mutex as StdMutex;

    #[test]
    fn credentials_never_reach_the_command_log() {
        assert_eq!(redact("https://me:secret@example.org/a/b.git"), "https://***@example.org/a/b.git");
        assert_eq!(redact("https://token@example.org/a/b.git"), "https://***@example.org/a/b.git");
        assert_eq!(redact("https://example.org/a/b@c.git"), "https://example.org/a/b@c.git");
        assert_eq!(redact("git@example.org:a/b.git"), "git@example.org:a/b.git");
    }

    #[test]
    fn folder_names_come_from_the_address() {
        assert_eq!(folder_name("https://example.org/me/project.git"), "project");
        assert_eq!(folder_name("git@example.org:me/project.git"), "project");
        assert_eq!(folder_name("https://example.org/me/project/"), "project");
        assert_eq!(folder_name("/srv/repos/tool"), "tool");
        assert_eq!(folder_name("C:\\repos\\tool.git"), "tool");
    }

    #[test]
    fn clone_reports_progress_and_cleans_up_after_a_failure() {
        let origin = Scratch::new("clone-origin");
        origin.write("a.txt", "one\n");
        origin.git(&["add", "-A"]);
        origin.git(&["commit", "-q", "-m", "init"]);

        let parent = Scratch::new("clone-dest");
        let job = CloneJob::default();
        let lines = StdMutex::new(Vec::new());
        let dest = parent.0.join("copy");
        clone_repo(&job, origin.0.to_str().unwrap(), &dest, |l| lines.lock().unwrap().push(l)).unwrap();
        assert!(dest.join("a.txt").exists());
        assert!(lines.lock().unwrap().iter().any(|l| l.starts_with("Cloning into")));

        // Destination taken: refused, with nothing touched.
        let again = clone_repo(&job, origin.0.to_str().unwrap(), &dest, |_| {});
        assert!(again.unwrap_err().contains("is not empty"));
        assert!(dest.join("a.txt").exists());

        // Missing source: the message of git comes back, and the folder created goes away.
        let missing = parent.0.join("nothing");
        let failed = clone_repo(&job, parent.0.join("absent").to_str().unwrap(), &missing, |_| {});
        assert!(failed.is_err());
        assert!(!missing.exists());

        assert!(clone_repo(&job, "--upload-pack=x", &missing, |_| {}).is_err());
        cancel(&job); // no clone in progress: no effect
    }
}
