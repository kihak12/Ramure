//! Secrets retenus dans le trousseau du système : le Gestionnaire d'identifiants sous Windows, le
//! Secret Service (GNOME Keyring, KWallet) sous Linux.
//!
//! Deux sortes de secrets, chacune rangée sous ce à quoi elle appartient :
//! - le mot de passe d'un compte sur un serveur, sous `utilisateur@serveur:port` ;
//! - la phrase secrète d'une clé, sous le chemin de son fichier.
//!
//! Un secret n'est écrit nulle part ailleurs : ni dans le carnet d'hôtes, ni dans un journal, ni
//! dans un message. Il ne repasse jamais par l'interface : c'est le cœur qui le lit au moment de
//! s'authentifier. Les appels au trousseau sont bloquants (il peut demander à être déverrouillé) :
//! les fonctions asynchrones d'ici les sortent de l'exécuteur.

use std::path::Path;

use serde::Deserialize;

use super::config;

/// Le nom sous lequel le trousseau range les secrets de l'application.
#[cfg_attr(test, allow(dead_code))]
const SERVICE: &str = "Ramure";

/// Un secret, désigné par ce à quoi il appartient.
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

/// Le nom du mot de passe de `user` sur `host:port`. Il suit le compte, pas la fiche du carnet :
/// changer l'adresse d'un hôte ne fait pas partir son ancien mot de passe vers un autre serveur.
pub fn password_account(user: &str, host: &str, port: u16) -> String {
    format!("ssh-password:{user}@{host}:{port}")
}

/// Le nom de la phrase secrète de la clé `path` : elle vaut pour tous les hôtes qui s'en servent.
/// Le chemin est ramené à une seule écriture : `~/.ssh/id` développé et le même chemin construit
/// pièce à pièce désignent la même clé, même sous Windows où les séparateurs et la casse varient.
pub fn passphrase_account(path: &Path) -> String {
    let normal: std::path::PathBuf = path.components().collect();
    let shown = normal.display().to_string();
    format!("ssh-passphrase:{}", if cfg!(windows) { shown.to_lowercase() } else { shown })
}

#[cfg(not(test))]
mod backend {
    use keyring::{Entry, Error};

    use super::SERVICE;

    /// Un message sans rien de secret : ce que le trousseau dit de son refus.
    fn told(e: Error) -> String {
        e.to_string()
    }

    pub fn available() -> bool {
        // L'initialisation ne suffit pas : sous Linux, le bus peut répondre sans qu'aucun trousseau
        // n'y soit. On lit donc une entrée qui n'existe pas.
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

/// Dans les tests, le trousseau est une table en mémoire : ils ne touchent pas à celui de la machine.
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

/// Sort un appel bloquant de l'exécuteur ; `fallback` s'il n'a pas pu tourner.
async fn off_thread<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static, fallback: T) -> T {
    tauri::async_runtime::spawn_blocking(call).await.unwrap_or(fallback)
}

/// Le secret retenu sous `account`, s'il y en a un. Un trousseau qui ne répond pas n'en a pas.
pub async fn held(account: String) -> Option<String> {
    off_thread(move || backend::get(&account).ok().flatten(), None).await
}

/// Retient `secret` sous `account`. L'erreur dit pourquoi le trousseau a refusé, sans le secret.
pub async fn keep(account: String, secret: String) -> Result<(), String> {
    off_thread(move || backend::set(&account, &secret), Err("le trousseau n’a pas répondu".into())).await
}

/// Oublie le secret retenu sous `account`. Ne rien avoir à oublier n'est pas une erreur.
pub async fn forget(account: String) -> Result<(), String> {
    off_thread(move || backend::forget(&account), Err("le trousseau n’a pas répondu".into())).await
}

/// Oublie sans attendre ni rien dire : pour un ménage qui ne doit pas retarder ce qui l'a causé.
pub fn forget_quietly(account: String) {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = backend::forget(&account);
    });
}

/// Le trousseau du système répond-il ? Sans lui, l'interface ne propose pas de retenir un secret.
#[tauri::command]
pub async fn secrets_available() -> bool {
    off_thread(backend::available, false).await
}

/// Pour chaque secret de `secrets`, dans l'ordre : vrai s'il est retenu. Sa valeur ne sort pas d'ici.
#[tauri::command]
pub async fn secrets_held(secrets: Vec<Secret>) -> Vec<bool> {
    let accounts: Vec<String> = secrets.iter().map(Secret::account).collect();
    let none = vec![false; accounts.len()];
    off_thread(move || accounts.iter().map(|account| matches!(backend::get(account), Ok(Some(_)))).collect(), none).await
}

/// Oublie un secret retenu.
#[tauri::command]
pub async fn secret_forget(secret: Secret) -> Result<(), String> {
    forget(secret.account()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_belongs_to_an_account_on_a_server() {
        assert_eq!(password_account("deploy", "serveur.example", 22), "ssh-password:deploy@serveur.example:22");
        // Le même utilisateur sur un autre port est un autre compte.
        assert_ne!(password_account("deploy", "serveur.example", 22), password_account("deploy", "serveur.example", 2222));
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
        let account = password_account("camille", "essai.example", 22);
        assert_eq!(held(account.clone()).await, None);
        keep(account.clone(), "s3cret".into()).await.unwrap();
        assert_eq!(held(account.clone()).await.as_deref(), Some("s3cret"));
        forget(account.clone()).await.unwrap();
        assert_eq!(held(account.clone()).await, None);
        // Oublier ce qui n'est pas retenu n'est pas une erreur.
        forget(account).await.unwrap();
    }
}
