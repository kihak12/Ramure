//! Driving the `git` binary installed on the machine.
//! Ramure does not reimplement Git: it runs the official commands and parses their output.

pub mod clone;
pub mod conflict;
pub mod graph;
pub mod log;
pub mod ops;
pub mod patch;
pub mod work;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use serde::Serialize;

/// A git command run by Ramure, as it appears in the command log.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLog {
    pub command: String,
    pub ms: f64,
    pub ok: bool,
}

/// Result of an operation that succeeded, with the commands it ran.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Done<T> {
    pub data: T,
    pub commands: Vec<CommandLog>,
}

/// Failure of an operation: the message of git, and the commands run until then.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failed {
    pub message: String,
    pub commands: Vec<CommandLog>,
}

/// Options of a git command. The default values suit most calls.
#[derive(Default, Clone, Copy)]
pub struct Opts<'a> {
    /// Written to the standard input of git.
    pub input: Option<&'a [u8]>,
    /// Exit codes to treat as successes, besides zero.
    pub accepted: &'a [i32],
    /// Environment variables added to those of Ramure.
    pub envs: &'a [(&'a str, &'a str)],
}

pub fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

/// Cleans up output meant for a terminal: git rewrites its progress line there with carriage
/// returns and erase sequences. Only the final state of each line is kept.
pub fn plain(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            // `ESC [ … letter` sequence: a color or a line erase.
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out.lines()
        .filter_map(|line| line.rsplit('\r').find(|part| !part.trim().is_empty()))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Base `git` command: no console window, no question, no editor.
pub fn command() -> Command {
    let mut cmd = Command::new("git");
    // Never a question asked in an invisible terminal.
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        // Nor an editor: `:` is git's “do nothing” editor, it keeps the proposed text.
        .env("GIT_EDITOR", ":")
        .env("GIT_SEQUENCE_EDITOR", ":");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Runs `git` in `dir` and returns its whole output, whatever the exit code.
pub fn exec(dir: &Path, args: &[&str], opts: Opts) -> Result<Output, String> {
    let mut cmd = command();
    cmd.arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args(args)
        .envs(opts.envs.iter().copied())
        .stdin(if opts.input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start git: {e}. Check that git is installed."))?;
    if let (Some(input), Some(mut stdin)) = (opts.input, child.stdin.take()) {
        // If git stops before reading everything, its error message is more useful than ours.
        let _ = stdin.write_all(input);
    }
    child
        .wait_with_output()
        .map_err(|e| format!("Could not run git: {e}"))
}

fn failure(out: &Output) -> String {
    let stderr = plain(&out.stderr);
    let stdout = plain(&out.stdout);
    // A merge with conflicts explains itself on the standard output, a rebase on the error
    // output: both are kept.
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => match out.status.code() {
            Some(code) => format!("The git command stopped with code {code} and wrote no message."),
            None => "The git command was interrupted and wrote no message.".to_string(),
        },
        (true, false) => stderr,
        (false, true) => stdout,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

/// What git wrote, standard output then error output: the progress of a push or a fetch goes
/// through the latter, even when it succeeds.
pub fn report(out: &Output) -> String {
    let mut report = plain(&out.stdout);
    let stderr = plain(&out.stderr);
    if !stderr.is_empty() {
        if !report.is_empty() {
            report.push('\n');
        }
        report.push_str(&stderr);
    }
    report
}

/// Runs `git` without logging it: for internal reads that would clutter the command log.
pub fn run(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = exec(dir, args, Opts::default())?;
    if out.status.success() { Ok(out.stdout) } else { Err(failure(&out)) }
}

/// Runs git with options and records the command in the command log.
pub fn step_with(
    journal: &mut Vec<CommandLog>,
    dir: &Path,
    args: &[&str],
    opts: Opts,
) -> Result<Output, String> {
    let start = Instant::now();
    let result = exec(dir, args, opts).and_then(|out| {
        let fine = out.status.success() || out.status.code().is_some_and(|c| opts.accepted.contains(&c));
        if fine { Ok(out) } else { Err(failure(&out)) }
    });
    journal.push(CommandLog {
        command: format!("git {}", args.join(" ")),
        ms: ms(start),
        ok: result.is_ok(),
    });
    result
}

/// Runs git, records the command in the command log and returns the standard output.
pub fn step(journal: &mut Vec<CommandLog>, dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    step_with(journal, dir, args, Opts::default()).map(|out| out.stdout)
}

/// A name given by the user (branch, tag, commit) must never be able to pass for an option of
/// git.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_control) {
        Err(format!("Invalid name: “{name}”. A name cannot be empty, start with “-” or contain control characters."))
    } else {
        Ok(())
    }
}

/// Runs a git operation off the interface thread and attaches its command log to it.
pub async fn blocking<T, F>(op: F) -> Result<Done<T>, Failed>
where
    T: Send + 'static,
    F: FnOnce(&mut Vec<CommandLog>) -> Result<T, String> + Send + 'static,
{
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut commands = Vec::new();
        let result = op(&mut commands);
        (result, commands)
    })
    .await;
    match joined {
        Ok((Ok(data), commands)) => Ok(Done { data, commands }),
        Ok((Err(message), commands)) => Err(Failed { message, commands }),
        Err(e) => Err(Failed { message: e.to_string(), commands: Vec::new() }),
    }
}

/// Loads the history of a repository, already laid out for the graph.
#[tauri::command]
pub async fn git_log(repo: String, limit: Option<u32>) -> Result<log::LogResult, String> {
    tauri::async_runtime::spawn_blocking(move || log::load(Path::new(&repo), limit))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_are_reduced_to_their_final_state() {
        let raw = b"Rebasing (1/3)\rRebasing (2/3)\rRebasing (3/3)\r\x1b[KSuccessfully rebased.\nremote: \x1b[32mok\x1b[0m\r\n\n";
        assert_eq!(plain(raw), "Successfully rebased.\nremote: ok");
        assert_eq!(plain(b""), "");
    }

    #[test]
    fn option_like_names_are_refused() {
        assert!(check_name("feat/x").is_ok());
        assert!(check_name("--force").is_err());
        assert!(check_name("").is_err());
        assert!(check_name("a\nb").is_err());
    }
}
