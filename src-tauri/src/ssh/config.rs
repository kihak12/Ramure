//! Lecture de `~/.ssh/config`, pour importer ses hôtes dans le carnet : Host, HostName,
//! User, Port, IdentityFile, et la présence d'un rebond par bastion.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEntry {
    pub alias: String,
    pub hostname: String,
    pub user: Option<String>,
    pub port: u16,
    pub identity_file: Option<String>,
    /// L'hôte passe par un bastion : pas encore pris en charge.
    pub proxy_jump: bool,
}

pub fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

/// Remplace un `~/` initial par le dossier personnel.
pub fn expand(path: &str) -> PathBuf {
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    match (rest, home()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

pub fn parse(text: &str) -> Vec<HostEntry> {
    let mut hosts: Vec<HostEntry> = Vec::new();
    // Indices des hôtes déclarés par le bloc `Host` en cours.
    let mut current: Vec<usize> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        let value = value.trim().trim_start_matches('=').trim().trim_matches('"');

        match key.to_ascii_lowercase().as_str() {
            "host" => {
                current.clear();
                for alias in value.split_whitespace() {
                    // Les motifs (`*`, `?`, `!`) ne sont pas des hôtes auxquels on se connecte.
                    if alias.contains(['*', '?', '!']) {
                        continue;
                    }
                    hosts.push(HostEntry {
                        alias: alias.to_string(),
                        hostname: alias.to_string(),
                        port: 22,
                        ..Default::default()
                    });
                    current.push(hosts.len() - 1);
                }
            }
            "match" => current.clear(),
            "hostname" => current.iter().for_each(|&i| hosts[i].hostname = value.to_string()),
            "user" => current.iter().for_each(|&i| hosts[i].user = Some(value.to_string())),
            "port" => {
                if let Ok(port) = value.parse() {
                    current.iter().for_each(|&i| hosts[i].port = port);
                }
            }
            "identityfile" => current.iter().for_each(|&i| {
                hosts[i].identity_file.get_or_insert_with(|| value.to_string());
            }),
            "proxyjump" | "proxycommand" => current.iter().for_each(|&i| hosts[i].proxy_jump = true),
            _ => {}
        }
    }
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_read_and_patterns_skipped() {
        let text = "\
# commentaire
Host *
    ServerAliveInterval 30

Host vps prod-web
    HostName vps.example.net
    User deploy
    Port 2222
    IdentityFile ~/.ssh/id_ed25519
    IdentityFile ~/.ssh/autre

Host interne
    HostName=10.0.2.15
    ProxyJump bastion
";
        let hosts = parse(text);
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0].alias, "vps");
        assert_eq!(hosts[1].alias, "prod-web");
        assert_eq!(hosts[0].hostname, "vps.example.net");
        assert_eq!(hosts[1].user.as_deref(), Some("deploy"));
        assert_eq!(hosts[0].port, 2222);
        assert_eq!(hosts[0].identity_file.as_deref(), Some("~/.ssh/id_ed25519"));
        assert_eq!(hosts[2].hostname, "10.0.2.15");
        assert!(hosts[2].proxy_jump);
        assert_eq!(hosts[2].port, 22);
    }
}
