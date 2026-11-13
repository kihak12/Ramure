//! Log of a terminal session: what was shown, as readable text, in a file.
//!
//! The stream of a session (SSH or local) goes through here before the interface: nothing makes
//! the round trip. Only what is shown is written; what the user types is never recorded on its
//! own, so a password typed without echo does not appear in it.
//!
//! The text is rendered line by line, as a terminal would without moving the cursor: control
//! sequences are removed, a carriage return rewrites the line (a progress bar keeps its final
//! state), a backspace moves back. A full-screen program (vim, htop, less) leaves only one line:
//! its display does not read as text.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;

/// The line that replaces the display of a full-screen program.
const FULL_SCREEN: &str = "[full-screen display omitted]";

/// What it takes to open or resume a log.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalOpen {
    pub path: String,
    /// First line of a new file: "Log of test-server, started Nov 11, 2026 at 11:05 PM".
    pub header: String,
    /// A line to write before what follows: the resumption after a reconnection.
    pub note: Option<String>,
    /// What the terminal showed already, when the log starts during a session.
    pub backlog: Option<String>,
}

/// Where the parsing of an escape sequence stands.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Escape {
    None,
    /// After ESC.
    Start,
    /// In `ESC [ … final`.
    Csi,
    /// In `ESC ] …` or `ESC P …`, up to BEL or `ESC \`.
    Text,
    /// An ESC in a text sequence: perhaps its end.
    TextEsc,
    /// `ESC (` and the like: one more byte, then the end.
    Charset,
}

/// The rendered text of a terminal stream.
#[derive(Debug)]
pub struct Render {
    line: Vec<char>,
    col: usize,
    escape: Escape,
    params: String,
    /// Bytes of a UTF-8 character split between two packets.
    pending: Vec<u8>,
    /// A full-screen program holds the screen: nothing is written.
    alternate: bool,
}

impl Default for Render {
    fn default() -> Self {
        Render { line: Vec::new(), col: 0, escape: Escape::None, params: String::new(), pending: Vec::new(), alternate: false }
    }
}

impl Render {
    /// Renders `bytes`; returns the finished lines, without their line ending.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut data = std::mem::take(&mut self.pending);
        data.extend_from_slice(bytes);
        let text = match std::str::from_utf8(&data) {
            Ok(text) => text.to_string(),
            Err(e) if e.error_len().is_none() => {
                // The end of the packet splits a character: it waits for the next packet.
                let valid = e.valid_up_to();
                self.pending = data[valid..].to_vec();
                String::from_utf8_lossy(&data[..valid]).into_owned()
            }
            Err(_) => String::from_utf8_lossy(&data).into_owned(),
        };
        let mut out = Vec::new();
        for c in text.chars() {
            self.char(c, &mut out);
        }
        out
    }

    fn char(&mut self, c: char, out: &mut Vec<String>) {
        match self.escape {
            Escape::Start => {
                self.escape = match c {
                    '[' => {
                        self.params.clear();
                        Escape::Csi
                    }
                    ']' | 'P' | '_' | '^' | 'X' => Escape::Text,
                    '(' | ')' | '*' | '+' | '#' | '%' => Escape::Charset,
                    _ => Escape::None,
                };
                return;
            }
            Escape::Csi => {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    self.escape = Escape::None;
                    self.csi(c, out);
                } else {
                    self.params.push(c);
                }
                return;
            }
            Escape::Text => {
                match c {
                    '\u{7}' => self.escape = Escape::None,
                    '\u{1b}' => self.escape = Escape::TextEsc,
                    _ => {}
                }
                return;
            }
            Escape::TextEsc => {
                self.escape = if c == '\\' { Escape::None } else { Escape::Text };
                return;
            }
            Escape::Charset => {
                self.escape = Escape::None;
                return;
            }
            Escape::None => {}
        }
        match c {
            '\u{1b}' => self.escape = Escape::Start,
            _ if self.alternate => {}
            '\n' => self.end_line(out),
            '\r' => self.col = 0,
            '\u{8}' => self.col = self.col.saturating_sub(1),
            '\t' => {
                let next = (self.col / 8 + 1) * 8;
                while self.col < next {
                    self.put(' ');
                }
            }
            _ if c.is_control() => {}
            _ => self.put(c),
        }
    }

    fn put(&mut self, c: char) {
        if self.col < self.line.len() {
            self.line[self.col] = c;
        } else {
            self.line.resize(self.col, ' ');
            self.line.push(c);
        }
        self.col += 1;
    }

    fn end_line(&mut self, out: &mut Vec<String>) {
        let text: String = self.line.iter().collect();
        out.push(text.trim_end().to_string());
        self.line.clear();
        self.col = 0;
    }

    fn csi(&mut self, last: char, out: &mut Vec<String>) {
        let private = self.params.starts_with('?');
        let numbers: Vec<usize> =
            self.params.trim_start_matches('?').split(';').map(|n| n.parse().unwrap_or(0)).collect();
        let n = numbers.first().copied().unwrap_or(0).max(1);
        match (private, last) {
            // Alternate screen: a full-screen program takes over, then gives it back.
            (true, 'h') if numbers.iter().any(|&m| matches!(m, 47 | 1047 | 1049)) => {
                if !self.alternate {
                    if !self.line.is_empty() {
                        self.end_line(out);
                    }
                    out.push(FULL_SCREEN.to_string());
                    self.alternate = true;
                }
            }
            (true, 'l') if numbers.iter().any(|&m| matches!(m, 47 | 1047 | 1049)) => {
                self.alternate = false;
                self.line.clear();
                self.col = 0;
            }
            _ if self.alternate => {}
            // Erase the end of the line (0), its start (1), or the whole line (2).
            (false, 'K') => match numbers.first().copied().unwrap_or(0) {
                0 => self.line.truncate(self.col),
                1 => (0..self.col.min(self.line.len())).for_each(|i| self.line[i] = ' '),
                _ => self.line.clear(),
            },
            (false, 'C') => self.col += n,
            (false, 'D') => self.col = self.col.saturating_sub(n),
            (false, 'G') => self.col = n - 1,
            _ => {}
        }
    }
}

/// An open log: its file and the rendering of its stream.
pub struct Journal {
    path: PathBuf,
    file: BufWriter<File>,
    render: Render,
}

impl Journal {
    pub fn open(open: &JournalOpen) -> Result<Self, String> {
        let path = PathBuf::from(&open.path);
        let shown = path.display().to_string();
        let failed = |e: std::io::Error| format!("{shown}: {}", explain(&e));
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(failed)?;
        }
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // A log can hold sensitive data: readable by its owner only.
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(failed)?;
        let fresh = file.metadata().map(|m| m.len() == 0).unwrap_or(true);
        let mut journal = Journal { path, file: BufWriter::new(file), render: Render::default() };
        let mut head = String::new();
        if fresh {
            head.push_str(&format!("{}\n\n", open.header));
        }
        if let Some(note) = &open.note {
            head.push_str(&format!("\n{note}\n\n"));
        }
        if let Some(backlog) = open.backlog.as_deref().filter(|b| !b.trim().is_empty()) {
            // The last line shown is often the prompt: it stays open, and what is typed next
            // continues it, as on the screen.
            let mut lines: Vec<&str> = backlog.trim_end_matches('\n').split('\n').collect();
            let last = lines.pop().unwrap_or_default();
            for line in lines {
                head.push_str(line.trim_end());
                head.push('\n');
            }
            journal.render.line = last.chars().collect();
            journal.render.col = journal.render.line.len();
        }
        journal.file.write_all(head.as_bytes()).map_err(failed)?;
        journal.file.flush().map_err(failed)?;
        Ok(journal)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for line in self.render.feed(bytes) {
            let _ = writeln!(self.file, "{line}");
        }
        let _ = self.file.flush();
    }

    /// Writes the current line, even unfinished (a prompt), and closes the file.
    pub fn close(mut self) -> PathBuf {
        if !self.render.line.is_empty() && !self.render.alternate {
            let text: String = self.render.line.iter().collect();
            let _ = writeln!(self.file, "{}", text.trim_end());
        }
        let _ = self.file.flush();
        self.path
    }
}

/// Why a log does not open, and what to do, when the cause is known; otherwise the text of the
/// system, as it is.
fn explain(e: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::PermissionDenied => "Access denied. Choose a folder your account can write to in the settings.".into(),
        ErrorKind::ReadOnlyFilesystem => "The disk is read-only. Choose another folder in the settings.".into(),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => "The disk is full. Free up some space, or choose another folder.".into(),
        ErrorKind::NotADirectory | ErrorKind::AlreadyExists => {
            "There is a file where the folder should be. Rename it, or choose another folder in the settings.".into()
        }
        _ => e.to_string(),
    }
}

/// The open logs, by session: `ssh:3`, `local:2`.
static JOURNALS: LazyLock<Mutex<HashMap<String, Journal>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn start(key: &str, open: &JournalOpen) -> Result<String, String> {
    let journal = Journal::open(open)?;
    let path = journal.path.display().to_string();
    let mut journals = JOURNALS.lock().map_err(|_| "The logs are unavailable. Restart Ramure.".to_string())?;
    if let Some(previous) = journals.insert(key.to_string(), journal) {
        previous.close();
    }
    Ok(path)
}

/// Gives the log of `key` what the session has just shown, if there is one.
pub fn feed(key: &str, bytes: &[u8]) {
    if let Ok(mut journals) = JOURNALS.lock() {
        if let Some(journal) = journals.get_mut(key) {
            journal.feed(bytes);
        }
    }
}

/// Closes the log of `key`; returns its path if there was one.
pub fn stop(key: &str) -> Option<String> {
    let journal = JOURNALS.lock().ok()?.remove(key)?;
    Some(journal.close().display().to_string())
}

/// The default log folder, in view: `Documents/Ramure/Logs`, or `Ramure/Logs` in the home folder
/// when the system has no Documents folder.
pub fn default_dir(documents: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    documents.or(home).map(|base| base.join("Ramure").join("Logs"))
}

/// The log folder when the setting is empty.
#[tauri::command]
pub fn journal_default_dir(app: tauri::AppHandle) -> Option<String> {
    use tauri::Manager;
    let documents = app.path().document_dir().ok();
    default_dir(documents.as_deref(), crate::ssh::config::home().as_deref()).map(|dir| dir.display().to_string())
}

/// Opens the log folder in the file manager of the system, creating it if needed.
#[tauri::command]
pub fn journal_open_dir(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    fs::create_dir_all(&dir).map_err(|e| format!("{path}: {e}"))?;
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("The file manager could not open the folder: open “{path}” by hand. ({e})"))
}

#[tauri::command]
pub fn journal_start(key: String, open: JournalOpen) -> Result<String, String> {
    start(&key, &open)
}

#[tauri::command]
pub fn journal_stop(key: String) -> Option<String> {
    stop(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(chunks: &[&[u8]]) -> Vec<String> {
        let mut r = Render::default();
        chunks.iter().flat_map(|c| r.feed(c)).collect()
    }

    #[test]
    fn removes_colors_and_titles() {
        let lines = render(&[b"\x1b]0;root@vm: ~\x07\x1b[01;32mroot@vm\x1b[00m:~# ls\r\n\x1b[0m\x1b[01;34mfolder\x1b[0m  file\r\n"]);
        assert_eq!(lines, vec!["root@vm:~# ls", "folder  file"]);
    }

    #[test]
    fn a_progress_bar_keeps_its_final_state() {
        let lines = render(&[b"Progress: 10%\rProgress: 50%\rProgress: 100%\r\n", b"done\n"]);
        assert_eq!(lines, vec!["Progress: 100%", "done"]);
        // A shorter line that erases the end of it.
        let lines = render(&[b"abcdef\r\x1b[Kxy\r\n"]);
        assert_eq!(lines, vec!["xy"]);
    }

    #[test]
    fn backspace_and_tab() {
        assert_eq!(render(&[b"wordx\x08 \x08s\r\n"]), vec!["words"]);
        assert_eq!(render(&[b"a\tb\r\n"]), vec!["a       b"]);
    }

    #[test]
    fn a_full_screen_program_leaves_only_one_line() {
        let lines = render(&[b"$ vim notes\r\n\x1b[?1049h\x1b[H\x1b[2Jvim content\r\n~\r\n", b"\x1b[?1049l$ next\r\n"]);
        assert_eq!(lines, vec!["$ vim notes", FULL_SCREEN, "$ next"]);
    }

    #[test]
    fn a_character_split_between_two_packets() {
        let text = "✓ done\n".as_bytes();
        let (a, b) = text.split_at(2); // splits the "✓"
        assert_eq!(render(&[a, b]), vec!["✓ done"]);
    }

    #[test]
    fn writes_a_readable_file_and_resumes_after_a_reconnection() {
        let dir = std::env::temp_dir().join(format!("ramure-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("Logs").join("test-server_2026-11-11_23-05-12.log");
        let open = JournalOpen {
            path: path.display().to_string(),
            header: "Log of test-server, started Nov 11, 2026 at 11:05 PM".into(),
            note: None,
            backlog: Some("root@vm:~# uptime\n 23:05:01 up 3 days\nroot@vm:~# ".into()),
        };
        start("ssh:1", &open).unwrap();
        // What is typed after the start continues the prompt already shown.
        feed("ssh:1", b"echo \x1b[1mok\x1b[0m\r\nok\r\nroot@vm:~# ");
        feed("other", b"goes nowhere\n");
        assert!(stop("ssh:1").unwrap().ends_with(".log"));
        assert_eq!(stop("ssh:1"), None);

        // Resuming after a lost connection: the same file, a line that marks the resumption.
        let resumed = JournalOpen { note: Some("— connection lost, resumed at 11:07 PM —".into()), backlog: None, ..open };
        start("ssh:2", &resumed).unwrap();
        feed("ssh:2", b"root@vm:~# exit\r\n");
        stop("ssh:2");

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Log of test-server, started Nov 11, 2026 at 11:05 PM\n\n\
             root@vm:~# uptime\n 23:05:01 up 3 days\n\
             root@vm:~# echo ok\nok\nroot@vm:~#\n\
             \n— connection lost, resumed at 11:07 PM —\n\n\
             root@vm:~# exit\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn says_why_a_log_does_not_open() {
        let dir = std::env::temp_dir().join(format!("ramure-journal-refused-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // A file in place of the log folder.
        fs::write(dir.join("Logs"), b"").unwrap();
        let open = JournalOpen { path: dir.join("Logs").join("a.log").display().to_string(), header: "x".into(), note: None, backlog: None };
        let error = start("refused", &open).unwrap_err();
        assert!(error.contains("There is a file where the folder should be"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_default_folder_is_in_view() {
        let docs = Path::new("/home/me/Documents");
        assert_eq!(default_dir(Some(docs), None).unwrap(), Path::new("/home/me/Documents/Ramure/Logs"));
        assert_eq!(default_dir(None, Some(Path::new("/home/me"))).unwrap(), Path::new("/home/me/Ramure/Logs"));
    }
}
