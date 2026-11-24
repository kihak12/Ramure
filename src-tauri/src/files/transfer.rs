//! Copies between two places: this machine and a host, two hosts, or two folders of one place.
//!
//! A transfer first counts what it copies, so that its progress is said in bytes, then copies
//! folder by folder, file by file. What cannot be copied is listed with why, and the rest goes
//! on; a lost connection or a cancel stops it. A file left half written is deleted: the files
//! already copied stay.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{paths, FilesState, Fs, FsError, Kind, Meta, Place};

/// The size of a read: large enough for the parallel requests of SFTP to fill the network.
const CHUNK: usize = 256 * 1024;
/// The least time between two progress reports.
const REPORT_EVERY: Duration = Duration::from_millis(100);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferParams {
    /// Chosen by the interface: `transfer_cancel` names it.
    id: u32,
    from: Place,
    to: Place,
    /// Files and folders of `from`.
    sources: Vec<String>,
    /// The folder of `to` they are copied into.
    dest: String,
    /// What to do with an item whose name `dest` already has.
    #[serde(default)]
    taken: Taken,
}

/// What a copy does with an item whose name the folder it goes into already has.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Taken {
    /// It is left out.
    #[default]
    Skip,
    /// It replaces what is there; a folder is merged into the one there.
    Replace,
    /// It is copied under a free name, "report (1).pdf", and both are kept.
    KeepBoth,
}

/// `name` with the number `n`: "report (1).pdf", the number before the extension of a file, after
/// the whole name of a folder or of a file whose only dot leads (".bashrc (1)").
pub fn numbered(name: &str, is_dir: bool, n: u32) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if !is_dir && dot > 0 => name.split_at(dot),
        _ => (name, ""),
    };
    format!("{stem} ({n}){ext}")
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Still counting what there is to copy.
    pub counting: bool,
    pub files_total: u64,
    pub files_done: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
    /// The file being copied, from the folder it is copied into.
    pub current: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    /// Names left out because the folder already had them.
    pub skipped: Vec<String>,
    /// Names copied under another one because the folder already had them: [name, new name].
    pub renamed: Vec<(String, String)>,
    /// Links to folders, which are not followed: what they lead to is not copied.
    pub links: Vec<String>,
    /// What could not be copied, and why. The rest was.
    pub failed: Vec<Failure>,
}

/// One thing to copy: a folder to create, or a file to write.
struct Item {
    src: String,
    dst: String,
    /// From the folder it is copied into, with "/": what the progress and the failures say.
    rel: String,
    meta: Meta,
}

/// What a link leads to, for a copy: a file is copied as a file, a folder is not followed.
enum Followed {
    File(Meta),
    Folder,
    Nothing,
}

async fn follow(fs: &Fs, path: &str, meta: Meta) -> Result<Followed, FsError> {
    if meta.kind != Kind::Link {
        return Ok(match meta.kind {
            Kind::Dir => Followed::Folder,
            Kind::File => Followed::File(meta),
            _ => Followed::Nothing,
        });
    }
    Ok(match fs.stat(path).await {
        Ok(target) if target.kind == Kind::Dir => Followed::Folder,
        Ok(target) if target.kind == Kind::File => Followed::File(target),
        // A link that leads nowhere, or to something that is not a file.
        _ => Followed::Nothing,
    })
}

fn stop_if_canceled(cancel: &AtomicBool) -> Result<(), FsError> {
    if cancel.load(Ordering::Relaxed) { Err(FsError::Canceled) } else { Ok(()) }
}

/// What a transfer copies, from where to where.
pub struct Job<'a> {
    pub from: &'a Fs,
    pub to: &'a Fs,
    /// The two places are one: a file is never copied onto itself, nor a folder into itself.
    pub same: bool,
    pub sources: &'a [String],
    pub dest: &'a str,
    pub taken: Taken,
}

/// Lists everything to copy, folders before what they hold.
async fn count(job: &Job<'_>, cancel: &AtomicBool, summary: &mut Summary) -> Result<Vec<Item>, FsError> {
    let Job { from, to, same, sources, dest, taken } = *job;
    let mut items = Vec::new();
    // The names this copy brings, and the free names it chose: none is given twice.
    let mut claimed: std::collections::HashSet<String> = sources.iter().map(|source| from.name_of(source)).collect();
    for source in sources {
        stop_if_canceled(cancel)?;
        let name = from.name_of(source);
        let mut target = to.join(dest, &name);
        let mut rel = name.clone();
        if same && target == *source {
            summary.failed.push(Failure { path: name, reason: "It is already in this folder.".into() });
            continue;
        }
        if same && paths::inside(source, dest, matches!(from, Fs::Local)) {
            summary.failed.push(Failure { path: name, reason: "A folder cannot be copied into itself.".into() });
            continue;
        }
        let Some(meta) = from.lstat(source).await? else {
            summary.failed.push(Failure { path: name, reason: "It no longer exists.".into() });
            continue;
        };
        let followed = follow(from, source, meta).await?;
        let is_dir = matches!(followed, Followed::Folder);
        if meta.kind == Kind::Link && is_dir {
            summary.links.push(name);
            continue;
        }
        if let Some(there) = to.lstat(&target).await? {
            if taken == Taken::Skip {
                summary.skipped.push(name);
                continue;
            }
            if taken == Taken::KeepBoth {
                // The first numbered name that neither the folder nor this copy has.
                let mut n = 1;
                let fresh = loop {
                    let candidate = numbered(&name, is_dir, n);
                    n += 1;
                    if !claimed.contains(&candidate) && to.lstat(&to.join(dest, &candidate)).await?.is_none() {
                        break candidate;
                    }
                };
                claimed.insert(fresh.clone());
                target = to.join(dest, &fresh);
                summary.renamed.push((name.clone(), fresh.clone()));
                rel = fresh;
            } else if there.is_dir() != is_dir {
                let reason = if is_dir {
                    "The folder has a file of that name: rename one of them first."
                } else {
                    "The folder has a folder of that name: rename one of them first."
                };
                summary.failed.push(Failure { path: name, reason: reason.into() });
                continue;
            }
        }
        match followed {
            Followed::File(meta) => items.push(Item { src: source.clone(), dst: target, rel, meta }),
            Followed::Nothing => {
                summary.failed.push(Failure { path: rel, reason: "It is neither a file nor a folder.".into() });
            }
            Followed::Folder => {
                // Depth first, without recursion; a folder is listed before what it holds.
                let mut pending = vec![(source.clone(), target, rel)];
                while let Some((src, dst, rel)) = pending.pop() {
                    stop_if_canceled(cancel)?;
                    items.push(Item { src: src.clone(), dst: dst.clone(), rel: rel.clone(), meta });
                    let children = match from.children(&src).await {
                        Ok(children) => children,
                        Err(FsError::Lost) => return Err(FsError::Lost),
                        Err(e) => {
                            summary.failed.push(Failure { path: rel, reason: e.message() });
                            items.pop();
                            continue;
                        }
                    };
                    for (child, meta) in children {
                        let (src, dst, rel) = (from.join(&src, &child), to.join(&dst, &child), format!("{rel}/{child}"));
                        match follow(from, &src, meta).await? {
                            Followed::Folder if meta.kind == Kind::Link => summary.links.push(rel),
                            Followed::Folder => pending.push((src, dst, rel)),
                            Followed::File(meta) => items.push(Item { src, dst, rel, meta }),
                            Followed::Nothing => {
                                summary.failed.push(Failure { path: rel, reason: "It is neither a file nor a folder.".into() })
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(items)
}

/// Copies one file: the bytes written, or why it could not be copied. A lost connection or a
/// cancel stops the whole transfer.
async fn copy_file(
    from: &Fs,
    to: &Fs,
    item: &Item,
    cancel: &AtomicBool,
    progress: &mut Progress,
    report: &mut impl FnMut(&Progress, bool),
) -> Result<Result<u64, String>, FsError> {
    let lost = || from.is_lost() || to.is_lost();
    let mut reader = match from.reader(&item.src).await {
        Ok(reader) => reader,
        Err(FsError::Lost) => return Err(FsError::Lost),
        Err(e) => return Ok(Err(e.message())),
    };
    // The file is written under a name of its own, then renamed: a file already there is replaced
    // only by a whole one, and a transfer that stops leaves no half file under the real name.
    let name = to.name_of(&item.dst);
    let part = match to.parent_of(&item.dst) {
        Some(dir) => to.join(&dir, &paths::part_name(&name)),
        None => return Ok(Err("The root of the disk cannot be written.".into())),
    };
    let mut writer = match to.writer(&part).await {
        Ok(writer) => writer,
        Err(FsError::Lost) => return Err(FsError::Lost),
        Err(e) => return Ok(Err(e.message())),
    };
    let mut buffer = vec![0u8; CHUNK];
    let mut written = 0u64;
    let outcome: Result<(), Result<String, FsError>> = async {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Err(FsError::Canceled));
            }
            let n = match reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) if lost() => return Err(Err(FsError::Lost)),
                Err(e) => return Err(Ok(format!("It could not be read: {e}"))),
            };
            if let Err(e) = writer.write_all(&buffer[..n]).await {
                return Err(if lost() { Err(FsError::Lost) } else { Ok(format!("It could not be written: {e}")) });
            }
            written += n as u64;
            progress.bytes_done += n as u64;
            report(progress, false);
        }
        // Closing the file is where a server says the last writes failed.
        match writer.shutdown().await {
            Ok(()) => Ok(()),
            Err(_) if lost() => Err(Err(FsError::Lost)),
            Err(e) => Err(Ok(format!("It could not be written: {e}"))),
        }
    }
    .await;
    drop(writer);
    let outcome = match outcome {
        Ok(()) => {
            if let Some(mode) = item.meta.mode {
                to.set_mode(&part, mode).await;
            }
            put_in_place(to, &part, &item.dst).await.map_err(|e| match e {
                FsError::Lost => Err(FsError::Lost),
                other => Ok(other.message()),
            })
        }
        Err(stop) => Err(stop),
    };
    match outcome {
        Ok(()) => Ok(Ok(written)),
        Err(stop) => {
            // A file left half written would pass for a copy: it goes.
            if !to.is_lost() {
                let _ = to.remove_file(&part).await;
            }
            progress.bytes_done -= written;
            match stop {
                Ok(reason) => Ok(Err(reason)),
                Err(e) => Err(e),
            }
        }
    }
}

/// Gives the written file its real name, in place of the file that may be there. A server
/// refuses to rename onto a file that exists (SFTP version 3): the old one goes first, once the
/// new one is whole.
async fn put_in_place(to: &Fs, part: &str, dst: &str) -> Result<(), FsError> {
    match to {
        // The system replaces the file in one step, on Windows as on Linux.
        Fs::Local => to.rename(part, dst).await,
        Fs::Remote(_) => {
            if to.lstat(dst).await?.is_some() {
                to.remove_file(dst).await?;
            }
            to.rename(part, dst).await
        }
    }
}

/// Copies `sources` into the folder `dest`. `report` hears the progress, at most every 100 ms
/// unless it is asked for at once (`true`).
pub async fn run(job: &Job<'_>, cancel: &AtomicBool, mut report: impl FnMut(&Progress, bool)) -> Result<Summary, FsError> {
    let Job { from, to, dest, .. } = *job;
    let mut summary = Summary::default();
    let mut progress = Progress { counting: true, ..Progress::default() };
    report(&progress, true);
    // The folder must be there: dropping onto a pane that has just lost it says so.
    match to.stat(dest).await {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(FsError::other(format!("{dest} is not a folder."))),
        Err(e) => return Err(e),
    }
    let items = count(job, cancel, &mut summary).await?;
    progress.counting = false;
    progress.files_total = items.iter().filter(|item| !item.meta.is_dir()).count() as u64;
    progress.bytes_total = items.iter().filter(|item| !item.meta.is_dir()).map(|item| item.meta.size).sum();
    report(&progress, true);

    // The folders that could not be created: what they hold is not tried.
    let mut broken: Vec<String> = Vec::new();
    let under = |rel: &str, broken: &[String]| broken.iter().any(|dir| rel.starts_with(&format!("{dir}/")));
    for item in &items {
        stop_if_canceled(cancel)?;
        if under(&item.rel, &broken) {
            continue;
        }
        if item.meta.is_dir() {
            let made = match to.lstat(&item.dst).await? {
                Some(there) if there.is_dir() => Ok(()),
                Some(_) => Err("A file of that name is in the way.".to_string()),
                None => match to.mkdir(&item.dst).await {
                    Ok(()) => Ok(()),
                    Err(FsError::Lost) => return Err(FsError::Lost),
                    Err(e) => Err(e.message()),
                },
            };
            match made {
                Ok(()) => summary.folders += 1,
                Err(reason) => {
                    summary.failed.push(Failure { path: item.rel.clone(), reason });
                    broken.push(item.rel.clone());
                }
            }
            continue;
        }
        progress.current = item.rel.clone();
        report(&progress, true);
        match copy_file(from, to, item, cancel, &mut progress, &mut report).await? {
            Ok(bytes) => {
                summary.files += 1;
                summary.bytes += bytes;
            }
            Err(reason) => summary.failed.push(Failure { path: item.rel.clone(), reason }),
        }
        progress.files_done += 1;
        report(&progress, true);
    }
    Ok(summary)
}

/// Copies files and folders from one place to another, and says the progress on `on_progress`.
/// It returns once the transfer is over.
#[tauri::command]
pub async fn transfer_start(state: State<'_, FilesState>, params: TransferParams, on_progress: Channel<Progress>) -> Result<Summary, FsError> {
    let from = state.fs(params.from)?;
    let to = state.fs(params.to)?;
    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut transfers) = state.transfers.lock() {
        transfers.insert(params.id, cancel.clone());
    }
    let mut last = Instant::now() - REPORT_EVERY;
    let job = Job {
        from: &from,
        to: &to,
        same: params.from == params.to,
        sources: &params.sources,
        dest: &params.dest,
        taken: params.taken,
    };
    let result = run(&job, &cancel, |progress, now| {
        if now || last.elapsed() >= REPORT_EVERY {
            last = Instant::now();
            let _ = on_progress.send(progress.clone());
        }
    })
    .await;
    if let Ok(mut transfers) = state.transfers.lock() {
        transfers.remove(&params.id);
    }
    result
}

/// Stops a transfer: the file being written is deleted, the files already copied stay.
#[tauri::command]
pub fn transfer_cancel(state: State<'_, FilesState>, id: u32) {
    if let Some(flag) = state.transfers.lock().ok().and_then(|transfers| transfers.get(&id).cloned()) {
        flag.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ramure-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn s(path: &std::path::Path) -> String {
        path.to_string_lossy().to_string()
    }

    async fn copy(sources: &[PathBuf], dest: &std::path::Path, replace: bool) -> Result<Summary, FsError> {
        copy_as(sources, dest, if replace { Taken::Replace } else { Taken::Skip }).await
    }

    async fn copy_as(sources: &[PathBuf], dest: &std::path::Path, taken: Taken) -> Result<Summary, FsError> {
        let sources: Vec<String> = sources.iter().map(|p| s(p)).collect();
        let cancel = AtomicBool::new(false);
        let dest = s(dest);
        let job = Job { from: &Fs::Local, to: &Fs::Local, same: true, sources: &sources, dest: &dest, taken };
        run(&job, &cancel, |_, _| {}).await
    }

    #[tokio::test]
    async fn a_folder_is_copied_with_what_it_holds() {
        let src = scratch("src");
        let dest = scratch("dest");
        std::fs::create_dir_all(src.join("site/css")).unwrap();
        std::fs::write(src.join("site/index.html"), b"<h1>hi</h1>").unwrap();
        std::fs::write(src.join("site/css/main.css"), vec![7u8; 600_000]).unwrap();
        std::fs::create_dir_all(src.join("site/empty")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(src.join("site/run.sh"), b"#!/bin/sh\n").unwrap();
            std::fs::set_permissions(src.join("site/run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
            std::os::unix::fs::symlink(&dest, src.join("site/elsewhere")).unwrap();
        }

        let summary = copy(&[src.join("site")], &dest, false).await.unwrap();
        assert_eq!(std::fs::read(dest.join("site/index.html")).unwrap(), b"<h1>hi</h1>");
        assert_eq!(std::fs::read(dest.join("site/css/main.css")).unwrap().len(), 600_000);
        assert!(dest.join("site/empty").is_dir());
        assert_eq!(summary.folders, 3);
        assert!(summary.failed.is_empty(), "{:?}", summary.failed);
        // Nothing is left under the name of a file being written.
        let names: Vec<String> = std::fs::read_dir(dest.join("site")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".ramure-part")), "{names:?}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dest.join("site/run.sh")).unwrap().permissions().mode() & 0o777, 0o755);
            // A link to a folder is not followed.
            assert_eq!(summary.links, vec!["site/elsewhere".to_string()]);
            assert!(!dest.join("site/elsewhere").exists());
        }
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[tokio::test]
    async fn a_name_already_there_is_skipped_or_replaced() {
        let src = scratch("src");
        let dest = scratch("dest");
        std::fs::write(src.join("a.txt"), b"new").unwrap();
        std::fs::write(dest.join("a.txt"), b"old").unwrap();

        let summary = copy(&[src.join("a.txt")], &dest, false).await.unwrap();
        assert_eq!(summary.skipped, vec!["a.txt".to_string()]);
        assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"old");

        let summary = copy(&[src.join("a.txt")], &dest, true).await.unwrap();
        assert_eq!(summary.files, 1);
        assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"new");
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn a_number_goes_before_the_extension_of_a_file() {
        assert_eq!(numbered("report.pdf", false, 1), "report (1).pdf");
        assert_eq!(numbered("archive.tar.gz", false, 2), "archive.tar (2).gz");
        assert_eq!(numbered(".bashrc", false, 1), ".bashrc (1)");
        assert_eq!(numbered("notes", false, 1), "notes (1)");
        assert_eq!(numbered("site.v2", true, 1), "site.v2 (1)");
    }

    #[tokio::test]
    async fn keeping_both_copies_under_the_first_free_name() {
        let src = scratch("src");
        let dest = scratch("dest");
        std::fs::write(src.join("a.txt"), b"new").unwrap();
        std::fs::write(src.join("a (1).txt"), b"other").unwrap();
        std::fs::create_dir_all(src.join("conf")).unwrap();
        std::fs::write(src.join("conf/x"), b"x").unwrap();
        std::fs::write(dest.join("a.txt"), b"old").unwrap();
        std::fs::write(dest.join("a (2).txt"), b"older").unwrap();
        std::fs::create_dir_all(dest.join("conf")).unwrap();

        let summary = copy_as(&[src.join("a.txt"), src.join("a (1).txt"), src.join("conf")], &dest, Taken::KeepBoth).await.unwrap();
        // "a (1).txt" is a name this copy brings, "a (2).txt" one the folder has: "a (3).txt" is free.
        assert_eq!(summary.renamed, vec![("a.txt".to_string(), "a (3).txt".to_string()), ("conf".to_string(), "conf (1)".to_string())]);
        assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"old");
        assert_eq!(std::fs::read(dest.join("a (3).txt")).unwrap(), b"new");
        assert_eq!(std::fs::read(dest.join("a (1).txt")).unwrap(), b"other");
        assert_eq!(std::fs::read(dest.join("a (2).txt")).unwrap(), b"older");
        // A folder is copied whole beside the one there, never merged.
        assert_eq!(std::fs::read(dest.join("conf (1)/x")).unwrap(), b"x");
        assert!(!dest.join("conf/x").exists());
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[tokio::test]
    async fn a_folder_replaces_a_folder_by_merging_but_never_a_file() {
        let src = scratch("src");
        let dest = scratch("dest");
        std::fs::create_dir_all(src.join("conf")).unwrap();
        std::fs::write(src.join("conf/a"), b"new").unwrap();
        std::fs::create_dir_all(dest.join("conf")).unwrap();
        std::fs::write(dest.join("conf/a"), b"old").unwrap();
        std::fs::write(dest.join("conf/b"), b"kept").unwrap();
        std::fs::create_dir_all(src.join("data")).unwrap();
        std::fs::write(dest.join("data"), b"a file").unwrap();

        let summary = copy(&[src.join("conf"), src.join("data")], &dest, true).await.unwrap();
        assert_eq!(std::fs::read(dest.join("conf/a")).unwrap(), b"new");
        assert_eq!(std::fs::read(dest.join("conf/b")).unwrap(), b"kept");
        assert_eq!(summary.failed.len(), 1);
        assert_eq!(summary.failed[0].path, "data");
        assert_eq!(std::fs::read(dest.join("data")).unwrap(), b"a file");
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[tokio::test]
    async fn a_file_is_never_copied_onto_itself_nor_a_folder_into_itself() {
        let src = scratch("src");
        std::fs::write(src.join("a.txt"), b"keep me").unwrap();
        std::fs::create_dir_all(src.join("app/build")).unwrap();

        let summary = copy(&[src.join("a.txt")], &src, true).await.unwrap();
        assert_eq!(summary.failed[0].reason, "It is already in this folder.");
        assert_eq!(std::fs::read(src.join("a.txt")).unwrap(), b"keep me");

        let summary = copy(&[src.join("app")], &src.join("app/build"), true).await.unwrap();
        assert_eq!(summary.failed[0].reason, "A folder cannot be copied into itself.");
        assert!(!src.join("app/build/app").exists());
        std::fs::remove_dir_all(&src).unwrap();
    }

    #[tokio::test]
    async fn a_canceled_transfer_stops_before_copying() {
        let src = scratch("src");
        let dest = scratch("dest");
        std::fs::write(src.join("a.txt"), b"x").unwrap();
        let cancel = AtomicBool::new(true);
        let (sources, dest_path) = ([s(&src.join("a.txt"))], s(&dest));
        let job = Job { from: &Fs::Local, to: &Fs::Local, same: true, sources: &sources, dest: &dest_path, taken: Taken::Skip };
        let result = run(&job, &cancel, |_, _| {}).await;
        assert_eq!(result.unwrap_err(), FsError::Canceled);
        assert!(!dest.join("a.txt").exists());
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[tokio::test]
    async fn the_progress_counts_bytes_and_files() {
        let src = scratch("src");
        let dest = scratch("dest");
        std::fs::write(src.join("a"), vec![1u8; 1000]).unwrap();
        std::fs::write(src.join("b"), vec![1u8; 500]).unwrap();
        let cancel = AtomicBool::new(false);
        let mut last = Progress::default();
        let (sources, dest_path) = ([s(&src.join("a")), s(&src.join("b"))], s(&dest));
        let job = Job { from: &Fs::Local, to: &Fs::Local, same: true, sources: &sources, dest: &dest_path, taken: Taken::Skip };
        run(&job, &cancel, |p, _| last = p.clone()).await.unwrap();
        assert_eq!((last.files_done, last.files_total, last.bytes_done, last.bytes_total), (2, 2, 1500, 1500));
        assert!(!last.counting);
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }
}
