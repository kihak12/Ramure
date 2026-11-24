//! The saved sessions of PuTTY, as hosts of the book (guidelines, 9.15, "Importing and exporting
//! hosts"): on Windows, a `.reg` export of `HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions`;
//! on Linux, the files of `~/.putty/sessions`.
//!
//! An SSH session becomes a host; a Telnet, serial, raw or rlogin one is listed, not imported, as is
//! one reached through a proxy. PuTTY keeps no password.

use std::path::Path;

use super::hostfile::{Found, plan_key};
use super::store::Host;

/// Where the hosts of PuTTY land.
pub const GROUP: &str = "Imported from PuTTY";

/// The sessions PuTTY keeps for its own defaults: not a server.
const DEFAULTS: &str = "Default Settings";

/// A name as PuTTY writes it, `My%20server`, decoded.
fn decode(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&name[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The values of one session, by name: `HostName`, `PortNumber`…
type Values = Vec<(String, String)>;

fn value<'a>(values: &'a Values, key: &str) -> Option<&'a str> {
    values.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
}

/// A session of PuTTY as a host of the book.
fn session(name: &str, values: &Values) -> Found {
    let mut hostname = value(values, "HostName").unwrap_or_default().to_string();
    let mut user = value(values, "UserName").unwrap_or_default().to_string();
    // PuTTY takes "user@host" in its host name too.
    if let Some((before, after)) = hostname.clone().split_once('@') {
        if user.is_empty() {
            user = before.to_string();
        }
        hostname = after.to_string();
    }
    let port = value(values, "PortNumber").and_then(|p| p.parse().ok()).filter(|p| *p != 0).unwrap_or(22);
    let mut host = Host { alias: name.to_string(), hostname, port, user, auth: "auto".into(), group: GROUP.into(), ..Host::default() };

    let protocol = value(values, "Protocol").unwrap_or("ssh").to_lowercase();
    let proxied = value(values, "ProxyMethod").is_some_and(|m| m != "0");
    let refused = if protocol != "ssh" {
        Some(format!("A {protocol} session: Ramure opens SSH only."))
    } else if proxied {
        Some("Reached through a proxy: Ramure cannot connect through one yet.".to_string())
    } else if host.hostname.is_empty() {
        Some("It has no address.".to_string())
    } else {
        None
    };

    let mut key = None;
    if let Some(path) = value(values, "PublicKeyFile") {
        host.auth = "keyFile".into();
        host.key_path = path.to_string();
        key = plan_key(path);
    }
    Found { host, refused, password: None, key }
}

/// The text of a `.reg` file: UTF-16 with its byte order mark, as `regedit` writes it, or UTF-8.
fn reg_text(bytes: &[u8]) -> String {
    if let [0xFF, 0xFE, rest @ ..] = bytes {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).trim_start_matches('\u{feff}').to_string()
}

/// A value of a `.reg` file: `"text"` with its `\\` and `\"`, or `dword:00000016`.
fn reg_value(raw: &str) -> String {
    if let Some(hex) = raw.strip_prefix("dword:") {
        return u32::from_str_radix(hex.trim(), 16).map(|n| n.to_string()).unwrap_or_default();
    }
    let inner = raw.trim().trim_start_matches('"').trim_end_matches('"');
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The sessions of a `.reg` export, or why it is not one.
pub fn read_reg(bytes: &[u8]) -> Result<Vec<Found>, String> {
    let not_ours = || {
        "This is not an export of the sessions of PuTTY: in regedit, export \
         HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions, then choose that file."
            .to_string()
    };
    let text = reg_text(bytes);
    let marker = "\\Software\\SimonTatham\\PuTTY\\Sessions\\";
    let mut sessions: Vec<(String, Values)> = Vec::new();
    let mut current: Option<usize> = None;
    for line in text.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = section.find(marker).map(|at| &section[at + marker.len()..]).filter(|name| !name.contains('\\')).map(|name| {
                sessions.push((decode(name), Vec::new()));
                sessions.len() - 1
            });
        } else if let (Some(at), Some((key, raw))) = (current, line.split_once('=')) {
            sessions[at].1.push((reg_value(key), reg_value(raw)));
        }
    }
    if sessions.is_empty() && !text.contains("SimonTatham\\PuTTY") {
        return Err(not_ours());
    }
    Ok(sessions.iter().filter(|(name, _)| name != DEFAULTS).map(|(name, values)| session(name, values)).collect())
}

/// The sessions of a folder of PuTTY on Linux, `~/.putty/sessions`: one file per session.
pub fn read_dir(dir: &Path) -> Result<Vec<Found>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("This folder could not be read: {e}."))?;
    let mut sessions: Vec<(String, Values)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !path.is_file() || file.starts_with('.') {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let values: Values = text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        if value(&values, "HostName").is_none() && value(&values, "Protocol").is_none() {
            continue;
        }
        sessions.push((decode(file), values));
    }
    if sessions.is_empty() {
        return Err("No sessions of PuTTY in this folder: choose ~/.putty/sessions.".to_string());
    }
    sessions.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    Ok(sessions.iter().filter(|(name, _)| name != DEFAULTS).map(|(name, values)| session(name, values)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG: &str = "Windows Registry Editor Version 5.00\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions]\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Default%20Settings]\r\n\
\"HostName\"=\"\"\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Web%20prod]\r\n\
\"HostName\"=\"deploy@10.0.0.9\"\r\n\
\"PortNumber\"=dword:00000016\r\n\
\"Protocol\"=\"ssh\"\r\n\
\"PublicKeyFile\"=\"C:\\\\nowhere\\\\id_web.ppk\"\r\n\
\"ProxyMethod\"=dword:00000000\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Switch]\r\n\
\"HostName\"=\"10.0.0.2\"\r\n\
\"Protocol\"=\"telnet\"\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Behind]\r\n\
\"HostName\"=\"10.0.0.3\"\r\n\
\"Protocol\"=\"ssh\"\r\n\
\"ProxyMethod\"=dword:00000002\r\n";

    #[test]
    fn the_ssh_sessions_of_a_reg_export_become_hosts() {
        // regedit writes UTF-16 with its byte order mark.
        let mut bytes = vec![0xFF, 0xFE];
        for unit in REG.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let found = read_reg(&bytes).unwrap();
        assert_eq!(found.len(), 3, "the default settings are not a server");
        let web = &found[0];
        assert_eq!((web.host.alias.as_str(), web.host.user.as_str(), web.host.hostname.as_str(), web.host.port), ("Web prod", "deploy", "10.0.0.9", 22));
        assert_eq!((web.host.auth.as_str(), web.host.key_path.as_str(), web.host.group.as_str()), ("keyFile", "C:\\nowhere\\id_web.ppk", GROUP));
        assert!(matches!(web.key, Some(super::super::hostfile::KeyPlan::Missing { .. })));
        assert!(web.refused.is_none());
        assert_eq!(found[1].refused.as_deref(), Some("A telnet session: Ramure opens SSH only."));
        assert_eq!(found[2].refused.as_deref(), Some("Reached through a proxy: Ramure cannot connect through one yet."));
        // The same, written in UTF-8.
        assert_eq!(read_reg(REG.as_bytes()).unwrap().len(), 3);
    }

    #[test]
    fn another_file_is_not_a_putty_export() {
        assert!(read_reg(b"{\"format\":\"ramure-hosts\"}").unwrap_err().starts_with("This is not an export of the sessions of PuTTY"));
    }

    #[test]
    fn the_session_files_of_linux_become_hosts() {
        let dir = std::env::temp_dir().join(format!("ramure-putty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("db%20server"), "HostName=10.0.0.7\nPortNumber=2222\nUserName=postgres\nProtocol=ssh\n").unwrap();
        std::fs::write(dir.join("Default%20Settings"), "HostName=\nProtocol=ssh\n").unwrap();
        let found = read_dir(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].host.alias.as_str(), found[0].host.user.as_str(), found[0].host.port), ("db server", "postgres", 2222));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(read_dir(&dir).unwrap_err().starts_with("This folder could not be read"));
    }
}
