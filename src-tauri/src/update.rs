//! In-app updates (guidelines, section 9.23): asking GitHub whether a newer version exists,
//! downloading it, checking its signature, installing it and restarting.
//!
//! Nothing here runs on its own: the interface decides when to check, and every other step
//! follows a click. The messages are those the guidelines quote.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::utils::config::BundleType;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Error, Update, UpdaterExt};
use tauri_plugin_window_state::AppHandleExt;

/// What the settings show before any check.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// The version that runs.
    version: String,
    /// False for a copy built from the sources: nothing can replace it.
    can_update: bool,
    /// How this copy was installed: what the system will do at the install.
    installer: Option<&'static str>,
}

/// A newer version, as the release announces it.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    version: String,
    /// When it was published, in milliseconds since the epoch, when the release says it.
    published: Option<i64>,
}

/// The progress of a download. `total` is missing when the server does not give the size.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    done: u64,
    total: Option<u64>,
    /// Everything was received: the signature is being checked.
    verifying: bool,
}

/// Why a step failed: what happened and what to do, then the reason of the system as it is.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    message: String,
    reason: Option<String>,
    /// The download is gone: the version is available again, not ready.
    discarded: bool,
}

impl Failure {
    fn new(message: &str, reason: Option<String>) -> Self {
        Self { message: message.into(), reason, discarded: false }
    }

    /// The download did not pass verification: it is not kept.
    fn not_verified() -> Self {
        Self { message: NOT_VERIFIED.into(), reason: None, discarded: true }
    }
}

/// The version found by the last check, the bytes of its download once verified, and the download
/// in progress, so that it can be canceled.
#[derive(Default)]
pub struct Updates {
    found: Mutex<Option<Update>>,
    bytes: Mutex<Option<Vec<u8>>>,
    download: Mutex<Option<tokio::task::AbortHandle>>,
}

const NO_NETWORK: &str = "Ramure could not reach GitHub. Check your connection, then try again.";
const NO_RELEASE: &str = "No published version was found on GitHub. Try again later.";
const NO_INSTALLER: &str = "The new version has no installer for this system yet. Try again later.";
const DOWNLOAD_FAILED: &str = "The download failed. Check your connection, then try again.";
const NOT_VERIFIED: &str = "The download did not pass verification and was discarded. Try again.";
const NOT_INSTALLED: &str = "The update was not installed. Ramure is unchanged.";
const NO_PASSWORD: &str =
    "The update was not installed: Ubuntu did not get the administrator password, or refused the package. Ramure is unchanged.";
const NOTHING_FOUND: &str = "No version is waiting. Check for updates again.";
const NOT_DOWNLOADED: &str = "The update is not downloaded yet. Download it again.";

/// The installer of this copy, by the name the release gives it.
fn installer(bundle: Option<&BundleType>) -> Option<&'static str> {
    match bundle? {
        BundleType::Deb => Some("deb"),
        BundleType::AppImage => Some("appimage"),
        BundleType::Nsis => Some("nsis"),
        _ => None,
    }
}

/// A copy can update itself when it came from an installer of a release, and when the releases
/// can be verified. A copy built from the sources has no installer; a development build never
/// updates.
fn can_update(dev: bool, bundle: Option<&BundleType>, pubkey: &str) -> bool {
    !dev && installer(bundle).is_some() && !pubkey.trim().is_empty()
}

/// The public key of the releases, from `tauri.conf.json`.
fn pubkey(app: &AppHandle) -> String {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|key| key.as_str())
        .unwrap_or_default()
        .to_string()
}

fn check_failure(error: &Error) -> Failure {
    match error {
        // The release exists, but not for this system: its installers are still being built.
        Error::TargetNotFound(_) | Error::TargetsNotFound(_) => Failure::new(NO_INSTALLER, None),
        // GitHub answered, with an error status: no release is published, or its `latest.json` is
        // missing. The connection is not at fault, and the reason of the library adds nothing.
        Error::ReleaseNotFound => Failure::new(NO_RELEASE, None),
        other => Failure::new(NO_NETWORK, Some(other.to_string())),
    }
}

/// A download fails on the way, or at its verification: whatever is not the network is the
/// signature, the key or the version signed.
fn download_failure(error: &Error) -> Failure {
    match error {
        Error::Reqwest(_) | Error::Network(_) | Error::Io(_) => Failure::new(DOWNLOAD_FAILED, Some(error.to_string())),
        _ => Failure::not_verified(),
    }
}

fn install_failure(error: &Error) -> Failure {
    match error {
        Error::AuthenticationFailed | Error::DebInstallFailed | Error::PackageInstallFailed => Failure::new(NO_PASSWORD, None),
        // The bytes were verified once more before the install.
        Error::Minisign(_) | Error::MissingSignedVersion => Failure::not_verified(),
        other => Failure::new(NOT_INSTALLED, Some(other.to_string())),
    }
}

#[tauri::command]
pub fn update_info(app: AppHandle) -> UpdateInfo {
    let bundle = tauri::utils::platform::bundle_type();
    let bundle = bundle.as_ref();
    UpdateInfo {
        version: app.package_info().version.to_string(),
        can_update: can_update(tauri::is_dev(), bundle, &pubkey(&app)),
        installer: installer(bundle),
    }
}

/// Asks whether a newer version exists. The answer is kept for the download.
#[tauri::command]
pub async fn update_check(app: AppHandle, state: State<'_, Updates>) -> Result<Option<Available>, Failure> {
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| check_failure(&e))?;
    let found = updater.check().await.map_err(|e| check_failure(&e))?;
    let available = found.as_ref().map(|update| Available {
        version: update.version.clone(),
        published: update.date.map(|date| date.unix_timestamp() * 1000),
    });
    let mut kept = state.found.lock().unwrap();
    // The same version, already downloaded, stays ready; another one starts over.
    if kept.as_ref().map(|u| &u.version) != found.as_ref().map(|u| &u.version) {
        *state.bytes.lock().unwrap() = None;
    }
    *kept = found;
    Ok(available)
}

/// Downloads the version found and checks its signature. `Ok(false)`: the download was canceled.
#[tauri::command]
pub async fn update_download(state: State<'_, Updates>, on_progress: Channel<Progress>) -> Result<bool, Failure> {
    let update = state.found.lock().unwrap().clone().ok_or_else(|| Failure::new(NOTHING_FOUND, None))?;
    let received = on_progress.clone();
    let task = tokio::spawn(async move {
        let mut done = 0u64;
        let mut total = None;
        update
            .download(
                |chunk, size| {
                    done += chunk as u64;
                    total = size;
                    let _ = received.send(Progress { done, total, verifying: false });
                },
                || {},
            )
            .await
            .map(|bytes| (bytes, total))
    });
    *state.download.lock().unwrap() = Some(task.abort_handle());
    let result = task.await;
    *state.download.lock().unwrap() = None;
    match result {
        Ok(Ok((bytes, total))) => {
            let _ = on_progress.send(Progress { done: bytes.len() as u64, total, verifying: true });
            *state.bytes.lock().unwrap() = Some(bytes);
            Ok(true)
        }
        Ok(Err(error)) => Err(download_failure(&error)),
        Err(join) if join.is_cancelled() => Ok(false),
        Err(join) => Err(Failure::new(DOWNLOAD_FAILED, Some(join.to_string()))),
    }
}

/// "Cancel": the download stops, and what was received is dropped.
#[tauri::command]
pub fn update_cancel(state: State<'_, Updates>) {
    if let Some(download) = state.download.lock().unwrap().take() {
        download.abort();
    }
}

/// Installs the downloaded version, then restarts. On Windows the installer takes over and Ramure
/// exits; elsewhere Ramure restarts itself once the files are in place. Returns only on a failure:
/// the download is kept, so that the install can be tried again.
#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, Updates>) -> Result<(), Failure> {
    let update = state.found.lock().unwrap().clone().ok_or_else(|| Failure::new(NOTHING_FOUND, None))?;
    let bytes = state.bytes.lock().unwrap().take().ok_or_else(|| Failure::new(NOT_DOWNLOADED, None))?;
    // A restart, or the installer of Windows, ends Ramure without the usual way out: the size and
    // the place of the window are kept now, so that the new version opens as this one was.
    let _ = app.save_window_state(crate::WINDOW_STATE);
    // The install waits for the system: on Ubuntu, for the administrator password.
    let (result, bytes) = tokio::task::spawn_blocking(move || (update.install(&bytes), bytes))
        .await
        .map_err(|join| Failure::new(NOT_INSTALLED, Some(join.to_string())))?;
    match result {
        Ok(()) => app.restart(),
        Err(error) => {
            let failure = install_failure(&error);
            // What did not pass verification is dropped; anything else can be tried again.
            if !failure.discarded {
                *state.bytes.lock().unwrap() = Some(bytes);
            }
            Err(failure)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_installed_copy_with_a_key_can_update() {
        assert!(can_update(false, Some(&BundleType::Deb), "key"));
        assert!(can_update(false, Some(&BundleType::AppImage), "key"));
        assert!(can_update(false, Some(&BundleType::Nsis), "key"));
        // Built from the sources: no installer to replace it.
        assert!(!can_update(false, None, "key"));
        // A development build never updates.
        assert!(!can_update(true, Some(&BundleType::Deb), "key"));
        // Without the public key, a release cannot be verified.
        assert!(!can_update(false, Some(&BundleType::Deb), ""));
        assert!(!can_update(false, Some(&BundleType::Deb), "  "));
        // An installer Ramure does not publish.
        assert!(!can_update(false, Some(&BundleType::Msi), "key"));
    }

    #[test]
    fn the_installer_has_the_name_of_the_release() {
        assert_eq!(installer(Some(&BundleType::Deb)), Some("deb"));
        assert_eq!(installer(Some(&BundleType::AppImage)), Some("appimage"));
        assert_eq!(installer(Some(&BundleType::Nsis)), Some("nsis"));
        assert_eq!(installer(None), None);
    }

    #[test]
    fn a_check_that_fails_says_what_to_do() {
        let offline = check_failure(&Error::Network("dns error".into()));
        assert_eq!(offline.message, "Ramure could not reach GitHub. Check your connection, then try again.");
        assert_eq!(offline.reason.as_deref(), Some("`dns error`"));
        // GitHub answered, but gave no version to read: the connection is not at fault.
        let unpublished = check_failure(&Error::ReleaseNotFound);
        assert_eq!(unpublished, Failure::new("No published version was found on GitHub. Try again later.", None));
        // The release is there, its installer for this system is not: nothing is wrong with the network.
        let missing = check_failure(&Error::TargetsNotFound(vec!["linux-x86_64-deb".into(), "linux-x86_64".into()]));
        assert_eq!(missing, Failure::new("The new version has no installer for this system yet. Try again later.", None));
    }

    #[test]
    fn a_download_fails_on_the_way_or_at_its_verification() {
        let cut = download_failure(&Error::Network("Download request failed with status: 502".into()));
        assert_eq!(cut.message, "The download failed. Check your connection, then try again.");
        assert!(cut.reason.is_some());
        // What is not the network is the signature: the reason would only name the internals.
        let forged = download_failure(&Error::MissingSignedVersion);
        assert_eq!(forged.message, "The download did not pass verification and was discarded. Try again.");
        assert_eq!(forged.reason, None);
        // It is not kept: the version is available again, not ready.
        assert!(forged.discarded);
        assert!(!cut.discarded);
        // The same at the install, where the signature is checked once more.
        assert_eq!(install_failure(&Error::MissingSignedVersion), forged);
    }

    #[test]
    fn an_install_that_fails_leaves_ramure_unchanged() {
        for refused in [Error::AuthenticationFailed, Error::PackageInstallFailed, Error::DebInstallFailed] {
            assert_eq!(
                install_failure(&refused).message,
                "The update was not installed: Ubuntu did not get the administrator password, or refused the package. Ramure is unchanged."
            );
        }
        let other = install_failure(&Error::TempDirNotFound);
        assert_eq!(other.message, "The update was not installed. Ramure is unchanged.");
        assert_eq!(other.reason.as_deref(), Some("failed to create temporary directory"));
        // The download is kept: the install can be tried again.
        assert!(!other.discarded);
    }
}
