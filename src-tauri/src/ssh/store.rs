//! Host book, in a SQLite file in the configuration folder of Ramure.
//! No secret is stored in it: neither a password nor a passphrase.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::config::{self, HostEntry};
use super::secrets;

/// Version of the schema, kept in `PRAGMA user_version`. 2: the tunnels of each host.
const SCHEMA_VERSION: i32 = 2;

/// Every row carries a unique identifier, a modification date and a deletion marker: a future
/// synchronization between machines will rely on them without changing the schema. The SFTP and
/// log columns are for the next milestones. A book written by an earlier version is brought up to
/// date by `migrate`.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS hosts (
    id              TEXT PRIMARY KEY,
    alias           TEXT NOT NULL,
    hostname        TEXT NOT NULL,
    port            INTEGER NOT NULL DEFAULT 22,
    user            TEXT NOT NULL DEFAULT '',
    auth            TEXT NOT NULL DEFAULT 'auto',
    key_path        TEXT NOT NULL DEFAULT '',
    grp             TEXT NOT NULL DEFAULT '',
    tags            TEXT NOT NULL DEFAULT '',
    color           TEXT NOT NULL DEFAULT '',
    startup_command TEXT NOT NULL DEFAULT '',
    sftp_remote_dir TEXT NOT NULL DEFAULT '',
    sftp_local_dir  TEXT NOT NULL DEFAULT '',
    log_sessions    INTEGER NOT NULL DEFAULT 0,
    proxy_jump      INTEGER NOT NULL DEFAULT 0,
    tunnels         TEXT NOT NULL DEFAULT '[]',
    updated_at      INTEGER NOT NULL,
    deleted         INTEGER NOT NULL DEFAULT 0
);
";

/// The kind of a tunnel, named after the flags of `ssh`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TunnelKind {
    /// `-L`: a port of this machine reaches a host and a port as the server sees them.
    #[default]
    Local,
    /// `-R`: a port of the server reaches a host and a port as this machine sees them.
    Remote,
    /// `-D`: a port of this machine is a SOCKS5 proxy that goes out through the server.
    Socks,
}

/// A tunnel of a host, kept in the book with it (guidelines, 9.15 and 9.21).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tunnel {
    /// Made by the interface.
    pub id: String,
    /// Empty for none: the interface shows the summary instead.
    pub name: String,
    pub kind: TunnelKind,
    /// Where it listens: this machine for a local forward and a SOCKS proxy, the server for a
    /// remote forward.
    pub listen_port: u16,
    /// As seen from the other side: the server for a local forward, this machine for a remote
    /// one. Empty for a SOCKS proxy.
    pub target_host: String,
    /// 0 for a SOCKS proxy.
    pub target_port: u16,
    /// “Let other machines connect”: listen on every interface, not on localhost only.
    pub bind_any: bool,
    /// “Start with the session”.
    pub autostart: bool,
}

impl Default for Tunnel {
    fn default() -> Self {
        Tunnel {
            id: String::new(),
            name: String::new(),
            kind: TunnelKind::Local,
            listen_port: 0,
            target_host: String::new(),
            target_port: 0,
            bind_any: false,
            // Checked at first in the form.
            autostart: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Host {
    /// Empty for a host that is not saved yet.
    pub id: String,
    pub alias: String,
    pub hostname: String,
    pub port: u16,
    pub user: String,
    /// `auto` (agent, then default keys), `keyFile` or `password`.
    pub auth: String,
    pub key_path: String,
    pub group: String,
    pub tags: Vec<String>,
    /// Index of the color in the palette of the interface, or empty.
    pub color: String,
    pub startup_command: String,
    /// The host goes through a bastion: imported, but it cannot be connected to yet.
    pub proxy_jump: bool,
    /// Every session of this host records its log.
    #[serde(default)]
    pub log_sessions: bool,
    /// The tunnels that start with its sessions, unless “Start with the session” is unchecked.
    pub tunnels: Vec<Tunnel>,
    /// Last modification, in Unix milliseconds.
    pub updated_at: i64,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn db(e: rusqlite::Error) -> String {
    format!("host book: {e}")
}

/// The tunnels of a row. A value that cannot be read gives none, rather than a host book that
/// cannot be opened: only Ramure writes it.
fn read_tunnels(json: &str) -> Vec<Tunnel> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        log::warn!("host book: tunnels not read: {e}");
        Vec::new()
    })
}

/// Brings a host book written by an earlier version of Ramure up to `SCHEMA_VERSION`, keeping its
/// rows. A book written by a later version is left as it is.
fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(db)?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    // 1 to 2: the tunnels of each host. A book created just now already has the column.
    let has_tunnels: bool = conn
        .query_row("SELECT count(*) > 0 FROM pragma_table_info('hosts') WHERE name = 'tunnels'", [], |row| {
            row.get(0)
        })
        .map_err(db)?;
    let add_tunnels = if has_tunnels { "" } else { "ALTER TABLE hosts ADD COLUMN tunnels TEXT NOT NULL DEFAULT '[]';" };
    conn.execute_batch(&format!("BEGIN; {add_tunnels} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"))
        .map_err(db)
}

pub struct Store(Mutex<Connection>);

impl Store {
    fn init(conn: Connection) -> Result<Self, String> {
        conn.execute_batch(SCHEMA).map_err(db)?;
        migrate(&conn)?;
        Ok(Store(Mutex::new(conn)))
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        Self::init(Connection::open(path).map_err(db)?)
    }

    /// Database in memory: for the tests, and as a fallback if the file cannot be opened.
    pub fn in_memory() -> Result<Self, String> {
        Self::init(Connection::open_in_memory().map_err(db)?)
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.0.lock().map_err(|_| "The host book is unavailable. Restart Ramure.".to_string())
    }

    /// Saved hosts, by group, then by alias.
    pub fn list(&self) -> Result<Vec<Host>, String> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT id, alias, hostname, port, user, auth, key_path, grp, tags, color,
                        startup_command, proxy_jump, updated_at, log_sessions, tunnels
                 FROM hosts WHERE deleted = 0
                 ORDER BY grp COLLATE NOCASE, alias COLLATE NOCASE",
            )
            .map_err(db)?;
        let rows = statement
            .query_map([], |row| {
                let tags: String = row.get(8)?;
                let tunnels: String = row.get(14)?;
                Ok(Host {
                    id: row.get(0)?,
                    alias: row.get(1)?,
                    hostname: row.get(2)?,
                    port: row.get(3)?,
                    user: row.get(4)?,
                    auth: row.get(5)?,
                    key_path: row.get(6)?,
                    group: row.get(7)?,
                    tags: tags.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect(),
                    color: row.get(9)?,
                    startup_command: row.get(10)?,
                    proxy_jump: row.get(11)?,
                    updated_at: row.get(12)?,
                    log_sessions: row.get(13)?,
                    tunnels: read_tunnels(&tunnels),
                })
            })
            .map_err(db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db)
    }

    /// Creates the host if it has no identifier, updates it otherwise. Returns the saved host.
    pub fn save(&self, mut host: Host) -> Result<Host, String> {
        host.alias = host.alias.trim().to_string();
        host.hostname = host.hostname.trim().to_string();
        host.user = host.user.trim().to_string();
        host.group = host.group.trim().to_string();
        host.tags = host.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        if host.alias.is_empty() {
            return Err("Enter a name for the host.".into());
        }
        if host.hostname.is_empty() {
            return Err("Enter an address for the host.".into());
        }
        if host.port == 0 {
            host.port = 22;
        }
        if !["auto", "keyFile", "password"].contains(&host.auth.as_str()) {
            host.auth = "auto".into();
        }
        for tunnel in &mut host.tunnels {
            tunnel.name = tunnel.name.trim().to_string();
            tunnel.target_host = tunnel.target_host.trim().to_string();
        }
        if host.id.is_empty() {
            host.id = uuid::Uuid::new_v4().to_string();
        }
        host.updated_at = now();
        let tunnels = serde_json::to_string(&host.tunnels).map_err(|e| format!("host book: {e}"))?;
        self.conn()?
            .execute(
                "INSERT INTO hosts (id, alias, hostname, port, user, auth, key_path, grp, tags, color,
                                    startup_command, proxy_jump, updated_at, log_sessions, tunnels, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0)
                 ON CONFLICT(id) DO UPDATE SET
                    alias = ?2, hostname = ?3, port = ?4, user = ?5, auth = ?6, key_path = ?7, grp = ?8,
                    tags = ?9, color = ?10, startup_command = ?11, proxy_jump = ?12, updated_at = ?13,
                    log_sessions = ?14, tunnels = ?15, deleted = 0",
                params![
                    host.id,
                    host.alias,
                    host.hostname,
                    host.port,
                    host.user,
                    host.auth,
                    host.key_path,
                    host.group,
                    host.tags.join(","),
                    host.color,
                    host.startup_command,
                    host.proxy_jump,
                    host.updated_at,
                    host.log_sessions,
                    tunnels,
                ],
            )
            .map_err(db)?;
        Ok(host)
    }

    /// Marks the host as deleted: the row stays, so that a synchronization knows it.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.conn()?
            .execute("UPDATE hosts SET deleted = 1, updated_at = ?2 WHERE id = ?1", params![id, now()])
            .map_err(db)?;
        Ok(())
    }

    /// Adds the hosts of `~/.ssh/config` whose alias is not in the book yet.
    /// Returns (added, already there).
    pub fn import(&self, entries: &[HostEntry]) -> Result<(usize, usize), String> {
        let known: Vec<String> = self.list()?.into_iter().map(|h| h.alias.to_lowercase()).collect();
        let (mut added, mut skipped) = (0, 0);
        for entry in entries {
            if known.contains(&entry.alias.to_lowercase()) {
                skipped += 1;
                continue;
            }
            self.save(Host {
                alias: entry.alias.clone(),
                hostname: entry.hostname.clone(),
                port: entry.port,
                user: entry.user.clone().unwrap_or_default(),
                auth: if entry.identity_file.is_some() { "keyFile".into() } else { "auto".into() },
                key_path: entry.identity_file.clone().unwrap_or_default(),
                group: "Imported from ~/.ssh/config".into(),
                proxy_jump: entry.proxy_jump,
                ..Host::default()
            })?;
            added += 1;
        }
        Ok((added, skipped))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    added: usize,
    skipped: usize,
}

#[tauri::command]
pub fn hosts_list(store: State<'_, Store>) -> Result<Vec<Host>, String> {
    store.list()
}

/// The name under which the keyring keeps the password of a host: that of its account on its
/// server, as a connection forms it.
fn password_account(host: &Host) -> String {
    let user = if host.user.is_empty() { super::current_user() } else { host.user.clone() };
    secrets::password_account(&user, &host.hostname, if host.port == 0 { 22 } else { host.port })
}

/// True if no host of `hosts` connects to the account `account` any more.
fn orphaned(hosts: &[Host], account: &str) -> bool {
    !hosts.iter().any(|host| password_account(host) == account)
}

/// Forgets the password kept for `account` if no host uses it any more.
fn forget_if_orphaned(store: &Store, account: String) {
    if store.list().is_ok_and(|hosts| orphaned(&hosts, &account)) {
        secrets::forget_quietly(account);
    }
}

/// Saves a host. If its address has changed, the password kept for the old one is forgotten.
#[tauri::command]
pub fn host_save(store: State<'_, Store>, host: Host) -> Result<Host, String> {
    let previous = if host.id.is_empty() { None } else { store.list()?.into_iter().find(|h| h.id == host.id) };
    let saved = store.save(host)?;
    if let Some(account) = previous.map(|h| password_account(&h)).filter(|old| *old != password_account(&saved)) {
        forget_if_orphaned(&store, account);
    }
    Ok(saved)
}

/// Deletes a host, and forgets the password kept for its account.
#[tauri::command]
pub fn host_delete(store: State<'_, Store>, id: String) -> Result<(), String> {
    let gone = store.list()?.into_iter().find(|h| h.id == id);
    store.delete(&id)?;
    if let Some(host) = gone {
        forget_if_orphaned(&store, password_account(&host));
    }
    Ok(())
}

/// Imports the hosts of `~/.ssh/config`.
#[tauri::command]
pub fn hosts_import(store: State<'_, Store>) -> Result<ImportReport, String> {
    let path = config::home().map(|home| home.join(".ssh").join("config")).ok_or(
        "Home folder not found: neither USERPROFILE (Windows) nor HOME (Linux) is set. Set one, then restart Ramure.",
    )?;
    let text = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "~/.ssh/config was not found: there is no host to import.".to_string(),
        _ => format!("{}: {e}", path.display()),
    })?;
    let (added, skipped) = store.import(&config::parse(&text))?;
    Ok(ImportReport { added, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(alias: &str, group: &str) -> Host {
        Host { alias: alias.into(), hostname: format!("{alias}.example.net"), port: 22, group: group.into(), ..Host::default() }
    }

    #[test]
    fn hosts_are_saved_updated_and_soft_deleted() {
        let store = Store::in_memory().unwrap();
        assert!(store.list().unwrap().is_empty());

        let mut vps = store.save(Host { tags: vec![" prod ".into(), "".into(), "web".into()], ..host("vps", "Servers") }).unwrap();
        assert_eq!(vps.id.len(), 36);
        assert_eq!(vps.tags, vec!["prod", "web"]);
        store.save(host("workshop", "")).unwrap();

        // Without a group first, then by alias.
        let aliases: Vec<_> = store.list().unwrap().into_iter().map(|h| h.alias).collect();
        assert_eq!(aliases, vec!["workshop", "vps"]);

        vps.port = 2222;
        vps.auth = "anything".into();
        vps.startup_command = "cd /srv/docker".into();
        let saved = store.save(vps.clone()).unwrap();
        assert_eq!(saved.id, vps.id);
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!((listed[1].port, listed[1].auth.as_str()), (2222, "auto"));
        assert_eq!(listed[1].startup_command, "cd /srv/docker");
        assert_eq!(listed[1].tags, vec!["prod", "web"]);

        store.delete(&vps.id).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        // The row still exists, marked as deleted.
        let rows: i64 = store.conn().unwrap().query_row("SELECT count(*) FROM hosts", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 2);

        assert!(store.save(Host { hostname: "x".into(), ..Host::default() }).is_err());
        assert!(store.save(Host { alias: "x".into(), ..Host::default() }).is_err());
    }

    #[test]
    fn a_password_is_orphaned_when_no_host_uses_its_account() {
        let mut one = host("web", "");
        one.hostname = "server.example".into();
        one.user = "deploy".into();
        one.port = 22;
        let mut twin = one.clone();
        twin.alias = "web (copy)".into();
        let account = password_account(&one);
        assert_eq!(account, "ssh-password:deploy@server.example:22");
        // Another host of the same account: the password is still used.
        assert!(!orphaned(&[twin.clone()], &account));
        // The same server under another user is another account.
        twin.user = "root".into();
        assert!(orphaned(&[twin], &account));
        assert!(orphaned(&[], &account));
        // Without a port, it is 22.
        one.port = 0;
        assert_eq!(password_account(&one), account);
    }

    #[test]
    fn import_skips_known_aliases() {
        let store = Store::in_memory().unwrap();
        store.save(host("vps", "")).unwrap();
        let entries = config::parse(
            "Host vps\n  HostName old.example.net\nHost internal\n  HostName 10.0.2.15\n  User deploy\n  IdentityFile ~/.ssh/id_ed25519\n  ProxyJump bastion\n",
        );
        assert_eq!(store.import(&entries).unwrap(), (1, 1));
        let hosts = store.list().unwrap();
        let imported = hosts.iter().find(|h| h.alias == "internal").unwrap();
        assert_eq!(imported.user, "deploy");
        assert_eq!((imported.auth.as_str(), imported.key_path.as_str()), ("keyFile", "~/.ssh/id_ed25519"));
        assert!(imported.proxy_jump);
        // The host already there is not overwritten.
        assert_eq!(hosts.iter().find(|h| h.alias == "vps").unwrap().hostname, "vps.example.net");
        assert_eq!(store.import(&entries).unwrap(), (0, 2));
    }

    /// The table of version 1, before the tunnels.
    const SCHEMA_V1: &str = "
    CREATE TABLE hosts (
        id              TEXT PRIMARY KEY,
        alias           TEXT NOT NULL,
        hostname        TEXT NOT NULL,
        port            INTEGER NOT NULL DEFAULT 22,
        user            TEXT NOT NULL DEFAULT '',
        auth            TEXT NOT NULL DEFAULT 'auto',
        key_path        TEXT NOT NULL DEFAULT '',
        grp             TEXT NOT NULL DEFAULT '',
        tags            TEXT NOT NULL DEFAULT '',
        color           TEXT NOT NULL DEFAULT '',
        startup_command TEXT NOT NULL DEFAULT '',
        sftp_remote_dir TEXT NOT NULL DEFAULT '',
        sftp_local_dir  TEXT NOT NULL DEFAULT '',
        log_sessions    INTEGER NOT NULL DEFAULT 0,
        proxy_jump      INTEGER NOT NULL DEFAULT 0,
        updated_at      INTEGER NOT NULL,
        deleted         INTEGER NOT NULL DEFAULT 0
    );
    PRAGMA user_version = 1;
    ";

    fn version(store: &Store) -> i32 {
        store.conn().unwrap().pragma_query_value(None, "user_version", |row| row.get(0)).unwrap()
    }

    fn tunnels() -> Vec<Tunnel> {
        vec![
            Tunnel {
                id: "t1".into(),
                name: " database ".into(),
                kind: TunnelKind::Local,
                listen_port: 5433,
                target_host: " localhost ".into(),
                target_port: 5432,
                bind_any: false,
                autostart: true,
            },
            Tunnel {
                id: "t2".into(),
                kind: TunnelKind::Remote,
                listen_port: 8080,
                target_host: "localhost".into(),
                target_port: 3000,
                bind_any: true,
                autostart: false,
                ..Tunnel::default()
            },
            Tunnel { id: "t3".into(), kind: TunnelKind::Socks, listen_port: 1080, ..Tunnel::default() },
        ]
    }

    #[test]
    fn a_version_1_book_gains_the_tunnels_and_keeps_its_rows() {
        let dir = std::env::temp_dir().join(format!("ramure-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ramure.db");
        {
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch(SCHEMA_V1).unwrap();
            conn.execute(
                "INSERT INTO hosts (id, alias, hostname, port, user, tags, updated_at)
                 VALUES ('h1', 'vps', 'vps.example.net', 2222, 'deploy', 'prod,web', 1)",
                [],
            )
            .unwrap();
        }

        let store = Store::open(&file).unwrap();
        assert_eq!(version(&store), 2);
        let hosts = store.list().unwrap();
        assert_eq!(hosts.len(), 1);
        let vps = &hosts[0];
        assert_eq!((vps.id.as_str(), vps.alias.as_str(), vps.port, vps.user.as_str()), ("h1", "vps", 2222, "deploy"));
        assert_eq!(vps.tags, vec!["prod", "web"]);
        assert_eq!(vps.tunnels, vec![]);

        // The migrated book takes tunnels, and opening it again changes nothing.
        store.save(Host { tunnels: tunnels(), ..vps.clone() }).unwrap();
        drop(store);
        let store = Store::open(&file).unwrap();
        assert_eq!(version(&store), 2);
        assert_eq!(store.list().unwrap()[0].tunnels.len(), 3);
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_book_is_at_the_current_version() {
        let store = Store::in_memory().unwrap();
        assert_eq!(version(&store), SCHEMA_VERSION);
        assert!(store.save(host("vps", "")).unwrap().tunnels.is_empty());
    }

    #[test]
    fn tunnels_round_trip_with_their_host() {
        let store = Store::in_memory().unwrap();
        let saved = store.save(Host { tunnels: tunnels(), ..host("vps", "") }).unwrap();
        assert_eq!((saved.tunnels[0].name.as_str(), saved.tunnels[0].target_host.as_str()), ("database", "localhost"));
        assert_eq!(store.list().unwrap(), vec![saved.clone()]);

        // An update replaces them.
        let fewer = store.save(Host { tunnels: saved.tunnels[1..2].to_vec(), ..saved.clone() }).unwrap();
        assert_eq!(store.list().unwrap()[0].tunnels, fewer.tunnels);

        // The shape the interface sends and reads.
        let json = serde_json::to_value(&saved.tunnels[1]).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "t2", "name": "", "kind": "remote", "listenPort": 8080, "targetHost": "localhost",
                "targetPort": 3000, "bindAny": true, "autostart": false,
            })
        );
        let back: Host = serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap();
        assert_eq!(back, saved);

        // Every field has a default: a host sent without tunnels has none, a tunnel without its
        // options starts with the session.
        let bare: Host = serde_json::from_str(r#"{"alias": "x", "hostname": "y"}"#).unwrap();
        assert!(bare.tunnels.is_empty());
        let socks: Tunnel = serde_json::from_str(r#"{"id": "s", "kind": "socks", "listenPort": 1080}"#).unwrap();
        assert_eq!(socks, Tunnel { id: "s".into(), kind: TunnelKind::Socks, listen_port: 1080, ..Tunnel::default() });
        assert!(socks.autostart && !socks.bind_any);

        // A value that cannot be read gives no tunnels rather than a book that cannot be opened.
        store.conn().unwrap().execute("UPDATE hosts SET tunnels = 'not json'", []).unwrap();
        assert!(store.list().unwrap()[0].tunnels.is_empty());
    }

    #[test]
    fn the_file_survives_a_reopen() {
        let dir = std::env::temp_dir().join(format!("ramure-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ramure.db");
        Store::open(&file).unwrap().save(host("vps", "")).unwrap();
        assert_eq!(Store::open(&file).unwrap().list().unwrap()[0].alias, "vps");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
