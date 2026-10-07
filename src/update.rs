use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;
use fs2::FileExt;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = "felipe-godoi/diffv";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    target_commitish: Option<String>,
    #[serde(default)]
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

/// Identifies a published build so it can be checked against `diffv --version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub channel: UpdateChannel,
    pub tag: String,
    pub commit: Option<String>,
    /// Upload time of the platform binary, e.g. "2026-10-06 21:21 UTC".
    pub built_at: Option<String>,
}

impl BuildInfo {
    fn from_release(channel: UpdateChannel, release: &Release, asset: &Asset) -> Self {
        let commit = release
            .target_commitish
            .as_deref()
            .filter(|c| is_commit(c))
            .or_else(|| release.body.as_deref().and_then(commit_in_text))
            .map(|c| c[..7].to_string());
        let built_at = asset.updated_at.as_deref().map(|t| {
            // "2026-10-06T21:21:19Z" → "2026-10-06 21:21 UTC"
            match t.split_once('T') {
                Some((date, time)) if time.len() >= 5 => format!("{} {} UTC", date, &time[..5]),
                _ => t.to_string(),
            }
        });
        Self {
            channel,
            tag: release.tag_name.clone(),
            commit,
            built_at,
        }
    }

    /// "nightly · b97105d · 2026-10-06 21:21 UTC"
    pub fn label(&self) -> String {
        let mut parts = vec![self.tag.clone()];
        parts.extend(self.commit.clone());
        parts.extend(self.built_at.clone());
        parts.join(" · ")
    }
}

fn is_commit(text: &str) -> bool {
    (7..=40).contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Rolling releases name their commit in the body, e.g. "latest commit on main (b97105d)".
fn commit_in_text(text: &str) -> Option<&str> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .find(|word| is_commit(word) && word.chars().any(|c| c.is_ascii_digit()))
}

/// Why an update could not be checked or installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateFailure {
    /// GitHub could not be reached (no connection, DNS, proxy).
    Offline,
    Timeout,
    /// GitHub answered with an HTTP error (rate limit, outage).
    Http,
    /// The channel's release tag does not exist.
    NoRelease,
    NoPlatformAsset,
    ChecksumUnavailable,
    ChecksumMismatch,
    /// The installation directory cannot be written.
    NotWritable(PathBuf),
    /// Another diffv holds the update lock.
    Locked,
    Other(String),
}

impl UpdateFailure {
    pub fn describe(&self, channel: UpdateChannel, language: Language) -> String {
        use UpdateFailure::*;
        match (self, language) {
            (Offline, Language::En) => "Could not reach GitHub (no network connection?)".into(),
            (Offline, Language::Pt) => {
                "Não foi possível acessar o GitHub (sem conexão de rede?)".into()
            }
            (Timeout, Language::En) => "GitHub did not respond in time (timeout)".into(),
            (Timeout, Language::Pt) => "O GitHub não respondeu a tempo (timeout)".into(),
            (Http, Language::En) => {
                "GitHub returned an error (rate limit or service unavailable)".into()
            }
            (Http, Language::Pt) => {
                "O GitHub retornou um erro (limite de requisições ou serviço indisponível)".into()
            }
            (NoRelease, Language::En) => {
                format!("No {} release is published on GitHub", channel.as_str())
            }
            (NoRelease, Language::Pt) => {
                format!("Nenhuma release {} publicada no GitHub", channel.as_str())
            }
            (NoPlatformAsset, Language::En) => "The release has no binary for this platform".into(),
            (NoPlatformAsset, Language::Pt) => {
                "A release não tem binário para esta plataforma".into()
            }
            (ChecksumUnavailable, Language::En) => {
                "The release has no SHA-256 checksum, so it was not installed".into()
            }
            (ChecksumUnavailable, Language::Pt) => {
                "A release não tem checksum SHA-256, então não foi instalada".into()
            }
            (ChecksumMismatch, Language::En) => {
                "The download failed SHA-256 verification and was discarded".into()
            }
            (ChecksumMismatch, Language::Pt) => {
                "O download não passou na verificação SHA-256 e foi descartado".into()
            }
            (NotWritable(dir), Language::En) => {
                format!("No write permission on {}", dir.display())
            }
            (NotWritable(dir), Language::Pt) => {
                format!("Sem permissão de escrita em {}", dir.display())
            }
            (Locked, Language::En) => {
                "Another diffv is already installing an update (lock busy)".into()
            }
            (Locked, Language::Pt) => {
                "Outro diffv já está instalando uma atualização (lock ocupado)".into()
            }
            (Other(detail), Language::En) => format!("Update error: {}", detail),
            (Other(detail), Language::Pt) => format!("Erro na atualização: {}", detail),
        }
    }
}

#[derive(Debug)]
pub struct UpdateError {
    pub channel: UpdateChannel,
    pub failure: UpdateFailure,
    /// The build that was being installed, when the failure happened after the check.
    pub target: Option<BuildInfo>,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.failure.describe(self.channel, Language::En))?;
        if let Some(target) = &self.target {
            write!(f, " (while updating to {})", target.label())?;
        }
        Ok(())
    }
}

impl std::error::Error for UpdateError {}

/// What the startup check reports to the TUI. Nothing is reported when diffv is
/// already up to date, so the user only hears about it when something happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEvent {
    /// A different build was found and is being downloaded.
    Downloading(BuildInfo),
    /// The verified build replaced the binary on disk; it runs from the next start.
    Installed(BuildInfo),
    Failed {
        channel: UpdateChannel,
        failure: UpdateFailure,
        target: Option<BuildInfo>,
    },
}

fn platform_asset() -> Option<String> {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        _ => return None,
    };
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        _ => return None,
    };
    Some(format!("diffv-{}-{}", arch, os))
}

fn curl_failure(code: Option<i32>) -> UpdateFailure {
    match code {
        Some(5..=7) => UpdateFailure::Offline,
        Some(28) => UpdateFailure::Timeout,
        Some(22) => UpdateFailure::Http,
        Some(code) => UpdateFailure::Other(format!("GitHub request failed (curl exit {})", code)),
        None => UpdateFailure::Other("GitHub request was interrupted".into()),
    }
}

fn io_failure(err: std::io::Error, dir: &Path) -> UpdateFailure {
    match err.kind() {
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => {
            UpdateFailure::NotWritable(dir.to_path_buf())
        }
        _ => UpdateFailure::Other(err.to_string()),
    }
}

// Redirect into a file rather than a pipe so large downloads cannot deadlock.
fn fetch(mut command: Command, destination: &Path, timeout: Duration) -> Result<(), UpdateFailure> {
    let other = |err: std::io::Error| UpdateFailure::Other(err.to_string());
    let file = File::create(destination).map_err(other)?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| UpdateFailure::Other(format!("could not run curl: {}", err)))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(other)? {
            if !status.success() {
                return Err(curl_failure(status.code()));
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(UpdateFailure::Timeout);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn curl(url: &str) -> Command {
    let mut command = Command::new("curl");
    command.args([
        "--fail",
        "--silent",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "2",
        "--max-time",
        "90",
        "--user-agent",
        "diffv-updater",
        url,
    ]);
    command
}

fn newer_release(release: &Release, current: &str) -> Result<bool> {
    let version = Version::parse(release.tag_name.trim_start_matches('v'))?;
    Ok(!release.draft
        && !release.prerelease
        && version.pre.is_empty()
        && version > Version::parse(current)?)
}

fn install_verified(download: &Path, executable: &Path, digest: &str) -> Result<(), UpdateFailure> {
    let dir = executable.parent().unwrap_or(executable);
    let expected = digest
        .strip_prefix("sha256:")
        .ok_or(UpdateFailure::ChecksumUnavailable)?;
    let bytes = fs::read(download).map_err(|err| UpdateFailure::Other(err.to_string()))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(UpdateFailure::ChecksumMismatch);
    }
    let permissions = fs::metadata(executable)
        .map_err(|err| io_failure(err, dir))?
        .permissions();
    fs::set_permissions(download, permissions).map_err(|err| io_failure(err, dir))?;
    File::open(download)
        .and_then(|file| file.sync_all())
        .map_err(|err| io_failure(err, dir))?;
    // Same filesystem: replacement is atomic; failures leave the existing binary intact.
    fs::rename(download, executable).map_err(|err| io_failure(err, dir))
}

/// Takes the update lock next to `executable`, downloads into a staging dir there and
/// replaces the binary only after the SHA-256 digest matches.
fn download_and_install(
    executable: &Path,
    digest: &str,
    download: impl FnOnce(&Path) -> Result<(), UpdateFailure>,
) -> Result<(), UpdateFailure> {
    let parent = executable
        .parent()
        .ok_or_else(|| UpdateFailure::Other("invalid executable path".into()))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join(".diffv-update.lock"))
        .map_err(|err| io_failure(err, parent))?;
    if lock.try_lock_exclusive().is_err() {
        return Err(UpdateFailure::Locked);
    }
    let result = tempfile::tempdir_in(parent)
        .map_err(|err| io_failure(err, parent))
        .and_then(|staging| {
            let file = staging.path().join("diffv");
            download(&file)?;
            install_verified(&file, executable, digest)
        });
    // Unlock explicitly: closing our fd is not enough while a child spawned concurrently by
    // another thread still holds an inherited copy (until its exec closes it), so a plain
    // drop can leave the lock held for a moment and the next updater would see `Locked`.
    let _ = lock.unlock();
    result
}

use crate::config::UpdateChannel;
use crate::core::models::Language;

pub fn check_and_install(channel: UpdateChannel) -> Result<Option<PathBuf>> {
    let outcome = check_and_install_internal(channel, false, &mut |_| {})?;
    Ok(outcome.map(|(path, _)| path))
}

/// Explicit `diffv update`: progress goes to stderr; returns the installed path and build.
pub fn check_and_install_verbose(channel: UpdateChannel) -> Result<Option<(PathBuf, BuildInfo)>> {
    Ok(check_and_install_internal(channel, true, &mut |build| {
        eprintln!("Updating diffv ({}) → {}…", channel.as_str(), build.label());
    })?)
}

/// Startup check run in the background while the TUI is on screen: never writes to
/// the terminal, every step that matters is handed to `report` so the TUI can show it.
pub fn check_on_startup(channel: UpdateChannel, mut report: impl FnMut(UpdateEvent)) {
    let result = check_and_install_internal(channel, false, &mut |build| {
        report(UpdateEvent::Downloading(build.clone()))
    });
    match result {
        Ok(Some((_, build))) => report(UpdateEvent::Installed(build)),
        Ok(None) => {}
        Err(err) => report(UpdateEvent::Failed {
            channel: err.channel,
            failure: err.failure,
            target: err.target,
        }),
    }
}

pub fn is_dev_executable(executable: &Path) -> bool {
    cfg!(debug_assertions)
        || std::env::var_os("CARGO").is_some()
        || std::env::var_os("CARGO_MANIFEST_DIR").is_some()
        || executable.components().any(|c| c.as_os_str() == "target")
}

fn fetch_release(
    channel: UpdateChannel,
    json: &Path,
    timeout: Duration,
) -> Result<(), UpdateFailure> {
    let url = |tag: &str| match tag {
        "latest" => format!("https://api.github.com/repos/{}/releases/latest", REPO),
        tag => format!(
            "https://api.github.com/repos/{}/releases/tags/{}",
            REPO, tag
        ),
    };
    // Beta and Nightly fall back to each other while only one rolling tag exists.
    let (primary, fallback) = match channel {
        UpdateChannel::Stable => return fetch(curl(&url("latest")), json, timeout),
        UpdateChannel::Beta => ("beta", "nightly"),
        UpdateChannel::Nightly => ("nightly", "beta"),
    };
    let Err(first) = fetch(curl(&url(primary)), json, timeout) else {
        return Ok(());
    };
    match fetch(curl(&url(fallback)), json, timeout) {
        Ok(()) => Ok(()),
        // Both tags answered with an HTTP error: the release is missing. Otherwise keep
        // the real cause (offline, timeout) instead of claiming there is no release.
        Err(UpdateFailure::Http) if first == UpdateFailure::Http => Err(UpdateFailure::NoRelease),
        Err(second) if first == UpdateFailure::Http => Err(second),
        Err(_) => Err(first),
    }
}

fn check_and_install_internal(
    channel: UpdateChannel,
    verbose: bool,
    on_download: &mut dyn FnMut(&BuildInfo),
) -> Result<Option<(PathBuf, BuildInfo)>, UpdateError> {
    let fail = |failure: UpdateFailure, target: Option<&BuildInfo>| UpdateError {
        channel,
        failure,
        target: target.cloned(),
    };
    let other = |err: &dyn std::fmt::Display| fail(UpdateFailure::Other(err.to_string()), None);

    let executable = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|err| other(&format!("cannot locate the diffv binary: {}", err)))?;

    // Not failures: development builds are protected on purpose and unsupported
    // platforms have nothing to install, so the startup check stays silent.
    if is_dev_executable(&executable) {
        if verbose {
            eprintln!(
                "diffv: Running from a development build ({}). Skipping update to protect local build.",
                executable.display()
            );
        }
        return Ok(None);
    }

    let Some(name) = platform_asset() else {
        if verbose {
            eprintln!("diffv: No precompiled binary available for this platform.");
        }
        return Ok(None);
    };
    let metadata = tempfile::tempdir().map_err(|err| other(&err))?;
    let json = metadata.path().join("release.json");
    if verbose {
        eprintln!("Checking for updates on {} channel...", channel.as_str());
    }

    let fetch_timeout = Duration::from_secs(if verbose { 10 } else { 3 });
    fetch_release(channel, &json, fetch_timeout).map_err(|failure| fail(failure, None))?;
    let release: Release = fs::read(&json)
        .map_err(|err| other(&err))
        .and_then(|bytes| {
            serde_json::from_slice(&bytes)
                .map_err(|err| other(&format!("unexpected GitHub response: {}", err)))
        })?;

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| fail(UpdateFailure::NoPlatformAsset, None))?;
    let build = BuildInfo::from_release(channel, &release, asset);
    let digest = || {
        asset
            .digest
            .as_deref()
            .ok_or_else(|| fail(UpdateFailure::ChecksumUnavailable, Some(&build)))
    };

    let should_update = match channel {
        UpdateChannel::Stable => {
            newer_release(&release, env!("CARGO_PKG_VERSION")).map_err(|err| other(&err))?
        }
        UpdateChannel::Beta | UpdateChannel::Nightly => {
            let digest = digest()?;
            let expected = digest.strip_prefix("sha256:").unwrap_or(digest);
            let current_bytes = fs::read(&executable).map_err(|err| other(&err))?;
            let current_digest = format!("{:x}", Sha256::digest(&current_bytes));
            !current_digest.eq_ignore_ascii_case(expected)
        }
    };

    if !should_update {
        return Ok(None);
    }

    let digest = digest()?;
    let prefix = format!("https://github.com/{}/releases/download/", REPO);
    if !asset.browser_download_url.starts_with(&prefix) {
        return Err(fail(
            UpdateFailure::Other("unexpected release download URL".into()),
            Some(&build),
        ));
    }
    download_and_install(&executable, digest, |file| {
        on_download(&build);
        fetch(
            curl(&asset.browser_download_url),
            file,
            Duration::from_secs(90),
        )
    })
    .map_err(|failure| fail(failure, Some(&build)))?;
    Ok(Some((executable, build)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            target_commitish: None,
            body: None,
            assets: vec![],
        }
    }
    #[test]
    fn only_newer_stable_versions_are_installed() {
        assert!(newer_release(&release("v0.10.0"), "0.2.0").unwrap());
        assert!(!newer_release(&release("v0.2.0"), "0.2.0").unwrap());
        assert!(!newer_release(&release("v0.1.0"), "0.2.0").unwrap());
        assert!(!newer_release(&release("v1.0.0-beta.1"), "0.2.0").unwrap());
        let mut draft = release("v1.0.0");
        draft.draft = true;
        assert!(!newer_release(&draft, "0.2.0").unwrap());
    }
    #[test]
    fn bad_checksum_preserves_installed_binary() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("installed");
        let download = dir.path().join("download");
        fs::write(&installed, "old").unwrap();
        fs::write(&download, "new").unwrap();
        assert_eq!(
            install_verified(&download, &installed, "sha256:bad"),
            Err(UpdateFailure::ChecksumMismatch)
        );
        assert_eq!(
            install_verified(&download, &installed, "md5:abc"),
            Err(UpdateFailure::ChecksumUnavailable)
        );
        assert_eq!(fs::read(&installed).unwrap(), b"old");
        let digest = format!("sha256:{:x}", Sha256::digest(b"new"));
        install_verified(&download, &installed, &digest).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), b"new");
    }
    #[test]
    fn network_timeout_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut command = Command::new("sleep");
        command.arg("5");
        let started = Instant::now();
        assert!(fetch(
            command,
            &dir.path().join("output"),
            Duration::from_millis(30)
        )
        .is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    fn exit_with(code: i32) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", &format!("exit {}", code)]);
        command
    }
    #[test]
    fn network_failures_keep_their_cause() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("output");
        let timeout = Duration::from_secs(5);
        // curl exit codes: 6 resolve host, 7 connect, 28 timeout, 22 HTTP error
        assert_eq!(
            fetch(exit_with(6), &out, timeout),
            Err(UpdateFailure::Offline)
        );
        assert_eq!(
            fetch(exit_with(7), &out, timeout),
            Err(UpdateFailure::Offline)
        );
        assert_eq!(
            fetch(exit_with(28), &out, timeout),
            Err(UpdateFailure::Timeout)
        );
        assert_eq!(
            fetch(exit_with(22), &out, timeout),
            Err(UpdateFailure::Http)
        );
        let mut sleep = Command::new("sleep");
        sleep.arg("5");
        assert_eq!(
            fetch(sleep, &out, Duration::from_millis(30)),
            Err(UpdateFailure::Timeout)
        );
    }
    fn installed_binary(content: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("diffv");
        fs::write(&executable, content).unwrap();
        (dir, executable)
    }
    fn sha(content: &[u8]) -> String {
        format!("sha256:{:x}", Sha256::digest(content))
    }
    #[test]
    fn install_replaces_binary_only_after_verified_download() {
        let (_dir, executable) = installed_binary(b"old");
        let mut downloaded = false;
        download_and_install(&executable, &sha(b"new"), |file| {
            downloaded = true;
            fs::write(file, b"new").map_err(|e| UpdateFailure::Other(e.to_string()))
        })
        .unwrap();
        assert!(downloaded);
        assert_eq!(fs::read(&executable).unwrap(), b"new");

        // Tampered download: refused, installed binary untouched
        assert_eq!(
            download_and_install(&executable, &sha(b"newer"), |file| {
                fs::write(file, b"evil").map_err(|e| UpdateFailure::Other(e.to_string()))
            }),
            Err(UpdateFailure::ChecksumMismatch)
        );
        assert_eq!(fs::read(&executable).unwrap(), b"new");

        // Network failure during the download is reported as such
        assert_eq!(
            download_and_install(&executable, &sha(b"newer"), |file| fetch(
                exit_with(7),
                file,
                Duration::from_secs(5)
            )),
            Err(UpdateFailure::Offline)
        );
        assert_eq!(fs::read(&executable).unwrap(), b"new");
    }
    #[test]
    fn busy_update_lock_is_reported() {
        let (dir, executable) = installed_binary(b"old");
        let held = File::create(dir.path().join(".diffv-update.lock")).unwrap();
        held.lock_exclusive().unwrap();
        assert_eq!(
            download_and_install(&executable, &sha(b"new"), |_| panic!("must not download")),
            Err(UpdateFailure::Locked)
        );
        assert_eq!(fs::read(&executable).unwrap(), b"old");
    }
    #[test]
    fn update_lock_is_released_while_other_threads_spawn_processes() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        // Children spawned by other threads briefly inherit our lock fd; before the explicit
        // unlock this made back-to-back updates in one test fail with `Locked`.
        let stop = Arc::new(AtomicBool::new(false));
        let spawner = {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let _ = exit_with(0).status();
                }
            })
        };
        let (_dir, executable) = installed_binary(b"old");
        let results: Vec<_> = (0..300)
            .map(|_| {
                download_and_install(&executable, &sha(b"new"), |file| {
                    fs::write(file, b"new").map_err(|e| UpdateFailure::Other(e.to_string()))
                })
            })
            .collect();
        stop.store(true, Ordering::Relaxed);
        spawner.join().unwrap();
        assert!(results.iter().all(Result::is_ok), "{:?}", results);
    }
    #[cfg(unix)]
    #[test]
    fn read_only_install_dir_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, executable) = installed_binary(b"old");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        // root ignores directory permissions; nothing to assert there
        let writable = fs::write(dir.path().join("probe"), b"").is_ok();
        let result =
            download_and_install(&executable, &sha(b"new"), |_| panic!("must not download"));
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        if !writable {
            assert_eq!(
                result,
                Err(UpdateFailure::NotWritable(dir.path().to_path_buf()))
            );
            assert_eq!(fs::read(&executable).unwrap(), b"old");
        }
    }
    #[test]
    fn build_info_identifies_the_rolling_build() {
        let release: Release = serde_json::from_str(
            r#"{"tag_name":"nightly","draft":false,"prerelease":true,"target_commitish":"main",
            "body":"Automated rolling nightly build from the latest commit on main (b97105d).",
            "assets":[{"name":"diffv-x86_64-unknown-linux-gnu",
            "browser_download_url":"https://github.com/felipe-godoi/diffv/releases/download/nightly/x",
            "digest":"sha256:00","updated_at":"2026-10-06T21:21:20Z"}]}"#,
        )
        .unwrap();
        let build = BuildInfo::from_release(UpdateChannel::Nightly, &release, &release.assets[0]);
        assert_eq!(build.commit.as_deref(), Some("b97105d"));
        assert_eq!(build.label(), "nightly · b97105d · 2026-10-06 21:21 UTC");
        // Words made only of hex letters ("acceded", "defaced") are not commits
        assert_eq!(commit_in_text("an acceded and defaced release"), None);
    }
    #[test]
    fn failures_are_explained_in_both_languages() {
        let failures = [
            UpdateFailure::Offline,
            UpdateFailure::Timeout,
            UpdateFailure::Http,
            UpdateFailure::NoRelease,
            UpdateFailure::NoPlatformAsset,
            UpdateFailure::ChecksumUnavailable,
            UpdateFailure::ChecksumMismatch,
            UpdateFailure::NotWritable("/opt/bin".into()),
            UpdateFailure::Locked,
            UpdateFailure::Other("boom".into()),
        ];
        for failure in failures {
            let en = failure.describe(UpdateChannel::Nightly, Language::En);
            let pt = failure.describe(UpdateChannel::Nightly, Language::Pt);
            assert!(
                !en.is_empty() && !pt.is_empty() && en != pt,
                "{:?}",
                failure
            );
        }
    }
    #[test]
    fn test_is_dev_executable() {
        let dev_path = PathBuf::from("/home/user/project/target/debug/diffv");
        assert!(is_dev_executable(&dev_path));
        let release_target_path = PathBuf::from("/home/user/project/target/release/diffv");
        assert!(is_dev_executable(&release_target_path));
    }
}
