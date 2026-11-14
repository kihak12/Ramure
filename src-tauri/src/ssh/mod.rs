//! Interactive SSH sessions: connection, fingerprint check, authentication, terminal.
//!
//! Each session lives in a tokio task. The interface talks to it through commands
//! (`ssh_write`, `ssh_resize`, `ssh_close`) and receives the output of the terminal as raw bytes
//! on a Tauri channel.

pub mod config;
pub mod export;
pub mod keyfiles;
pub mod known_hosts;
pub mod secrets;
pub mod socks;
pub mod store;
pub mod tunnels;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use russh::client::{self, Handle};
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelOpenFailure, Disconnect};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

use known_hosts::Known;
use tunnels::Tunnels;

/// Keys tried in automatic mode, in the order of OpenSSH.
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
    /// SSH agent, then the default keys of `~/.ssh`.
    Auto,
    KeyFile { path: String, passphrase: Option<String> },
    /// Without a password: the one the keyring keeps for this account, if there is one.
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
    /// Fingerprint the user has just accepted for an unknown server.
    accept_fingerprint: Option<String>,
    /// True to add this fingerprint to `known_hosts`; otherwise it is good for this session only.
    #[serde(default)]
    remember_host: bool,
    /// Command sent to the terminal as soon as the shell is ready.
    startup_command: Option<String>,
    /// True to keep the typed secret in the system keyring, once it has worked.
    #[serde(default)]
    remember_secret: bool,
    /// The log to open before the output arrives: the option of the host, or a resumed log.
    #[serde(default)]
    journal: Option<crate::journal::JournalOpen>,
    /// The stable key of the tab: its tunnels outlive a connection. Empty: no tunnels.
    #[serde(default)]
    group: String,
}

/// What testing a host needs: connecting and authenticating, without opening a terminal.
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

/// Result of a test that succeeded: how the server was recognized and the user authenticated.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tested {
    known_host: bool,
    /// The fingerprint was to be remembered and `known_hosts` could not be written: why.
    learn_failed: Option<String>,
    auth_method: String,
    connect_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SshError {
    /// Server missing from `known_hosts`: the user has to check the fingerprint.
    HostKeyUnknown { fingerprint: String, algorithm: String },
    /// The fingerprint no longer matches the recorded one: the connection is blocked.
    HostKeyChanged { fingerprint: String, line: usize },
    /// The key `path` is protected. `refused`: the passphrase tried does not unlock it.
    NeedPassphrase { path: String, refused: Option<Refused> },
    /// A password is needed. `refused`: the one the keyring kept was refused, and forgotten.
    NeedPassword { refused: bool },
    Auth { message: String },
    Other { message: String },
}

/// Where a secret that did not work came from: typed just now, or kept in the keyring. A kept
/// secret that is refused is forgotten: it will not be offered to the server a second time.
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
    /// False if the fingerprint was accepted for this session only.
    known_host: bool,
    /// The fingerprint was to be remembered and `known_hosts` could not be written: why.
    learn_failed: Option<String>,
    auth_method: String,
    connect_ms: f64,
    /// The secret was to be kept and the keyring refused it: what the keyring says, without the secret.
    keep_failed: Option<String>,
    /// The requested log could not be opened: why. The session opens all the same.
    journal_failed: Option<String>,
}

/// Why a session ended. Only a lost connection opens again by itself.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Ending {
    /// The user closed the tab or clicked “Disconnect”.
    User,
    /// The server closed the session: `exit`, end of the shell, exit code.
    Exit,
    /// The connection dropped without anyone closing it: network cut, sleep, timed out.
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
    /// Accepted for this session. `learn_failed`: it was to be remembered, and was not.
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
    /// The tunnels, and the group of the session, where the server's connections to a remote
    /// forward go. None for a test, which starts no tunnel.
    forwards: Option<(Tunnels, String)>,
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
                // The user trusts this server: it is added to known_hosts, as OpenSSH does. If the
                // write fails, the trust is good for the session.
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

    /// A connection to a port the server listens on for a remote forward.
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        match &self.forwards {
            Some((tunnels, group)) => tunnels.forwarded(group, connected_port, channel, reply),
            None => reply.reject(ChannelOpenFailure::AdministrativelyProhibited).await,
        }
        Ok(())
    }
}

/// What a key gave: whether the server accepted it, and where its passphrase came from.
struct KeyTried {
    accepted: bool,
    /// The passphrase came from the keyring.
    retained: bool,
    /// The typed passphrase that unlocked the key: to keep if asked.
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
            // The typed passphrase first; otherwise the one the keyring keeps for this key.
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
                    // The kept passphrase no longer unlocks the key: it is forgotten, and asked for again.
                    let _ = secrets::forget(account).await;
                    return Err(SshError::NeedPassphrase { path: shown, refused: Some(Refused::Retained) });
                }
                Err(_) => return Err(SshError::NeedPassphrase { path: shown, refused: Some(Refused::Typed) }),
            }
        }
        Err(e) => return Err(other(format!("{shown}: {e}"))),
    };
    let hash = handle.best_supported_rsa_hash().await.map_err(other)?.flatten();
    let result = handle
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(other)?;
    Ok(KeyTried { accepted: result.success(), retained, typed })
}

/// Tries each identity of the SSH agent. Returns the comment of the accepted key.
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

/// A password ready to use: typed just now, or kept in the keyring.
struct Password {
    value: String,
    retained: bool,
}

/// An authentication that succeeded.
struct Authenticated {
    /// How: “password”, “key id_ed25519”, “SSH agent”…
    method: String,
    /// The typed secret that worked, and the name to keep it under if asked.
    typed: Option<(String, String)>,
}

/// The name of a key as it is shown, followed by what unlocked it if that was the keyring.
fn key_method(name: &str, retained: bool) -> String {
    if retained { format!("key {name}, passphrase from the keyring") } else { format!("key {name}") }
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
                    Authenticated { method: "password from the keyring".into(), typed: None }
                } else {
                    Authenticated { method: "password".into(), typed: Some((password_account, password.value)) }
                })
            } else if password.retained {
                // The kept password is no longer the right one: it is forgotten, and asked for again.
                let _ = secrets::forget(password_account).await;
                Err(SshError::NeedPassword { refused: true })
            } else {
                Err(SshError::Auth { message: "The server refused this password.".into() })
            }
        }
        Auth::KeyFile { path, passphrase } => {
            let path = config::expand(path);
            let passphrase = passphrase.as_deref().filter(|p| !p.is_empty());
            let tried = auth_key_file(handle, user, &path, passphrase).await?;
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if tried.accepted {
                Ok(Authenticated {
                    method: key_method(&name, tried.retained),
                    typed: tried.typed.map(|passphrase| (secrets::passphrase_account(&path), passphrase)),
                })
            } else {
                Err(SshError::Auth {
                    message: format!(
                        "The server refused the key {name}. Add its public key to the server’s authorized_keys, or choose another key in the host form."
                    ),
                })
            }
        }
        Auth::Auto => {
            if let Some(comment) = auth_agent(handle, user).await {
                let method = if comment.is_empty() { "SSH agent".into() } else { format!("SSH agent ({comment})") };
                return Ok(Authenticated { method, typed: None });
            }
            // First encrypted key met: its passphrase is asked for if nothing else gets through.
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
                    message: "None of the keys of the SSH agent or of ~/.ssh was accepted. Choose a key file in the host form, or add your key to the SSH agent."
                        .into(),
                }),
            }
        }
    }
}

/// Loop of a session: relays the keystrokes to the server and the output to the interface.
async fn pump(
    mut channel: russh::Channel<client::Msg>,
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    on_data: Channel,
    mut startup: Option<String>,
    key: &str,
) -> (Option<u32>, Option<String>, Ending) {
    let mut exit_status = None;
    // The server said the session was ending (exit code, end of stream): it is not lost.
    let mut ended_by_server = false;
    // The startup command is sent once the shell has been quiet for 250 ms, and after 3 s at the latest.
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
                    crate::journal::feed(key, &data);
                    let _ = on_data.send(InvokeResponseBody::Raw(data.to_vec()));
                }
                Some(ChannelMsg::ExtendedData { data, .. }) => {
                    crate::journal::feed(key, &data);
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

/// The channel fell silent without a close message: the session was ending if the server had said
/// so, otherwise the connection is lost.
fn ending_without_close(ended_by_server: bool) -> Ending {
    if ended_by_server { Ending::Exit } else { Ending::Lost }
}

/// The user of this machine: the user of a host that names none.
pub fn current_user() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default()
}

/// Values the form offers: the current user, the hosts of `~/.ssh/config`, the keys present.
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

/// A connection open and authenticated, ready to receive a channel.
struct Opened {
    handle: Handle<ClientHandler>,
    fingerprint: String,
    known_host: bool,
    learn_failed: Option<String>,
    auth_method: String,
    /// The typed secret that worked, and the name to keep it under if asked.
    typed: Option<(String, String)>,
}

/// Connects to `host:port`, checks the fingerprint of the server and authenticates `user`. Shared
/// by a session and the test of a host. `forwards`: where the connections of the remote forwards
/// of a session go; none for a test.
async fn open_authenticated(
    host: &str,
    port: u16,
    user: &str,
    auth: &Auth,
    accept_fingerprint: Option<String>,
    remember_host: bool,
    forwards: Option<(Tunnels, String)>,
) -> Result<Opened, SshError> {
    let host = host.trim();
    // The password is found before connecting: without it, there is no point bothering the server.
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
        forwards,
    };

    let connecting = client::connect(config, (host, port), handler);
    let connection = tokio::time::timeout(Duration::from_secs(15), connecting).await;
    let seen = verdict.lock().ok().and_then(|mut slot| slot.take());

    let mut handle = match connection {
        Err(_) => return Err(other(format!("{host}:{port} is not responding (timed out after 15 s)."))),
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
            let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
            Err(e)
        }
    }
}

/// Tests a host: connection, fingerprint and authentication, then disconnects at once.
/// No terminal is opened, nothing is saved, except the fingerprint the user asks to remember.
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
        None,
    )
    .await?;
    let _ = opened.handle.disconnect(Disconnect::ByApplication, "", "en").await;
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
    tunnels: State<'_, Tunnels>,
    params: ConnectParams,
    on_data: Channel,
) -> Result<Connected, SshError> {
    let started = Instant::now();
    let group = params.group.clone();
    let forwards = (!group.is_empty()).then(|| (tunnels.inner().clone(), group.clone()));
    let Opened { handle, fingerprint, known_host, learn_failed, auth_method, typed } = open_authenticated(
        &params.host,
        params.port,
        &params.user,
        &params.auth,
        params.accept_fingerprint.clone(),
        params.remember_host,
        forwards,
    )
    .await?;

    // The secret has worked: now is when it is kept, if that was asked. A keyring that refuses does
    // not stop the session: the interface will say so.
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
        .map_err(|_| other("The session state is unavailable. Restart Ramure."))?
        .insert(id, tx);

    // The log opens before the output is read: nothing escapes it.
    let key = format!("ssh:{id}");
    let journal_failed = params.journal.as_ref().and_then(|open| crate::journal::start(&key, open).err());

    let startup = params.startup_command.filter(|command| !command.trim().is_empty());
    // The session is ready: its tunnels go through this connection until its pump ends.
    let handle = Arc::new(handle);
    let tunnels = tunnels.inner().clone();
    if !group.is_empty() {
        tunnels.attach(&group, &handle);
    }
    tauri::async_runtime::spawn(async move {
        let (exit_status, reason, ending) = pump(channel, rx, on_data, startup, &key).await;
        crate::journal::stop(&key);
        // Before the interface hears of the end: its tunnels wait for the next connection.
        if !group.is_empty() {
            tunnels.detach(&group, &handle);
        }
        let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
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
        journal_failed,
    })
}

fn send(state: &SshState, id: u32, cmd: Cmd) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|_| "The session state is unavailable. Restart Ramure.".to_string())?;
    let tx = sessions.get(&id).ok_or_else(|| "This session is closed.".to_string())?;
    tx.send(cmd).map_err(|_| "This session is closed.".to_string())
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
