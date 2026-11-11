//! Écrire des hôtes du carnet dans `~/.ssh/config`, pour qu'ils s'ouvrent aussi avec `ssh <nom>`.
//!
//! Le fichier est modifié ligne à ligne : seules les lignes HostName, User, Port et IdentityFile du
//! bloc `Host <nom>` d'un hôte exporté changent, ou un bloc s'ajoute. Le reste du fichier, ses
//! commentaires, ses autres directives et ses fins de ligne, ne bouge pas. Ramure n'efface jamais
//! une ligne. L'ancien fichier est copié dans `config.ramure.bak` avant l'écriture.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use super::config;
use super::store::{Host, Store};

/// Ce que l'export ferait d'un hôte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub alias: String,
    /// `added`, `modified`, `current` ou `blocked`.
    pub status: &'static str,
    /// Ajouté : ce qui sera écrit ; modifié : ce qui change ; non exporté : pourquoi.
    pub detail: String,
}

/// Ce qu'un hôte demande au fichier.
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

    /// Les directives écrites pour un hôte ajouté, dans l'ordre d'OpenSSH.
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

    /// Ce qui sera écrit, en une ligne : `deploy@10.0.0.9 · port 2222 · clé ~/.ssh/id_ed25519`.
    fn summary(&self) -> String {
        let mut parts = vec![match &self.user {
            Some(user) => format!("{user}@{}", self.hostname),
            None => self.hostname.clone(),
        }];
        if self.port != 22 {
            parts.push(format!("port {}", self.port));
        }
        match (&self.identity_file, self.password) {
            (Some(key), _) => parts.push(format!("clé {key}")),
            (None, true) => parts.push("sans clé\u{a0}: le mot de passe n’est jamais écrit".into()),
            (None, false) => parts.push("agent SSH et clés par défaut".into()),
        }
        parts.join(" · ")
    }
}

/// Un bloc `Host` ou `Match` du fichier : ses lignes, de l'en-tête à la ligne qui précède le bloc
/// suivant.
struct Block {
    /// Les noms de l'en-tête `Host` ; vide pour un bloc `Match`.
    patterns: Vec<String>,
    header: usize,
    end: usize,
}

/// Le mot-clé d'une ligne et sa valeur, sans guillemets : `Port 2222`, `Port=2222`.
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

/// La première valeur d'une directive dans un bloc, et sa ligne.
fn value_in(lines: &[String], block: &Block, wanted: &str) -> Option<(usize, String)> {
    (block.header + 1..block.end).find_map(|i| match directive(&lines[i]) {
        Some((key, value)) if key == wanted => Some((i, value)),
        _ => None,
    })
}

/// Ce qui changerait dans le bloc d'un hôte : (directive, valeur du fichier, valeur de Ramure).
fn changes(lines: &[String], block: &Block, wanted: &Wanted) -> Vec<(&'static str, Option<String>, String)> {
    let mut out = Vec::new();
    let hostname = value_in(lines, block, "hostname").map(|(_, v)| v);
    // Sans HostName, ssh se connecte au nom lui-même.
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

/// Pourquoi un hôte ne peut pas être écrit, s'il ne peut pas l'être.
fn blocked(host: &Host, wanted: &Wanted, lines: &[String], all: &[Block], taken: &[String]) -> Option<String> {
    if wanted.alias.chars().any(char::is_whitespace) {
        return Some("Son nom contient une espace, que ssh lirait comme deux noms\u{a0}: renomme l’hôte.".into());
    }
    if wanted.alias.contains(['*', '?', '!', '"', '#']) {
        return Some("Son nom contient un caractère que ssh lirait comme un motif\u{a0}: renomme l’hôte.".into());
    }
    if host.proxy_jump {
        return Some("Il passe par un bastion, dont Ramure ne garde pas l’adresse\u{a0}: écris son entrée à la main, avec ProxyJump.".into());
    }
    if taken.contains(&wanted.alias) {
        return Some("Un autre hôte du carnet porte ce nom\u{a0}: renomme l’un des deux.".into());
    }
    let shared = all.iter().find(|b| b.patterns.len() > 1 && b.patterns.contains(&wanted.alias));
    if let Some(block) = shared {
        let header = lines[block.header].trim();
        return Some(format!(
            "Son nom est dans le bloc «\u{a0}{header}\u{a0}» du fichier, avec d’autres\u{a0}: modifie ce bloc à la main."
        ));
    }
    None
}

/// Le bloc propre à un nom : `Host <nom>`, seul sur sa ligne. Le premier, comme pour ssh.
fn own<'a>(all: &'a [Block], alias: &str) -> Option<&'a Block> {
    all.iter().find(|b| b.patterns.len() == 1 && b.patterns[0] == alias)
}

/// Le plan de l'export : une ligne par hôte, dans l'ordre du carnet.
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
                            .map(|(key, from, to)| format!("{key} {} → {to}", from.as_deref().unwrap_or("(absent)")))
                            .collect();
                        row("modified", said.join(" · "))
                    }
                }
            }
        })
        .collect()
}

/// Les lignes du fichier, et sa fin de ligne : CRLF s'il en a, comme sous Windows.
fn split(text: &str) -> (Vec<String>, &'static str) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let body = text.strip_suffix('\n').unwrap_or(text);
    let lines = if text.is_empty() { Vec::new() } else { body.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect() };
    (lines, eol)
}

fn quoted(value: &str) -> String {
    if value.chars().any(char::is_whitespace) { format!("\"{value}\"") } else { value.to_string() }
}

/// Le texte du fichier une fois les hôtes `chosen` écrits. Ceux que le plan ne dit ni ajoutés ni
/// modifiés sont laissés de côté.
pub fn apply(text: &str, hosts: &[Host], chosen: &[String]) -> (String, usize) {
    let rows = plan(text, hosts);
    let (mut lines, eol) = split(text);
    let picked: Vec<&Host> = hosts
        .iter()
        .filter(|h| chosen.contains(&h.id))
        .filter(|h| rows.iter().any(|r| r.id == h.id && matches!(r.status, "added" | "modified")))
        .collect();

    // Les blocs modifiés, du bas vers le haut : les numéros de ligne au-dessus ne bougent pas.
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
                    // La ligne garde son retrait et l'orthographe de son mot-clé.
                    let line = &lines[i];
                    let lead = &line[..line.len() - line.trim_start().len()];
                    let word = line.trim_start().split(|c: char| c.is_whitespace() || c == '=').next().unwrap_or(key);
                    lines[i] = format!("{lead}{word} {}", quoted(&value));
                }
                None => inserts.push(format!("{indent}{key} {}", quoted(&value))),
            }
        }
        // Les directives manquantes, après la dernière ligne non vide du bloc.
        let mut at = block.end;
        while at > block.header + 1 && lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        for (k, line) in inserts.into_iter().enumerate() {
            lines.insert(at + k, line);
        }
    }

    // Les blocs ajoutés, avant `Host *` s'il y en a un : la première valeur trouvée l'emporte.
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
    config::home().map(|home| home.join(".ssh").join("config")).ok_or_else(|| "Dossier personnel introuvable\u{a0}: ni USERPROFILE (Windows) ni HOME (Linux) n’est défini. Définis-en un, puis relance Ramure.".to_string())
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name("config.ramure.bak")
}

fn read(path: &Path) -> Result<String, String> {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| {
            "~/.ssh/config n’est pas en UTF-8\u{a0}: enregistre-le en UTF-8 dans un éditeur, puis recommence.".to_string()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("~/.ssh/config n’a pas pu être lu\u{a0}: vérifie que ton compte peut le lire, puis rouvre la boîte. ({e})")),
    }
}

/// Écrit `text` à la place de `path`, par un fichier voisin renommé : un fichier à moitié écrit
/// n'est jamais lu par ssh.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let failed = |e: std::io::Error| {
        format!("~/.ssh/config n’a pas pu être écrit\u{a0}: vérifie que ni le fichier ni ~/.ssh ne sont en lecture seule, puis recommence. ({e})")
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(failed)?;
    }
    let temporary = path.with_file_name("config.ramure.tmp");
    fs::write(&temporary, text).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // ssh refuse un fichier que d'autres peuvent écrire : celui d'avant garde ses droits.
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
    /// La copie de l'ancien fichier, s'il y en avait un.
    pub backup: Option<String>,
    /// Le texte écrit : l'annulation ne remet l'ancien fichier que s'il n'a pas changé depuis.
    pub written: String,
}

pub fn export_to(path: &Path, hosts: &[Host], chosen: &[String]) -> Result<Exported, String> {
    let before = read(path)?;
    let (text, count) = apply(&before, hosts, chosen);
    if count == 0 {
        return Err("Aucun des hôtes cochés n’est à écrire\u{a0}: le fichier a changé entre-temps. Ferme la boîte et rouvre-la.".into());
    }
    let backup = if path.exists() {
        let copy = backup_path(path);
        fs::copy(path, &copy).map_err(|e| format!("L’ancien fichier n’a pas pu être copié, et rien n’a été écrit\u{a0}: vérifie que ~/.ssh n’est pas en lecture seule, puis recommence. ({e})"))?;
        Some(copy.display().to_string())
    } else {
        None
    };
    write(path, &text)?;
    Ok(Exported { count, backup, written: text })
}

pub fn undo_at(path: &Path, written: &str, had_backup: bool) -> Result<(), String> {
    if read(path)? != written {
        return Err("~/.ssh/config a changé depuis l’export\u{a0}: il est resté tel quel. L’ancien fichier est dans ~/.ssh/config.ramure.bak.".into());
    }
    if had_backup {
        let old = fs::read_to_string(backup_path(path)).map_err(|e| {
            format!("~/.ssh/config.ramure.bak n’a pas pu être lu\u{a0}: remets-le à la place de ~/.ssh/config à la main. ({e})")
        })?;
        write(path, &old)
    } else {
        fs::remove_file(path).map_err(|e| format!("~/.ssh/config n’a pas pu être retiré\u{a0}: supprime-le à la main. ({e})"))
    }
}

/// Les hôtes du carnet à considérer : tous, ou ceux de `ids`.
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
    // Tout le carnet, pour que deux hôtes du même nom se voient l'un l'autre.
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
# Mes serveurs
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
    fn le_plan_dit_ajoute_modifie_a_jour_et_non_exporte() {
        let mut key = host("4", "neuf", "10.0.0.9", "deploy", 2222);
        key.auth = "keyFile".into();
        key.key_path = "~/.ssh/id_ed25519".into();
        let mut jump = host("5", "interne", "10.0.2.15", "", 22);
        jump.proxy_jump = true;
        let hosts = vec![
            host("1", "vps", "vps.example.net", "deploy", 2222),
            host("2", "web", "web.example.net", "", 22),
            host("3", "atelier (copie)", "a.example.net", "", 22),
            key,
            jump,
            host("6", "vps", "autre.example.net", "", 22),
        ];
        let rows = plan(FILE, &hosts);
        assert_eq!(status(&rows, "vps"), ("modified", "User admin → deploy · Port (absent) → 2222".into()));
        assert_eq!(status(&rows, "web").0, "blocked");
        assert!(status(&rows, "web").1.contains("Host web prod"));
        assert_eq!(status(&rows, "atelier (copie)").0, "blocked");
        assert_eq!(status(&rows, "neuf"), ("added", "deploy@10.0.0.9 · port 2222 · clé ~/.ssh/id_ed25519".into()));
        assert!(status(&rows, "interne").1.contains("bastion"));
        // Le second « vps » du carnet : bloqué, le premier garde le nom.
        assert_eq!(rows[5].status, "blocked");

        let same = vec![host("1", "vps", "vps.example.net", "admin", 22)];
        assert_eq!(plan(FILE, &same)[0].status, "current");
    }

    #[test]
    fn l_ecriture_ne_touche_que_les_lignes_de_ramure() {
        let hosts = vec![host("1", "vps", "vps.example.net", "deploy", 2222), host("2", "neuf", "10.0.0.9", "", 22)];
        let (text, count) = apply(FILE, &hosts, &["1".into(), "2".into()]);
        assert_eq!(count, 2);
        assert_eq!(
            text,
            "\
# Mes serveurs
Include ~/.ssh/config.d/*

Host vps
    HostName vps.example.net
    User deploy
    ForwardAgent yes
    Port 2222

Host web prod
    HostName web.example.net

Host neuf
    HostName 10.0.0.9

Host *
    ServerAliveInterval 30
"
        );
        // Une seconde fois : tout est à jour, rien ne change.
        assert!(plan(&text, &hosts).iter().all(|r| r.status == "current"));
        assert_eq!(apply(&text, &hosts, &["1".into(), "2".into()]).1, 0);
    }

    #[test]
    fn garde_les_fins_de_ligne_les_retraits_et_les_mots_cles() {
        let file = "Host vps\r\n\thostname=old.example.net\r\n\tuser admin\r\n";
        let hosts = vec![host("1", "vps", "new.example.net", "admin", 22)];
        let (text, _) = apply(file, &hosts, &["1".into()]);
        assert_eq!(text, "Host vps\r\n\thostname new.example.net\r\n\tuser admin\r\n");
        // Un hôte non coché n'est pas écrit.
        assert_eq!(apply(file, &hosts, &[]).0, file);
    }

    #[test]
    fn ajoute_a_un_fichier_vide_ou_sans_bloc_general() {
        let mut key = host("1", "neuf", "10.0.0.9", "moi", 22);
        key.auth = "keyFile".into();
        key.key_path = "C:\\Users\\Jean Dupont\\.ssh\\id_ed25519".into();
        let (text, _) = apply("", &[key.clone()], &["1".into()]);
        assert_eq!(
            text,
            "Host neuf\n    HostName 10.0.0.9\n    User moi\n    IdentityFile \"C:\\Users\\Jean Dupont\\.ssh\\id_ed25519\"\n"
        );
        let (text, _) = apply("Host a\n    HostName a.net", &[key], &["1".into()]);
        assert!(text.starts_with("Host a\n    HostName a.net\n\nHost neuf\n"));
    }

    #[test]
    fn exporte_copie_l_ancien_fichier_et_annule() {
        let dir = std::env::temp_dir().join(format!("ramure-export-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(".ssh").join("config");
        let hosts = vec![host("1", "neuf", "10.0.0.9", "", 22)];

        // Sans fichier : il est créé, et l'annulation le retire.
        let done = export_to(&path, &hosts, &["1".into()]).unwrap();
        assert_eq!(done.count, 1);
        assert!(done.backup.is_none());
        assert!(fs::read_to_string(&path).unwrap().contains("Host neuf"));
        undo_at(&path, &done.written, false).unwrap();
        assert!(!path.exists());

        // Avec un fichier : la copie le garde, l'annulation le remet.
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, FILE).unwrap();
        let done = export_to(&path, &hosts, &["1".into()]).unwrap();
        assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), FILE);
        // Un fichier retouché depuis l'export n'est pas remplacé.
        fs::write(&path, format!("{}# retouche\n", done.written)).unwrap();
        assert!(undo_at(&path, &done.written, true).unwrap_err().contains("a changé"));
        fs::write(&path, &done.written).unwrap();
        undo_at(&path, &done.written, true).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), FILE);
        // Rien à écrire : refusé, rien ne bouge.
        let current = fs::read_to_string(&path).unwrap();
        assert!(export_to(&path, &hosts, &[]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), current);
        let _ = fs::remove_dir_all(&dir);
    }
}
