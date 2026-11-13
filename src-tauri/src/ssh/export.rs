//! Writing hosts of the book into `~/.ssh/config`, so that they open with `ssh <name>` too.
//!
//! The file is changed line by line: only the HostName, User, Port and IdentityFile lines of the
//! `Host <name>` block of an exported host change, or a block is added. The rest of the file, its
//! comments, its other directives and its line endings, does not move. Ramure never deletes a
//! line. The old file is copied to `config.ramure.bak` before writing.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use super::config;
use super::store::{Host, Store};

/// What the export would do with a host.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub alias: String,
    /// `added`, `modified`, `current` or `blocked`.
    pub status: &'static str,
    /// Added: what will be written; changed: what changes; not exported: why.
    pub detail: String,
}

/// What a host asks of the file.
struct Wanted {
    alias: String,
    hostname: String,
    user: Option<String>,
    port: u16,
    identity_file: Option<String>,
    password: bool,
}

impl Wanted {
    fn of(host: &Host) -> Self {
        let user = Some(host.user.trim().to_string()).filter(|u| !u.is_empty());
        let key = host.key_path.trim();
        Wanted {
            alias: host.alias.trim().to_string(),
            hostname: host.hostname.trim().to_string(),
            user,
            port: host.port,
            identity_file: (host.auth == "keyFile" && !key.is_empty()).then(|| key.to_string()),
            password: host.auth == "password",
        }
    }

    /// The directives written for an added host, in the order of OpenSSH.
    fn directives(&self) -> Vec<(&'static str, String)> {
        let mut out = vec![("HostName", self.hostname.clone())];
        if let Some(user) = &self.user {
            out.push(("User", user.clone()));
        }
        if self.port != 22 {
            out.push(("Port", self.port.to_string()));
        }
        if let Some(key) = &self.identity_file {
            out.push(("IdentityFile", key.clone()));
        }
        out
    }

    /// What will be written, on one line: `deploy@10.0.0.9 · port 2222 · key ~/.ssh/id_ed25519`.
    fn summary(&self) -> String {
        let mut parts = vec![match &self.user {
            Some(user) => format!("{user}@{}", self.hostname),
            None => self.hostname.clone(),
        }];
        if self.port != 22 {
            parts.push(format!("port {}", self.port));
        }
        match (&self.identity_file, self.password) {
            (Some(key), _) => parts.push(format!("key {key}")),
            (None, true) => parts.push("no key: the password is never written".into()),
            (None, false) => parts.push("SSH agent and default keys".into()),
        }
        parts.join(" · ")
    }
}

/// A `Host` or `Match` block of the file: its lines, from the header to the line before the next
/// block.
struct Block {
    /// The names of the `Host` header; empty for a `Match` block.
    patterns: Vec<String>,
    header: usize,
    end: usize,
}

/// The keyword of a line and its value, without quotes: `Port 2222`, `Port=2222`.
fn directive(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once(|c: char| c.is_whitespace() || c == '=')?;
    let value = value.trim().trim_start_matches('=').trim().trim_matches('"');
    Some((key.to_ascii_lowercase(), value.to_string()))
}

fn blocks(lines: &[String]) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some((key, value)) = directive(line) else { continue };
        if key == "host" || key == "match" {
            if let Some(last) = out.last_mut() {
                last.end = i;
            }
            let patterns = if key == "host" {
                value.split_whitespace().map(|p| p.trim_matches('"').to_string()).collect()
            } else {
                Vec::new()
            };
            out.push(Block { patterns, header: i, end: lines.len() });
        }
    }
    out
}

/// The first value of a directive in a block, and its line.
fn value_in(lines: &[String], block: &Block, wanted: &str) -> Option<(usize, String)> {
    (block.header + 1..block.end).find_map(|i| match directive(&lines[i]) {
        Some((key, value)) if key == wanted => Some((i, value)),
        _ => None,
    })
}

/// What would change in the block of a host: (directive, value in the file, value in Ramure).
fn changes(lines: &[String], block: &Block, wanted: &Wanted) -> Vec<(&'static str, Option<String>, String)> {
    let mut out = Vec::new();
    let hostname = value_in(lines, block, "hostname").map(|(_, v)| v);
    // Without HostName, ssh connects to the name itself.
    if hostname.as_deref().unwrap_or(&wanted.alias) != wanted.hostname {
        out.push(("HostName", hostname, wanted.hostname.clone()));
    }
    if let Some(user) = &wanted.user {
        let current = value_in(lines, block, "user").map(|(_, v)| v);
        if current.as_deref() != Some(user.as_str()) {
            out.push(("User", current, user.clone()));
        }
    }
    let port = value_in(lines, block, "port").map(|(_, v)| v);
    if port.as_deref().unwrap_or("22").parse::<u16>().ok() != Some(wanted.port) {
        out.push(("Port", port, wanted.port.to_string()));
    }
    if let Some(key) = &wanted.identity_file {
        let current = value_in(lines, block, "identityfile").map(|(_, v)| v);
        if current.as_deref() != Some(key.as_str()) {
            out.push(("IdentityFile", current, key.clone()));
        }
    }
    out
}

/// Why a host cannot be written, if it cannot be.
fn blocked(host: &Host, wanted: &Wanted, lines: &[String], all: &[Block], taken: &[String]) -> Option<String> {
    if wanted.alias.chars().any(char::is_whitespace) {
        return Some("Its name has a space, which ssh would read as two names: rename the host.".into());
    }
    if wanted.alias.contains(['*', '?', '!', '"', '#']) {
        return Some("Its name has a character that ssh would read as a pattern: rename the host.".into());
    }
    if host.proxy_jump {
        return Some("It goes through a bastion, whose address Ramure does not keep: write its entry by hand, with ProxyJump.".into());
    }
    if taken.contains(&wanted.alias) {
        return Some("Another host has the same name: rename one of the two.".into());
    }
    let shared = all.iter().find(|b| b.patterns.len() > 1 && b.patterns.contains(&wanted.alias));
    if let Some(block) = shared {
        let header = lines[block.header].trim();
        return Some(format!(
            "Its name is in the block “{header}” of the file, with others: edit that block by hand."
        ));
    }
    None
}

/// The block of a name of its own: `Host <name>`, alone on its line. The first one, as for ssh.
fn own<'a>(all: &'a [Block], alias: &str) -> Option<&'a Block> {
    all.iter().find(|b| b.patterns.len() == 1 && b.patterns[0] == alias)
}

/// The plan of the export: one row per host, in the order of the book.
pub fn plan(text: &str, hosts: &[Host]) -> Vec<Row> {
    let lines = split(text).0;
    let all = blocks(&lines);
    let mut taken: Vec<String> = Vec::new();
    hosts
        .iter()
        .map(|host| {
            let wanted = Wanted::of(host);
            let row = |status, detail| Row { id: host.id.clone(), alias: wanted.alias.clone(), status, detail };
            if let Some(reason) = blocked(host, &wanted, &lines, &all, &taken) {
                return row("blocked", reason);
            }
            taken.push(wanted.alias.clone());
            match own(&all, &wanted.alias) {
                None => row("added", wanted.summary()),
                Some(block) => {
                    let found = changes(&lines, block, &wanted);
                    if found.is_empty() {
                        row("current", String::new())
                    } else {
                        let said: Vec<String> = found
                            .iter()
                            .map(|(key, from, to)| format!("{key} {} → {to}", from.as_deref().unwrap_or("(none)")))
                            .collect();
                        row("modified", said.join(" · "))
                    }
                }
            }
        })
        .collect()
}

/// The lines of the file, and its line ending: CRLF if it has any, as on Windows.
fn split(text: &str) -> (Vec<String>, &'static str) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let body = text.strip_suffix('\n').unwrap_or(text);
    let lines = if text.is_empty() { Vec::new() } else { body.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect() };
    (lines, eol)
}

fn quoted(value: &str) -> String {
    if value.chars().any(char::is_whitespace) { format!("\"{value}\"") } else { value.to_string() }
}

/// The text of the file once the `chosen` hosts are written. Those the plan says are neither
/// added nor changed are left aside.
pub fn apply(text: &str, hosts: &[Host], chosen: &[String]) -> (String, usize) {
    let rows = plan(text, hosts);
    let (mut lines, eol) = split(text);
    let picked: Vec<&Host> = hosts
        .iter()
        .filter(|h| chosen.contains(&h.id))
        .filter(|h| rows.iter().any(|r| r.id == h.id && matches!(r.status, "added" | "modified")))
        .collect();

    // The changed blocks, from the bottom up: the line numbers above them do not move.
    let all = blocks(&lines);
    let mut edits: Vec<(&Block, Wanted)> =
        picked.iter().filter_map(|h| own(&all, h.alias.trim()).map(|b| (b, Wanted::of(h)))).collect();
    edits.sort_by_key(|(b, _)| std::cmp::Reverse(b.header));
    for (block, wanted) in &edits {
        let indent = (block.header + 1..block.end)
            .find_map(|i| directive(&lines[i]).map(|_| lines[i][..lines[i].len() - lines[i].trim_start().len()].to_string()))
            .unwrap_or_else(|| "    ".to_string());
        let mut inserts: Vec<String> = Vec::new();
        for (key, _, value) in changes(&lines, block, wanted) {
            match value_in(&lines, block, &key.to_ascii_lowercase()) {
                Some((i, _)) => {
                    // The line keeps its indentation and the spelling of its keyword.
                    let line = &lines[i];
                    let lead = &line[..line.len() - line.trim_start().len()];
                    let word = line.trim_start().split(|c: char| c.is_whitespace() || c == '=').next().unwrap_or(key);
                    lines[i] = format!("{lead}{word} {}", quoted(&value));
                }
                None => inserts.push(format!("{indent}{key} {}", quoted(&value))),
            }
        }
        // The missing directives, after the last line of the block that is not empty.
        let mut at = block.end;
        while at > block.header + 1 && lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        for (k, line) in inserts.into_iter().enumerate() {
            lines.insert(at + k, line);
        }
    }

    // The added blocks, before `Host *` if there is one: the first value found wins.
    let added: Vec<Wanted> = picked.iter().filter(|h| own(&blocks(&lines), h.alias.trim()).is_none()).map(|h| Wanted::of(h)).collect();
    if !added.is_empty() {
        let mut fresh: Vec<String> = Vec::new();
        for wanted in &added {
            fresh.push(format!("Host {}", wanted.alias));
            fresh.extend(wanted.directives().into_iter().map(|(key, value)| format!("    {key} {}", quoted(&value))));
            fresh.push(String::new());
        }
        let star = blocks(&lines).into_iter().find(|b| b.patterns == ["*"]).map(|b| b.header);
        match star {
            Some(at) => {
                for (k, line) in fresh.into_iter().enumerate() {
                    lines.insert(at + k, line);
                }
            }
            None => {
                if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    lines.push(String::new());
                }
                fresh.pop();
                lines.extend(fresh);
            }
        }
    }

    let mut out = lines.join(eol);
    out.push_str(eol);
    (out, picked.len())
}

fn config_path() -> Result<PathBuf, String> {
    config::home().map(|home| home.join(".ssh").join("config")).ok_or_else(|| "Home folder not found: neither USERPROFILE (Windows) nor HOME (Linux) is set. Set one, then restart Ramure.".to_string())
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name("config.ramure.bak")
}

fn read(path: &Path) -> Result<String, String> {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| {
            "~/.ssh/config is not in UTF-8: save it as UTF-8 in an editor, then try again.".to_string()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("~/.ssh/config could not be read: check that your account can read it, then open the dialog again. ({e})")),
    }
}

/// Writes `text` in place of `path`, through a file beside it that is renamed: ssh never reads a
/// file written halfway.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let failed = |e: std::io::Error| {
        format!("~/.ssh/config could not be written: check that neither the file nor ~/.ssh is read-only, then try again. ({e})")
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(failed)?;
    }
    let temporary = path.with_file_name("config.ramure.tmp");
    fs::write(&temporary, text).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // ssh refuses a file that others can write: the one from before keeps its permissions.
        let mode = fs::metadata(path).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o600) & !0o022;
        let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(mode));
    }
    fs::rename(&temporary, path).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        failed(e)
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub count: usize,
    /// The copy of the old file, if there was one.
    pub backup: Option<String>,
    /// The text written: undoing puts the old file back only if this one has not changed since.
    pub written: String,
}

pub fn export_to(path: &Path, hosts: &[Host], chosen: &[String]) -> Result<Exported, String> {
    let before = read(path)?;
    let (text, count) = apply(&before, hosts, chosen);
    if count == 0 {
        return Err("None of the checked hosts needs to be written: the file has changed in the meantime. Close the dialog and open it again.".into());
    }
    let backup = if path.exists() {
        let copy = backup_path(path);
        fs::copy(path, &copy).map_err(|e| format!("The old file could not be copied, and nothing was written: check that ~/.ssh is not read-only, then try again. ({e})"))?;
        Some(copy.display().to_string())
    } else {
        None
    };
    write(path, &text)?;
    Ok(Exported { count, backup, written: text })
}

pub fn undo_at(path: &Path, written: &str, had_backup: bool) -> Result<(), String> {
    if read(path)? != written {
        return Err("~/.ssh/config has changed since the export: it was left as it is. The old file is in ~/.ssh/config.ramure.bak.".into());
    }
    if had_backup {
        let old = fs::read_to_string(backup_path(path)).map_err(|e| {
            format!("~/.ssh/config.ramure.bak could not be read: put it back in place of ~/.ssh/config by hand. ({e})")
        })?;
        write(path, &old)
    } else {
        fs::remove_file(path).map_err(|e| format!("~/.ssh/config could not be removed: delete it by hand. ({e})"))
    }
}

/// The hosts of the book to consider: all of them, or those of `ids`.
fn hosts_of(store: &Store, ids: &Option<Vec<String>>) -> Result<Vec<Host>, String> {
    let hosts = store.list()?;
    Ok(match ids {
        Some(ids) => hosts.into_iter().filter(|h| ids.contains(&h.id)).collect(),
        None => hosts,
    })
}

#[tauri::command]
pub fn ssh_config_preview(store: State<'_, Store>, ids: Option<Vec<String>>) -> Result<Vec<Row>, String> {
    let text = read(&config_path()?)?;
    Ok(plan(&text, &hosts_of(&store, &ids)?))
}

#[tauri::command]
pub fn ssh_config_export(store: State<'_, Store>, ids: Vec<String>) -> Result<Exported, String> {
    // The whole book, so that two hosts with the same name see each other.
    export_to(&config_path()?, &store.list()?, &ids)
}

#[tauri::command]
pub fn ssh_config_undo(written: String, had_backup: bool) -> Result<(), String> {
    undo_at(&config_path()?, &written, had_backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(id: &str, alias: &str, hostname: &str, user: &str, port: u16) -> Host {
        Host {
            id: id.into(),
            alias: alias.into(),
            hostname: hostname.into(),
            user: user.into(),
            port,
            auth: "auto".into(),
            ..Host::default()
        }
    }

    fn status(rows: &[Row], alias: &str) -> (&'static str, String) {
        let row = rows.iter().find(|r| r.alias == alias).unwrap();
        (row.status, row.detail.clone())
    }

    const FILE: &str = "\
# My servers
Include ~/.ssh/config.d/*

Host vps
    HostName vps.example.net
    User admin
    ForwardAgent yes

Host web prod
    HostName web.example.net

Host *
    ServerAliveInterval 30
";

    #[test]
    fn the_plan_says_added_changed_up_to_date_and_not_exported() {
        let mut key = host("4", "new", "10.0.0.9", "deploy", 2222);
        key.auth = "keyFile".into();
        key.key_path = "~/.ssh/id_ed25519".into();
        let mut jump = host("5", "internal", "10.0.2.15", "", 22);
        jump.proxy_jump = true;
        let hosts = vec![
            host("1", "vps", "vps.example.net", "deploy", 2222),
            host("2", "web", "web.example.net", "", 22),
            host("3", "workshop (copy)", "a.example.net", "", 22),
            key,
            jump,
            host("6", "vps", "other.example.net", "", 22),
        ];
        let rows = plan(FILE, &hosts);
        assert_eq!(status(&rows, "vps"), ("modified", "User admin → deploy · Port (none) → 2222".into()));
        assert_eq!(status(&rows, "web").0, "blocked");
        assert!(status(&rows, "web").1.contains("Host web prod"));
        assert_eq!(status(&rows, "workshop (copy)").0, "blocked");
        assert_eq!(status(&rows, "new"), ("added", "deploy@10.0.0.9 · port 2222 · key ~/.ssh/id_ed25519".into()));
        assert!(status(&rows, "internal").1.contains("bastion"));
        // The second "vps" of the book: blocked, the first one keeps the name.
        assert_eq!(rows[5].status, "blocked");

        let same = vec![host("1", "vps", "vps.example.net", "admin", 22)];
        assert_eq!(plan(FILE, &same)[0].status, "current");
    }

    #[test]
    fn writing_touches_only_the_lines_of_ramure() {
        let hosts = vec![host("1", "vps", "vps.example.net", "deploy", 2222), host("2", "new", "10.0.0.9", "", 22)];
        let (text, count) = apply(FILE, &hosts, &["1".into(), "2".into()]);
        assert_eq!(count, 2);
        assert_eq!(
            text,
            "\
# My servers
Include ~/.ssh/config.d/*

Host vps
    HostName vps.example.net
    User deploy
    ForwardAgent yes
    Port 2222

Host web prod
    HostName web.example.net

Host new
    HostName 10.0.0.9

Host *
    ServerAliveInterval 30
"
        );
        // A second time: everything is up to date, nothing changes.
        assert!(plan(&text, &hosts).iter().all(|r| r.status == "current"));
        assert_eq!(apply(&text, &hosts, &["1".into(), "2".into()]).1, 0);
    }

    #[test]
    fn keeps_line_endings_indentation_and_keywords() {
        let file = "Host vps\r\n\thostname=old.example.net\r\n\tuser admin\r\n";
        let hosts = vec![host("1", "vps", "new.example.net", "admin", 22)];
        let (text, _) = apply(file, &hosts, &["1".into()]);
        assert_eq!(text, "Host vps\r\n\thostname new.example.net\r\n\tuser admin\r\n");
        // A host that is not checked is not written.
        assert_eq!(apply(file, &hosts, &[]).0, file);
    }

    #[test]
    fn adds_to_an_empty_file_or_one_without_a_general_block() {
        let mut key = host("1", "new", "10.0.0.9", "me", 22);
        key.auth = "keyFile".into();
        key.key_path = "C:\\Users\\Jane Doe\\.ssh\\id_ed25519".into();
        let (text, _) = apply("", &[key.clone()], &["1".into()]);
        assert_eq!(
            text,
            "Host new\n    HostName 10.0.0.9\n    User me\n    IdentityFile \"C:\\Users\\Jane Doe\\.ssh\\id_ed25519\"\n"
        );
        let (text, _) = apply("Host a\n    HostName a.net", &[key], &["1".into()]);
        assert!(text.starts_with("Host a\n    HostName a.net\n\nHost new\n"));
    }

    #[test]
    fn exports_copies_the_old_file_and_undoes() {
        let dir = std::env::temp_dir().join(format!("ramure-export-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(".ssh").join("config");
        let hosts = vec![host("1", "new", "10.0.0.9", "", 22)];

        // Without a file: it is created, and undoing removes it.
        let done = export_to(&path, &hosts, &["1".into()]).unwrap();
        assert_eq!(done.count, 1);
        assert!(done.backup.is_none());
        assert!(fs::read_to_string(&path).unwrap().contains("Host new"));
        undo_at(&path, &done.written, false).unwrap();
        assert!(!path.exists());

        // With a file: the copy keeps it, undoing puts it back.
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, FILE).unwrap();
        let done = export_to(&path, &hosts, &["1".into()]).unwrap();
        assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), FILE);
        // A file edited since the export is not replaced.
        fs::write(&path, format!("{}# edited\n", done.written)).unwrap();
        assert!(undo_at(&path, &done.written, true).unwrap_err().contains("has changed"));
        fs::write(&path, &done.written).unwrap();
        undo_at(&path, &done.written, true).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), FILE);
        // Nothing to write: refused, nothing moves.
        let current = fs::read_to_string(&path).unwrap();
        assert!(export_to(&path, &hosts, &[]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), current);
        let _ = fs::remove_dir_all(&dir);
    }
}
