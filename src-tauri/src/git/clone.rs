//! Création d'un dépôt local : clonage, avec progression et annulation, ou initialisation.

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

/// Clonage en cours, s'il y en a un. Un seul à la fois.
#[derive(Default)]
pub struct CloneJob {
    child: Mutex<Option<Child>>,
    cancelled: AtomicBool,
}

#[derive(Default)]
pub struct CloneState(pub Arc<CloneJob>);

/// Masque l'identifiant et le mot de passe d'une adresse (`https://moi:secret@hôte/…`),
/// pour que le journal ne les conserve jamais.
pub fn redact(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    match rest[..authority_end].rfind('@') {
        Some(at) => format!("{scheme}://***@{}", &rest[at + 1..]),
        None => url.to_string(),
    }
}

/// Nom de dossier proposé pour une adresse : `https://hôte/moi/projet.git` → `projet`.
/// L'interface applique la même règle (`folderName` dans `CloneDialog.svelte`).
#[cfg(test)]
pub fn folder_name(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches(['/', '\\']);
    let last = trimmed.rsplit(['/', '\\', ':']).next().unwrap_or("");
    last.strip_suffix(".git").unwrap_or(last).to_string()
}

/// Clone `url` dans `dest`. `progress` reçoit chaque ligne d'avancement de git.
pub fn clone_repo(job: &CloneJob, url: &str, dest: &Path, progress: impl Fn(String)) -> Result<(), String> {
    let url = url.trim();
    check_name(url)?;
    let parent = dest.parent().filter(|p| p.is_dir()).ok_or("Le dossier parent n'existe pas.")?;
    let existed = dest.exists();
    if existed && fs::read_dir(dest).map(|mut entries| entries.next().is_some()).unwrap_or(true) {
        return Err(format!("Le dossier « {} » existe déjà et n'est pas vide.", dest.display()));
    }

    let mut slot = job.child.lock().map_err(|_| "état du clonage indisponible")?;
    if slot.is_some() {
        return Err("Un clonage est déjà en cours.".into());
    }
    let mut cmd = command();
    cmd.current_dir(parent)
        // `--` : l'adresse et le dossier ne peuvent pas être pris pour des options.
        .args(["clone", "--progress", "--", url])
        .arg(dest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("impossible de lancer git : {e}"))?;
    let mut stderr = child.stderr.take().ok_or("sortie de git indisponible")?;
    job.cancelled.store(false, Ordering::SeqCst);
    *slot = Some(child);
    drop(slot);

    // git réécrit sa ligne d'avancement avec des retours chariot : chaque fragment est un état.
    let mut tail: Vec<String> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut emit = |bytes: &[u8]| {
        let line = text(bytes);
        if line.is_empty() {
            return;
        }
        // Les lignes de progression (« Receiving objects: 42% ») ne servent pas de message d'erreur.
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

    let child = job.child.lock().map_err(|_| "état du clonage indisponible")?.take();
    let success = child.and_then(|mut c| c.wait().ok()).is_some_and(|status| status.success());
    if success {
        return Ok(());
    }
    // On ne supprime que ce que ce clonage a créé.
    if !existed {
        let _ = fs::remove_dir_all(dest);
    }
    if job.cancelled.load(Ordering::SeqCst) {
        Err("Clonage annulé.".into())
    } else if tail.is_empty() {
        Err("git clone a échoué.".into())
    } else {
        Err(tail.join("\n"))
    }
}

/// Interrompt le clonage en cours. Sans effet s'il n'y en a pas.
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

/// Initialise un dépôt dans un dossier existant. Sans danger sur un dépôt déjà initialisé.
#[tauri::command]
pub async fn git_init(path: String) -> Result<Done<String>, Failed> {
    super::blocking(move |j| {
        let dir = Path::new(&path);
        if !dir.is_dir() {
            return Err(format!("Le dossier « {path} » n'existe pas."));
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
    fn credentials_never_reach_the_journal() {
        assert_eq!(redact("https://moi:secret@exemple.org/a/b.git"), "https://***@exemple.org/a/b.git");
        assert_eq!(redact("https://jeton@exemple.org/a/b.git"), "https://***@exemple.org/a/b.git");
        assert_eq!(redact("https://exemple.org/a/b@c.git"), "https://exemple.org/a/b@c.git");
        assert_eq!(redact("git@exemple.org:a/b.git"), "git@exemple.org:a/b.git");
    }

    #[test]
    fn folder_names_come_from_the_address() {
        assert_eq!(folder_name("https://exemple.org/moi/projet.git"), "projet");
        assert_eq!(folder_name("git@exemple.org:moi/projet.git"), "projet");
        assert_eq!(folder_name("https://exemple.org/moi/projet/"), "projet");
        assert_eq!(folder_name("/srv/depots/outil"), "outil");
        assert_eq!(folder_name("C:\\depots\\outil.git"), "outil");
    }

    #[test]
    fn clone_reports_progress_and_cleans_up_after_a_failure() {
        let origin = Scratch::new("clone-origin");
        origin.write("a.txt", "un\n");
        origin.git(&["add", "-A"]);
        origin.git(&["commit", "-q", "-m", "init"]);

        let parent = Scratch::new("clone-dest");
        let job = CloneJob::default();
        let lines = StdMutex::new(Vec::new());
        let dest = parent.0.join("copie");
        clone_repo(&job, origin.0.to_str().unwrap(), &dest, |l| lines.lock().unwrap().push(l)).unwrap();
        assert!(dest.join("a.txt").exists());
        assert!(lines.lock().unwrap().iter().any(|l| l.starts_with("Cloning into")));

        // Destination occupée : refus, sans rien toucher.
        let again = clone_repo(&job, origin.0.to_str().unwrap(), &dest, |_| {});
        assert!(again.unwrap_err().contains("n'est pas vide"));
        assert!(dest.join("a.txt").exists());

        // Source inexistante : le message de git remonte, et le dossier créé disparaît.
        let missing = parent.0.join("rien");
        let failed = clone_repo(&job, parent.0.join("absent").to_str().unwrap(), &missing, |_| {});
        assert!(failed.is_err());
        assert!(!missing.exists());

        assert!(clone_repo(&job, "--upload-pack=x", &missing, |_| {}).is_err());
        cancel(&job); // aucun clonage en cours : sans effet
    }
}
