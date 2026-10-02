use crate::core::models::Hunk;
use crate::git::patch::{
    generate_hunk_patch, generate_partial_hunk_patch, generate_partial_unstage_patch,
};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn stage_partial_hunk(
    repo_root: &Path,
    file_path: &Path,
    hunk: &Hunk,
    selected_indices: &[usize],
) -> anyhow::Result<()> {
    let patch = generate_partial_hunk_patch(file_path, hunk, selected_indices);
    apply_to_index(repo_root, &patch, &[])
        .map_err(|e| anyhow::anyhow!("Failed to stage partial hunk: {}", e))
}

pub fn unstage_partial_hunk(
    repo_root: &Path,
    file_path: &Path,
    hunk: &Hunk,
    selected_indices: &[usize],
) -> anyhow::Result<()> {
    let patch = generate_partial_unstage_patch(file_path, hunk, selected_indices);
    apply_to_index(repo_root, &patch, &["--reverse"])
        .map_err(|e| anyhow::anyhow!("Failed to unstage partial hunk: {}", e))
}

fn apply_to_index(repo_root: &Path, patch: &str, extra: &[&str]) -> anyhow::Result<()> {
    let mut child = Command::new("git")
        .args(["apply", "--cached", "--unidiff-zero"])
        .args(extra)
        .arg("-")
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(patch.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub fn stage_hunk(repo_root: &Path, file_path: &Path, hunk: &Hunk) -> anyhow::Result<()> {
    let patch = generate_hunk_patch(file_path, hunk);
    let mut child = Command::new("git")
        .args(["apply", "--cached", "--unidiff-zero", "-"])
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(patch.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to stage hunk: {}", err_msg);
    }
    Ok(())
}

pub fn unstage_hunk(repo_root: &Path, file_path: &Path, hunk: &Hunk) -> anyhow::Result<()> {
    let patch = generate_hunk_patch(file_path, hunk);
    let mut child = Command::new("git")
        .args(["apply", "--cached", "--reverse", "--unidiff-zero", "-"])
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(patch.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to unstage hunk: {}", err_msg);
    }
    Ok(())
}

pub fn discard_hunk(repo_root: &Path, file_path: &Path, hunk: &Hunk) -> anyhow::Result<()> {
    let patch = generate_hunk_patch(file_path, hunk);
    let mut child = Command::new("git")
        .args(["apply", "--reverse", "--unidiff-zero", "-"])
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(patch.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to discard hunk: {}", err_msg);
    }
    Ok(())
}

pub fn stage_file(repo_root: &Path, file_path: &Path) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(["add", "--"])
        .arg(file_path)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to stage file: {}", err_msg);
    }
    Ok(())
}

pub fn unstage_file(repo_root: &Path, file_path: &Path) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(["restore", "--staged", "--"])
        .arg(file_path)
        .current_dir(repo_root)
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        _ => {
            // Fallback for older git
            let fallback = Command::new("git")
                .args(["reset", "HEAD", "--"])
                .arg(file_path)
                .current_dir(repo_root)
                .output()?;
            if !fallback.status.success() {
                let err_msg = String::from_utf8_lossy(&fallback.stderr);
                anyhow::bail!("Failed to unstage file: {}", err_msg);
            }
            Ok(())
        }
    }
}

pub fn discard_file(repo_root: &Path, file_path: &Path, is_untracked: bool) -> anyhow::Result<()> {
    if is_untracked {
        let full_path = repo_root.join(file_path);
        if full_path.is_file() {
            std::fs::remove_file(full_path)?;
        } else if full_path.is_dir() {
            std::fs::remove_dir_all(full_path)?;
        }
        return Ok(());
    }

    let output = Command::new("git")
        .args(["restore", "--"])
        .arg(file_path)
        .current_dir(repo_root)
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        _ => {
            let fallback = Command::new("git")
                .args(["checkout", "HEAD", "--"])
                .arg(file_path)
                .current_dir(repo_root)
                .output()?;
            if !fallback.status.success() {
                let err_msg = String::from_utf8_lossy(&fallback.stderr);
                anyhow::bail!("Failed to discard file changes: {}", err_msg);
            }
            Ok(())
        }
    }
}
