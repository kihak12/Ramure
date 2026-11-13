//! SSH keys of `~/.ssh`: list them, generate one, import one, give the public key to copy. The
//! keys stay in `~/.ssh`, which is the reference; Ramure keeps no copy of them.

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
    /// Path of the private key, as it is written in a host: `~/.ssh/id_ed25519`.
    pub path: String,
    /// Full line of the public key, to paste on a server or a Git host.
    pub public: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub comment: String,
    /// The private key is protected by a passphrase.
    pub encrypted: bool,
}

fn ssh_dir() -> Result<PathBuf, String> {
    config::home().map(|home| home.join(".ssh")).ok_or_else(|| {
        "Home folder not found: neither USERPROFILE (Windows) nor HOME (Linux) is set. Set one, then restart Ramure."
            .to_string()
    })
}

fn describe(dir: &Path, name: &str) -> Option<KeyInfo> {
    let private = dir.join(name);
    let public = PublicKey::read_openssh_file(dir.join(format!("{name}.pub"))).ok()?;
    if !private.is_file() {
        return None;
    }
    // A PEM key says in its header whether it is protected. A key in a format that cannot be read
    // is given as not encrypted.
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

/// Keys of `dir`: the files that have their `.pub` beside them.
pub fn list_in(dir: &Path) -> Vec<KeyInfo> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().and_then(|n| n.strip_suffix(".pub")).map(str::to_string))
        .collect();
    names.sort();
    names.iter().filter_map(|name| describe(dir, name)).collect()
}

/// Generates an ed25519 key in `dir`. Refuses to overwrite an existing file.
pub fn generate_in(dir: &Path, name: &str, passphrase: &str, comment: &str) -> Result<KeyInfo, String> {
    let name = name.trim();
    if !plain_name(name) {
        return Err("Use only letters, digits, “_”, “-” and “.” in the name of the key.".into());
    }
    let private = dir.join(name);
    let public = dir.join(format!("{name}.pub"));
    if private.exists() || public.exists() {
        return Err(format!("A key named “{name}” already exists in ~/.ssh: choose another name."));
    }
    let failed = |e: &dyn std::fmt::Display| format!("The key could not be created: {e}");

    fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // OpenSSH refuses a ~/.ssh folder that others can read.
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }

    let mut rng = rand::rng();
    let mut key = PrivateKey::random(&mut rng, Algorithm::Ed25519).map_err(|e| failed(&e))?;
    key.set_comment(comment.trim());
    let stored = if passphrase.is_empty() { key } else { key.encrypt(&mut rng, passphrase).map_err(|e| failed(&e))? };
    // The library creates the private file readable and writable by its owner only.
    stored.write_openssh_file(&private, LineEnding::LF).map_err(|e| failed(&e))?;
    if let Err(e) = stored.public_key().write_openssh_file(&public) {
        let _ = fs::remove_file(&private);
        return Err(failed(&e));
    }
    describe(dir, name)
        .ok_or_else(|| "The key was created but could not be read back: close the dialog and open it again.".to_string())
}

/// A key file name without surprises: no path, no space, no hidden file.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// A private key read from a file, ready to be copied into `~/.ssh`.
struct Imported {
    /// The content to write: the file as it is, or converted to the OpenSSH format (PuTTY key).
    private: Vec<u8>,
    /// The public key, when the file carries it or it can be derived from it.
    public: Option<PublicKey>,
}

/// Reads a private key: OpenSSH, PEM, or PuTTY without a passphrase (converted). Says why a file
/// is not one, and what to do.
fn read_private(text: &str) -> Result<Imported, String> {
    let trimmed = text.trim_start();
    if PublicKey::from_openssh(text.trim()).is_ok() {
        return Err("This is a public key: choose the file without “.pub”.".into());
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
            Err(_) => Err("This PuTTY key is protected by a passphrase: export it to the OpenSSH format in \
                 PuTTYgen (Conversions, Export OpenSSH key), then import the exported file."
                .into()),
        };
    }
    if trimmed.starts_with("-----BEGIN ") {
        // Protected: it opens, and its public key can be read, only with the passphrase.
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

/// A PEM key protected by a passphrase: encrypted PKCS#8, or the old PKCS#5 format.
fn pem_encrypted(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line == "-----BEGIN ENCRYPTED PRIVATE KEY-----" || line == "Proc-Type: 4,ENCRYPTED"
    })
}

fn unreadable() -> String {
    "This file is not a private key that Ramure can read (OpenSSH, PEM or PuTTY).".into()
}

/// Copies the private key `source` into `dir`, under the same name, with its public key: the
/// `.pub` beside it, or the one the key carries. Never overwrites anything.
pub fn import_in(dir: &Path, source: &Path) -> Result<KeyInfo, String> {
    let file_name = source.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    // A converted PuTTY key loses its extension: it is no longer a .ppk file.
    let name = file_name.strip_suffix(".ppk").unwrap_or(file_name);
    if !plain_name(name) {
        return Err("Rename the file before importing it: the name of a key can only contain letters, digits, \
             “_”, “-” and “.”, with no spaces."
            .into());
    }
    let size = fs::metadata(source).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "This file was not found: choose it again.".to_string(),
        std::io::ErrorKind::PermissionDenied => {
            "Access denied: your account cannot read this file. Copy it to your home folder first.".into()
        }
        _ => format!("{}: {e}", source.display()),
    })?;
    let size = size.len();
    if size > 64 * 1024 {
        return Err(unreadable());
    }
    let text = fs::read_to_string(source).map_err(|_| unreadable())?;
    let imported = read_private(&text)?;

    // The `.pub` beside it, if there is one, must be that of this key.
    let beside = PublicKey::read_openssh_file(source.with_file_name(format!("{file_name}.pub")))
        .ok()
        .or_else(|| PublicKey::read_openssh_file(source.with_file_name(format!("{name}.pub"))).ok());
    let public = match (beside, imported.public) {
        (Some(file), Some(carried)) if file.key_data() != carried.key_data() => {
            return Err("The .pub file next to this key is not its public key: remove it, then try again.".into());
        }
        (Some(file), _) => file,
        (None, Some(carried)) => carried,
        (None, None) => {
            return Err(format!(
                "This key is protected and its public key is not next to it: put it in the same folder, \
                 named “{name}.pub”, then try again."
            ));
        }
    };

    let private = dir.join(name);
    let public_path = dir.join(format!("{name}.pub"));
    if source.parent().is_some_and(|parent| parent == dir) && private == source {
        return Err("This key is already in ~/.ssh.".into());
    }
    if private.exists() || public_path.exists() {
        let same = fs::read(&private).is_ok_and(|bytes| bytes == imported.private);
        return Err(if same {
            format!("This key is already in ~/.ssh, named “{name}”.")
        } else {
            format!("A file named “{name}” already exists in ~/.ssh: rename the key before importing it.")
        });
    }

    let failed = |e: &dyn std::fmt::Display| format!("The key could not be copied: {e}");
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
    describe(dir, name)
        .ok_or_else(|| "The key was copied but could not be read back: close the dialog and open it again.".to_string())
}

/// Creates a file that does not exist yet: a private key readable and writable by its owner only,
/// as OpenSSH requires.
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
    // Encrypting a key with a passphrase takes a moment: off the thread of the interface.
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

        let plain = generate_in(&dir, "id_test", "", "me@workshop").unwrap();
        assert_eq!(plain.algorithm, "ssh-ed25519");
        assert!(plain.public.starts_with("ssh-ed25519 ") && plain.public.ends_with("me@workshop"));
        assert!(plain.fingerprint.starts_with("SHA256:"));
        assert!(!plain.encrypted);
        assert_eq!(plain.path, "~/.ssh/id_test");

        let locked = generate_in(&dir, "id_secret", "secret passphrase", "").unwrap();
        assert!(locked.encrypted);
        // The encrypted key opens with its passphrase, and not without it.
        assert!(russh::keys::load_secret_key(dir.join("id_secret"), Some("secret passphrase")).is_ok());
        assert!(russh::keys::load_secret_key(dir.join("id_secret"), None).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join("id_test")).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        assert!(generate_in(&dir, "id_test", "", "").unwrap_err().contains("already exists"));
        assert!(generate_in(&dir, "../elsewhere", "", "").is_err());
        assert!(generate_in(&dir, "", "", "").is_err());

        // A `.pub` on its own is not a key.
        fs::write(dir.join("orphan.pub"), &plain.public).unwrap();
        let names: Vec<_> = list_in(&dir).into_iter().map(|k| k.name).collect();
        assert_eq!(names, vec!["id_secret", "id_test"]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Two folders: the source, elsewhere, and an empty `~/.ssh`.
    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("ramure-import-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (source, ssh) = (root.join("usb-drive"), root.join(".ssh"));
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
    fn imports_an_openssh_key_with_its_public_key() {
        let (source, ssh) = dirs("openssh");
        let made = generate_in(&source, "id_portable", "", "me@laptop").unwrap();
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
        // A second time: already there, nothing is overwritten.
        assert!(import_in(&ssh, &source.join("id_portable")).unwrap_err().contains("already in ~/.ssh"));
        // Another key with the same name: refused, with what to do.
        let (other, _) = dirs("openssh-other");
        generate_in(&other, "id_portable", "", "").unwrap();
        assert!(import_in(&ssh, &other.join("id_portable")).unwrap_err().contains("rename the key"));
        assert_eq!(fs::read(ssh.join("id_portable")).unwrap(), fs::read(source.join("id_portable")).unwrap());
    }

    #[test]
    fn takes_the_public_key_from_a_protected_key_without_its_pub() {
        let (source, ssh) = dirs("no-pub");
        let made = generate_in(&source, "id_alone", "passphrase", "").unwrap();
        fs::remove_file(source.join("id_alone.pub")).unwrap();
        let key = import_in(&ssh, &source.join("id_alone")).unwrap();
        assert_eq!(key.fingerprint, made.fingerprint);
        assert!(key.encrypted);
    }

    #[test]
    fn imports_a_pem_key() {
        let (source, ssh) = dirs("pem");
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();

        // In the clear: the public key is derived from it, the file is copied as it is.
        fs::write(source.join("deploy.pem"), pem(&key, None)).unwrap();
        let imported = import_in(&ssh, &source.join("deploy.pem")).unwrap();
        assert_eq!(imported.fingerprint, fingerprint);
        assert!(!imported.encrypted);

        // Protected, without a .pub: its public key cannot be read, the import says what to do.
        fs::write(source.join("prod.pem"), pem(&key, Some("passphrase"))).unwrap();
        assert!(import_in(&ssh, &source.join("prod.pem")).unwrap_err().contains("prod.pem.pub"));
        // With its .pub: imported, and given as protected.
        fs::write(source.join("prod.pem.pub"), key.public_key().to_openssh().unwrap()).unwrap();
        let locked = import_in(&ssh, &source.join("prod.pem")).unwrap();
        assert_eq!(locked.fingerprint, fingerprint);
        assert!(locked.encrypted);
    }

    /// Public test vector of the ssh-key library (tests/examples/id_ed25519.ppk).
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
    fn converts_a_putty_key_to_the_openssh_format() {
        let (source, ssh) = dirs("putty");
        fs::write(source.join("server.ppk"), PPK).unwrap();
        let key = import_in(&ssh, &source.join("server.ppk")).unwrap();
        // The name loses its extension: the file is now an OpenSSH key, which `ssh` reads.
        assert_eq!(key.name, "server");
        assert_eq!(key.comment, "user@example.com");
        let written = fs::read_to_string(ssh.join("server")).unwrap();
        assert!(written.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(russh::keys::load_secret_key(ssh.join("server"), None).is_ok());

        // Protected: refused, with the way to export it from PuTTYgen.
        let locked = PPK.replace("Encryption: none", "Encryption: aes256-cbc");
        fs::write(source.join("prod.ppk"), locked).unwrap();
        assert!(import_in(&ssh, &source.join("prod.ppk")).unwrap_err().contains("PuTTYgen"));
        // Damaged, without a passphrase: unreadable, not "protected".
        fs::write(source.join("broken.ppk"), PPK.replace("Private-MAC: 94", "Private-MAC: 00")).unwrap();
        assert!(import_in(&ssh, &source.join("broken.ppk")).unwrap_err().contains("is not a private key"));
    }

    #[test]
    fn refuses_what_is_not_a_private_key() {
        let (source, ssh) = dirs("refused");
        let made = generate_in(&source, "id_a", "", "").unwrap();
        assert!(import_in(&ssh, &source.join("id_a.pub")).unwrap_err().contains("public key"));
        fs::write(source.join("notes.txt"), "not a key").unwrap();
        assert!(import_in(&ssh, &source.join("notes.txt")).unwrap_err().contains("is not a private key"));
        assert!(import_in(&ssh, &source.join("missing")).unwrap_err().contains("not found"));
        fs::copy(source.join("id_a"), source.join("my key")).unwrap();
        assert!(import_in(&ssh, &source.join("my key")).unwrap_err().contains("Rename the file"));
        // A .pub beside it that is not its own.
        let other = generate_in(&source, "id_b", "", "").unwrap();
        fs::write(source.join("id_a.pub"), &other.public).unwrap();
        assert!(import_in(&ssh, &source.join("id_a")).unwrap_err().contains("is not its public key"));
        assert_ne!(made.fingerprint, other.fingerprint);
        // Nothing was written.
        assert!(list_in(&ssh).is_empty());
    }
}
