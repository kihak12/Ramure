//! Paths, as each place writes them: those of this machine follow its system, those of a server
//! are always written with "/" (SFTP).

use std::path::Path;

/// A path of this machine as it is shown: without the `\\?\` prefix that Windows puts before
/// a resolved path.
pub fn local_display(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text.to_string()
    }
}

/// The folder above `path` on this machine; none at the root of a disk.
pub fn local_parent(path: &str) -> Option<String> {
    Path::new(path).parent().map(|parent| parent.to_string_lossy().to_string()).filter(|p| !p.is_empty())
}

pub fn local_join(dir: &str, name: &str) -> String {
    Path::new(dir).join(name).to_string_lossy().to_string()
}

pub fn local_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string())
}

pub fn remote_join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') { format!("{dir}{name}") } else { format!("{dir}/{name}") }
}

/// The folder above `path` on a server; none at the root.
pub fn remote_parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".into()),
        Some(at) => Some(trimmed[..at].to_string()),
        None => None,
    }
}

pub fn remote_name(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    trimmed.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or(path).to_string()
}

/// A path typed for a server: `~` is the home folder, which SFTP reaches with a relative path.
pub fn remote_tilde(path: &str) -> String {
    match path {
        "" | "~" | "~/" => ".".into(),
        _ => match path.strip_prefix("~/") {
            Some(rest) => rest.to_string(),
            None => path.to_string(),
        },
    }
}

/// The name a file is written under before it takes its own: hidden, beside it.
pub fn part_name(name: &str) -> String {
    format!(".{name}.ramure-part")
}

/// A name for a new folder or a rename: one name, not a path.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Enter a name.".into());
    }
    if name == "." || name == ".." {
        return Err(format!("“{name}” cannot be used as a name."));
    }
    if let Some(c) = name.chars().find(|c| matches!(c, '/' | '\\' | '\0')) {
        return Err(format!("A name cannot contain “{c}”."));
    }
    Ok(())
}

/// True if `dest` is `source` or lies inside it: a folder cannot go into itself.
pub fn inside(source: &str, dest: &str, local: bool) -> bool {
    if local {
        if cfg!(windows) {
            return Path::new(&dest.to_lowercase()).starts_with(source.to_lowercase());
        }
        return Path::new(dest).starts_with(source);
    }
    let source = source.trim_end_matches('/');
    dest == source || dest.starts_with(&format!("{source}/")) || (source.is_empty() && dest.starts_with('/'))
}

/// The two paths of a rename are on two disks: the system cannot move one there.
pub fn crosses_devices(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::CrossesDevices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_path_has_a_parent_until_the_root() {
        assert_eq!(remote_parent("/var/www/site"), Some("/var/www".into()));
        assert_eq!(remote_parent("/var/www/"), Some("/var".into()));
        assert_eq!(remote_parent("/var"), Some("/".into()));
        assert_eq!(remote_parent("/"), None);
    }

    #[test]
    fn remote_paths_join_with_a_slash() {
        assert_eq!(remote_join("/", "etc"), "/etc");
        assert_eq!(remote_join("/home/deploy", "app"), "/home/deploy/app");
        assert_eq!(remote_name("/home/deploy/app"), "app");
        assert_eq!(remote_name("/home/deploy/app/"), "app");
        assert_eq!(remote_name("/"), "/");
    }

    #[test]
    fn the_tilde_is_the_home_folder_on_a_server() {
        assert_eq!(remote_tilde(""), ".");
        assert_eq!(remote_tilde("~"), ".");
        assert_eq!(remote_tilde("~/logs"), "logs");
        assert_eq!(remote_tilde("/srv"), "/srv");
    }

    #[test]
    fn a_name_is_not_a_path() {
        assert!(check_name("report.txt").is_ok());
        assert!(check_name(".config").is_ok());
        assert_eq!(check_name(""), Err("Enter a name.".into()));
        assert!(check_name("..").is_err());
        assert_eq!(check_name("a/b"), Err("A name cannot contain “/”.".into()));
        assert!(check_name("a\\b").is_err());
    }

    #[test]
    fn a_folder_cannot_go_into_itself() {
        assert!(inside("/srv/app", "/srv/app", false));
        assert!(inside("/srv/app", "/srv/app/build", false));
        assert!(!inside("/srv/app", "/srv/application", false));
        assert!(!inside("/srv/app", "/srv", false));
        assert!(inside("/home/me/a", "/home/me/a/b", true));
        assert!(!inside("/home/me/a", "/home/me/ab", true));
    }

    #[test]
    fn the_verbatim_prefix_of_windows_is_not_shown() {
        assert_eq!(local_display(Path::new(r"\\?\C:\Users\me")), r"C:\Users\me");
        assert_eq!(local_display(Path::new(r"\\?\UNC\nas\share")), r"\\nas\share");
        assert_eq!(local_display(Path::new("/home/me")), "/home/me");
    }

    #[cfg(unix)]
    #[test]
    fn a_local_path_has_a_parent_until_the_root() {
        assert_eq!(local_parent("/home/me"), Some("/home".into()));
        assert_eq!(local_parent("/home"), Some("/".into()));
        assert_eq!(local_parent("/"), None);
        assert_eq!(local_join("/home", "me"), "/home/me");
        assert_eq!(local_name("/home/me"), "me");
    }
}
