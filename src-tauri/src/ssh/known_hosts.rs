//! Le fichier `known_hosts` d'OpenSSH : reconnaître un serveur, et l'y inscrire.
//!
//! russh sait le lire, mais il abandonne tout le fichier à la première ligne qu'il ne comprend
//! pas : un octet qui n'est pas de l'UTF-8, deux espaces de suite, une clé `ssh-dss`. Le serveur
//! passait alors pour inconnu, et chaque connexion l'inscrivait une fois de plus. Ici, comme dans
//! OpenSSH, une ligne illisible est ignorée, et seulement elle.
//!
//! Les lignes à marqueur (`@cert-authority`, `@revoked`) sont ignorées, comme russh le faisait :
//! Ramure ne vérifie pas les certificats d'hôte.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use data_encoding::BASE64;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::{self, PublicKey};
use sha1::Sha1;

use super::config;

/// Ce que `known_hosts` dit d'un serveur et de sa clé.
#[derive(Debug, PartialEq)]
pub enum Known {
    /// Une ligne pour ce serveur porte cette clé.
    Trusted,
    /// Aucune ligne ne porte cette clé, mais la ligne `line` en donne une autre du même type.
    Changed { line: usize },
    /// Rien sur ce serveur pour ce type de clé, ou pas de fichier.
    Unknown,
}

/// `~/.ssh/known_hosts`, là où OpenSSH le cherche, sous Windows comme sous Linux.
fn path() -> Option<PathBuf> {
    config::home().map(|home| home.join(".ssh").join("known_hosts"))
}

pub fn check(host: &str, port: u16, key: &PublicKey) -> Known {
    match path() {
        Some(path) => check_path(&path, host, port, key),
        None => Known::Unknown,
    }
}

/// Inscrit la clé du serveur dans `known_hosts`. En cas d'échec, dit pourquoi.
pub fn learn(host: &str, port: u16, key: &PublicKey) -> Result<(), String> {
    let path = path().ok_or("Dossier personnel introuvable\u{a0}: ni USERPROFILE (Windows) ni HOME (Linux) n’est défini.")?;
    learn_path(&path, host, port, key)
}

/// Un fichier absent, ou qu'on ne peut pas ouvrir, ne connaît aucun serveur.
pub fn check_path(path: &Path, host: &str, port: u16, key: &PublicKey) -> Known {
    match fs::read(path) {
        Ok(bytes) => check_text(&decode(&bytes).0, host, port, key),
        Err(_) => Known::Unknown,
    }
}

pub fn check_text(text: &str, host: &str, port: u16, key: &PublicKey) -> Known {
    let names = names(host, port);
    let mut changed = None;
    for (index, line) in text.split('\n').enumerate() {
        let Some((recorded, patterns)) = parse_line(line) else { continue };
        if !matches(patterns, &names) {
            continue;
        }
        if recorded.key_data() == key.key_data() {
            return Known::Trusted;
        }
        if recorded.algorithm() == key.algorithm() && changed.is_none() {
            changed = Some(index + 1);
        }
    }
    match changed {
        Some(line) => Known::Changed { line },
        None => Known::Unknown,
    }
}

/// Ajoute la ligne du serveur à la fin du fichier, dans son encodage, puis relit le fichier pour
/// s'assurer qu'elle y est. Une clé déjà connue n'est pas inscrite une seconde fois.
pub fn learn_path(path: &Path, host: &str, port: u16, key: &PublicKey) -> Result<(), String> {
    if check_path(path, host, port, key) == Known::Trusted {
        return Ok(());
    }
    let failed = |e: std::io::Error| format!("{}\u{a0}: {}", path.display(), explain(&e));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}\u{a0}: {}", dir.display(), explain_dir(&e)))?;
    }
    // Sous Windows, ouvrir un dossier répond « accès refusé » : le cas est reconnu avant.
    if path.is_dir() {
        return Err(failed(ErrorKind::IsADirectory.into()));
    }
    let existing = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(failed(e)),
    };
    let (text, encoding) = decode(&existing);

    let bare = PublicKey::new(key.key_data().clone(), "");
    let openssh = bare.to_openssh().map_err(|e| e.to_string())?;
    let mut entry = String::new();
    if !text.is_empty() && !text.ends_with('\n') {
        entry.push('\n');
    }
    entry.push_str(&format!("{} {openssh}\n", written_name(host, port)));

    let mut file = OpenOptions::new().append(true).create(true).open(path).map_err(failed)?;
    file.write_all(&encoding.encode(&entry)).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    drop(file);

    if check_path(path, host, port, key) != Known::Trusted {
        let advice = "Ouvre le fichier pour vérifier sa dernière ligne.";
        return Err(format!("{}\u{a0}: la ligne écrite n’a pas été retrouvée à la relecture. {advice}", path.display()));
    }
    Ok(())
}

/// Une erreur du système, en français et avec ce qu'il faut faire, quand la cause est connue ;
/// sinon le texte du système, tel quel (en anglais sous Linux, dans la langue de Windows sinon).
fn explain(e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::PermissionDenied => {
            "accès refusé. Vérifie que le fichier n’est pas en lecture seule et qu’il appartient à ton compte.".into()
        }
        ErrorKind::ReadOnlyFilesystem => "le disque est en lecture seule.".into(),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => {
            "le disque est plein. Libère de la place, puis reconnecte-toi.".into()
        }
        ErrorKind::IsADirectory => {
            "c’est un dossier, pas un fichier. Renomme-le, puis reconnecte-toi\u{a0}: le fichier sera créé.".into()
        }
        _ => e.to_string(),
    }
}

/// Le dossier `.ssh` n'a pas pu être créé : un fichier porte souvent ce nom.
fn explain_dir(e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::AlreadyExists | ErrorKind::NotADirectory => {
            "un fichier porte ce nom, à la place du dossier. Renomme-le, puis reconnecte-toi.".into()
        }
        _ => explain(e),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

impl Encoding {
    fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Encoding::Utf8 => text.as_bytes().to_vec(),
            Encoding::Utf16Le => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            Encoding::Utf16Be => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        }
    }
}

/// Le texte du fichier. Sous Windows, PowerShell 5 écrit en UTF-16, et le Bloc-notes a pu
/// l'enregistrer en ANSI : un octet invalide devient un caractère de remplacement, sans
/// empêcher de lire les autres lignes.
fn decode(bytes: &[u8]) -> (String, Encoding) {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|pair| [pair[0], pair[1]])
            .map(|pair| if le { u16::from_le_bytes(pair) } else { u16::from_be_bytes(pair) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    let (text, encoding) = if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        (utf16(rest, true), Encoding::Utf16Le)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        (utf16(rest, false), Encoding::Utf16Be)
    } else {
        let rest = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        (String::from_utf8_lossy(rest).into_owned(), Encoding::Utf8)
    };
    // Des lignes UTF-16 ajoutées à un fichier UTF-8 se lisent une fois leurs octets nuls retirés.
    (text.replace('\0', ""), encoding)
}

/// La clé d'une ligne, et ses noms d'hôtes. `None` pour un commentaire, une ligne vide, une ligne
/// à marqueur ou une ligne illisible.
fn parse_line(line: &str) -> Option<(PublicKey, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
        return None;
    }
    let mut fields = line.split_whitespace();
    let patterns = fields.next()?;
    let _algorithm = fields.next()?;
    let key = keys::parse_public_key_base64(fields.next()?).ok()?;
    Some((key, patterns))
}

/// Les noms sous lesquels OpenSSH inscrit ce serveur : `hôte` sur le port 22, `[hôte]:port`
/// ailleurs. Sur le port 22, `[hôte]:22` vaut aussi.
fn names(host: &str, port: u16) -> Vec<String> {
    let host = host.trim().to_lowercase();
    let bracketed = format!("[{host}]:{port}");
    if port == 22 { vec![host, bracketed] } else { vec![bracketed] }
}

fn written_name(host: &str, port: u16) -> String {
    names(host, port).swap_remove(0)
}

/// Une liste de motifs séparés par des virgules : un nom, un nom haché `|1|sel|hachage`, un motif
/// avec `*` et `?`, ou une exclusion `!motif`, qui l'emporte sur tout le reste de la ligne.
fn matches(patterns: &str, names: &[String]) -> bool {
    let mut found = false;
    for pattern in patterns.split(',').filter(|pattern| !pattern.is_empty()) {
        if let Some(hashed) = pattern.strip_prefix("|1|") {
            found |= names.iter().any(|name| hash_matches(hashed, name));
            continue;
        }
        let (negated, pattern) = match pattern.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, pattern),
        };
        let pattern = pattern.to_lowercase();
        if names.iter().any(|name| wildcard(pattern.as_bytes(), name.as_bytes())) {
            if negated {
                return false;
            }
            found = true;
        }
    }
    found
}

fn hash_matches(hashed: &str, name: &str) -> bool {
    let Some((salt, hash)) = hashed.split_once('|') else { return false };
    let (Ok(salt), Ok(hash)) = (BASE64.decode(salt.as_bytes()), BASE64.decode(hash.as_bytes())) else {
        return false;
    };
    let Ok(mac) = Hmac::<Sha1>::new_from_slice(&salt) else { return false };
    mac.chain_update(name.as_bytes()).verify_slice(&hash).is_ok()
}

/// `*` : n'importe quelle suite de caractères ; `?` : un caractère.
fn wildcard(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => (0..=name.len()).any(|skip| wildcard(rest, &name[skip..])),
        Some((first, rest)) => match name.split_first() {
            Some((c, tail)) if *first == b'?' || first == c => wildcard(rest, tail),
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED25519: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIF4pvy9vQCDx1D/pPpnEE+xHMBN66+9lmfhnl+Tz+n2A";
    const OTHER: &str = "AAAAC3NzaC1lZDI1NTE5AAAAILhE/8jpGXy6yuxZSpVmBc3C7a6proh0a85HJPbiMoyi";
    const ECDSA: &str = "AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBC9qkQgTXQMOjTirxHiCezHGNghDsQNX+m7wZzHfp94VOlFWr1q37sB981Y0nVuMEDc2qNceXnOrwtampPjsImM=";
    /// `serveur.exemple`, puis `[serveur.exemple]:2222`, hachés par `ssh-keygen -H`.
    const HASHED_22: &str = "|1|XWXzXS0pdVw/54dGPCI2klpW6Eg=|uxAO7Et/xluNqVuSuHCDGPl/Tj8=";
    const HASHED_2222: &str = "|1|iuHbCiDGZ0XsaYE0QHUI729pZjE=|WJ477LZzkcDO9sBOirjkNQZL1LA=";

    fn key(base64: &str) -> PublicKey {
        keys::parse_public_key_base64(base64).unwrap()
    }

    fn check(text: &str, host: &str, port: u16) -> Known {
        check_text(text, host, port, &key(ED25519))
    }

    #[test]
    fn reconnait_le_nom_et_le_port() {
        let text = format!("serveur.exemple ssh-ed25519 {ED25519}\n[autre]:2222 ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&text, "serveur.exemple", 22), Known::Trusted);
        assert_eq!(check(&text, "autre", 2222), Known::Trusted);
        assert_eq!(check(&text, "autre", 22), Known::Unknown);
        assert_eq!(check(&text, "serveur.exemple", 2222), Known::Unknown);
        assert_eq!(check(&format!("[serveur.exemple]:22 ssh-ed25519 {ED25519}"), "serveur.exemple", 22), Known::Trusted);
    }

    #[test]
    fn ignore_les_majuscules_et_les_espaces_autour_du_nom() {
        let text = format!("Serveur.Exemple ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&text, "serveur.exemple", 22), Known::Trusted);
        assert_eq!(check(&text, " SERVEUR.exemple ", 22), Known::Trusted);
    }

    #[test]
    fn une_ligne_illisible_n_empeche_pas_de_lire_les_autres() {
        let text = format!(
            "# commentaire\r\n\
             serveur.exemple ssh-dss AAAAB3NzaC1kc3MAAACBAP\r\n\
             serveur.exemple  ssh-ed25519\t{ED25519}  commentaire\r\n\
             ligne-tronquee\r\n\
             @cert-authority *.exemple ssh-ed25519 {OTHER}\r\n"
        );
        assert_eq!(check(&text, "serveur.exemple", 22), Known::Trusted);
    }

    #[test]
    fn lit_les_listes_les_motifs_et_les_exclusions() {
        let list = format!("alpha,serveur.exemple,10.0.0.5 ssh-ed25519 {ED25519}");
        assert_eq!(check(&list, "10.0.0.5", 22), Known::Trusted);
        let pattern = format!("*.exemple,!secret.exemple ssh-ed25519 {ED25519}");
        assert_eq!(check(&pattern, "serveur.exemple", 22), Known::Trusted);
        assert_eq!(check(&pattern, "secret.exemple", 22), Known::Unknown);
        let single = format!("serveur-? ssh-ed25519 {ED25519}");
        assert_eq!(check(&single, "serveur-1", 22), Known::Trusted);
        assert_eq!(check(&single, "serveur-12", 22), Known::Unknown);
    }

    #[test]
    fn lit_les_noms_haches() {
        let text = format!("{HASHED_22} ssh-ed25519 {ED25519}\n{HASHED_2222} ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&text, "serveur.exemple", 22), Known::Trusted);
        assert_eq!(check(&text, "serveur.exemple", 2222), Known::Trusted);
        assert_eq!(check(&text, "autre.exemple", 22), Known::Unknown);
    }

    #[test]
    fn une_autre_cle_du_meme_type_est_un_changement() {
        let text = format!("# une\n\nserveur.exemple ssh-ed25519 {OTHER}\n");
        assert_eq!(check(&text, "serveur.exemple", 22), Known::Changed { line: 3 });
        // Une clé d'un autre type ne dit rien de celle-ci.
        let other_type = format!("serveur.exemple ecdsa-sha2-nistp256 {ECDSA}\n");
        assert_eq!(check(&other_type, "serveur.exemple", 22), Known::Unknown);
        // La bonne clé, plus loin, l'emporte sur une ancienne.
        let both = format!("serveur.exemple ssh-ed25519 {OTHER}\nserveur.exemple ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&both, "serveur.exemple", 22), Known::Trusted);
    }

    #[test]
    fn decode_les_encodages_de_windows() {
        let line = format!("serveur.exemple ssh-ed25519 {ED25519}\r\n");
        let mut latin1 = b"# caf\xe9\r\n".to_vec();
        latin1.extend_from_slice(line.as_bytes());
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(line.as_bytes());
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(Encoding::Utf16Le.encode(&line));
        let mut utf16_be = vec![0xFE, 0xFF];
        utf16_be.extend(Encoding::Utf16Be.encode(&line));
        let mut appended = b"# debut en UTF-8\n".to_vec();
        appended.extend(Encoding::Utf16Le.encode(&line));
        for bytes in [latin1, bom, utf16, utf16_be, appended] {
            assert_eq!(check(&decode(&bytes).0, "serveur.exemple", 22), Known::Trusted);
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ramure-known-hosts-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join(".ssh").join("known_hosts")
    }

    #[test]
    fn inscrit_une_seule_fois_et_cree_le_dossier() {
        let path = scratch("une-fois");
        let key = key(ED25519);
        learn_path(&path, "Serveur.Exemple", 2222, &key).unwrap();
        learn_path(&path, "serveur.exemple", 2222, &key).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), format!("[serveur.exemple]:2222 ssh-ed25519 {ED25519}\n"));
        assert_eq!(check_path(&path, "serveur.exemple", 2222, &key), Known::Trusted);
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn complete_un_fichier_sans_le_reecrire() {
        let path = scratch("complete");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Sans fin de ligne, avec un octet ANSI : la ligne ajoutée commence sur une ligne neuve.
        fs::write(&path, b"# caf\xe9\nautre ssh-ed25519 AAAA").unwrap();
        learn_path(&path, "serveur.exemple", 22, &key(ED25519)).unwrap();
        let mut expected = b"# caf\xe9\nautre ssh-ed25519 AAAA\n".to_vec();
        expected.extend_from_slice(format!("serveur.exemple ssh-ed25519 {ED25519}\n").as_bytes());
        assert_eq!(fs::read(&path).unwrap(), expected);
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn complete_un_fichier_utf16_dans_son_encodage() {
        let path = scratch("utf16");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(Encoding::Utf16Le.encode("autre ssh-ed25519 AAAA\r\n"));
        fs::write(&path, &bytes).unwrap();
        learn_path(&path, "serveur.exemple", 22, &key(ED25519)).unwrap();
        let (text, encoding) = decode(&fs::read(&path).unwrap());
        assert_eq!(encoding, Encoding::Utf16Le);
        assert_eq!(text, format!("autre ssh-ed25519 AAAA\r\nserveur.exemple ssh-ed25519 {ED25519}\n"));
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn dit_pourquoi_l_inscription_echoue() {
        let path = scratch("echec");
        // Un dossier à la place du fichier : impossible d'y écrire, même en administrateur.
        fs::create_dir_all(&path).unwrap();
        let error = learn_path(&path, "serveur.exemple", 22, &key(ED25519)).unwrap_err();
        assert!(error.starts_with(&path.display().to_string()), "{error}");
        assert!(error.contains("c’est un dossier, pas un fichier"), "{error}");
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn dit_quand_un_fichier_prend_la_place_du_dossier() {
        let path = scratch("fichier-ssh");
        let dir = path.parent().unwrap();
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        fs::write(dir, b"").unwrap();
        let error = learn_path(&path, "serveur.exemple", 22, &key(ED25519)).unwrap_err();
        assert!(error.starts_with(&dir.display().to_string()), "{error}");
        assert!(error.contains("un fichier porte ce nom"), "{error}");
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn traduit_les_erreurs_courantes() {
        let said = |kind: ErrorKind| explain(&kind.into());
        assert!(said(ErrorKind::PermissionDenied).starts_with("accès refusé. Vérifie"));
        assert_eq!(said(ErrorKind::ReadOnlyFilesystem), "le disque est en lecture seule.");
        assert!(said(ErrorKind::StorageFull).starts_with("le disque est plein"));
        // Une cause inconnue garde le texte du système.
        let other = std::io::Error::other("panne inattendue");
        assert_eq!(explain(&other), "panne inattendue");
    }
}
