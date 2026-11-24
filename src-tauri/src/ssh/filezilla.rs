//! The sites of a FileZilla export (File, Export) or of its Site Manager (`sitemanager.xml`), as
//! hosts of the book (guidelines, 9.15, "Importing and exporting hosts").
//!
//! An SFTP site becomes a host; an FTP or FTPS site is listed, not imported: Ramure opens SSH and
//! SFTP only. A password FileZilla kept in clear (base64) is read, and kept in the system keyring
//! only when the user asks; one it kept encrypted with a master password cannot be read.

use base64::Engine;

use super::hostfile::{Found, plan_key};
use super::store::Host;

/// Where the hosts of a FileZilla export land.
pub const GROUP: &str = "Imported from FileZilla";

/// The text of the child `name` of `node`, trimmed.
fn child<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    node.children().find(|c| c.has_tag_name(name)).and_then(|c| c.text()).map(str::trim).filter(|t| !t.is_empty())
}

/// The sites of a FileZilla export, or why it is not one.
pub fn read(text: &str) -> Result<Vec<Found>, String> {
    let not_ours = || "This is not a FileZilla export: in FileZilla, use File, then Export, and choose the file it writes.".to_string();
    let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')).map_err(|_| not_ours())?;
    let root = doc.root_element();
    if !root.has_tag_name("FileZilla3") {
        return Err(not_ours());
    }
    let Some(servers) = root.children().find(|c| c.has_tag_name("Servers")) else {
        return Err(not_ours());
    };
    // Every site, in the order of the file, those of the folders included.
    Ok(servers.descendants().filter(|n| n.has_tag_name("Server")).map(site).collect())
}

/// A site of FileZilla as a host of the book.
fn site(node: roxmltree::Node) -> Found {
    let hostname = child(node, "Host").unwrap_or_default().to_string();
    let alias = child(node, "Name").map(str::to_string).unwrap_or_else(|| hostname.clone());
    let port = child(node, "Port").and_then(|p| p.parse().ok()).unwrap_or(22);
    let user = child(node, "User").unwrap_or_default().to_string();
    let mut host = Host { alias, hostname, port, user, auth: "auto".into(), group: GROUP.into(), ..Host::default() };

    // 1 is SFTP; 0, 3, 4 and 5 are FTP in its forms.
    let protocol = child(node, "Protocol").unwrap_or("0");
    let refused = match protocol {
        "1" => None,
        "0" | "3" | "4" | "5" => Some("FTP is not supported: Ramure opens SSH and SFTP.".to_string()),
        _ => Some("This protocol is not supported: Ramure opens SSH and SFTP.".to_string()),
    }
    .or_else(|| host.hostname.is_empty().then(|| "It has no address.".to_string()));

    // How it signs in: 1 normal, 2 ask, 3 interactive, 4 account, 5 key file.
    let mut password = None;
    let mut key = None;
    match child(node, "Logontype").unwrap_or("1") {
        "5" => {
            let path = child(node, "Keyfile").unwrap_or_default().to_string();
            host.auth = "keyFile".into();
            host.key_path = path.clone();
            key = plan_key(&path);
        }
        "1" | "4" => {
            host.auth = "password".into();
            let pass = node.children().find(|c| c.has_tag_name("Pass"));
            // A password in clear, written in base64; "crypt" is encrypted by a master password.
            if let Some(pass) = pass.filter(|p| p.attribute("encoding") == Some("base64")) {
                password = pass
                    .text()
                    .and_then(|t| base64::engine::general_purpose::STANDARD.decode(t.trim()).ok())
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .filter(|p| !p.is_empty());
            } else if let Some(text) = pass.and_then(|p| p.text()).map(str::trim).filter(|t| !t.is_empty()) {
                if pass.and_then(|p| p.attribute("encoding")).is_none() {
                    password = Some(text.to_string());
                }
            }
        }
        "2" | "3" => host.auth = "password".into(),
        _ => {}
    }
    Found { host, refused, password, key }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>
<FileZilla3 version="3.66.4" platform="windows">
  <Servers>
    <Server>
      <Host>files.example.com</Host>
      <Port>22</Port>
      <Protocol>1</Protocol>
      <Type>0</Type>
      <User>alice</User>
      <Pass encoding="base64">czNjcmV0</Pass>
      <Logontype>1</Logontype>
      <Name>Files</Name>
    </Server>
    <Folder expanded="1">Clients
      <Server>
        <Host>ftp.acme.example</Host>
        <Port>21</Port>
        <Protocol>0</Protocol>
        <User>acme</User>
        <Logontype>1</Logontype>
        <Name>Acme FTP</Name>
      </Server>
      <Server>
        <Host>10.0.0.9</Host>
        <Port>2222</Port>
        <Protocol>1</Protocol>
        <User>deploy</User>
        <Logontype>5</Logontype>
        <Keyfile>/nowhere/id_deploy.ppk</Keyfile>
        <Name>Acme web</Name>
      </Server>
      <Server>
        <Host>vault.acme.example</Host>
        <Protocol>1</Protocol>
        <User>ops</User>
        <Pass encoding="crypt" pubkey="abc">xyz</Pass>
        <Logontype>1</Logontype>
        <Name>Vault</Name>
      </Server>
    </Folder>
  </Servers>
</FileZilla3>"#;

    #[test]
    fn the_sftp_sites_become_hosts_and_the_ftp_ones_are_refused() {
        let found = read(EXPORT).unwrap();
        assert_eq!(found.len(), 4);
        let files = &found[0];
        assert_eq!((files.host.alias.as_str(), files.host.hostname.as_str(), files.host.port), ("Files", "files.example.com", 22));
        assert_eq!((files.host.user.as_str(), files.host.auth.as_str(), files.host.group.as_str()), ("alice", "password", GROUP));
        assert_eq!(files.password.as_deref(), Some("s3cret"));
        assert!(files.refused.is_none());

        assert_eq!(found[1].refused.as_deref(), Some("FTP is not supported: Ramure opens SSH and SFTP."));

        let web = &found[2];
        assert_eq!((web.host.port, web.host.auth.as_str(), web.host.key_path.as_str()), (2222, "keyFile", "/nowhere/id_deploy.ppk"));
        assert!(matches!(web.key, Some(super::super::hostfile::KeyPlan::Missing { .. })));

        // A password encrypted with a master password cannot be read: it is asked when connecting.
        assert_eq!((found[3].host.auth.as_str(), found[3].password.as_deref()), ("password", None));
    }

    #[test]
    fn another_file_is_not_a_filezilla_export() {
        assert!(read("{\"format\":\"ramure-hosts\"}").unwrap_err().starts_with("This is not a FileZilla export"));
        assert!(read("<Other><Servers/></Other>").unwrap_err().starts_with("This is not a FileZilla export"));
    }
}
