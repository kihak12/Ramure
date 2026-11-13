//! The `known_hosts` file of OpenSSH: recognizing a server, and adding it there.
//!
//! russh can read it, but it gives up the whole file at the first line it does not understand: a
//! byte that is not UTF-8, two spaces in a row, an `ssh-dss` key. The server then passed for
//! unknown, and every connection added it once more. Here, as in OpenSSH, an unreadable line is
//! skipped, and only that line.
//!
//! Lines with a marker (`@cert-authority`, `@revoked`) are skipped, as russh did: Ramure does not
//! check host certificates.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use data_encoding::BASE64;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::{self, PublicKey};
use sha1::Sha1;

use super::config;

/// What `known_hosts` says of a server and its key.
#[derive(Debug, PartialEq)]
pub enum Known {
    /// A line for this server holds this key.
    Trusted,
    /// No line holds this key, but line `line` gives another one of the same type.
    Changed { line: usize },
    /// Nothing on this server for this type of key, or no file.
    Unknown,
}

/// `~/.ssh/known_hosts`, where OpenSSH looks for it, on Windows as on Linux.
fn path() -> Option<PathBuf> {
    config::home().map(|home| home.join(".ssh").join("known_hosts"))
}

pub fn check(host: &str, port: u16, key: &PublicKey) -> Known {
    match path() {
        Some(path) => check_path(&path, host, port, key),
        None => Known::Unknown,
    }
}

/// Adds the key of the server to `known_hosts`. On failure, says why.
pub fn learn(host: &str, port: u16, key: &PublicKey) -> Result<(), String> {
    let path = path().ok_or(
        "Home folder not found: neither USERPROFILE (Windows) nor HOME (Linux) is set. \
         Set one, then restart Ramure.",
    )?;
    learn_path(&path, host, port, key)
}

/// A file that is missing, or that cannot be opened, knows no server.
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

/// Appends the line of the server at the end of the file, in its encoding, then reads the file
/// again to make sure the line is there. A key already known is not added a second time.
pub fn learn_path(path: &Path, host: &str, port: u16, key: &PublicKey) -> Result<(), String> {
    if check_path(path, host, port, key) == Known::Trusted {
        return Ok(());
    }
    let failed = |e: std::io::Error| format!("{}: {}", path.display(), explain(&e));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {}", dir.display(), explain_dir(&e)))?;
    }
    // On Windows, opening a folder answers “access denied”: the case is recognized first.
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
        let advice = "Open the file to check its last line.";
        return Err(format!("{}: The new line was not found when the file was read back. {advice}", path.display()));
    }
    Ok(())
}

/// An error of the system, in plain words and with what to do, when the cause is known; otherwise
/// the text of the system, as it is (in English on Linux, in the language of Windows otherwise).
fn explain(e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::PermissionDenied => {
            "Access denied. Check that the file is not read-only and that it belongs to your account.".into()
        }
        ErrorKind::ReadOnlyFilesystem => "The disk is read-only. Make it writable, then reconnect.".into(),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => {
            "The disk is full. Free up some space, then reconnect.".into()
        }
        ErrorKind::IsADirectory => {
            "This is a folder, not a file. Rename it, then reconnect: the file will be created.".into()
        }
        _ => e.to_string(),
    }
}

/// The `.ssh` folder could not be created: a file often has that name.
fn explain_dir(e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::AlreadyExists | ErrorKind::NotADirectory => {
            "There is a file where the folder should be. Rename it, then reconnect.".into()
        }
        ErrorKind::PermissionDenied => {
            "Access denied. Check that the folder is not read-only and that it belongs to your account.".into()
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

/// The text of the file. On Windows, PowerShell 5 writes UTF-16, and Notepad may have saved it in
/// ANSI: an invalid byte becomes a replacement character, without preventing the other lines from
/// being read.
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
    // UTF-16 lines appended to a UTF-8 file can be read once their null bytes are removed.
    (text.replace('\0', ""), encoding)
}

/// The key of a line, and its host names. `None` for a comment, an empty line, a line with a marker
/// or an unreadable line.
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

/// The names under which OpenSSH records this server: `host` on port 22, `[host]:port` elsewhere.
/// On port 22, `[host]:22` is good too.
fn names(host: &str, port: u16) -> Vec<String> {
    let host = host.trim().to_lowercase();
    let bracketed = format!("[{host}]:{port}");
    if port == 22 { vec![host, bracketed] } else { vec![bracketed] }
}

fn written_name(host: &str, port: u16) -> String {
    names(host, port).swap_remove(0)
}

/// A list of patterns separated by commas: a name, a hashed name `|1|salt|hash`, a pattern with `*`
/// and `?`, or an exclusion `!pattern`, which wins over all the rest of the line.
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

/// `*`: any run of characters; `?`: one character.
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
    /// `server.example`, then `[server.example]:2222`, hashed as `ssh-keygen -H` does.
    const HASHED_22: &str = "|1|XWXzXS0pdVw/54dGPCI2klpW6Eg=|hlf/vf6x80NyIn6OnkdJGthLR6U=";
    const HASHED_2222: &str = "|1|iuHbCiDGZ0XsaYE0QHUI729pZjE=|oDSPCxDrq542SdBP9Orhj9VG9qw=";

    fn key(base64: &str) -> PublicKey {
        keys::parse_public_key_base64(base64).unwrap()
    }

    fn check(text: &str, host: &str, port: u16) -> Known {
        check_text(text, host, port, &key(ED25519))
    }

    #[test]
    fn recognizes_the_name_and_the_port() {
        let text = format!("server.example ssh-ed25519 {ED25519}\n[other]:2222 ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&text, "server.example", 22), Known::Trusted);
        assert_eq!(check(&text, "other", 2222), Known::Trusted);
        assert_eq!(check(&text, "other", 22), Known::Unknown);
        assert_eq!(check(&text, "server.example", 2222), Known::Unknown);
        assert_eq!(check(&format!("[server.example]:22 ssh-ed25519 {ED25519}"), "server.example", 22), Known::Trusted);
    }

    #[test]
    fn ignores_case_and_spaces_around_the_name() {
        let text = format!("Server.Example ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&text, "server.example", 22), Known::Trusted);
        assert_eq!(check(&text, " SERVER.example ", 22), Known::Trusted);
    }

    #[test]
    fn an_unreadable_line_does_not_stop_reading_the_others() {
        let text = format!(
            "# comment\r\n\
             server.example ssh-dss AAAAB3NzaC1kc3MAAACBAP\r\n\
             server.example  ssh-ed25519\t{ED25519}  comment\r\n\
             truncated-line\r\n\
             @cert-authority *.example ssh-ed25519 {OTHER}\r\n"
        );
        assert_eq!(check(&text, "server.example", 22), Known::Trusted);
    }

    #[test]
    fn reads_lists_patterns_and_exclusions() {
        let list = format!("alpha,server.example,10.0.0.5 ssh-ed25519 {ED25519}");
        assert_eq!(check(&list, "10.0.0.5", 22), Known::Trusted);
        let pattern = format!("*.example,!secret.example ssh-ed25519 {ED25519}");
        assert_eq!(check(&pattern, "server.example", 22), Known::Trusted);
        assert_eq!(check(&pattern, "secret.example", 22), Known::Unknown);
        let single = format!("server-? ssh-ed25519 {ED25519}");
        assert_eq!(check(&single, "server-1", 22), Known::Trusted);
        assert_eq!(check(&single, "server-12", 22), Known::Unknown);
    }

    #[test]
    fn reads_hashed_names() {
        let text = format!("{HASHED_22} ssh-ed25519 {ED25519}\n{HASHED_2222} ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&text, "server.example", 22), Known::Trusted);
        assert_eq!(check(&text, "server.example", 2222), Known::Trusted);
        assert_eq!(check(&text, "other.example", 22), Known::Unknown);
    }

    #[test]
    fn another_key_of_the_same_type_is_a_change() {
        let text = format!("# one\n\nserver.example ssh-ed25519 {OTHER}\n");
        assert_eq!(check(&text, "server.example", 22), Known::Changed { line: 3 });
        // A key of another type says nothing of this one.
        let other_type = format!("server.example ecdsa-sha2-nistp256 {ECDSA}\n");
        assert_eq!(check(&other_type, "server.example", 22), Known::Unknown);
        // The right key, further down, wins over an old one.
        let both = format!("server.example ssh-ed25519 {OTHER}\nserver.example ssh-ed25519 {ED25519}\n");
        assert_eq!(check(&both, "server.example", 22), Known::Trusted);
    }

    #[test]
    fn decodes_the_encodings_of_windows() {
        let line = format!("server.example ssh-ed25519 {ED25519}\r\n");
        let mut latin1 = b"# caf\xe9\r\n".to_vec();
        latin1.extend_from_slice(line.as_bytes());
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(line.as_bytes());
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(Encoding::Utf16Le.encode(&line));
        let mut utf16_be = vec![0xFE, 0xFF];
        utf16_be.extend(Encoding::Utf16Be.encode(&line));
        let mut appended = b"# starts in UTF-8\n".to_vec();
        appended.extend(Encoding::Utf16Le.encode(&line));
        for bytes in [latin1, bom, utf16, utf16_be, appended] {
            assert_eq!(check(&decode(&bytes).0, "server.example", 22), Known::Trusted);
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ramure-known-hosts-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join(".ssh").join("known_hosts")
    }

    #[test]
    fn adds_once_and_creates_the_folder() {
        let path = scratch("once");
        let key = key(ED25519);
        learn_path(&path, "Server.Example", 2222, &key).unwrap();
        learn_path(&path, "server.example", 2222, &key).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), format!("[server.example]:2222 ssh-ed25519 {ED25519}\n"));
        assert_eq!(check_path(&path, "server.example", 2222, &key), Known::Trusted);
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn appends_to_a_file_without_rewriting_it() {
        let path = scratch("complete");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // No line ending, and an ANSI byte: the appended line starts on a new line.
        fs::write(&path, b"# caf\xe9\nother ssh-ed25519 AAAA").unwrap();
        learn_path(&path, "server.example", 22, &key(ED25519)).unwrap();
        let mut expected = b"# caf\xe9\nother ssh-ed25519 AAAA\n".to_vec();
        expected.extend_from_slice(format!("server.example ssh-ed25519 {ED25519}\n").as_bytes());
        assert_eq!(fs::read(&path).unwrap(), expected);
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn appends_to_a_utf16_file_in_its_encoding() {
        let path = scratch("utf16");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(Encoding::Utf16Le.encode("other ssh-ed25519 AAAA\r\n"));
        fs::write(&path, &bytes).unwrap();
        learn_path(&path, "server.example", 22, &key(ED25519)).unwrap();
        let (text, encoding) = decode(&fs::read(&path).unwrap());
        assert_eq!(encoding, Encoding::Utf16Le);
        assert_eq!(text, format!("other ssh-ed25519 AAAA\r\nserver.example ssh-ed25519 {ED25519}\n"));
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn says_why_adding_the_server_fails() {
        let path = scratch("failure");
        // A folder in place of the file: it cannot be written, even as an administrator.
        fs::create_dir_all(&path).unwrap();
        let error = learn_path(&path, "server.example", 22, &key(ED25519)).unwrap_err();
        assert!(error.starts_with(&path.display().to_string()), "{error}");
        assert!(error.contains("This is a folder, not a file"), "{error}");
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn says_when_a_file_stands_where_the_folder_should_be() {
        let path = scratch("ssh-file");
        let dir = path.parent().unwrap();
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        fs::write(dir, b"").unwrap();
        let error = learn_path(&path, "server.example", 22, &key(ED25519)).unwrap_err();
        assert!(error.starts_with(&dir.display().to_string()), "{error}");
        assert!(error.contains("There is a file where the folder should be"), "{error}");
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn explains_the_common_errors() {
        let said = |kind: ErrorKind| explain(&kind.into());
        assert!(said(ErrorKind::PermissionDenied).starts_with("Access denied. Check"));
        assert!(said(ErrorKind::ReadOnlyFilesystem).starts_with("The disk is read-only. Make it"));
        assert!(explain_dir(&ErrorKind::PermissionDenied.into()).contains("the folder is not read-only"));
        assert!(said(ErrorKind::StorageFull).starts_with("The disk is full"));
        // An unknown cause keeps the text of the system.
        let other = std::io::Error::other("unexpected failure");
        assert_eq!(explain(&other), "unexpected failure");
    }
}
