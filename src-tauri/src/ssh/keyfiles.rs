//! Clés SSH de `~/.ssh` : les lister, en générer une, en importer une, donner la clé publique à
//! copier. Les clés restent dans `~/.ssh`, qui est la référence ; Ramure n'en garde aucune copie.

use std::fs;
use std::path::{Path, PathBuf};

use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, HashAlg, PrivateKey, PublicKey, decode_secret_key};
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
    // Une clé PEM dit si elle est protégée dans son en-tête. Une clé dans un format illisible est
    // donnée comme non chiffrée.
    let encrypted = match PrivateKey::read_openssh_file(&private) {
        Ok(key) => key.is_encrypted(),
        Err(_) => fs::read_to_string(&private).map(|text| pem_encrypted(&text)).unwrap_or(false),
    };
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
    if !plain_name(name) {
        return Err("Le nom de la clé ne peut contenir que des lettres, des chiffres, «\u{a0}_\u{a0}», «\u{a0}-\u{a0}» et «\u{a0}.\u{a0}».".into());
    }
    let private = dir.join(name);
    let public = dir.join(format!("{name}.pub"));
    if private.exists() || public.exists() {
        return Err(format!("Une clé nommée «\u{a0}{name}\u{a0}» existe déjà dans ~/.ssh\u{a0}: choisis un autre nom."));
    }
    let failed = |e: &dyn std::fmt::Display| format!("La clé n’a pas pu être créée\u{a0}: {e}");

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
    describe(dir, name).ok_or_else(|| "La clé a été créée mais n’a pas pu être relue\u{a0}: ferme la boîte et rouvre-la.".to_string())
}

/// Un nom de fichier de clé sans surprise : ni chemin, ni espace, ni fichier caché.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Une clé privée lue dans un fichier, prête à être copiée dans `~/.ssh`.
struct Imported {
    /// Le contenu à écrire : le fichier tel quel, ou converti au format OpenSSH (clé PuTTY).
    private: Vec<u8>,
    /// La clé publique, quand le fichier la porte ou qu'elle s'en déduit.
    public: Option<PublicKey>,
}

/// Lit une clé privée : OpenSSH, PEM, ou PuTTY sans phrase secrète (convertie). Dit pourquoi un
/// fichier n'en est pas une, et quoi faire.
fn read_private(text: &str) -> Result<Imported, String> {
    let trimmed = text.trim_start();
    if PublicKey::from_openssh(text.trim()).is_ok() {
        return Err("C’est une clé publique\u{a0}: choisis le fichier sans «\u{a0}.pub\u{a0}».".into());
    }
    if trimmed.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----") {
        let key = PrivateKey::from_openssh(text).map_err(|_| unreadable())?;
        return Ok(Imported { private: text.as_bytes().to_vec(), public: Some(key.public_key().clone()) });
    }
    if trimmed.starts_with("PuTTY-User-Key-File-") {
        let protected = !text.lines().any(|line| line.trim() == "Encryption: none");
        return match decode_secret_key(text, None) {
            Ok(key) => {
                let openssh = key.to_openssh(LineEnding::LF).map_err(|e| e.to_string())?;
                Ok(Imported { private: openssh.as_bytes().to_vec(), public: Some(key.public_key().clone()) })
            }
            Err(_) if !protected => Err(unreadable()),
            Err(_) => Err("Cette clé PuTTY est protégée par une phrase secrète\u{a0}: exporte-la au format \
                 OpenSSH dans PuTTYgen (Conversions, Export OpenSSH key), puis importe le fichier obtenu."
                .into()),
        };
    }
    if trimmed.starts_with("-----BEGIN ") {
        // Protégée : elle ne s'ouvre, et sa clé publique ne se lit, qu'avec la phrase secrète.
        if pem_encrypted(text) {
            return Ok(Imported { private: text.as_bytes().to_vec(), public: None });
        }
        return match decode_secret_key(text, None) {
            Ok(key) => Ok(Imported { private: text.as_bytes().to_vec(), public: Some(key.public_key().clone()) }),
            Err(_) => Err(unreadable()),
        };
    }
    Err(unreadable())
}

/// Une clé PEM protégée par une phrase secrète : PKCS#8 chiffrée, ou l'ancien format PKCS#5.
fn pem_encrypted(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line == "-----BEGIN ENCRYPTED PRIVATE KEY-----" || line == "Proc-Type: 4,ENCRYPTED"
    })
}

fn unreadable() -> String {
    "Ce fichier n’est pas une clé privée que Ramure sait lire (OpenSSH, PEM ou PuTTY).".into()
}

/// Copie la clé privée `source` dans `dir`, sous le même nom, avec sa clé publique : le `.pub`
/// voisin, ou celle que porte la clé. N'écrase jamais rien.
pub fn import_in(dir: &Path, source: &Path) -> Result<KeyInfo, String> {
    let file_name = source.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    // Une clé PuTTY convertie perd son extension : ce n'est plus un fichier .ppk.
    let name = file_name.strip_suffix(".ppk").unwrap_or(file_name);
    if !plain_name(name) {
        return Err(format!(
            "Renomme le fichier avant de l’importer\u{a0}: le nom d’une clé ne peut contenir que des lettres, \
             des chiffres, «\u{a0}_\u{a0}», «\u{a0}-\u{a0}» et «\u{a0}.\u{a0}», sans espace."
        ));
    }
    let size = fs::metadata(source).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "Ce fichier est introuvable\u{a0}: choisis-le de nouveau.".to_string(),
        std::io::ErrorKind::PermissionDenied => {
            "Accès refusé\u{a0}: ton compte ne peut pas lire ce fichier. Copie-le d’abord dans ton dossier personnel.".into()
        }
        _ => format!("{}\u{a0}: {e}", source.display()),
    })?;
    let size = size.len();
    if size > 64 * 1024 {
        return Err(unreadable());
    }
    let text = fs::read_to_string(source).map_err(|_| unreadable())?;
    let imported = read_private(&text)?;

    // Le `.pub` voisin, s'il y en a un, doit être celui de cette clé.
    let beside = PublicKey::read_openssh_file(source.with_file_name(format!("{file_name}.pub")))
        .ok()
        .or_else(|| PublicKey::read_openssh_file(source.with_file_name(format!("{name}.pub"))).ok());
    let public = match (beside, imported.public) {
        (Some(file), Some(carried)) if file.key_data() != carried.key_data() => {
            return Err("Le fichier .pub voisin n’est pas la clé publique de cette clé\u{a0}: retire-le, puis recommence.".into());
        }
        (Some(file), _) => file,
        (None, Some(carried)) => carried,
        (None, None) => {
            return Err(format!(
                "Cette clé est protégée et sa clé publique n’est pas à côté d’elle\u{a0}: place-la dans le même \
                 dossier, sous le nom «\u{a0}{name}.pub\u{a0}», puis recommence."
            ));
        }
    };

    let private = dir.join(name);
    let public_path = dir.join(format!("{name}.pub"));
    if source.parent().is_some_and(|parent| parent == dir) && private == source {
        return Err("Cette clé est déjà dans ~/.ssh.".into());
    }
    if private.exists() || public_path.exists() {
        let same = fs::read(&private).is_ok_and(|bytes| bytes == imported.private);
        return Err(if same {
            format!("Cette clé est déjà dans ~/.ssh, sous le nom «\u{a0}{name}\u{a0}».")
        } else {
            format!("Un fichier nommé «\u{a0}{name}\u{a0}» existe déjà dans ~/.ssh\u{a0}: renomme la clé avant de l’importer.")
        });
    }

    let failed = |e: &dyn std::fmt::Display| format!("La clé n’a pas pu être copiée\u{a0}: {e}");
    fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    write_new(&private, &imported.private, true).map_err(|e| failed(&e))?;
    let line = format!("{}\n", public.to_openssh().map_err(|e| failed(&e))?);
    if let Err(e) = write_new(&public_path, line.as_bytes(), false) {
        let _ = fs::remove_file(&private);
        return Err(failed(&e));
    }
    describe(dir, name).ok_or_else(|| "La clé a été copiée mais n’a pas pu être relue\u{a0}: ferme la boîte et rouvre-la.".to_string())
}

/// Crée un fichier qui n'existe pas encore : une clé privée en lecture et écriture pour son seul
/// propriétaire, comme OpenSSH l'exige.
fn write_new(path: &Path, bytes: &[u8], secret: bool) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if secret { 0o600 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = secret;
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
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

#[tauri::command]
pub async fn ssh_key_import(path: String) -> Result<KeyInfo, String> {
    let dir = ssh_dir()?;
    tauri::async_runtime::spawn_blocking(move || import_in(&dir, Path::new(&path)))
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

    /// Deux dossiers : la source, ailleurs, et un `~/.ssh` vide.
    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("ramure-import-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (source, ssh) = (root.join("cle-usb"), root.join(".ssh"));
        fs::create_dir_all(&source).unwrap();
        (source, ssh)
    }

    fn pem(key: &PrivateKey, passphrase: Option<&str>) -> String {
        let mut out = Vec::new();
        match passphrase {
            Some(pass) => russh::keys::encode_pkcs8_pem_encrypted(key, pass.as_bytes(), 16, &mut out).unwrap(),
            None => russh::keys::encode_pkcs8_pem(key, &mut out).unwrap(),
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn importe_une_cle_openssh_avec_sa_cle_publique() {
        let (source, ssh) = dirs("openssh");
        let made = generate_in(&source, "id_portable", "", "moi@portable").unwrap();
        let key = import_in(&ssh, &source.join("id_portable")).unwrap();
        assert_eq!(key.fingerprint, made.fingerprint);
        assert_eq!(key.path, "~/.ssh/id_portable");
        assert_eq!(fs::read(ssh.join("id_portable")).unwrap(), fs::read(source.join("id_portable")).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(ssh.join("id_portable")).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(&ssh).unwrap().permissions().mode() & 0o777, 0o700);
        }
        // Une seconde fois : déjà là, rien n'est écrasé.
        assert!(import_in(&ssh, &source.join("id_portable")).unwrap_err().contains("déjà dans ~/.ssh"));
        // Une autre clé du même nom : refusée, avec quoi faire.
        let (other, _) = dirs("openssh-autre");
        generate_in(&other, "id_portable", "", "").unwrap();
        assert!(import_in(&ssh, &other.join("id_portable")).unwrap_err().contains("renomme la clé"));
        assert_eq!(fs::read(ssh.join("id_portable")).unwrap(), fs::read(source.join("id_portable")).unwrap());
    }

    #[test]
    fn tire_la_cle_publique_d_une_cle_protegee_sans_pub() {
        let (source, ssh) = dirs("sans-pub");
        let made = generate_in(&source, "id_seule", "phrase", "").unwrap();
        fs::remove_file(source.join("id_seule.pub")).unwrap();
        let key = import_in(&ssh, &source.join("id_seule")).unwrap();
        assert_eq!(key.fingerprint, made.fingerprint);
        assert!(key.encrypted);
    }

    #[test]
    fn importe_une_cle_pem() {
        let (source, ssh) = dirs("pem");
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();

        // En clair : la clé publique s'en déduit, le fichier est copié tel quel.
        fs::write(source.join("deploy.pem"), pem(&key, None)).unwrap();
        let imported = import_in(&ssh, &source.join("deploy.pem")).unwrap();
        assert_eq!(imported.fingerprint, fingerprint);
        assert!(!imported.encrypted);

        // Protégée, sans .pub : sa clé publique ne se lit pas, l'import dit quoi faire.
        fs::write(source.join("prod.pem"), pem(&key, Some("phrase"))).unwrap();
        assert!(import_in(&ssh, &source.join("prod.pem")).unwrap_err().contains("prod.pem.pub"));
        // Avec son .pub : importée, et donnée comme protégée.
        fs::write(source.join("prod.pem.pub"), key.public_key().to_openssh().unwrap()).unwrap();
        let locked = import_in(&ssh, &source.join("prod.pem")).unwrap();
        assert_eq!(locked.fingerprint, fingerprint);
        assert!(locked.encrypted);
    }

    /// Vecteur de test public de la bibliothèque ssh-key (tests/examples/id_ed25519.ppk).
    const PPK: &str = "PuTTY-User-Key-File-3: ssh-ed25519
Encryption: none
Comment: user@example.com
Public-Lines: 2
AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XF
Sqti
Private-Lines: 1
AAAAILYGwiLRDBba4WxwpNRRc0cuxhfgXGVpINJuVsCPtZHt
Private-MAC: 94140d0344fad6aa1bf7b71e9c93db11ccac8a232f8a51e11c024869d608c82d
";

    #[test]
    fn convertit_une_cle_putty_au_format_openssh() {
        let (source, ssh) = dirs("putty");
        fs::write(source.join("serveur.ppk"), PPK).unwrap();
        let key = import_in(&ssh, &source.join("serveur.ppk")).unwrap();
        // Le nom perd son extension : le fichier est maintenant une clé OpenSSH, que `ssh` lit.
        assert_eq!(key.name, "serveur");
        assert_eq!(key.comment, "user@example.com");
        let written = fs::read_to_string(ssh.join("serveur")).unwrap();
        assert!(written.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(russh::keys::load_secret_key(ssh.join("serveur"), None).is_ok());

        // Protégée : refusée, avec la façon de l'exporter depuis PuTTYgen.
        let locked = PPK.replace("Encryption: none", "Encryption: aes256-cbc");
        fs::write(source.join("prod.ppk"), locked).unwrap();
        assert!(import_in(&ssh, &source.join("prod.ppk")).unwrap_err().contains("PuTTYgen"));
        // Abîmée, sans phrase secrète : illisible, pas « protégée ».
        fs::write(source.join("casse.ppk"), PPK.replace("Private-MAC: 94", "Private-MAC: 00")).unwrap();
        assert!(import_in(&ssh, &source.join("casse.ppk")).unwrap_err().contains("n’est pas une clé privée"));
    }

    #[test]
    fn refuse_ce_qui_n_est_pas_une_cle_privee() {
        let (source, ssh) = dirs("refus");
        let made = generate_in(&source, "id_a", "", "").unwrap();
        assert!(import_in(&ssh, &source.join("id_a.pub")).unwrap_err().contains("clé publique"));
        fs::write(source.join("notes.txt"), "pas une clé").unwrap();
        assert!(import_in(&ssh, &source.join("notes.txt")).unwrap_err().contains("n’est pas une clé privée"));
        assert!(import_in(&ssh, &source.join("absente")).unwrap_err().contains("introuvable"));
        fs::copy(source.join("id_a"), source.join("ma cle")).unwrap();
        assert!(import_in(&ssh, &source.join("ma cle")).unwrap_err().contains("Renomme le fichier"));
        // Un .pub voisin qui n'est pas le sien.
        let other = generate_in(&source, "id_b", "", "").unwrap();
        fs::write(source.join("id_a.pub"), &other.public).unwrap();
        assert!(import_in(&ssh, &source.join("id_a")).unwrap_err().contains("n’est pas la clé publique"));
        assert_ne!(made.fingerprint, other.fingerprint);
        // Rien n'a été écrit.
        assert!(list_in(&ssh).is_empty());
    }
}
