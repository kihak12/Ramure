//! What Ramure asks of the system outside its own windows: showing a folder in its file manager,
//! and opening a file in another program.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Where `program` is: a path as it is, if it is a file; a bare name, searched in the folders of
/// `PATH`, with the extensions of `PATHEXT` on Windows (`code` is `code.cmd` there).
pub fn find_program(program: &str) -> Option<PathBuf> {
    let program = program.trim();
    if program.is_empty() {
        return None;
    }
    let given = Path::new(program);
    if given.components().count() > 1 || given.is_absolute() {
        return given.is_file().then(|| given.to_path_buf());
    }
    let extensions: Vec<String> = if cfg!(windows) {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        std::iter::once(String::new()).chain(pathext.split(';').filter(|e| !e.is_empty()).map(str::to_string)).collect()
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        extensions.iter().find_map(|ext| {
            let candidate = dir.join(format!("{program}{ext}"));
            candidate.is_file().then_some(candidate)
        })
    })
}

/// Whether a file reads as text: no NUL byte in its first 8 kB (guidelines, 9.24). An empty file is
/// text.
pub fn is_text(path: &Path) -> std::io::Result<bool> {
    let mut head = Vec::with_capacity(8192);
    std::fs::File::open(path)?.take(8192).read_to_end(&mut head)?;
    Ok(!head.contains(&0))
}

/// Opens `file` in `program`, its only argument, or in the program of the system when there is
/// none: File Explorer on Windows, `open` on macOS, `xdg-open` elsewhere.
fn launch(file: &Path, program: Option<&Path>) -> std::io::Result<()> {
    let mut command = match program {
        // A batch file runs through the command interpreter on Windows.
        Some(found) if cfg!(windows) && found.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat")) => {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(found);
            c
        }
        Some(found) => Command::new(found),
        None if cfg!(windows) => Command::new("explorer"),
        None if cfg!(target_os = "macos") => Command::new("open"),
        None => Command::new("xdg-open"),
    };
    let target = if cfg!(windows) { PathBuf::from(file.display().to_string().replace('/', "\\")) } else { file.to_path_buf() };
    command.arg(target).spawn().map(|_| ())
}

/// Opens a file of this machine (guidelines, 9.24): in `program` when one is given, else in the
/// `editor` when the file is text, else in the program of the system. The error says what to do.
#[tauri::command]
pub fn open_file(path: String, program: Option<String>, editor: Option<String>) -> Result<(), String> {
    let file = PathBuf::from(&path);
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
    if !file.is_file() {
        return Err(format!("{name} is no longer there. It may have been moved or deleted."));
    }
    let wanted = match (program.filter(|p| !p.trim().is_empty()), editor.filter(|e| !e.trim().is_empty())) {
        (Some(program), _) => Some(program),
        (None, Some(editor)) if is_text(&file).unwrap_or(false) => Some(editor),
        _ => None,
    };
    let found = match &wanted {
        Some(program) => Some(find_program(program).ok_or_else(|| format!("No program named {program} was found. Check its name or its path in the settings, or choose another with “Open with…”."))?),
        None => None,
    };
    launch(&file, found.as_deref()).map_err(|e| match &wanted {
        Some(program) => format!("{program} could not be started: {e}. Check that it can be run, or choose another with “Open with…”."),
        None => format!("No program of the system opens {name}. Choose one with “Open with…”. ({e})"),
    })
}

/// Whether a program can be found, for the settings and "Open with…" (guidelines, 9.18).
#[tauri::command]
pub fn program_found(program: String) -> bool {
    find_program(&program).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_told_by_its_first_bytes() {
        let dir = std::env::temp_dir().join(format!("ramure-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = dir.join("nginx.conf");
        let binary = dir.join("image.png");
        let empty = dir.join("empty");
        std::fs::write(&text, "server {\n  listen 80;\n}\n").unwrap();
        std::fs::write(&binary, [0x89, b'P', b'N', b'G', 0, 0, 0, 13]).unwrap();
        std::fs::write(&empty, "").unwrap();
        assert!(is_text(&text).unwrap());
        assert!(!is_text(&binary).unwrap());
        assert!(is_text(&empty).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_program_is_found_by_its_name_or_its_path() {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let found = find_program(shell).expect("the shell is on the PATH");
        assert!(found.is_file());
        assert_eq!(find_program(&found.display().to_string()), Some(found));
        assert_eq!(find_program("ramure-no-such-program-8f3a"), None);
        assert_eq!(find_program("  "), None);
    }

    #[test]
    fn a_missing_file_is_said() {
        let error = open_file("/nowhere/ramure-8f3a.txt".into(), None, None).unwrap_err();
        assert!(error.contains("no longer there"), "{error}");
    }

    #[test]
    fn a_missing_folder_is_said() {
        let missing = std::env::temp_dir().join("ramure-no-such-folder-8f3a");
        let error = open_folder(missing.display().to_string()).unwrap_err();
        assert!(error.contains("no longer exists"), "{error}");
    }
}
