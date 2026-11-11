//! Sessions SSH interactives : connexion, vérification de l'empreinte, authentification, terminal.
//!
//! Chaque session vit dans une tâche tokio. L'interface lui parle par des commandes
//! (`ssh_write`, `ssh_resize`, `ssh_close`) et reçoit la sortie du terminal en octets bruts
//! sur un canal Tauri.

pub mod config;
pub mod keyfiles;
pub mod known_hosts;
pub mod secrets;
pub mod store;

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

use known_hosts::Known;

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
    /// Sans mot de passe : celui que le trousseau retient pour ce compte, s'il y en a un.
    Password { password: Option<String> },
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
    /// Vrai pour inscrire cette empreinte dans `known_hosts` ; sinon elle ne vaut que pour cette session.
    #[serde(default)]
    remember_host: bool,
    /// Commande envoyée dans le terminal dès que le shell est prêt.
    startup_command: Option<String>,
    /// Vrai pour retenir dans le trousseau du système le secret tapé, une fois qu'il a servi.
    #[serde(default)]
    remember_secret: bool,
}

/// Ce qu'il faut pour tester un hôte : se connecter et s'authentifier, sans ouvrir de terminal.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestParams {
    host: String,
    port: u16,
    user: String,
    auth: Auth,
    accept_fingerprint: Option<String>,
    #[serde(default)]
    remember_host: bool,
}

/// Résultat d'un test réussi : comment le serveur a été reconnu et l'utilisateur authentifié.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tested {
    known_host: bool,
    /// L'empreinte devait être mémorisée et `known_hosts` n'a pas pu être écrit : pourquoi.
    learn_failed: Option<String>,
    auth_method: String,
    connect_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SshError {
    /// Serveur absent de `known_hosts` : à l'utilisateur de vérifier l'empreinte.
    HostKeyUnknown { fingerprint: String, algorithm: String },
    /// L'empreinte ne correspond plus à celle enregistrée : connexion bloquée.
    HostKeyChanged { fingerprint: String, line: usize },
    /// La clé `path` est protégée. `refused` : la phrase secrète essayée ne la déverrouille pas.
    NeedPassphrase { path: String, refused: Option<Refused> },
    /// Il faut un mot de passe. `refused` : celui que le trousseau retenait a été refusé, et oublié.
    NeedPassword { refused: bool },
    Auth { message: String },
    Other { message: String },
}

/// D'où venait un secret qui n'a pas servi : tapé à l'instant, ou retenu dans le trousseau. Un
/// secret retenu qui est refusé est oublié : il ne sera pas présenté une seconde fois au serveur.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Refused {
    Typed,
    Retained,
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
    /// L'empreinte devait être mémorisée et `known_hosts` n'a pas pu être écrit : pourquoi.
    learn_failed: Option<String>,
    auth_method: String,
    connect_ms: f64,
    /// Le secret devait être retenu et le trousseau l'a refusé : ce qu'il en dit, sans le secret.
    keep_failed: Option<String>,
}

/// Pourquoi une session s'est terminée. Seule une connexion perdue se rouvre d'elle-même.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Ending {
    /// L'utilisateur a fermé l'onglet ou cliqué « Déconnecter ».
    User,
    /// Le serveur a fermé la session : `exit`, fin du shell, code de sortie.
    Exit,
    /// La connexion est tombée sans que personne ne la ferme : réseau coupé, veille, délai dépassé.
    Lost,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Closed {
    id: u32,
    exit_status: Option<u32>,
    reason: Option<String>,
    ending: Ending,
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
    /// Acceptée pour cette session. `learn_failed` : elle devait être mémorisée, et ne l'a pas été.
    AcceptedOnce { fingerprint: String, learn_failed: Option<String> },
    Unknown { fingerprint: String, algorithm: String },
    Changed { fingerprint: String, line: usize },
}

struct ClientHandler {
    host: String,
    port: u16,
    accepted: Option<String>,
    remember: bool,
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

        let verdict = match known_hosts::check(&self.host, self.port, &key) {
            Known::Trusted => HostVerdict::Trusted(fingerprint),
            Known::Changed { line } => HostVerdict::Changed { fingerprint, line },
            Known::Unknown if self.accepted.as_deref() == Some(fingerprint.as_str()) => {
                // L'utilisateur fait confiance à ce serveur : on l'inscrit dans known_hosts,
                // comme le fait OpenSSH. Si l'écriture échoue, la confiance vaut pour la session.
                match self.remember.then(|| known_hosts::learn(&self.host, self.port, &key)) {
                    Some(Ok(())) => HostVerdict::Trusted(fingerprint),
                    Some(Err(reason)) => HostVerdict::AcceptedOnce { fingerprint, learn_failed: Some(reason) },
                    None => HostVerdict::AcceptedOnce { fingerprint, learn_failed: None },
                }
            }
            Known::Unknown => HostVerdict::Unknown { fingerprint, algorithm },
        };
        let trusted = matches!(verdict, HostVerdict::Trusted(_) | HostVerdict::AcceptedOnce { .. });
        if let Ok(mut slot) = self.verdict.lock() {
            *slot = Some(verdict);
        }
        Ok(trusted)
    }
}

/// Ce qu'a donné une clé : si le serveur l'a acceptée, et d'où venait sa phrase secrète.
struct KeyTried {
    accepted: bool,
    /// La phrase secrète venait du trousseau.
    retained: bool,
    /// La phrase secrète tapée qui a déverrouillé la clé : à retenir si on le demande.
    typed: Option<String>,
}

async fn auth_key_file(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    path: &Path,
    passphrase: Option<&str>,
) -> Result<KeyTried, SshError> {
    let shown = path.display().to_string();
    let mut retained = false;
    let mut typed = None;
    let key = match keys::load_secret_key(path, None) {
        Ok(key) => key,
        Err(keys::Error::KeyIsEncrypted) => {
            let account = secrets::passphrase_account(path);
            // La phrase tapée d'abord ; sinon celle que le trousseau retient pour cette clé.
            let (candidate, from_keyring) = match passphrase {
                Some(given) => (given.to_string(), false),
                None => match secrets::held(account.clone()).await {
                    Some(kept) => (kept, true),
                    None => return Err(SshError::NeedPassphrase { path: shown, refused: None }),
                },
            };
            match keys::load_secret_key(path, Some(&candidate)) {
                Ok(key) => {
                    if from_keyring {
                        retained = true;
                    } else {
                        typed = Some(candidate);
                    }
                    key
                }
                Err(_) if from_keyring => {
                    // La phrase retenue ne déverrouille plus la clé : elle est oubliée, et redemandée.
                    let _ = secrets::forget(account).await;
                    return Err(SshError::NeedPassphrase { path: shown, refused: Some(Refused::Retained) });
                }
                Err(_) => return Err(SshError::NeedPassphrase { path: shown, refused: Some(Refused::Typed) }),
            }
        }
        Err(e) => return Err(other(format!("{shown} : {e}"))),
    };
    let hash = handle.best_supported_rsa_hash().await.map_err(other)?.flatten();
    let result = handle
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(other)?;
    Ok(KeyTried { accepted: result.success(), retained, typed })
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

/// Un mot de passe prêt à servir : tapé à l'instant, ou retenu dans le trousseau.
struct Password {
    value: String,
    retained: bool,
}

/// Une authentification réussie.
struct Authenticated {
    /// Comment : « mot de passe », « clé id_ed25519 », « agent SSH »…
    method: String,
    /// Le secret tapé qui a servi, et le nom sous lequel le retenir si on le demande.
    typed: Option<(String, String)>,
}

/// Le nom d'une clé tel qu'on le montre, suivi de ce qui l'a déverrouillée si c'est le trousseau.
fn key_method(name: &str, retained: bool) -> String {
    if retained { format!("clé {name}, phrase secrète du trousseau") } else { format!("clé {name}") }
}

async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    auth: &Auth,
    password: Option<Password>,
    password_account: String,
) -> Result<Authenticated, SshError> {
    match auth {
        Auth::Password { .. } => {
            let Some(password) = password else {
                return Err(SshError::NeedPassword { refused: false });
            };
            let result = handle
                .authenticate_password(user, password.value.as_str())
                .await
                .map_err(other)?;
            if result.success() {
                Ok(if password.retained {
                    Authenticated { method: "mot de passe du trousseau".into(), typed: None }
                } else {
                    Authenticated { method: "mot de passe".into(), typed: Some((password_account, password.value)) }
                })
            } else if password.retained {
                // Le mot de passe retenu n'est plus le bon : il est oublié, et redemandé.
                let _ = secrets::forget(password_account).await;
                Err(SshError::NeedPassword { refused: true })
            } else {
                Err(SshError::Auth { message: "Le serveur a refusé ce mot de passe.".into() })
            }
        }
        Auth::KeyFile { path, passphrase } => {
            let path = config::expand(path);
            let passphrase = passphrase.as_deref().filter(|p| !p.is_empty());
            let tried = auth_key_file(handle, user, &path, passphrase).await?;
            if tried.accepted {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                Ok(Authenticated {
                    method: key_method(&name, tried.retained),
                    typed: tried.typed.map(|passphrase| (secrets::passphrase_account(&path), passphrase)),
                })
            } else {
                Err(SshError::Auth {
                    message: format!("Le serveur a refusé la clé {}.", path.display()),
                })
            }
        }
        Auth::Auto => {
            if let Some(comment) = auth_agent(handle, user).await {
                let method = if comment.is_empty() { "agent SSH".into() } else { format!("agent SSH ({comment})") };
                return Ok(Authenticated { method, typed: None });
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
                        Ok(tried) if tried.accepted => {
                            return Ok(Authenticated { method: key_method(name, tried.retained), typed: None });
                        }
                        Err(SshError::NeedPassphrase { path, refused }) => {
                            encrypted.get_or_insert((path, refused));
                        }
                        Ok(_) | Err(_) => {}
                    }
                }
            }
            match encrypted {
                Some((path, refused)) => Err(SshError::NeedPassphrase { path, refused }),
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
) -> (Option<u32>, Option<String>, Ending) {
    let mut exit_status = None;
    // Le serveur a dit que la session finissait (code de sortie, fin de flux) : elle n'est pas perdue.
    let mut ended_by_server = false;
    // La commande de démarrage part quand le shell s'est tu 250 ms, et au plus tard après 3 s.
    let quiet = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(quiet);

    let (reason, ending) = loop {
        tokio::select! {
            _ = &mut quiet, if startup.is_some() => {
                if let Some(command) = startup.take() {
                    let line = format!("{command}\n");
                    if let Err(e) = channel.data(line.as_bytes()).await {
                        break (Some(e.to_string()), Ending::Lost);
                    }
                }
            }
            cmd = rx.recv() => match cmd {
                Some(Cmd::Data(bytes)) => {
                    if let Err(e) = channel.data(&bytes[..]).await {
                        break (Some(e.to_string()), Ending::Lost);
                    }
                }
                Some(Cmd::Resize(cols, rows)) => {
                    let _ = channel.window_change(cols, rows, 0, 0).await;
                }
                Some(Cmd::Close) | None => {
                    let _ = channel.eof().await;
                    let _ = channel.close().await;
                    break (None, Ending::User);
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
                Some(ChannelMsg::ExitStatus { exit_status: code }) => {
                    exit_status = Some(code);
                    ended_by_server = true;
                }
                Some(ChannelMsg::ExitSignal { .. }) | Some(ChannelMsg::Eof) => ended_by_server = true,
                Some(ChannelMsg::Close) => break (None, Ending::Exit),
                None => break (None, ending_without_close(ended_by_server)),
                _ => {}
            },
        }
    };
    (exit_status, reason, ending)
}

/// Le canal s'est tu sans message de fermeture : la session finissait si le serveur l'avait
/// annoncé, sinon la connexion est perdue.
fn ending_without_close(ended_by_server: bool) -> Ending {
    if ended_by_server { Ending::Exit } else { Ending::Lost }
}

/// L'utilisateur de cette machine : celui d'un hôte qui n'en précise pas.
pub fn current_user() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default()
}

/// Valeurs proposées par le formulaire : utilisateur courant, hôtes de `~/.ssh/config`, clés présentes.
#[tauri::command]
pub fn ssh_defaults() -> Defaults {
    let user = current_user();
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

/// Une connexion ouverte et authentifiée, prête à recevoir un canal.
struct Opened {
    handle: Handle<ClientHandler>,
    fingerprint: String,
    known_host: bool,
    learn_failed: Option<String>,
    auth_method: String,
    /// Le secret tapé qui a servi, et le nom sous lequel le retenir si on le demande.
    typed: Option<(String, String)>,
}

/// Se connecte à `host:port`, vérifie l'empreinte du serveur et authentifie `user`. Commun à une
/// session et au test d'un hôte.
async fn open_authenticated(
    host: &str,
    port: u16,
    user: &str,
    auth: &Auth,
    accept_fingerprint: Option<String>,
    remember_host: bool,
) -> Result<Opened, SshError> {
    let host = host.trim();
    // Le mot de passe est trouvé avant de se connecter : sans lui, inutile de déranger le serveur.
    let password_account = secrets::password_account(user, host, port);
    let password = match auth {
        Auth::Password { password } => match password.as_deref().filter(|p| !p.is_empty()) {
            Some(given) => Some(Password { value: given.to_string(), retained: false }),
            None => match secrets::held(password_account.clone()).await {
                Some(kept) => Some(Password { value: kept, retained: true }),
                None => return Err(SshError::NeedPassword { refused: false }),
            },
        },
        _ => None,
    };

    let verdict = Arc::new(Mutex::new(None));
    let config = Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    });
    let handler = ClientHandler {
        host: host.to_string(),
        port,
        accepted: accept_fingerprint,
        remember: remember_host,
        verdict: verdict.clone(),
    };

    let connecting = client::connect(config, (host, port), handler);
    let connection = tokio::time::timeout(Duration::from_secs(15), connecting).await;
    let seen = verdict.lock().ok().and_then(|mut slot| slot.take());

    let mut handle = match connection {
        Err(_) => return Err(other(format!("{host}:{port} ne répond pas (délai de 15 s dépassé)."))),
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
    let (fingerprint, known_host, learn_failed) = match seen {
        Some(HostVerdict::Trusted(fingerprint)) => (fingerprint, true, None),
        Some(HostVerdict::AcceptedOnce { fingerprint, learn_failed }) => (fingerprint, false, learn_failed),
        _ => (String::new(), false, None),
    };

    match authenticate(&mut handle, user, auth, password, password_account).await {
        Ok(Authenticated { method, typed }) => {
            Ok(Opened { handle, fingerprint, known_host, learn_failed, auth_method: method, typed })
        }
        Err(e) => {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "fr").await;
            Err(e)
        }
    }
}

/// Teste un hôte : connexion, empreinte et authentification, puis déconnexion aussitôt.
/// Aucun terminal n'est ouvert, rien n'est enregistré, sauf l'empreinte que l'utilisateur
/// demande de mémoriser.
#[tauri::command]
pub async fn ssh_test(params: TestParams) -> Result<Tested, SshError> {
    let started = Instant::now();
    let opened = open_authenticated(
        &params.host,
        params.port,
        &params.user,
        &params.auth,
        params.accept_fingerprint,
        params.remember_host,
    )
    .await?;
    let _ = opened.handle.disconnect(Disconnect::ByApplication, "", "fr").await;
    Ok(Tested {
        known_host: opened.known_host,
        learn_failed: opened.learn_failed,
        auth_method: opened.auth_method,
        connect_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    state: State<'_, SshState>,
    params: ConnectParams,
    on_data: Channel,
) -> Result<Connected, SshError> {
    let started = Instant::now();
    let Opened { handle, fingerprint, known_host, learn_failed, auth_method, typed } = open_authenticated(
        &params.host,
        params.port,
        &params.user,
        &params.auth,
        params.accept_fingerprint.clone(),
        params.remember_host,
    )
    .await?;

    // Le secret a servi : c'est maintenant qu'il est retenu, si on l'a demandé. Un trousseau qui
    // refuse n'empêche pas la session : l'interface le dira.
    let mut keep_failed = None;
    if params.remember_secret {
        if let Some((account, secret)) = typed {
            keep_failed = secrets::keep(account, secret).await.err();
        }
    }

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
        let (exit_status, reason, ending) = pump(channel, rx, on_data, startup).await;
        let _ = handle.disconnect(Disconnect::ByApplication, "", "fr").await;
        if let Ok(mut sessions) = app.state::<SshState>().sessions.lock() {
            sessions.remove(&id);
        }
        let _ = app.emit("ssh-closed", Closed { id, exit_status, reason, ending });
    });

    Ok(Connected {
        id,
        fingerprint,
        known_host,
        learn_failed,
        auth_method,
        connect_ms: started.elapsed().as_secs_f64() * 1000.0,
        keep_failed,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_that_falls_silent_is_lost_unless_the_server_ended_it() {
        assert_eq!(ending_without_close(false), Ending::Lost);
        assert_eq!(ending_without_close(true), Ending::Exit);
    }
}
