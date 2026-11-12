//! Carnet d'hôtes, dans un fichier SQLite du dossier de configuration de Ramure.
//! Aucun secret n'y figure : ni mot de passe, ni phrase secrète.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::config::{self, HostEntry};
use super::secrets;

/// Version du schéma, tenue dans `PRAGMA user_version`.
const SCHEMA_VERSION: i32 = 1;

/// Chaque ligne porte un identifiant unique, une date de modification et un marqueur de
/// suppression : une future synchronisation entre machines s'appuiera dessus sans changer
/// le schéma. Les colonnes SFTP et journal servent aux jalons suivants.
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
    updated_at      INTEGER NOT NULL,
    deleted         INTEGER NOT NULL DEFAULT 0
);
";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Host {
    /// Vide pour un hôte qui n'est pas encore enregistré.
    pub id: String,
    pub alias: String,
    pub hostname: String,
    pub port: u16,
    pub user: String,
    /// `auto` (agent puis clés par défaut), `keyFile` ou `password`.
    pub auth: String,
    pub key_path: String,
    pub group: String,
    pub tags: Vec<String>,
    /// Indice de couleur dans la palette de l'interface, ou vide.
    pub color: String,
    pub startup_command: String,
    /// L'hôte passe par un bastion : importé, mais pas encore connectable.
    pub proxy_jump: bool,
    /// Chaque session de cet hôte enregistre son journal.
    #[serde(default)]
    pub log_sessions: bool,
    /// Dernière modification, en millisecondes Unix.
    pub updated_at: i64,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn db(e: rusqlite::Error) -> String {
    format!("carnet d'hôtes : {e}")
}

pub struct Store(Mutex<Connection>);

impl Store {
    fn init(conn: Connection) -> Result<Self, String> {
        conn.execute_batch(SCHEMA).map_err(db)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION).map_err(db)?;
        Ok(Store(Mutex::new(conn)))
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        Self::init(Connection::open(path).map_err(db)?)
    }

    /// Base en mémoire : pour les tests, et en secours si le fichier ne peut pas être ouvert.
    pub fn in_memory() -> Result<Self, String> {
        Self::init(Connection::open_in_memory().map_err(db)?)
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.0.lock().map_err(|_| "carnet d'hôtes indisponible".to_string())
    }

    /// Hôtes enregistrés, par groupe puis par alias.
    pub fn list(&self) -> Result<Vec<Host>, String> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT id, alias, hostname, port, user, auth, key_path, grp, tags, color,
                        startup_command, proxy_jump, updated_at, log_sessions
                 FROM hosts WHERE deleted = 0
                 ORDER BY grp COLLATE NOCASE, alias COLLATE NOCASE",
            )
            .map_err(db)?;
        let rows = statement
            .query_map([], |row| {
                let tags: String = row.get(8)?;
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
                })
            })
            .map_err(db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db)
    }

    /// Crée l'hôte s'il n'a pas d'identifiant, le met à jour sinon. Renvoie l'hôte enregistré.
    pub fn save(&self, mut host: Host) -> Result<Host, String> {
        host.alias = host.alias.trim().to_string();
        host.hostname = host.hostname.trim().to_string();
        host.user = host.user.trim().to_string();
        host.group = host.group.trim().to_string();
        host.tags = host.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        if host.alias.is_empty() {
            return Err("L'hôte doit avoir un nom.".into());
        }
        if host.hostname.is_empty() {
            return Err("L'hôte doit avoir une adresse.".into());
        }
        if host.port == 0 {
            host.port = 22;
        }
        if !["auto", "keyFile", "password"].contains(&host.auth.as_str()) {
            host.auth = "auto".into();
        }
        if host.id.is_empty() {
            host.id = uuid::Uuid::new_v4().to_string();
        }
        host.updated_at = now();
        self.conn()?
            .execute(
                "INSERT INTO hosts (id, alias, hostname, port, user, auth, key_path, grp, tags, color,
                                    startup_command, proxy_jump, updated_at, log_sessions, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0)
                 ON CONFLICT(id) DO UPDATE SET
                    alias = ?2, hostname = ?3, port = ?4, user = ?5, auth = ?6, key_path = ?7, grp = ?8,
                    tags = ?9, color = ?10, startup_command = ?11, proxy_jump = ?12, updated_at = ?13,
                    log_sessions = ?14, deleted = 0",
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
                ],
            )
            .map_err(db)?;
        Ok(host)
    }

    /// Marque l'hôte comme supprimé : la ligne reste, pour qu'une synchronisation le sache.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.conn()?
            .execute("UPDATE hosts SET deleted = 1, updated_at = ?2 WHERE id = ?1", params![id, now()])
            .map_err(db)?;
        Ok(())
    }

    /// Ajoute les hôtes de `~/.ssh/config` dont l'alias n'est pas déjà dans le carnet.
    /// Renvoie (ajoutés, déjà présents).
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
                group: "Importés de ~/.ssh/config".into(),
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

/// Le nom sous lequel le trousseau retient le mot de passe d'un hôte : celui de son compte sur son
/// serveur, tel qu'une connexion le forme.
fn password_account(host: &Host) -> String {
    let user = if host.user.is_empty() { super::current_user() } else { host.user.clone() };
    secrets::password_account(&user, &host.hostname, if host.port == 0 { 22 } else { host.port })
}

/// Vrai si plus aucune fiche de `hosts` ne se connecte au compte `account`.
fn orphaned(hosts: &[Host], account: &str) -> bool {
    !hosts.iter().any(|host| password_account(host) == account)
}

/// Oublie le mot de passe retenu pour `account` si plus aucune fiche ne s'en sert.
fn forget_if_orphaned(store: &Store, account: String) {
    if store.list().is_ok_and(|hosts| orphaned(&hosts, &account)) {
        secrets::forget_quietly(account);
    }
}

/// Enregistre une fiche. Si son adresse a changé, le mot de passe retenu pour l'ancienne est oublié.
#[tauri::command]
pub fn host_save(store: State<'_, Store>, host: Host) -> Result<Host, String> {
    let previous = if host.id.is_empty() { None } else { store.list()?.into_iter().find(|h| h.id == host.id) };
    let saved = store.save(host)?;
    if let Some(account) = previous.map(|h| password_account(&h)).filter(|old| *old != password_account(&saved)) {
        forget_if_orphaned(&store, account);
    }
    Ok(saved)
}

/// Supprime une fiche, et oublie le mot de passe retenu pour son compte.
#[tauri::command]
pub fn host_delete(store: State<'_, Store>, id: String) -> Result<(), String> {
    let gone = store.list()?.into_iter().find(|h| h.id == id);
    store.delete(&id)?;
    if let Some(host) = gone {
        forget_if_orphaned(&store, password_account(&host));
    }
    Ok(())
}

/// Importe les hôtes de `~/.ssh/config`.
#[tauri::command]
pub fn hosts_import(store: State<'_, Store>) -> Result<ImportReport, String> {
    let path = config::home().map(|home| home.join(".ssh").join("config")).ok_or("dossier personnel introuvable")?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{} : {e}", path.display()))?;
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

        let mut vps = store.save(Host { tags: vec![" prod ".into(), "".into(), "web".into()], ..host("vps", "Serveurs") }).unwrap();
        assert_eq!(vps.id.len(), 36);
        assert_eq!(vps.tags, vec!["prod", "web"]);
        store.save(host("atelier", "")).unwrap();

        // Sans groupe d'abord, puis par alias.
        let aliases: Vec<_> = store.list().unwrap().into_iter().map(|h| h.alias).collect();
        assert_eq!(aliases, vec!["atelier", "vps"]);

        vps.port = 2222;
        vps.auth = "n'importe quoi".into();
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
        // La ligne existe toujours, marquée supprimée.
        let rows: i64 = store.conn().unwrap().query_row("SELECT count(*) FROM hosts", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 2);

        assert!(store.save(Host { hostname: "x".into(), ..Host::default() }).is_err());
        assert!(store.save(Host { alias: "x".into(), ..Host::default() }).is_err());
    }

    #[test]
    fn a_password_is_orphaned_when_no_host_uses_its_account() {
        let mut one = host("web", "");
        one.hostname = "serveur.example".into();
        one.user = "deploy".into();
        one.port = 22;
        let mut twin = one.clone();
        twin.alias = "web (copie)".into();
        let account = password_account(&one);
        assert_eq!(account, "ssh-password:deploy@serveur.example:22");
        // Une autre fiche du même compte : le mot de passe sert encore.
        assert!(!orphaned(&[twin.clone()], &account));
        // Le même serveur sous un autre utilisateur est un autre compte.
        twin.user = "root".into();
        assert!(orphaned(&[twin], &account));
        assert!(orphaned(&[], &account));
        // Sans port, c'est le 22.
        one.port = 0;
        assert_eq!(password_account(&one), account);
    }

    #[test]
    fn import_skips_known_aliases() {
        let store = Store::in_memory().unwrap();
        store.save(host("vps", "")).unwrap();
        let entries = config::parse(
            "Host vps\n  HostName ancien.example.net\nHost interne\n  HostName 10.0.2.15\n  User deploy\n  IdentityFile ~/.ssh/id_ed25519\n  ProxyJump bastion\n",
        );
        assert_eq!(store.import(&entries).unwrap(), (1, 1));
        let hosts = store.list().unwrap();
        let imported = hosts.iter().find(|h| h.alias == "interne").unwrap();
        assert_eq!(imported.user, "deploy");
        assert_eq!((imported.auth.as_str(), imported.key_path.as_str()), ("keyFile", "~/.ssh/id_ed25519"));
        assert!(imported.proxy_jump);
        // L'hôte déjà présent n'est pas écrasé.
        assert_eq!(hosts.iter().find(|h| h.alias == "vps").unwrap().hostname, "vps.example.net");
        assert_eq!(store.import(&entries).unwrap(), (0, 2));
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
