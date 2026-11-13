//! Secrets kept in the system keyring: the Credential Manager on Windows, the Secret Service
//! (GNOME Keyring, KWallet) on Linux.
//!
//! Two kinds of secrets, each stored under what it belongs to:
//! - the password of an account on a server, under `user@server:port`;
//! - the passphrase of a key, under the path of its file.
//!
//! A secret is written nowhere else: not in the host book, not in a log, not in a message. It never
//! goes back through the interface: the core reads it when it authenticates. Calls to the keyring
//! block (it may ask to be unlocked): the async functions here move them off the executor.

use std::path::Path;

use serde::Deserialize;

use super::config;

/// The name under which the keyring stores the secrets of the application.
#[cfg_attr(test, allow(dead_code))]
const SERVICE: &str = "Ramure";

/// A secret, named by what it belongs to.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Secret {
    Password { user: String, host: String, port: u16 },
    Passphrase { path: String },
}

impl Secret {
    fn account(&self) -> String {
        match self {
            Secret::Password { user, host, port } => password_account(user, host, *port),
            Secret::Passphrase { path } => passphrase_account(&config::expand(path)),
        }
    }
}

/// The name of the password of `user` on `host:port`. It follows the account, not the entry of the
/// host book: changing the address of a host does not send its old password to another server.
pub fn password_account(user: &str, host: &str, port: u16) -> String {
    format!("ssh-password:{user}@{host}:{port}")
}

/// The name of the passphrase of the key `path`: it is good for every host that uses the key.
/// The path is brought to a single spelling: `~/.ssh/id` expanded and the same path built piece by
/// piece name the same key, even on Windows, where separators and case vary.
pub fn passphrase_account(path: &Path) -> String {
    let normal: std::path::PathBuf = path.components().collect();
    let shown = normal.display().to_string();
    format!("ssh-passphrase:{}", if cfg!(windows) { shown.to_lowercase() } else { shown })
}

#[cfg(not(test))]
mod backend {
    use keyring::{Entry, Error};

    use super::SERVICE;

    /// A message with nothing secret in it: what the keyring says of its refusal.
    fn told(e: Error) -> String {
        e.to_string()
    }

    pub fn available() -> bool {
        // Creating the entry is not enough: on Linux, the bus can answer with no keyring behind it.
        // So an entry that does not exist is read.
        match Entry::new(SERVICE, "ramure-probe").map(|entry| entry.get_password()) {
            Ok(Ok(_)) | Ok(Err(Error::NoEntry)) => true,
            Ok(Err(_)) | Err(_) => false,
        }
    }

    pub fn get(account: &str) -> Result<Option<String>, String> {
        match Entry::new(SERVICE, account).map_err(told)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(told(e)),
        }
    }

    pub fn set(account: &str, secret: &str) -> Result<(), String> {
        Entry::new(SERVICE, account).map_err(told)?.set_password(secret).map_err(told)
    }

    pub fn forget(account: &str) -> Result<(), String> {
        match Entry::new(SERVICE, account).map_err(told)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(e) => Err(told(e)),
        }
    }
}

/// In tests, the keyring is a table in memory: they do not touch the keyring of the machine.
#[cfg(test)]
mod backend {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    static HELD: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

    pub fn available() -> bool {
        true
    }

    pub fn get(account: &str) -> Result<Option<String>, String> {
        Ok(HELD.lock().unwrap().get(account).cloned())
    }

    pub fn set(account: &str, secret: &str) -> Result<(), String> {
        HELD.lock().unwrap().insert(account.to_string(), secret.to_string());
        Ok(())
    }

    pub fn forget(account: &str) -> Result<(), String> {
        HELD.lock().unwrap().remove(account);
        Ok(())
    }
}

/// Moves a blocking call off the executor; `fallback` if it could not run.
async fn off_thread<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static, fallback: T) -> T {
    tauri::async_runtime::spawn_blocking(call).await.unwrap_or(fallback)
}

/// The secret kept under `account`, if there is one. A keyring that does not respond has none.
pub async fn held(account: String) -> Option<String> {
    off_thread(move || backend::get(&account).ok().flatten(), None).await
}

/// Keeps `secret` under `account`. The error says why the keyring refused, without the secret.
pub async fn keep(account: String, secret: String) -> Result<(), String> {
    off_thread(move || backend::set(&account, &secret), Err("the keyring did not respond".into())).await
}

/// Forgets the secret kept under `account`. Having nothing to forget is not an error.
pub async fn forget(account: String) -> Result<(), String> {
    off_thread(move || backend::forget(&account), Err("the keyring did not respond".into())).await
}

/// Forgets without waiting or saying anything: for a cleanup that must not delay what caused it.
pub fn forget_quietly(account: String) {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = backend::forget(&account);
    });
}

/// Does the system keyring respond? Without it, the interface does not offer to keep a secret.
#[tauri::command]
pub async fn secrets_available() -> bool {
    off_thread(backend::available, false).await
}

/// For each secret of `secrets`, in order: true if it is kept. Its value does not leave this module.
#[tauri::command]
pub async fn secrets_held(secrets: Vec<Secret>) -> Vec<bool> {
    let accounts: Vec<String> = secrets.iter().map(Secret::account).collect();
    let none = vec![false; accounts.len()];
    off_thread(move || accounts.iter().map(|account| matches!(backend::get(account), Ok(Some(_)))).collect(), none).await
}

/// Forgets a kept secret.
#[tauri::command]
pub async fn secret_forget(secret: Secret) -> Result<(), String> {
    forget(secret.account()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_belongs_to_an_account_on_a_server() {
        assert_eq!(password_account("deploy", "server.example", 22), "ssh-password:deploy@server.example:22");
        // The same user on another port is another account.
        assert_ne!(password_account("deploy", "server.example", 22), password_account("deploy", "server.example", 2222));
    }

    #[test]
    fn a_passphrase_belongs_to_a_key_file() {
        let secret = Secret::Passphrase { path: "/home/camille/.ssh/id_ed25519".into() };
        assert_eq!(secret.account(), "ssh-passphrase:/home/camille/.ssh/id_ed25519");
    }

    #[cfg(unix)]
    #[test]
    fn two_writings_of_a_path_name_the_same_key() {
        let plain = passphrase_account(Path::new("/home/camille/.ssh/id_ed25519"));
        assert_eq!(passphrase_account(Path::new("/home/camille/.ssh//id_ed25519")), plain);
        assert_eq!(passphrase_account(Path::new("/home/camille/./.ssh/id_ed25519")), plain);
    }

    #[tokio::test]
    async fn a_secret_is_kept_found_and_forgotten() {
        let account = password_account("camille", "test.example", 22);
        assert_eq!(held(account.clone()).await, None);
        keep(account.clone(), "s3cret".into()).await.unwrap();
        assert_eq!(held(account.clone()).await.as_deref(), Some("s3cret"));
        forget(account.clone()).await.unwrap();
        assert_eq!(held(account.clone()).await, None);
        // Forgetting what is not kept is not an error.
        forget(account).await.unwrap();
    }
}
