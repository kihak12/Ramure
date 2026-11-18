//! The file explorer (module Files): the folders of this machine, and those of the hosts of the
//! book over SFTP.
//!
//! A pane of the interface shows one place: this machine, or an SFTP connection opened for it.
//! Every command takes the place it acts on, so that one code path lists, renames or copies
//! files wherever they are. Transfers between two places are in `transfer`.

pub mod paths;
pub mod transfer;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use russh::client::Handle;
use russh::Disconnect;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, FileType, OpenFlags, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::ssh::{self, Auth, ClientHandler, Opened, SshError};

/// A place files live in: this machine, or an SFTP connection opened by `sftp_connect`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Place {
    Local,
    Remote { id: u32 },
}

/// An SFTP connection: the session of the subsystem, and the SSH connection that carries it.
pub struct Remote {
    sftp: SftpSession,
    handle: Handle<ClientHandler>,
}

#[derive(Default)]
pub struct FilesState {
    remotes: Mutex<HashMap<u32, Arc<Remote>>>,
    next_id: AtomicU32,
    /// The transfers under way, by the id the interface gave them: setting the flag cancels one.
    transfers: Mutex<HashMap<u32, Arc<AtomicBool>>>,
}

/// Where a command acts: the place resolved to what reaches it.
#[derive(Clone)]
pub enum Fs {
    Local,
    Remote(Arc<Remote>),
}

impl FilesState {
    fn fs(&self, place: Place) -> Result<Fs, FsError> {
        match place {
            Place::Local => Ok(Fs::Local),
            Place::Remote { id } => {
                let remotes = self.remotes.lock().map_err(|_| FsError::other("The state of the connections is unavailable. Restart Ramure."))?;
                remotes.get(&id).cloned().map(Fs::Remote).ok_or(FsError::Lost)
            }
        }
    }
}

/// Why a command failed. The interface says it as it is, after what it was doing.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsError {
    /// The connection to the host is gone: the pane offers to connect again.
    Lost,
    /// The transfer was canceled by the user.
    Canceled,
    /// A full sentence: what happened, and what to do when there is something to do.
    Other { message: String },
}

impl FsError {
    pub fn other(message: impl Into<String>) -> Self {
        FsError::Other { message: message.into() }
    }

    /// The sentence, for a list of failures.
    pub fn message(&self) -> String {
        match self {
            FsError::Lost => "The connection to the host was lost.".into(),
            FsError::Canceled => "Canceled.".into(),
            FsError::Other { message } => message.clone(),
        }
    }
}

/// An error of this machine, said with the path it is about.
pub fn local_error(e: &std::io::Error, path: &str) -> FsError {
    use std::io::ErrorKind;
    FsError::other(match e.kind() {
        ErrorKind::NotFound => format!("{path} does not exist, or no longer does."),
        ErrorKind::PermissionDenied => format!("Access denied to {path}. Check its permissions."),
        ErrorKind::AlreadyExists => format!("{path} already exists."),
        ErrorKind::DirectoryNotEmpty => format!("{path} is not empty."),
        ErrorKind::NotADirectory => format!("{path} is not a folder."),
        ErrorKind::IsADirectory => format!("{path} is a folder."),
        ErrorKind::StorageFull => format!("No space left for {path}: free some room on the disk."),
        ErrorKind::ReadOnlyFilesystem => format!("{path} is on a read-only disk."),
        _ => format!("{path}: {e}"),
    })
}

impl Remote {
    /// An error of the server, said with the path it is about. A connection that has dropped
    /// is said as such, whatever the request that found it.
    pub fn error(&self, e: &SftpError, path: &str) -> FsError {
        if self.handle.is_closed() {
            return FsError::Lost;
        }
        FsError::other(match e {
            SftpError::Status(status) => match status.status_code {
                StatusCode::NoSuchFile => format!("{path} does not exist, or no longer does."),
                StatusCode::PermissionDenied => format!("Access denied to {path}. Check its permissions on the server."),
                StatusCode::NoConnection | StatusCode::ConnectionLost => return FsError::Lost,
                StatusCode::OpUnsupported => format!("The server does not support this operation on {path}."),
                _ if !status.error_message.trim().is_empty() && status.error_message.trim() != "Failure" => {
                    format!("{path}: {}", status.error_message.trim())
                }
                // OpenSSH says "Failure" alone for most refusals: a folder that is not empty, a
                // name that is taken.
                _ => format!("The server refused the operation on {path}. The name may be taken, or the folder not empty."),
            },
            SftpError::Timeout => format!("The server did not answer in time about {path}."),
            other => format!("{path}: {other}"),
        })
    }
}

/// What a row of a pane shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Dir,
    File,
    Link,
    Other,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    /// A folder, or a link that leads to one: it opens like a folder.
    pub dir: bool,
    pub size: u64,
    /// Seconds since the epoch; none when the system does not say.
    pub modified: Option<i64>,
    /// Permission bits (`0o755`); none on Windows.
    pub mode: Option<u32>,
    /// A name that starts with a dot, or a file Windows hides.
    pub hidden: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// The folder, as the system names it once resolved.
    pub path: String,
    /// The folder above it; none at the root.
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}

/// Seconds since the epoch, before or after it.
fn seconds(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).ok(),
        Err(before) => i64::try_from(before.duration().as_secs()).ok().map(|s| -s),
    }
}

/// What the system says of a file of this machine, without following a link.
fn local_entry(name: String, meta: &std::fs::Metadata, target: Option<&std::fs::Metadata>) -> Entry {
    let kind = if meta.is_symlink() {
        Kind::Link
    } else if meta.is_dir() {
        Kind::Dir
    } else if meta.is_file() {
        Kind::File
    } else {
        Kind::Other
    };
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        Some(meta.permissions().mode() & 0o7777)
    };
    #[cfg(not(unix))]
    let mode = None;
    #[cfg(windows)]
    let hidden_by_system = {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_HIDDEN
        meta.file_attributes() & 0x2 != 0
    };
    #[cfg(not(windows))]
    let hidden_by_system = false;
    let shown = target.unwrap_or(meta);
    Entry {
        hidden: name.starts_with('.') || hidden_by_system,
        name,
        kind,
        dir: shown.is_dir(),
        size: if shown.is_file() { shown.len() } else { 0 },
        modified: shown.modified().ok().and_then(seconds),
        mode,
    }
}

/// What the server says of a file, from the attributes of its folder; `target` follows a link.
fn remote_entry(name: String, attrs: &FileAttributes, target: Option<&FileAttributes>) -> Entry {
    let kind = match attrs.file_type() {
        FileType::Dir => Kind::Dir,
        FileType::File => Kind::File,
        FileType::Symlink => Kind::Link,
        FileType::Other => Kind::Other,
    };
    let shown = target.unwrap_or(attrs);
    let dir = shown.file_type() == FileType::Dir;
    Entry {
        hidden: name.starts_with('.'),
        name,
        kind,
        dir,
        size: if dir { 0 } else { shown.size.unwrap_or(0) },
        modified: shown.mtime.map(i64::from),
        mode: attrs.permissions.map(|mode| mode & 0o7777),
    }
}

impl Fs {
    /// The folder a pane opens on: the home folder.
    pub async fn home(&self) -> Result<String, FsError> {
        match self {
            Fs::Local => ssh::config::home()
                .map(|home| home.to_string_lossy().to_string())
                .ok_or_else(|| FsError::other("Your home folder could not be found.")),
            Fs::Remote(remote) => remote.sftp.canonicalize(".").await.map_err(|e| remote.error(&e, "your home folder")),
        }
    }

    /// The folder `path`, resolved: `~` and `..` are followed, links too.
    pub async fn resolve(&self, path: &str) -> Result<String, FsError> {
        let path = path.trim();
        match self {
            Fs::Local => {
                // `~` alone, or nothing, is the home folder; `expand` only knows `~/`.
                let expanded = match (path, ssh::config::home()) {
                    ("" | "~", Some(home)) => home,
                    _ => ssh::config::expand(path),
                };
                let shown = expanded.to_string_lossy().to_string();
                let resolved = tokio::fs::canonicalize(&expanded).await.map_err(|e| local_error(&e, &shown))?;
                Ok(paths::local_display(&resolved))
            }
            Fs::Remote(remote) => {
                let wanted = paths::remote_tilde(path);
                remote.sftp.canonicalize(wanted.as_str()).await.map_err(|e| remote.error(&e, path))
            }
        }
    }

    /// The content of the folder `path`, in the order of the system.
    pub async fn list(&self, path: &str) -> Result<Listing, FsError> {
        let path = self.resolve(path).await?;
        match self {
            Fs::Local => {
                let mut dir = tokio::fs::read_dir(&path).await.map_err(|e| local_error(&e, &path))?;
                let mut entries = Vec::new();
                while let Some(item) = dir.next_entry().await.map_err(|e| local_error(&e, &path))? {
                    let name = item.file_name().to_string_lossy().to_string();
                    // A file that vanishes between the list and its metadata is left out.
                    let Ok(meta) = tokio::fs::symlink_metadata(item.path()).await else { continue };
                    let target = if meta.is_symlink() { tokio::fs::metadata(item.path()).await.ok() } else { None };
                    entries.push(local_entry(name, &meta, target.as_ref()));
                }
                let parent = paths::local_parent(&path);
                Ok(Listing { path, parent, entries })
            }
            Fs::Remote(remote) => {
                let dir = remote.sftp.read_dir(path.as_str()).await.map_err(|e| remote.error(&e, &path))?;
                let mut entries = Vec::new();
                for item in dir {
                    let name = item.file_name();
                    if name == "." || name == ".." {
                        continue;
                    }
                    let attrs = item.metadata();
                    let target = if attrs.file_type() == FileType::Symlink {
                        remote.sftp.metadata(paths::remote_join(&path, &name)).await.ok()
                    } else {
                        None
                    };
                    entries.push(remote_entry(name, &attrs, target.as_ref()));
                }
                let parent = paths::remote_parent(&path);
                Ok(Listing { path, parent, entries })
            }
        }
    }

    /// The path of `name` in the folder `dir`, as this place writes it.
    pub fn join(&self, dir: &str, name: &str) -> String {
        match self {
            Fs::Local => paths::local_join(dir, name),
            Fs::Remote(_) => paths::remote_join(dir, name),
        }
    }

    /// The last part of a path.
    pub fn name_of(&self, path: &str) -> String {
        match self {
            Fs::Local => paths::local_name(path),
            Fs::Remote(_) => paths::remote_name(path),
        }
    }

    /// The folder that holds `path`; none at the root.
    pub fn parent_of(&self, path: &str) -> Option<String> {
        match self {
            Fs::Local => paths::local_parent(path),
            Fs::Remote(_) => paths::remote_parent(path),
        }
    }

    /// What is at `path`, without following a link; none when nothing is there.
    pub async fn lstat(&self, path: &str) -> Result<Option<Meta>, FsError> {
        match self {
            Fs::Local => match tokio::fs::symlink_metadata(path).await {
                Ok(meta) => Ok(Some(Meta::local(&meta))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(local_error(&e, path)),
            },
            Fs::Remote(remote) => match remote.sftp.symlink_metadata(path).await {
                Ok(attrs) => Ok(Some(Meta::remote(&attrs))),
                Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => Ok(None),
                Err(e) => Err(remote.error(&e, path)),
            },
        }
    }

    /// What is at `path`, following a link.
    pub async fn stat(&self, path: &str) -> Result<Meta, FsError> {
        match self {
            Fs::Local => tokio::fs::metadata(path).await.map(|meta| Meta::local(&meta)).map_err(|e| local_error(&e, path)),
            Fs::Remote(remote) => remote.sftp.metadata(path).await.map(|a| Meta::remote(&a)).map_err(|e| remote.error(&e, path)),
        }
    }

    /// The names of the folder `path`, with what each is, without following links.
    pub async fn children(&self, path: &str) -> Result<Vec<(String, Meta)>, FsError> {
        match self {
            Fs::Local => {
                let mut dir = tokio::fs::read_dir(path).await.map_err(|e| local_error(&e, path))?;
                let mut out = Vec::new();
                while let Some(item) = dir.next_entry().await.map_err(|e| local_error(&e, path))? {
                    let Ok(meta) = tokio::fs::symlink_metadata(item.path()).await else { continue };
                    out.push((item.file_name().to_string_lossy().to_string(), Meta::local(&meta)));
                }
                Ok(out)
            }
            Fs::Remote(remote) => {
                let dir = remote.sftp.read_dir(path).await.map_err(|e| remote.error(&e, path))?;
                Ok(dir
                    .filter(|item| item.file_name() != "." && item.file_name() != "..")
                    .map(|item| (item.file_name(), Meta::remote(&item.metadata())))
                    .collect())
            }
        }
    }

    pub async fn mkdir(&self, path: &str) -> Result<(), FsError> {
        match self {
            Fs::Local => tokio::fs::create_dir(path).await.map_err(|e| local_error(&e, path)),
            Fs::Remote(remote) => remote.sftp.create_dir(path).await.map_err(|e| remote.error(&e, path)),
        }
    }

    /// Creates an empty file; refused if something of that name is already there, so that
    /// nothing is ever emptied.
    pub async fn create_file(&self, path: &str) -> Result<(), FsError> {
        match self {
            Fs::Local => tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .await
                .map(drop)
                .map_err(|e| local_error(&e, path)),
            Fs::Remote(remote) => remote
                .sftp
                .open_with_flags(path, OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE)
                .await
                .map(drop)
                .map_err(|e| remote.error(&e, path)),
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        match self {
            Fs::Local => tokio::fs::rename(from, to).await.map_err(|e| {
                if paths::crosses_devices(&e) {
                    FsError::other(format!("{to} is on another disk than {from}: copy it there, then delete the original."))
                } else {
                    local_error(&e, from)
                }
            }),
            Fs::Remote(remote) => remote.sftp.rename(from, to).await.map_err(|e| remote.error(&e, from)),
        }
    }

    pub async fn remove_file(&self, path: &str) -> Result<(), FsError> {
        match self {
            Fs::Local => tokio::fs::remove_file(path).await.map_err(|e| local_error(&e, path)),
            Fs::Remote(remote) => remote.sftp.remove_file(path).await.map_err(|e| remote.error(&e, path)),
        }
    }

    pub async fn remove_dir(&self, path: &str) -> Result<(), FsError> {
        match self {
            Fs::Local => tokio::fs::remove_dir(path).await.map_err(|e| local_error(&e, path)),
            Fs::Remote(remote) => remote.sftp.remove_dir(path).await.map_err(|e| remote.error(&e, path)),
        }
    }

    /// Deletes `path`, and what a folder holds. A link is deleted, never what it leads to.
    pub async fn remove_all(&self, path: &str) -> Result<(), FsError> {
        let Some(top) = self.lstat(path).await? else { return Ok(()) };
        if !top.is_dir() {
            return self.remove_file(path).await;
        }
        // Depth first, without recursion: the folders are emptied, then removed from the deepest.
        let mut pending = vec![path.to_string()];
        let mut folders = Vec::new();
        while let Some(dir) = pending.pop() {
            for (name, meta) in self.children(&dir).await? {
                let child = self.join(&dir, &name);
                if meta.is_dir() {
                    pending.push(child);
                } else {
                    self.remove_file(&child).await?;
                }
            }
            folders.push(dir);
        }
        for dir in folders.iter().rev() {
            self.remove_dir(dir).await?;
        }
        Ok(())
    }

    /// Opens a file to read it.
    pub async fn reader(&self, path: &str) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, FsError> {
        match self {
            Fs::Local => Ok(Box::new(tokio::fs::File::open(path).await.map_err(|e| local_error(&e, path))?)),
            Fs::Remote(remote) => Ok(Box::new(remote.sftp.open(path).await.map_err(|e| remote.error(&e, path))?)),
        }
    }

    /// Creates a file, or empties the one that is there, to write it.
    pub async fn writer(&self, path: &str) -> Result<Box<dyn tokio::io::AsyncWrite + Send + Unpin>, FsError> {
        match self {
            Fs::Local => Ok(Box::new(tokio::fs::File::create(path).await.map_err(|e| local_error(&e, path))?)),
            Fs::Remote(remote) => Ok(Box::new(remote.sftp.create(path).await.map_err(|e| remote.error(&e, path))?)),
        }
    }

    /// Gives a file the permissions of its original, where both sides have some. Best effort:
    /// a copy that keeps the default permissions is still a copy.
    pub async fn set_mode(&self, path: &str, mode: u32) {
        let mode = mode & 0o7777;
        match self {
            Fs::Local => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await;
                }
                #[cfg(not(unix))]
                let _ = (path, mode);
            }
            Fs::Remote(remote) => {
                let attrs = FileAttributes { permissions: Some(mode), ..FileAttributes::empty() };
                let _ = remote.sftp.set_metadata(path, attrs).await;
            }
        }
    }

    pub fn is_lost(&self) -> bool {
        matches!(self, Fs::Remote(remote) if remote.handle.is_closed())
    }
}

/// What a file is, for a transfer: its kind, its size, its permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    pub kind: Kind,
    pub size: u64,
    pub mode: Option<u32>,
}

impl Meta {
    fn local(meta: &std::fs::Metadata) -> Self {
        let kind = if meta.is_symlink() {
            Kind::Link
        } else if meta.is_dir() {
            Kind::Dir
        } else if meta.is_file() {
            Kind::File
        } else {
            Kind::Other
        };
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            Some(meta.permissions().mode() & 0o7777)
        };
        #[cfg(not(unix))]
        let mode = None;
        Meta { kind, size: meta.len(), mode }
    }

    fn remote(attrs: &FileAttributes) -> Self {
        let kind = match attrs.file_type() {
            FileType::Dir => Kind::Dir,
            FileType::File => Kind::File,
            FileType::Symlink => Kind::Link,
            FileType::Other => Kind::Other,
        };
        Meta { kind, size: attrs.size.unwrap_or(0), mode: attrs.permissions.map(|m| m & 0o7777) }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }
}

// --- Connections --------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpParams {
    host: String,
    port: u16,
    user: String,
    auth: Auth,
    accept_fingerprint: Option<String>,
    #[serde(default)]
    remember_host: bool,
    /// True to keep the typed secret in the system keyring, once it has worked.
    #[serde(default)]
    remember_secret: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpOpened {
    id: u32,
    /// The home folder of the user on the server.
    home: String,
    known_host: bool,
    /// The fingerprint was to be remembered and `known_hosts` could not be written: why.
    learn_failed: Option<String>,
    auth_method: String,
    connect_ms: f64,
    /// The secret was to be kept and the keyring refused it: what the keyring says.
    keep_failed: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SftpClosed {
    id: u32,
}

/// Opens an SFTP connection: the same connection, fingerprint check and authentication as a
/// terminal, then the `sftp` subsystem instead of a shell.
#[tauri::command]
pub async fn sftp_connect(app: AppHandle, state: State<'_, FilesState>, params: SftpParams) -> Result<SftpOpened, SshError> {
    let started = Instant::now();
    let Opened { handle, known_host, learn_failed, auth_method, typed, .. } = ssh::open_authenticated(
        &params.host,
        params.port,
        &params.user,
        &params.auth,
        params.accept_fingerprint.clone(),
        params.remember_host,
        None,
    )
    .await?;

    let mut keep_failed = None;
    if params.remember_secret
        && let Some((account, secret)) = typed
    {
        keep_failed = ssh::secrets::keep(account, secret).await.err();
    }

    let opened = async {
        let channel = handle.channel_open_session().await.map_err(ssh::other)?;
        channel.request_subsystem(true, "sftp").await.map_err(ssh::other)?;
        let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| {
            ssh::other(format!("The server did not start SFTP ({e}). Check that its SSH service offers the sftp subsystem."))
        })?;
        // Long enough for a busy server to answer about a large folder.
        sftp.set_timeout(30);
        let home = sftp.canonicalize(".").await.unwrap_or_else(|_| "/".to_string());
        Ok::<_, SshError>((sftp, home))
    }
    .await;
    let (sftp, home) = match opened {
        Ok(done) => done,
        Err(e) => {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
            return Err(e);
        }
    };

    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let remote = Arc::new(Remote { sftp, handle });
    state
        .remotes
        .lock()
        .map_err(|_| ssh::other("The state of the connections is unavailable. Restart Ramure."))?
        .insert(id, remote.clone());

    // A connection that drops while nobody uses it is said at once: the pane offers to connect
    // again rather than waiting for the next click to fail.
    let watcher = Arc::downgrade(&remote);
    drop(remote);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let Some(remote) = watcher.upgrade() else { return };
            if remote.handle.is_closed() {
                if let Ok(mut remotes) = app.state::<FilesState>().remotes.lock() {
                    remotes.remove(&id);
                }
                let _ = app.emit("sftp-closed", SftpClosed { id });
                return;
            }
        }
    });

    Ok(SftpOpened {
        id,
        home,
        known_host,
        learn_failed,
        auth_method,
        connect_ms: started.elapsed().as_secs_f64() * 1000.0,
        keep_failed,
    })
}

/// Closes an SFTP connection. A transfer that still uses it fails, as it would on a lost connection.
#[tauri::command]
pub async fn sftp_close(state: State<'_, FilesState>, id: u32) -> Result<(), String> {
    let remote = state.remotes.lock().map_err(|_| "The state of the connections is unavailable.".to_string())?.remove(&id);
    if let Some(remote) = remote {
        let _ = remote.sftp.close().await;
        let _ = remote.handle.disconnect(Disconnect::ByApplication, "", "en").await;
    }
    Ok(())
}

// --- Browsing -----------------------------------------------------------------------------------

/// The home folder of a place: where a pane opens when its host gives no start folder.
#[tauri::command]
pub async fn files_home(state: State<'_, FilesState>, place: Place) -> Result<String, FsError> {
    state.fs(place)?.home().await
}

#[tauri::command]
pub async fn files_list(state: State<'_, FilesState>, place: Place, path: String) -> Result<Listing, FsError> {
    state.fs(place)?.list(&path).await
}

/// Checks that a name typed by the user names one file of the folder, not a path.
fn valid_name(name: &str) -> Result<&str, FsError> {
    let name = name.trim();
    paths::check_name(name).map_err(FsError::other)?;
    Ok(name)
}

/// Creates the folder `name` in `dir`. Its path.
#[tauri::command]
pub async fn files_mkdir(state: State<'_, FilesState>, place: Place, dir: String, name: String) -> Result<String, FsError> {
    let fs = state.fs(place)?;
    let path = fs.join(&dir, valid_name(&name)?);
    if fs.lstat(&path).await?.is_some() {
        return Err(FsError::other(format!("“{}” already exists in {dir}. Choose another name.", name.trim())));
    }
    fs.mkdir(&path).await?;
    Ok(path)
}

/// Creates the empty file `name` in `dir`. Its path.
#[tauri::command]
pub async fn files_create(state: State<'_, FilesState>, place: Place, dir: String, name: String) -> Result<String, FsError> {
    let fs = state.fs(place)?;
    let path = fs.join(&dir, valid_name(&name)?);
    if fs.lstat(&path).await?.is_some() {
        return Err(FsError::other(format!("“{}” already exists in {dir}. Choose another name.", name.trim())));
    }
    fs.create_file(&path).await?;
    Ok(path)
}

/// Gives `path` a new name, in the same folder. Its new path.
#[tauri::command]
pub async fn files_rename(state: State<'_, FilesState>, place: Place, path: String, name: String) -> Result<String, FsError> {
    let fs = state.fs(place)?;
    let name = valid_name(&name)?;
    let dir = match place {
        Place::Local => paths::local_parent(&path),
        Place::Remote { .. } => paths::remote_parent(&path),
    }
    .ok_or_else(|| FsError::other("The root of the disk cannot be renamed."))?;
    let to = fs.join(&dir, name);
    if to == path {
        return Ok(to);
    }
    // A name that differs only by case is the same file on Windows: renaming it is allowed.
    let same_file = place == Place::Local && cfg!(windows) && to.to_lowercase() == path.to_lowercase();
    if !same_file && fs.lstat(&to).await?.is_some() {
        return Err(FsError::other(format!("“{name}” already exists in {dir}. Choose another name.")));
    }
    fs.rename(&path, &to).await?;
    Ok(to)
}

/// Deletes these files and folders, and what the folders hold. Stops at the first failure.
#[tauri::command]
pub async fn files_remove(state: State<'_, FilesState>, place: Place, paths: Vec<String>) -> Result<(), FsError> {
    let fs = state.fs(place)?;
    for path in &paths {
        fs.remove_all(path).await?;
    }
    Ok(())
}

/// The names among `names` that already exist in `dir`: a copy there would replace them.
#[tauri::command]
pub async fn files_existing(state: State<'_, FilesState>, place: Place, dir: String, names: Vec<String>) -> Result<Vec<String>, FsError> {
    let fs = state.fs(place)?;
    let mut taken = Vec::new();
    for name in names {
        if fs.lstat(&fs.join(&dir, &name)).await?.is_some() {
            taken.push(name);
        }
    }
    Ok(taken)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
    /// How many items were moved.
    moved: usize,
    /// The names that were left where they are, because the folder already has one.
    taken: Vec<String>,
}

/// Moves these files and folders into `dest`, in the same place: a rename, nothing is copied.
/// An item whose name `dest` already has stays where it is.
#[tauri::command]
pub async fn files_move(state: State<'_, FilesState>, place: Place, sources: Vec<String>, dest: String) -> Result<Moved, FsError> {
    let fs = state.fs(place)?;
    let mut moved = 0;
    let mut taken = Vec::new();
    for source in &sources {
        let name = fs.name_of(source);
        let to = fs.join(&dest, &name);
        if &to == source {
            continue;
        }
        if paths::inside(source, &dest, place == Place::Local) {
            return Err(FsError::other(format!("“{name}” cannot be moved into itself.")));
        }
        if fs.lstat(&to).await?.is_some() {
            taken.push(name);
            continue;
        }
        fs.rename(source, &to).await?;
        moved += 1;
    }
    Ok(Moved { moved, taken })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An SFTP connection for the tests that need a server: `RAMURE_SFTP_TEST` names it as
    /// `user@host:port:key`. Without it, those tests return at once.
    async fn test_remote() -> Option<Arc<Remote>> {
        let spec = std::env::var("RAMURE_SFTP_TEST").ok()?;
        let (account, rest) = spec.split_once('@')?;
        let mut parts = rest.splitn(3, ':');
        let (host, port, key) = (parts.next()?, parts.next()?.parse::<u16>().ok()?, parts.next()?);
        let auth = || Auth::KeyFile { path: key.to_string(), passphrase: None };
        let opened = match ssh::open_authenticated(host, port, account, &auth(), None, false, None).await {
            Ok(opened) => opened,
            Err(SshError::HostKeyUnknown { fingerprint, .. }) => {
                ssh::open_authenticated(host, port, account, &auth(), Some(fingerprint), false, None).await.ok()?
            }
            Err(e) => panic!("{e:?}"),
        };
        let channel = opened.handle.channel_open_session().await.unwrap();
        channel.request_subsystem(true, "sftp").await.unwrap();
        let sftp = SftpSession::new(channel.into_stream()).await.unwrap();
        Some(Arc::new(Remote { sftp, handle: opened.handle }))
    }

    #[tokio::test]
    async fn files_go_both_ways_between_this_machine_and_a_server() {
        let Some(remote) = test_remote().await else { return };
        let server = Fs::Remote(remote.clone());
        let base = format!("/tmp/ramure-sftp-{}", uuid::Uuid::new_v4());
        server.mkdir(&base).await.unwrap();

        // Up: a folder of this machine goes to the server.
        let here = std::env::temp_dir().join(format!("ramure-up-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(here.join("site/css")).unwrap();
        std::fs::write(here.join("site/index.html"), b"<h1>hi</h1>").unwrap();
        std::fs::write(here.join("site/css/big.css"), vec![3u8; 2_500_000]).unwrap();
        let sources = [here.join("site").to_string_lossy().to_string()];
        let cancel = AtomicBool::new(false);
        let job = transfer::Job { from: &Fs::Local, to: &server, same: false, sources: &sources, dest: &base, replace: false };
        let summary = transfer::run(&job, &cancel, |_, _| {}).await.unwrap();
        assert!(summary.failed.is_empty(), "{:?}", summary.failed);
        assert_eq!(summary.files, 2);
        let listing = server.list(&format!("{base}/site/css")).await.unwrap();
        assert_eq!(listing.entries.iter().map(|e| (e.name.as_str(), e.size)).collect::<Vec<_>>(), vec![("big.css", 2_500_000)]);

        // Again, replacing: the file is whole, and no part file is left.
        let job = transfer::Job { replace: true, ..job };
        transfer::run(&job, &cancel, |_, _| {}).await.unwrap();
        let names: Vec<String> = server.list(&format!("{base}/site")).await.unwrap().entries.into_iter().map(|e| e.name).collect();
        assert!(names.iter().all(|n| !n.ends_with(".ramure-part")), "{names:?}");

        // Server to server, through this machine.
        server.mkdir(&format!("{base}/copy")).await.unwrap();
        let sources = [format!("{base}/site")];
        let dest = format!("{base}/copy");
        let job = transfer::Job { from: &server, to: &server, same: true, sources: &sources, dest: &dest, replace: false };
        transfer::run(&job, &cancel, |_, _| {}).await.unwrap();
        assert_eq!(server.stat(&format!("{base}/copy/site/css/big.css")).await.unwrap().size, 2_500_000);

        // Down: back to this machine.
        let down = std::env::temp_dir().join(format!("ramure-down-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&down).unwrap();
        let sources = [format!("{base}/copy/site")];
        let dest = down.to_string_lossy().to_string();
        let job = transfer::Job { from: &server, to: &Fs::Local, same: false, sources: &sources, dest: &dest, replace: false };
        transfer::run(&job, &cancel, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(down.join("site/index.html")).unwrap(), b"<h1>hi</h1>");
        assert_eq!(std::fs::metadata(down.join("site/css/big.css")).unwrap().len(), 2_500_000);

        // Rename and delete on the server.
        server.rename(&format!("{base}/copy"), &format!("{base}/renamed")).await.unwrap();
        assert!(server.lstat(&format!("{base}/renamed/site")).await.unwrap().is_some());
        server.remove_all(&base).await.unwrap();
        assert!(server.lstat(&base).await.unwrap().is_none());
        assert_eq!(server.resolve("~").await.unwrap(), server.home().await.unwrap());

        std::fs::remove_dir_all(&here).unwrap();
        std::fs::remove_dir_all(&down).unwrap();
    }

    #[test]
    fn a_remote_entry_says_what_a_link_leads_to() {
        let mut link = FileAttributes::empty();
        link.set_symlink(true);
        link.permissions = Some(link.permissions.unwrap_or(0) | 0o777);
        let mut folder = FileAttributes::empty();
        folder.set_dir(true);
        folder.mtime = Some(1_700_000_000);
        let entry = remote_entry("www".into(), &link, Some(&folder));
        assert_eq!(entry.kind, Kind::Link);
        assert!(entry.dir);
        assert_eq!(entry.size, 0);
        assert_eq!(entry.modified, Some(1_700_000_000));
        assert_eq!(entry.mode, Some(0o777));
    }

    #[test]
    fn a_dot_file_is_hidden() {
        let mut file = FileAttributes::empty();
        file.set_regular(true);
        file.size = Some(42);
        let entry = remote_entry(".bashrc".into(), &file, None);
        assert!(entry.hidden);
        assert!(!entry.dir);
        assert_eq!(entry.size, 42);
    }

    #[tokio::test]
    async fn the_home_folder_is_where_a_pane_of_this_machine_opens() {
        let Some(home) = ssh::config::home() else { return };
        let Ok(home) = std::fs::canonicalize(home) else { return };
        let expected = paths::local_display(&home);
        assert_eq!(Fs::Local.resolve("").await.unwrap(), expected);
        assert_eq!(Fs::Local.resolve("~").await.unwrap(), expected);
        assert_eq!(Fs::Local.list("").await.unwrap().path, expected);
    }

    #[tokio::test]
    async fn a_local_folder_is_listed_with_its_links() {
        let dir = std::env::temp_dir().join(format!("ramure-list-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("sub"), dir.join("link")).unwrap();
        let listing = Fs::Local.list(&dir.to_string_lossy()).await.unwrap();
        let find = |name: &str| listing.entries.iter().find(|e| e.name == name).cloned();
        assert_eq!(find("a.txt").map(|e| (e.kind, e.size)), Some((Kind::File, 5)));
        assert_eq!(find("sub").map(|e| (e.kind, e.dir)), Some((Kind::Dir, true)));
        #[cfg(unix)]
        assert_eq!(find("link").map(|e| (e.kind, e.dir)), Some((Kind::Link, true)));
        assert!(listing.parent.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_new_file_is_empty_and_never_replaces_one_that_is_there() {
        let dir = std::env::temp_dir().join(format!("ramure-create-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.txt").to_string_lossy().to_string();
        Fs::Local.create_file(&path).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        std::fs::write(&path, b"kept").unwrap();
        assert!(Fs::Local.create_file(&path).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"kept");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_new_file_on_a_server_never_replaces_one_that_is_there() {
        let Some(remote) = test_remote().await else { return };
        let server = Fs::Remote(remote);
        let base = format!("/tmp/ramure-sftp-{}", uuid::Uuid::new_v4());
        server.mkdir(&base).await.unwrap();
        let path = format!("{base}/notes.txt");
        server.create_file(&path).await.unwrap();
        assert_eq!(server.lstat(&path).await.unwrap().map(|m| m.size), Some(0));
        assert!(server.create_file(&path).await.is_err());
        server.remove_all(&base).await.unwrap();
    }

    #[tokio::test]
    async fn removing_a_folder_removes_what_it_holds_but_not_what_a_link_leads_to() {
        let dir = std::env::temp_dir().join(format!("ramure-remove-{}", uuid::Uuid::new_v4()));
        let keep = std::env::temp_dir().join(format!("ramure-keep-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::create_dir_all(&keep).unwrap();
        std::fs::write(dir.join("a/b/c.txt"), b"x").unwrap();
        std::fs::write(keep.join("kept.txt"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&keep, dir.join("a/link")).unwrap();
        Fs::Local.remove_all(&dir.to_string_lossy()).await.unwrap();
        assert!(!dir.exists());
        assert!(keep.join("kept.txt").exists());
        std::fs::remove_dir_all(&keep).unwrap();
    }
}
