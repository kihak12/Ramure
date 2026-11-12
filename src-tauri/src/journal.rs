//! Journal d'une session de terminal : ce qui s'est affiché, en texte lisible, dans un fichier.
//!
//! Le flux d'une session (SSH ou locale) passe ici avant l'interface : rien ne fait l'aller et
//! retour. Seul ce qui s'affiche est écrit ; ce que l'utilisateur tape n'est jamais enregistré à
//! part, et un mot de passe tapé sans écho n'y paraît donc pas.
//!
//! Le texte est rendu ligne par ligne, comme un terminal le ferait sans bouger le curseur : les
//! séquences de contrôle sont retirées, un retour chariot réécrit la ligne (une barre de
//! progression garde son état final), un retour arrière recule. Un programme plein écran (vim,
//! htop, less) ne laisse qu'une ligne : son affichage ne se lit pas comme du texte.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;

/// La ligne qui remplace l'affichage d'un programme plein écran.
const FULL_SCREEN: &str = "[affichage plein écran omis]";

/// Ce qu'il faut pour ouvrir ou reprendre un journal.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalOpen {
    pub path: String,
    /// Première ligne d'un fichier neuf : « Journal de serveur-test, commencé le … ».
    pub header: String,
    /// Une ligne à écrire avant la suite : la reprise après une reconnexion.
    pub note: Option<String>,
    /// Ce que le terminal montrait déjà, quand on commence en cours de session.
    pub backlog: Option<String>,
}

/// Où en est l'analyse d'une séquence d'échappement.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Escape {
    None,
    /// Après ESC.
    Start,
    /// Dans `ESC [ … final`.
    Csi,
    /// Dans `ESC ] …` ou `ESC P …`, jusqu'à BEL ou `ESC \`.
    Text,
    /// Un ESC dans une séquence de texte : peut-être sa fin.
    TextEsc,
    /// `ESC (` et consorts : un octet de plus, puis fin.
    Charset,
}

/// Le texte rendu d'un flux de terminal.
#[derive(Debug)]
pub struct Render {
    line: Vec<char>,
    col: usize,
    escape: Escape,
    params: String,
    /// Octets d'un caractère UTF-8 coupé entre deux paquets.
    pending: Vec<u8>,
    /// Un programme plein écran occupe l'écran : rien n'est écrit.
    alternate: bool,
}

impl Default for Render {
    fn default() -> Self {
        Render { line: Vec::new(), col: 0, escape: Escape::None, params: String::new(), pending: Vec::new(), alternate: false }
    }
}

impl Render {
    /// Rend `bytes` ; renvoie les lignes terminées, sans leur fin de ligne.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut data = std::mem::take(&mut self.pending);
        data.extend_from_slice(bytes);
        let text = match std::str::from_utf8(&data) {
            Ok(text) => text.to_string(),
            Err(e) if e.error_len().is_none() => {
                // La fin du paquet coupe un caractère : il attend le paquet suivant.
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
            // Écran alternatif : un programme plein écran prend la main, puis la rend.
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
            // Effacer la fin de la ligne (0), son début (1), ou toute la ligne (2).
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

/// Un journal ouvert : son fichier et le rendu de son flux.
pub struct Journal {
    path: PathBuf,
    file: BufWriter<File>,
    render: Render,
}

impl Journal {
    pub fn open(open: &JournalOpen) -> Result<Self, String> {
        let path = PathBuf::from(&open.path);
        let shown = path.display().to_string();
        let failed = |e: std::io::Error| format!("{shown}\u{a0}: {e}");
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(failed)?;
        }
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Un journal peut contenir des données sensibles : lisible par son seul propriétaire.
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
            // La dernière ligne affichée est souvent l'invite : elle reste ouverte, et ce qui sera
            // tapé ensuite la continue, comme à l'écran.
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

    /// Écrit la ligne en cours, même inachevée (une invite), et ferme le fichier.
    pub fn close(mut self) -> PathBuf {
        if !self.render.line.is_empty() && !self.render.alternate {
            let text: String = self.render.line.iter().collect();
            let _ = writeln!(self.file, "{}", text.trim_end());
        }
        let _ = self.file.flush();
        self.path
    }
}

/// Les journaux ouverts, par session : `ssh:3`, `local:2`.
static JOURNALS: LazyLock<Mutex<HashMap<String, Journal>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn start(key: &str, open: &JournalOpen) -> Result<String, String> {
    let journal = Journal::open(open)?;
    let path = journal.path.display().to_string();
    let mut journals = JOURNALS.lock().map_err(|_| "journaux indisponibles".to_string())?;
    if let Some(previous) = journals.insert(key.to_string(), journal) {
        previous.close();
    }
    Ok(path)
}

/// Donne au journal de `key` ce que la session vient d'afficher, s'il y en a un.
pub fn feed(key: &str, bytes: &[u8]) {
    if let Ok(mut journals) = JOURNALS.lock() {
        if let Some(journal) = journals.get_mut(key) {
            journal.feed(bytes);
        }
    }
}

/// Ferme le journal de `key` ; renvoie son chemin s'il y en avait un.
pub fn stop(key: &str) -> Option<String> {
    let journal = JOURNALS.lock().ok()?.remove(key)?;
    Some(journal.close().display().to_string())
}

/// Le dossier des journaux par défaut, visible : `Documents/Ramure/Journaux`, ou le dossier
/// personnel quand le système n'a pas de dossier Documents.
pub fn default_dir(documents: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    documents.or(home).map(|base| base.join("Ramure").join("Journaux"))
}

/// Le dossier des journaux quand le réglage est vide.
#[tauri::command]
pub fn journal_default_dir(app: tauri::AppHandle) -> Option<String> {
    use tauri::Manager;
    let documents = app.path().document_dir().ok();
    default_dir(documents.as_deref(), crate::ssh::config::home().as_deref()).map(|dir| dir.display().to_string())
}

/// Ouvre le dossier des journaux dans le gestionnaire de fichiers du système, en le créant au besoin.
#[tauri::command]
pub fn journal_open_dir(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    fs::create_dir_all(&dir).map_err(|e| format!("{path}\u{a0}: {e}"))?;
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
        .map_err(|e| format!("Le gestionnaire de fichiers n’a pas pu s’ouvrir\u{a0}: ouvre {path} à la main. ({e})"))
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
    fn retire_les_couleurs_et_les_titres() {
        let lines = render(&[b"\x1b]0;root@vm: ~\x07\x1b[01;32mroot@vm\x1b[00m:~# ls\r\n\x1b[0m\x1b[01;34mdossier\x1b[0m  fichier\r\n"]);
        assert_eq!(lines, vec!["root@vm:~# ls", "dossier  fichier"]);
    }

    #[test]
    fn une_barre_de_progression_garde_son_etat_final() {
        let lines = render(&[b"Progression : 10%\rProgression : 50%\rProgression : 100%\r\n", b"fini\n"]);
        assert_eq!(lines, vec!["Progression : 100%", "fini"]);
        // Une ligne plus courte qui en efface la fin.
        let lines = render(&[b"abcdef\r\x1b[Kxy\r\n"]);
        assert_eq!(lines, vec!["xy"]);
    }

    #[test]
    fn retour_arriere_et_tabulation() {
        assert_eq!(render(&[b"motx\x08 \x08s\r\n"]), vec!["mots"]);
        assert_eq!(render(&[b"a\tb\r\n"]), vec!["a       b"]);
    }

    #[test]
    fn un_programme_plein_ecran_ne_laisse_qu_une_ligne() {
        let lines = render(&[b"$ vim notes\r\n\x1b[?1049h\x1b[H\x1b[2Jcontenu de vim\r\n~\r\n", b"\x1b[?1049l$ suite\r\n"]);
        assert_eq!(lines, vec!["$ vim notes", FULL_SCREEN, "$ suite"]);
    }

    #[test]
    fn un_caractere_coupe_entre_deux_paquets() {
        let text = "déjà fait\n".as_bytes();
        let (a, b) = text.split_at(2); // coupe le « é »
        assert_eq!(render(&[a, b]), vec!["déjà fait"]);
    }

    #[test]
    fn ecrit_un_fichier_lisible_et_reprend_apres_une_reconnexion() {
        let dir = std::env::temp_dir().join(format!("ramure-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("Journaux").join("serveur-test_2026-11-11_23-05-12.log");
        let open = JournalOpen {
            path: path.display().to_string(),
            header: "Journal de serveur-test, commencé le 11 nov. 2026 à 23:05".into(),
            note: None,
            backlog: Some("root@vm:~# uptime\n 23:05:01 up 3 days\nroot@vm:~# ".into()),
        };
        start("ssh:1", &open).unwrap();
        // Ce qui est tapé après le début continue l'invite déjà affichée.
        feed("ssh:1", b"echo \x1b[1mok\x1b[0m\r\nok\r\nroot@vm:~# ");
        feed("autre", b"ne va nulle part\n");
        assert!(stop("ssh:1").unwrap().ends_with(".log"));
        assert_eq!(stop("ssh:1"), None);

        // Reprise après une connexion perdue : même fichier, une ligne de reprise.
        let resumed = JournalOpen { note: Some("— connexion perdue, reprise à 23:07 —".into()), backlog: None, ..open };
        start("ssh:2", &resumed).unwrap();
        feed("ssh:2", b"root@vm:~# exit\r\n");
        stop("ssh:2");

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Journal de serveur-test, commencé le 11 nov. 2026 à 23:05\n\n\
             root@vm:~# uptime\n 23:05:01 up 3 days\n\
             root@vm:~# echo ok\nok\nroot@vm:~#\n\
             \n— connexion perdue, reprise à 23:07 —\n\n\
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
    fn dossier_par_defaut_visible() {
        let docs = Path::new("/home/moi/Documents");
        assert_eq!(default_dir(Some(docs), None).unwrap(), Path::new("/home/moi/Documents/Ramure/Journaux"));
        assert_eq!(default_dir(None, Some(Path::new("/home/moi"))).unwrap(), Path::new("/home/moi/Ramure/Journaux"));
    }
}
