//! What Ramure asks of the system outside its own windows: showing a folder in its file manager.

use std::path::{Path, PathBuf};

/// Shows `dir` in the file manager of the system: File Explorer on Windows, `open` on macOS, the
/// default file manager elsewhere (`xdg-open`). The error says what to do: open it by hand.
pub fn show_folder(dir: &Path) -> Result<(), String> {
    let (program, target) = if cfg!(windows) {
        // File Explorer reads a path with forward slashes as a name: it opens Documents instead.
        ("explorer", dir.display().to_string().replace('/', "\\"))
    } else if cfg!(target_os = "macos") {
        ("open", dir.display().to_string())
    } else {
        ("xdg-open", dir.display().to_string())
    };
    std::process::Command::new(program)
        .arg(&target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("The file manager could not open the folder: open “{}” by hand. ({e})", dir.display()))
}

/// Opens a folder that exists, such as that of a repository, in the file manager (guidelines, 9.2).
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("The folder “{path}” no longer exists. It may have been moved or deleted."));
    }
    show_folder(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_folder_is_said() {
        let missing = std::env::temp_dir().join("ramure-no-such-folder-8f3a");
        let error = open_folder(missing.display().to_string()).unwrap_err();
        assert!(error.contains("no longer exists"), "{error}");
    }
}
