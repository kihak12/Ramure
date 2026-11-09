//! Sessions SSH interactives : connexion, vérification de l'empreinte, authentification, terminal.
//!
//! Chaque session vit dans une tâche tokio. L'interface lui parle par des commandes
//! (`ssh_write`, `ssh_resize`, `ssh_close`) et reçoit la sortie du terminal en octets bruts
//! sur un canal Tauri.

pub mod config;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use russh::client::{self, Handle};
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

/// Clés essayées en mode automatique, dans l'ordre d'OpenSSH.
const DEFAULT_KEYS: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

enum Cmd {
    Data(Vec<u8>),
    Resize(u32, u32),
    Close,
}

#[derive(Default)]
pub struct SshState {
    sessions: Mutex<HashMap<u32, mpsc::UnboundedSender<Cmd>>>,
    next_id: AtomicU32,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Auth {
    /// Agent SSH, puis clés par défaut de `~/.ssh`.
    Auto,
    KeyFile { path: String, passphrase: Option<String> },
    Password { password: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectParams {
    host: String,
    port: u16,
    user: String,
    auth: Auth,
    cols: u32,
    rows: u32,
    /// Empreinte que l'utilisateur vient d'accepter pour un serveur inconnu.
    accept_fingerprint: Option<String>,
    /// Commande envoyée dans le terminal dès que le shell est prêt.
    startup_command: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SshError {
    /// Serveur absent de `known_hosts` : à l'utilisateur de vérifier l'empreinte.
    HostKeyUnknown { fingerprint: String, algorithm: String },
    /// L'empreinte ne correspond plus à celle enregistrée : connexion bloquée.
    HostKeyChanged { fingerprint: String, line: usize },
    NeedPassphrase { path: String },
    Auth { message: String },
    Other { message: String },
}

fn other(e: impl std::fmt::Display) -> SshError {
    SshError::Other { message: e.to_string() }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connected {
    id: u32,
    fingerprint: String,
    /// Faux si l'empreinte a été acceptée pour cette session seulement.
    known_host: bool,
    auth_method: String,
    connect_ms: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Closed {
    id: u32,
    exit_status: Option<u32>,
    reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
    user: String,
    hosts: Vec<config::HostEntry>,
    key_files: Vec<String>,
}

enum HostVerdict {
    Trusted(String),
    AcceptedOnce(String),
    Unknown { fingerprint: String, algorithm: String },
    Changed { fingerprint: String, line: usize },
}

struct ClientHandler {
    host: String,
    port: u16,
    accepted: Option<String>,
    verdict: Arc<Mutex<Option<HostVerdict>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        let algorithm = key.algorithm().as_str().to_string();

        let verdict = match keys::check_known_hosts(&self.host, self.port, &key) {
            Ok(true) => HostVerdict::Trusted(fingerprint),
            Err(keys::Error::KeyChanged { line }) => HostVerdict::Changed { fingerprint, line },
            // Hôte inconnu, ou `known_hosts` absent ou illisible.
            Ok(false) | Err(_) => {
                if self.accepted.as_deref() == Some(fingerprint.as_str()) {
                    HostVerdict::AcceptedOnce(fingerprint)
                } else {
                    HostVerdict::Unknown { fingerprint, algorithm }
                }
            }
        };
        let trusted = matches!(verdict, HostVerdict::Trusted(_) | HostVerdict::AcceptedOnce(_));
        if let Ok(mut slot) = self.verdict.lock() {
            *slot = Some(verdict);
        }
        Ok(trusted)
    }
}

async fn auth_key_file(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    path: &Path,
    passphrase: Option<&str>,
) -> Result<bool, SshError> {
    let key = match keys::load_secret_key(path, passphrase) {
        Ok(key) => key,
        Err(keys::Error::KeyIsEncrypted) => {
            return Err(SshError::NeedPassphrase { path: path.display().to_string() });
        }
        Err(e) => return Err(other(format!("{} : {e}", path.display()))),
    };
    let hash = handle.best_supported_rsa_hash().await.map_err(other)?.flatten();
    let result = handle
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(other)?;
    Ok(result.success())
}

/// Essaie chaque identité de l'agent SSH. Renvoie le commentaire de la clé acceptée.
async fn auth_agent(handle: &mut Handle<ClientHandler>, user: &str) -> Option<String> {
    use russh::keys::agent::client::AgentClient;

    #[cfg(windows)]
    let agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await;
    #[cfg(unix)]
    let agent = AgentClient::connect_env().await;

    let mut agent = agent.ok()?;
    let identities = agent.request_identities().await.ok()?;
    let rsa_hash = handle.best_supported_rsa_hash().await.ok()?.flatten();

    for identity in identities {
        let key = identity.public_key().into_owned();
        let hash = if key.algorithm().is_rsa() { rsa_hash } else { None };
        if let Ok(result) = handle.authenticate_publickey_with(user, key, hash, &mut agent).await {
            if result.success() {
                return Some(identity.comment().to_string());
            }
        }
    }
    None
}

async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    auth: &Auth,
) -> Result<String, SshError> {
    match auth {
        Auth::Password { password } => {
            let result = handle
                .authenticate_password(user, password.as_str())
                .await
                .map_err(other)?;
            if result.success() {
                Ok("mot de passe".into())
            } else {
                Err(SshError::Auth { message: "Le serveur a refusé ce mot de passe.".into() })
            }
        }
        Auth::KeyFile { path, passphrase } => {
            let path = config::expand(path);
            let passphrase = passphrase.as_deref().filter(|p| !p.is_empty());
            if auth_key_file(handle, user, &path, passphrase).await? {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                Ok(format!("clé {name}"))
            } else {
                Err(SshError::Auth {
                    message: format!("Le serveur a refusé la clé {}.", path.display()),
                })
            }
        }
        Auth::Auto => {
            if let Some(comment) = auth_agent(handle, user).await {
                return Ok(if comment.is_empty() { "agent SSH".into() } else { format!("agent SSH ({comment})") });
            }
            // Première clé chiffrée rencontrée : on demandera sa passphrase si rien d'autre ne passe.
            let mut encrypted = None;
            if let Some(dir) = config::home().map(|home| home.join(".ssh")) {
                for name in DEFAULT_KEYS {
                    let path = dir.join(name);
                    if !path.is_file() {
                        continue;
                    }
                    match auth_key_file(handle, user, &path, None).await {
                        Ok(true) => return Ok(format!("clé {name}")),
                        Err(SshError::NeedPassphrase { path }) => {
                            encrypted.get_or_insert(path);
                        }
                        Ok(false) | Err(_) => {}
                    }
                }
            }
            match encrypted {
                Some(path) => Err(SshError::NeedPassphrase { path }),
                None => Err(SshError::Auth {
                    message: "Aucune clé acceptée : ni celles de l'agent SSH, ni les clés par défaut de ~/.ssh."
                        .into(),
                }),
            }
        }
    }
}

/// Boucle d'une session : relaie la frappe vers le serveur et la sortie vers l'interface.
async fn pump(
    mut channel: russh::Channel<client::Msg>,
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    on_data: Channel,
    mut startup: Option<String>,
) -> (Option<u32>, Option<String>) {
    let mut exit_status = None;
    // La commande de démarrage part quand le shell s'est tu 250 ms, et au plus tard après 3 s.
    let quiet = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(quiet);

    let reason = loop {
        tokio::select! {
            _ = &mut quiet, if startup.is_some() => {
                if let Some(command) = startup.take() {
                    let line = format!("{command}\n");
                    if let Err(e) = channel.data(line.as_bytes()).await {
                        break Some(e.to_string());
                    }
                }
            }
            cmd = rx.recv() => match cmd {
                Some(Cmd::Data(bytes)) => {
                    if let Err(e) = channel.data(&bytes[..]).await {
                        break Some(e.to_string());
                    }
                }
                Some(Cmd::Resize(cols, rows)) => {
                    let _ = channel.window_change(cols, rows, 0, 0).await;
                }
                Some(Cmd::Close) | None => {
                    let _ = channel.eof().await;
                    let _ = channel.close().await;
                    break None;
                }
            },
            msg = channel.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    if startup.is_some() {
                        quiet.as_mut().reset(tokio::time::Instant::now() + Duration::from_millis(250));
                    }
                    let _ = on_data.send(InvokeResponseBody::Raw(data.to_vec()));
                }
                Some(ChannelMsg::ExtendedData { data, .. }) => {
                    let _ = on_data.send(InvokeResponseBody::Raw(data.to_vec()));
                }
                Some(ChannelMsg::ExitStatus { exit_status: code }) => exit_status = Some(code),
                Some(ChannelMsg::Close) | None => break None,
                _ => {}
            },
        }
    };
    (exit_status, reason)
}

/// Valeurs proposées par le formulaire : utilisateur courant, hôtes de `~/.ssh/config`, clés présentes.
#[tauri::command]
pub fn ssh_defaults() -> Defaults {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    let ssh_dir = config::home().map(|home| home.join(".ssh"));
    let hosts = ssh_dir
        .as_ref()
        .and_then(|dir| std::fs::read_to_string(dir.join("config")).ok())
        .map(|text| config::parse(&text))
        .unwrap_or_default();
    let key_files = match &ssh_dir {
        Some(dir) => DEFAULT_KEYS
            .iter()
            .filter(|name| dir.join(name).is_file())
            .map(|name| format!("~/.ssh/{name}"))
            .collect(),
        None => Vec::new(),
    };
    Defaults { user, hosts, key_files }
}

#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    state: State<'_, SshState>,
    params: ConnectParams,
    on_data: Channel,
) -> Result<Connected, SshError> {
    let started = Instant::now();
    let verdict = Arc::new(Mutex::new(None));
    let config = Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    });
    let handler = ClientHandler {
        host: params.host.clone(),
        port: params.port,
        accepted: params.accept_fingerprint.clone(),
        verdict: verdict.clone(),
    };

    let connecting = client::connect(config, (params.host.as_str(), params.port), handler);
    let connection = tokio::time::timeout(Duration::from_secs(15), connecting).await;
    let seen = verdict.lock().ok().and_then(|mut slot| slot.take());

    let mut handle = match connection {
        Err(_) => return Err(other(format!("{}:{} ne répond pas (délai de 15 s dépassé).", params.host, params.port))),
        Ok(Err(e)) => {
            return Err(match seen {
                Some(HostVerdict::Unknown { fingerprint, algorithm }) => {
                    SshError::HostKeyUnknown { fingerprint, algorithm }
                }
                Some(HostVerdict::Changed { fingerprint, line }) => {
                    SshError::HostKeyChanged { fingerprint, line }
                }
                _ => other(e),
            });
        }
        Ok(Ok(handle)) => handle,
    };
    let (fingerprint, known_host) = match seen {
        Some(HostVerdict::Trusted(fingerprint)) => (fingerprint, true),
        Some(HostVerdict::AcceptedOnce(fingerprint)) => (fingerprint, false),
        _ => (String::new(), false),
    };

    let auth_method = match authenticate(&mut handle, &params.user, &params.auth).await {
        Ok(method) => method,
        Err(e) => {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "fr").await;
            return Err(e);
        }
    };

    let channel = handle.channel_open_session().await.map_err(other)?;
    channel
        .request_pty(false, "xterm-256color", params.cols.max(2), params.rows.max(2), 0, 0, &[])
        .await
        .map_err(other)?;
    channel.request_shell(false).await.map_err(other)?;

    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let (tx, rx) = mpsc::unbounded_channel();
    state
        .sessions
        .lock()
        .map_err(|_| other("état des sessions indisponible"))?
        .insert(id, tx);

    let startup = params.startup_command.filter(|command| !command.trim().is_empty());
    tauri::async_runtime::spawn(async move {
        let (exit_status, reason) = pump(channel, rx, on_data, startup).await;
        let _ = handle.disconnect(Disconnect::ByApplication, "", "fr").await;
        if let Ok(mut sessions) = app.state::<SshState>().sessions.lock() {
            sessions.remove(&id);
        }
        let _ = app.emit("ssh-closed", Closed { id, exit_status, reason });
    });

    Ok(Connected {
        id,
        fingerprint,
        known_host,
        auth_method,
        connect_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

fn send(state: &SshState, id: u32, cmd: Cmd) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|_| "état des sessions indisponible".to_string())?;
    let tx = sessions.get(&id).ok_or_else(|| "cette session est fermée".to_string())?;
    tx.send(cmd).map_err(|_| "cette session est fermée".to_string())
}

#[tauri::command]
pub fn ssh_write(state: State<'_, SshState>, id: u32, data: String) -> Result<(), String> {
    send(&state, id, Cmd::Data(data.into_bytes()))
}

#[tauri::command]
pub fn ssh_resize(state: State<'_, SshState>, id: u32, cols: u32, rows: u32) -> Result<(), String> {
    send(&state, id, Cmd::Resize(cols.max(2), rows.max(2)))
}

#[tauri::command]
pub fn ssh_close(state: State<'_, SshState>, id: u32) -> Result<(), String> {
    send(&state, id, Cmd::Close)
}
