use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
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
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
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

// Redirect into a file rather than a pipe so large downloads cannot deadlock.
fn fetch(mut command: Command, destination: &Path, timeout: Duration) -> Result<()> {
    let file = File::create(destination)?;
    let mut child = command.stdin(Stdio::null()).stdout(file).stderr(Stdio::null()).spawn()?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() { bail!("GitHub request failed ({})", status); }
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("GitHub request timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn curl(url: &str) -> Command {
    let mut command = Command::new("curl");
    command.args(["--fail", "--silent", "--location", "--proto", "=https", "--proto-redir", "=https", "--connect-timeout", "2", "--max-time", "90", "--user-agent", "diffv-updater", url]);
    command
}

fn newer_release(release: &Release, current: &str) -> Result<bool> {
    let version = Version::parse(release.tag_name.trim_start_matches('v'))?;
    Ok(!release.draft && !release.prerelease && version.pre.is_empty() && version > Version::parse(current)?)
}

fn install_verified(download: &Path, executable: &Path, digest: &str) -> Result<()> {
    let expected = digest.strip_prefix("sha256:").context("Release asset has no SHA-256 digest")?;
    let actual = format!("{:x}", Sha256::digest(fs::read(download)?));
    if actual != expected { bail!("Downloaded binary failed SHA-256 verification"); }
    fs::set_permissions(download, fs::metadata(executable)?.permissions())?;
    File::open(download)?.sync_all()?;
    // Same filesystem: replacement is atomic; failures leave the existing binary intact.
    fs::rename(download, executable).context("Could not replace diffv (check installation directory permissions)")?;
    Ok(())
}

use crate::config::UpdateChannel;

pub fn check_and_install(channel: UpdateChannel) -> Result<Option<PathBuf>> {
    check_and_install_internal(channel, false)
}

pub fn check_and_install_verbose(channel: UpdateChannel) -> Result<Option<PathBuf>> {
    check_and_install_internal(channel, true)
}

fn check_and_install_internal(channel: UpdateChannel, verbose: bool) -> Result<Option<PathBuf>> {
    let Some(name) = platform_asset() else {
        if verbose {
            eprintln!("diffv: No precompiled binary available for this platform.");
        }
        return Ok(None);
    };
    let metadata = tempfile::tempdir()?;
    let json = metadata.path().join("release.json");
    if verbose {
        eprintln!("Checking for updates on {} channel...", channel.as_str());
    }

    let fetch_timeout = Duration::from_secs(if verbose { 10 } else { 3 });

    let fetch_res = match channel {
        UpdateChannel::Stable => {
            fetch(curl(&format!("https://api.github.com/repos/{}/releases/latest", REPO)), &json, fetch_timeout)
        }
        UpdateChannel::Beta => {
            // Try explicit 'beta' tag, fallback to 'nightly' if no separate beta tag
            let beta_res = fetch(curl(&format!("https://api.github.com/repos/{}/releases/tags/beta", REPO)), &json, fetch_timeout);
            if beta_res.is_err() {
                fetch(curl(&format!("https://api.github.com/repos/{}/releases/tags/nightly", REPO)), &json, fetch_timeout)
            } else {
                beta_res
            }
        }
        UpdateChannel::Nightly => {
            // Try 'nightly' tag, fallback to 'beta'
            let nightly_res = fetch(curl(&format!("https://api.github.com/repos/{}/releases/tags/nightly", REPO)), &json, fetch_timeout);
            if nightly_res.is_err() {
                fetch(curl(&format!("https://api.github.com/repos/{}/releases/tags/beta", REPO)), &json, fetch_timeout)
            } else {
                nightly_res
            }
        }
    };

    if let Err(err) = fetch_res {
        match channel {
            UpdateChannel::Beta => bail!("No beta pre-release found on GitHub yet."),
            UpdateChannel::Nightly => bail!("No nightly build found on GitHub yet (nightly builds are generated on push to main)."),
            UpdateChannel::Stable => return Err(err),
        }
    }
    let release: Release = serde_json::from_slice(&fs::read(&json)?)?;

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .context("No release binary for this platform")?;
    let digest = asset
        .digest
        .as_deref()
        .context("Release asset checksum unavailable")?;

    let executable = std::env::current_exe()?.canonicalize()?;

    let should_update = match channel {
        UpdateChannel::Stable => newer_release(&release, env!("CARGO_PKG_VERSION"))?,
        UpdateChannel::Beta | UpdateChannel::Nightly => {
            let expected = digest.strip_prefix("sha256:").unwrap_or(digest);
            let current_bytes = fs::read(&executable)?;
            let current_digest = format!("{:x}", Sha256::digest(&current_bytes));
            !current_digest.eq_ignore_ascii_case(expected)
        }
    };

    if !should_update {
        return Ok(None);
    }

    let parent = executable.parent().context("Invalid executable path")?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join(".diffv-update.lock"))?;
    if lock.try_lock_exclusive().is_err() {
        return Ok(None);
    }
    let staging = tempfile::tempdir_in(parent)?;
    let download = staging.path().join("diffv");
    eprintln!(
        "Updating diffv ({}) → {}…",
        channel.as_str(),
        release.tag_name
    );
    let prefix = format!("https://github.com/{}/releases/download/", REPO);
    if !asset.browser_download_url.starts_with(&prefix) {
        bail!("Unexpected release download URL");
    }
    fetch(
        curl(&asset.browser_download_url),
        &download,
        Duration::from_secs(90),
    )?;
    install_verified(&download, &executable, digest)?;
    Ok(Some(executable))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release {
        Release { tag_name: tag.into(), draft: false, prerelease: false, assets: vec![] }
    }
    #[test]
    fn only_newer_stable_versions_are_installed() {
        assert!(newer_release(&release("v0.10.0"), "0.2.0").unwrap());
        assert!(!newer_release(&release("v0.2.0"), "0.2.0").unwrap());
        assert!(!newer_release(&release("v0.1.0"), "0.2.0").unwrap());
        assert!(!newer_release(&release("v1.0.0-beta.1"), "0.2.0").unwrap());
        let mut draft = release("v1.0.0"); draft.draft = true;
        assert!(!newer_release(&draft, "0.2.0").unwrap());
    }
    #[test]
    fn bad_checksum_preserves_installed_binary() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("installed");
        let download = dir.path().join("download");
        fs::write(&installed, "old").unwrap(); fs::write(&download, "new").unwrap();
        assert!(install_verified(&download, &installed, "sha256:bad").is_err());
        assert_eq!(fs::read(&installed).unwrap(), b"old");
        let digest = format!("sha256:{:x}", Sha256::digest(b"new"));
        install_verified(&download, &installed, &digest).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), b"new");
    }
    #[test]
    fn network_timeout_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut command = Command::new("sleep"); command.arg("5");
        let started = Instant::now();
        assert!(fetch(command, &dir.path().join("output"), Duration::from_millis(30)).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
