//! Clés SSH de `~/.ssh` : les lister, en générer une, donner la clé publique à copier.
//! Les clés restent dans `~/.ssh`, qui est la référence ; Ramure n'en garde aucune copie.

use std::fs;
use std::path::{Path, PathBuf};

use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, HashAlg, PrivateKey, PublicKey};
use serde::Serialize;

use super::config;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub name: String,
    /// Chemin de la clé privée, tel qu'on l'écrit dans un hôte : `~/.ssh/id_ed25519`.
    pub path: String,
    /// Ligne complète de la clé publique, à coller sur un serveur ou une forge.
    pub public: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub comment: String,
    /// La clé privée est protégée par une phrase secrète.
    pub encrypted: bool,
}

fn ssh_dir() -> Result<PathBuf, String> {
    config::home().map(|home| home.join(".ssh")).ok_or_else(|| "dossier personnel introuvable".to_string())
}

fn describe(dir: &Path, name: &str) -> Option<KeyInfo> {
    let private = dir.join(name);
    let public = PublicKey::read_openssh_file(dir.join(format!("{name}.pub"))).ok()?;
    if !private.is_file() {
        return None;
    }
    // Une clé dans un format que la bibliothèque ne lit pas est simplement donnée comme non chiffrée.
    let encrypted = PrivateKey::read_openssh_file(&private).map(|key| key.is_encrypted()).unwrap_or(false);
    Some(KeyInfo {
        name: name.to_string(),
        path: format!("~/.ssh/{name}"),
        public: public.to_openssh().ok()?,
        algorithm: public.algorithm().as_str().to_string(),
        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
        comment: public.comment().to_string(),
        encrypted,
    })
}

/// Clés de `dir` : les fichiers accompagnés de leur `.pub`.
pub fn list_in(dir: &Path) -> Vec<KeyInfo> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().and_then(|n| n.strip_suffix(".pub")).map(str::to_string))
        .collect();
    names.sort();
    names.iter().filter_map(|name| describe(dir, name)).collect()
}

/// Génère une clé ed25519 dans `dir`. Refuse d'écraser un fichier existant.
pub fn generate_in(dir: &Path, name: &str, passphrase: &str, comment: &str) -> Result<KeyInfo, String> {
    let name = name.trim();
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !plain {
        return Err("Le nom de la clé ne peut contenir que des lettres, des chiffres, « _ », « - » et « . ».".into());
    }
    let private = dir.join(name);
    let public = dir.join(format!("{name}.pub"));
    if private.exists() || public.exists() {
        return Err(format!("Une clé nommée « {name} » existe déjà dans ~/.ssh."));
    }
    let failed = |e: &dyn std::fmt::Display| format!("la clé n'a pas pu être créée : {e}");

    fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // OpenSSH refuse un dossier ~/.ssh lisible par d'autres.
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }

    let mut rng = rand::rng();
    let mut key = PrivateKey::random(&mut rng, Algorithm::Ed25519).map_err(|e| failed(&e))?;
    key.set_comment(comment.trim());
    let stored = if passphrase.is_empty() { key } else { key.encrypt(&mut rng, passphrase).map_err(|e| failed(&e))? };
    // La bibliothèque crée le fichier privé en lecture et écriture pour son seul propriétaire.
    stored.write_openssh_file(&private, LineEnding::LF).map_err(|e| failed(&e))?;
    if let Err(e) = stored.public_key().write_openssh_file(&public) {
        let _ = fs::remove_file(&private);
        return Err(failed(&e));
    }
    describe(dir, name).ok_or_else(|| "la clé a été créée mais n'a pas pu être relue".to_string())
}

#[tauri::command]
pub fn ssh_keys_list() -> Result<Vec<KeyInfo>, String> {
    Ok(list_in(&ssh_dir()?))
}

#[tauri::command]
pub async fn ssh_key_generate(name: String, passphrase: String, comment: String) -> Result<KeyInfo, String> {
    let dir = ssh_dir()?;
    // Le chiffrement d'une clé par phrase secrète prend un instant : hors du fil de l'interface.
    tauri::async_runtime::spawn_blocking(move || generate_in(&dir, &name, &passphrase, &comment))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_generated_listed_and_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("ramure-keys-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        let plain = generate_in(&dir, "id_test", "", "moi@atelier").unwrap();
        assert_eq!(plain.algorithm, "ssh-ed25519");
        assert!(plain.public.starts_with("ssh-ed25519 ") && plain.public.ends_with("moi@atelier"));
        assert!(plain.fingerprint.starts_with("SHA256:"));
        assert!(!plain.encrypted);
        assert_eq!(plain.path, "~/.ssh/id_test");

        let locked = generate_in(&dir, "id_secret", "phrase secrète", "").unwrap();
        assert!(locked.encrypted);
        // La clé chiffrée s'ouvre avec sa phrase, et pas sans.
        assert!(russh::keys::load_secret_key(dir.join("id_secret"), Some("phrase secrète")).is_ok());
        assert!(russh::keys::load_secret_key(dir.join("id_secret"), None).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join("id_test")).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        assert!(generate_in(&dir, "id_test", "", "").unwrap_err().contains("existe déjà"));
        assert!(generate_in(&dir, "../ailleurs", "", "").is_err());
        assert!(generate_in(&dir, "", "", "").is_err());

        // Un `.pub` orphelin n'est pas une clé.
        fs::write(dir.join("seul.pub"), &plain.public).unwrap();
        let names: Vec<_> = list_in(&dir).into_iter().map(|k| k.name).collect();
        assert_eq!(names, vec!["id_secret", "id_test"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
