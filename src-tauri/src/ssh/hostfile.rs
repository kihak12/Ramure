//! Hosts handed from one book to another, in a file of Ramure's own, and the preview of an import
//! from such a file (guidelines, 9.15, "Importing and exporting hosts").
//!
//! The file holds what the book keeps of a host, never a secret: no password, no passphrase. An
//! import adds hosts, and nothing else: a host of the book is never changed or deleted by it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use super::config;
use super::store::{Host, Store, Tunnel};

/// What the file says it is: the import knows a file of Ramure by it.
pub const FORMAT: &str = "ramure-hosts";
/// The version of the file. A later version may add fields, which an earlier Ramure leaves out.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostFile {
    format: String,
    version: u32,
    hosts: Vec<Shared>,
}

/// A host as the file holds it: what the book keeps, without its identifier, its date or any secret.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Shared {
    alias: String,
    hostname: String,
    port: u16,
    user: String,
    auth: String,
    key_path: String,
    group: String,
    tags: Vec<String>,
    color: String,
    startup_command: String,
    proxy_jump: bool,
    log_sessions: bool,
    tunnels: Vec<Tunnel>,
}

/// A key under `~/.ssh` is written `~/.ssh/name`, which means the same on the machine of whoever
/// gets the file; any other path is kept as it is.
fn portable_key(path: &str) -> String {
    let ssh = config::home().map(|home| home.join(".ssh"));
    match ssh.and_then(|ssh| Path::new(path).strip_prefix(&ssh).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/.ssh/{}", rest.to_string_lossy().replace('\\', "/")),
        None => path.to_string(),
    }
}

impl From<&Host> for Shared {
    fn from(host: &Host) -> Self {
        Shared {
            alias: host.alias.clone(),
            hostname: host.hostname.clone(),
            port: host.port,
            user: host.user.clone(),
            auth: host.auth.clone(),
            key_path: portable_key(&host.key_path),
            group: host.group.clone(),
            tags: host.tags.clone(),
            color: host.color.clone(),
            startup_command: host.startup_command.clone(),
            proxy_jump: host.proxy_jump,
            log_sessions: host.log_sessions,
            tunnels: host.tunnels.clone(),
        }
    }
}

impl From<Shared> for Host {
    fn from(shared: Shared) -> Self {
        Host {
            alias: shared.alias,
            hostname: shared.hostname,
            port: if shared.port == 0 { 22 } else { shared.port },
            user: shared.user,
            auth: shared.auth,
            key_path: shared.key_path,
            group: shared.group,
            tags: shared.tags,
            color: shared.color,
            startup_command: shared.startup_command,
            proxy_jump: shared.proxy_jump,
            log_sessions: shared.log_sessions,
            tunnels: shared.tunnels,
            ..Host::default()
        }
    }
}

/// The text of the file for `hosts`, in their order.
pub fn write(hosts: &[Host]) -> String {
    let file = HostFile { format: FORMAT.into(), version: VERSION, hosts: hosts.iter().map(Shared::from).collect() };
    let mut text = serde_json::to_string_pretty(&file).unwrap_or_default();
    text.push('\n');
    text
}

/// The hosts of a file of Ramure, or why it is not one.
pub fn read(text: &str) -> Result<Vec<Found>, String> {
    let not_ours = || "This is not a file of hosts exported by Ramure.".to_string();
    let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| not_ours())?;
    if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err(not_ours());
    }
    let file: HostFile = serde_json::from_value(value).map_err(|e| format!("This file of hosts cannot be read: {e}."))?;
    Ok(file
        .hosts
        .into_iter()
        .map(|shared| {
            let host = Host::from(shared);
            let refused = (host.alias.trim().is_empty() || host.hostname.trim().is_empty())
                .then(|| "It has no name or no address.".to_string());
            Found { host, refused, password: None, key: None }
        })
        .collect())
}

/// A host found in a source, and what keeps it from coming into the book.
#[derive(Debug, Clone)]
pub struct Found {
    pub host: Host,
    /// Why it cannot come over, said in its row; none when it can.
    pub refused: Option<String>,
    /// A password the source holds: kept, in the system keyring only, when the user asks.
    pub password: Option<String>,
    /// What becomes of a key that OpenSSH cannot read as it is, or that is not on this machine.
    pub key: Option<KeyPlan>,
}

/// What an import does with the key a source points to (guidelines, 9.15).
#[derive(Debug, Clone, PartialEq)]
pub enum KeyPlan {
    /// A PuTTY key with no passphrase: converted into `~/.ssh/<name>` as the host is added.
    Convert { source: PathBuf, into: String },
    /// A PuTTY key with a passphrase: the host comes without it.
    Protected { name: String },
    /// Not on this machine: the host comes without it.
    Missing { path: String },
    /// `~/.ssh` has a file of that name already: the host comes without it.
    Taken { name: String, into: String },
}

/// What becomes of the key `path` of a source: none when OpenSSH reads it as it is.
pub fn plan_key(path: &str) -> Option<KeyPlan> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let full = config::expand(path);
    let Ok(text) = std::fs::read_to_string(&full) else {
        return Some(KeyPlan::Missing { path: path.to_string() });
    };
    let file = full.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    if !file.to_lowercase().ends_with(".ppk") {
        return None;
    }
    if !text.lines().any(|line| line.trim() == "Encryption: none") {
        return Some(KeyPlan::Protected { name: file });
    }
    let name = file[..file.len() - 4].to_string();
    let into = format!("~/.ssh/{name}");
    if config::expand(&into).exists() {
        return Some(KeyPlan::Taken { name: file, into });
    }
    Some(KeyPlan::Convert { source: full, into })
}

/// What the import would do with a host found: one row of the preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Its place among the hosts found.
    pub index: usize,
    /// `added`, `renamed` (its name is taken by another server), `same` (the server is in the
    /// book under another name) or `refused`.
    pub status: &'static str,
    /// The name it takes in the book.
    pub alias: String,
    /// Added: what it is; renamed: what the name is in the book; same: under which name; refused: why.
    pub detail: String,
    /// Checked at first.
    pub checked: bool,
}

/// The preview of an import.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub rows: Vec<Row>,
    /// The hosts already in the book, under the same name and at the same address: not listed.
    pub up_to_date: usize,
    /// The hosts found with a password.
    pub passwords: usize,
    /// The PuTTY keys that the import converts into `~/.ssh`.
    pub keys: usize,
}

/// The server a host reaches: its user, its address and its port.
fn server(host: &Host) -> (String, String, u16) {
    let port = if host.port == 0 { 22 } else { host.port };
    (host.user.trim().to_string(), host.hostname.trim().to_lowercase(), port)
}

/// `deploy@10.0.0.9`, `10.0.0.9:2222`.
fn address(host: &Host) -> String {
    let (user, hostname, port) = (host.user.trim(), host.hostname.trim(), if host.port == 0 { 22 } else { host.port });
    let at = if user.is_empty() { hostname.to_string() } else { format!("{user}@{hostname}") };
    if port == 22 { at } else { format!("{at}:{port}") }
}

/// What an added host is, on one line: `deploy@10.0.0.9 · port 2222 · key ~/.ssh/id_ed25519`.
fn summary(found: &Found) -> String {
    let host = &found.host;
    let mut parts = vec![if host.user.trim().is_empty() { host.hostname.trim().to_string() } else { format!("{}@{}", host.user.trim(), host.hostname.trim()) }];
    if host.port != 22 && host.port != 0 {
        parts.push(format!("port {}", host.port));
    }
    parts.push(match (&found.key, host.auth.as_str()) {
        (Some(KeyPlan::Convert { source, into }), _) => {
            let name = source.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            format!("key {name}, converted to {into}")
        }
        (Some(KeyPlan::Protected { name }), _) => {
            format!("without its key: {name} has a passphrase; export it from PuTTYgen, then import it in Manage keys…")
        }
        (Some(KeyPlan::Missing { path }), _) => format!("without its key: {path} is not on this machine; SSH agent and default keys"),
        (Some(KeyPlan::Taken { name, into }), _) => format!("without its key: {into} already exists; import {name} in Manage keys… under another name"),
        (None, "keyFile") if !host.key_path.trim().is_empty() => {
            let key = host.key_path.trim();
            if config::expand(key).exists() { format!("key {key}") } else { format!("key {key}, not found on this machine") }
        }
        (None, "password") if found.password.is_some() => "password found in the file".into(),
        (None, "password") => "password asked when you connect".into(),
        _ => "SSH agent and default keys".into(),
    });
    parts.join(" · ")
}

/// `name (2)`, `name (3)`: the first that `taken` does not have, case aside.
fn free_alias(alias: &str, taken: &HashSet<String>) -> String {
    (2..).map(|n| format!("{alias} ({n})")).find(|candidate| !taken.contains(&candidate.to_lowercase())).unwrap_or_default()
}

/// The plan of an import of `found` into a book that holds `book`: a row per host to add, renamed,
/// already there under another name, or refused, in the order of the source. A host already in the
/// book under the same name and at the same address is counted, not listed.
pub fn plan(found: &[Found], book: &[Host]) -> Preview {
    let mut taken: HashSet<String> = book.iter().map(|h| h.alias.trim().to_lowercase()).collect();
    let mut rows = Vec::new();
    let mut up_to_date = 0;
    for (index, item) in found.iter().enumerate() {
        let host = &item.host;
        let alias = host.alias.trim().to_string();
        if let Some(reason) = &item.refused {
            rows.push(Row { index, status: "refused", alias, detail: reason.clone(), checked: false });
            continue;
        }
        let named = book.iter().find(|h| h.alias.trim().eq_ignore_ascii_case(&alias));
        if named.is_some_and(|h| server(h) == server(host)) {
            up_to_date += 1;
            continue;
        }
        let row = if let Some(other) = named.or_else(|| taken.contains(&alias.to_lowercase()).then_some(host)) {
            // The name is taken by another server, in the book or earlier in the same source.
            let fresh = free_alias(&alias, &taken);
            let detail = if named.is_some() {
                format!("{alias} is {} in the book · this one is {}", address(other), address(host))
            } else {
                format!("{alias} comes twice in the file · this one is {}", address(host))
            };
            Row { index, status: "renamed", alias: fresh, detail, checked: true }
        } else if let Some(same) = book.iter().find(|h| server(h) == server(host)) {
            Row { index, status: "same", alias: alias.clone(), detail: format!("Already in the book as “{}”.", same.alias.trim()), checked: false }
        } else {
            Row { index, status: "added", alias: alias.clone(), detail: summary(item), checked: true }
        };
        taken.insert(row.alias.to_lowercase());
        rows.push(row);
    }
    // What the rows that can be checked carry: a host already in the book is not one of them.
    let listed: Vec<&Found> = rows.iter().filter(|r| r.status != "refused").map(|r| &found[r.index]).collect();
    let passwords = listed.iter().filter(|f| f.password.is_some()).count();
    let keys = listed.iter().filter(|f| matches!(f.key, Some(KeyPlan::Convert { .. }))).count();
    Preview { rows, up_to_date, passwords, keys }
}

/// The import read, waiting for the user to choose: the passwords it holds stay here, never in the
/// interface.
#[derive(Default)]
pub struct Pending(Mutex<Option<(Vec<Found>, Vec<Row>)>>);

/// What an import added.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub added: usize,
    /// The passwords that the keyring refused to keep.
    pub passwords_refused: usize,
    /// The keys converted into `~/.ssh`, and those that could not be, whose hosts came without them.
    pub keys_converted: usize,
    pub keys_failed: Vec<String>,
}

/// Adds the hosts of the rows `chosen`, under the names of their rows; with `remember`, the
/// passwords they carry go into the system keyring.
pub fn apply(store: &Store, found: &[Found], rows: &[Row], chosen: &[usize], remember: bool) -> Result<Imported, String> {
    let mut done = Imported { added: 0, passwords_refused: 0, keys_converted: 0, keys_failed: Vec::new() };
    let ssh_dir = config::home().map(|home| home.join(".ssh"));
    for row in rows.iter().filter(|r| r.status != "refused" && chosen.contains(&r.index)) {
        let Some(item) = found.get(row.index) else { continue };
        let mut host = Host { id: String::new(), alias: row.alias.clone(), ..item.host.clone() };
        // A key OpenSSH cannot read as it is: converted, or the host comes without it.
        let converted = match (&item.key, &ssh_dir) {
            (Some(KeyPlan::Convert { source, .. }), Some(dir)) => match super::keyfiles::import_in(dir, source) {
                Ok(key) => {
                    done.keys_converted += 1;
                    Some(key.path)
                }
                Err(e) => {
                    done.keys_failed.push(format!("{}: {e}", host.alias));
                    None
                }
            },
            _ => None,
        };
        if item.key.is_some() {
            match converted {
                Some(path) => (host.auth, host.key_path) = ("keyFile".into(), path),
                None => (host.auth, host.key_path) = ("auto".into(), String::new()),
            }
        }
        let saved = store.save(host)?;
        done.added += 1;
        if let (true, Some(password)) = (remember, &item.password) {
            if super::secrets::keep_now(&super::store::password_account(&saved), password).is_err() {
                done.passwords_refused += 1;
            }
        }
    }
    Ok(done)
}

/// Writes the hosts `ids`, in the order of the book, into the file `path`.
#[tauri::command]
pub fn hosts_file_export(store: State<'_, Store>, ids: Vec<String>, path: String) -> Result<usize, String> {
    let hosts: Vec<Host> = store.list()?.into_iter().filter(|h| ids.contains(&h.id)).collect();
    std::fs::write(&path, write(&hosts)).map_err(|e| format!("The file could not be written: {e}."))?;
    Ok(hosts.len())
}

/// Reads a source of hosts and says what its import would do. `source`: `ramure`, `filezilla` or `putty`.
#[tauri::command]
pub fn hosts_import_preview(store: State<'_, Store>, pending: State<'_, Pending>, source: String, path: String) -> Result<Preview, String> {
    let read_text = || {
        std::fs::read_to_string(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "This file was not found: choose it again.".to_string(),
            std::io::ErrorKind::InvalidData => "This file is not a text file.".to_string(),
            _ => format!("This file could not be read: {e}."),
        })
    };
    let found = match source.as_str() {
        "ramure" => read(&read_text()?)?,
        "filezilla" => super::filezilla::read(&read_text()?)?,
        // A folder of session files on Linux, a .reg export on Windows.
        "putty" if Path::new(&path).is_dir() => super::putty::read_dir(Path::new(&path))?,
        "putty" => super::putty::read_reg(&std::fs::read(&path).map_err(|e| format!("This file could not be read: {e}."))?)?,
        _ => return Err(format!("Unknown source: {source}.")),
    };
    let preview = plan(&found, &store.list()?);
    if let Ok(mut held) = pending.0.lock() {
        *held = Some((found, preview.rows.clone()));
    }
    Ok(preview)
}

/// Adds the hosts of the rows `chosen` of the last preview. Off the main thread: the keyring may
/// take a moment to answer.
#[tauri::command]
pub async fn hosts_import_apply(store: State<'_, Store>, pending: State<'_, Pending>, chosen: Vec<usize>, remember_passwords: bool) -> Result<Imported, String> {
    let held = pending.0.lock().map_err(|_| "The import was lost: choose the file again.".to_string())?.take();
    let (found, rows) = held.ok_or("The import was lost: choose the file again.")?;
    apply(&store, &found, &rows, &chosen, remember_passwords)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(alias: &str, user: &str, hostname: &str, port: u16) -> Host {
        Host { alias: alias.into(), user: user.into(), hostname: hostname.into(), port, auth: "auto".into(), ..Host::default() }
    }

    fn found(host: Host) -> Found {
        Found { host, refused: None, password: None, key: None }
    }

    #[test]
    fn a_file_holds_the_hosts_without_their_id_nor_any_secret() {
        let mut web = host("web", "deploy", "10.0.0.9", 2222);
        web.id = "abc".into();
        web.updated_at = 42;
        web.group = "Production".into();
        web.tags = vec!["nginx".into()];
        let text = write(&[web.clone()]);
        assert!(text.contains("\"format\": \"ramure-hosts\""));
        assert!(!text.contains("abc") && !text.contains("updatedAt"));
        let back = read(&text).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].host, Host { id: String::new(), updated_at: 0, ..web });
        assert!(back[0].password.is_none());
    }

    #[test]
    fn a_file_of_another_kind_is_refused() {
        assert_eq!(read("{\"hosts\": []}").unwrap_err(), "This is not a file of hosts exported by Ramure.");
        assert_eq!(read("<xml/>").unwrap_err(), "This is not a file of hosts exported by Ramure.");
        let nameless = read("{\"format\":\"ramure-hosts\",\"version\":1,\"hosts\":[{\"hostname\":\"x\"}]}").unwrap();
        assert_eq!(nameless[0].refused.as_deref(), Some("It has no name or no address."));
    }

    #[test]
    fn the_preview_adds_renames_points_out_the_same_server_and_counts_the_rest() {
        let book = vec![host("web", "deploy", "10.0.0.9", 22), host("staging", "root", "10.0.0.5", 2222)];
        let source = vec![
            found(host("web", "deploy", "10.0.0.9", 22)),     // the same: counted
            found(host("Web", "root", "10.0.0.1", 22)),       // the name of another server
            found(host("stage", "root", "10.0.0.5", 2222)),   // a server already in the book
            found(host("db", "postgres", "10.0.0.7", 5432)),  // new
            found(host("db", "postgres", "10.0.0.8", 22)),    // twice in the file
            Found { refused: Some("FTP is not supported.".into()), ..found(host("ftp", "", "ftp.example", 21)) },
        ];
        let preview = plan(&source, &book);
        assert_eq!(preview.up_to_date, 1);
        let said: Vec<_> = preview.rows.iter().map(|r| (r.status, r.alias.as_str(), r.checked)).collect();
        assert_eq!(
            said,
            vec![("renamed", "Web (2)", true), ("same", "stage", false), ("added", "db", true), ("renamed", "db (2)", true), ("refused", "ftp", false)]
        );
        assert_eq!(preview.rows[0].detail, "Web is deploy@10.0.0.9 in the book · this one is root@10.0.0.1");
        assert_eq!(preview.rows[1].detail, "Already in the book as “staging”.");
        assert_eq!(preview.rows[2].detail, "postgres@10.0.0.7 · port 5432 · SSH agent and default keys");
        assert_eq!(preview.rows[3].detail, "db comes twice in the file · this one is postgres@10.0.0.8");
    }

    #[test]
    fn an_import_adds_the_rows_chosen_under_their_names() {
        let store = Store::in_memory().unwrap();
        store.save(host("web", "deploy", "10.0.0.9", 22)).unwrap();
        let source = vec![found(host("web", "root", "10.0.0.1", 22)), found(host("db", "postgres", "10.0.0.7", 22)), found(host("cache", "", "10.0.0.3", 22))];
        let preview = plan(&source, &store.list().unwrap());
        let done = apply(&store, &source, &preview.rows, &[0, 1], false).unwrap();
        assert_eq!(done.added, 2);
        let mut aliases: Vec<_> = store.list().unwrap().into_iter().map(|h| h.alias).collect();
        aliases.sort();
        assert_eq!(aliases, vec!["db", "web", "web (2)"]);
    }

    #[test]
    fn a_putty_key_is_converted_unless_protected_missing_or_its_name_taken() {
        let dir = std::env::temp_dir().join(format!("ramure-keys-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let open = dir.join("id_open_test_key.ppk");
        std::fs::write(&open, "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\n").unwrap();
        let locked = dir.join("id_locked.ppk");
        std::fs::write(&locked, "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: aes256-cbc\n").unwrap();
        let plain = dir.join("id_plain");
        std::fs::write(&plain, "-----BEGIN OPENSSH PRIVATE KEY-----\n").unwrap();

        let path = |p: &std::path::Path| p.to_string_lossy().to_string();
        assert_eq!(plan_key(&path(&open)), Some(KeyPlan::Convert { source: open.clone(), into: "~/.ssh/id_open_test_key".into() }));
        assert_eq!(plan_key(&path(&locked)), Some(KeyPlan::Protected { name: "id_locked.ppk".into() }));
        assert_eq!(plan_key(&path(&plain)), None);
        assert_eq!(plan_key("/nowhere/id.ppk"), Some(KeyPlan::Missing { path: "/nowhere/id.ppk".into() }));
        assert_eq!(plan_key(""), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_host_whose_key_cannot_come_is_added_without_it() {
        let store = Store::in_memory().unwrap();
        let mut web = host("web", "deploy", "10.0.0.9", 22);
        web.auth = "keyFile".into();
        web.key_path = "/nowhere/id.ppk".into();
        let source = vec![Found { key: plan_key("/nowhere/id.ppk"), ..found(web) }];
        let preview = plan(&source, &[]);
        assert_eq!(preview.rows[0].detail, "deploy@10.0.0.9 · without its key: /nowhere/id.ppk is not on this machine; SSH agent and default keys");
        assert_eq!(preview.keys, 0);
        apply(&store, &source, &preview.rows, &[0], false).unwrap();
        let saved = store.list().unwrap().remove(0);
        assert_eq!((saved.auth.as_str(), saved.key_path.as_str()), ("auto", ""));
    }

    #[test]
    fn a_password_goes_into_the_keyring_only_when_asked() {
        let store = Store::in_memory().unwrap();
        let mut sftp = host("files", "alice", "files.example", 22);
        sftp.auth = "password".into();
        let source = vec![Found { password: Some("s3cret".into()), ..found(sftp) }];
        let preview = plan(&source, &[]);
        assert_eq!(preview.passwords, 1);
        // Already in the book: no row, and no password to remember.
        let mut there = host("files", "alice", "files.example", 22);
        there.auth = "password".into();
        assert_eq!(plan(&source, &[there]).passwords, 0);
        assert_eq!(preview.rows[0].detail, "alice@files.example · password found in the file");
        apply(&store, &source, &preview.rows, &[0], false).unwrap();
        let saved = store.list().unwrap().remove(0);
        let account = |h: &Host| super::super::store::password_account(h);
        assert_eq!(super::super::secrets::held_now(&account(&saved)), None);
        let other = Store::in_memory().unwrap();
        apply(&other, &source, &preview.rows, &[0], true).unwrap();
        let saved = other.list().unwrap().remove(0);
        assert_eq!(super::super::secrets::held_now(&account(&saved)).as_deref(), Some("s3cret"));
    }
}
