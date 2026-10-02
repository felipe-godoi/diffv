use std::io::Write;
use std::process::{Command, Stdio};

pub fn is_fzf_available() -> bool {
    Command::new("fzf")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn search_files_fzf(files: &[String]) -> anyhow::Result<Option<String>> {
    let mut child = Command::new("fzf")
        .args([
            "--prompt=󰈞 Files> ",
            "--layout=reverse",
            "--border=rounded",
            "--info=inline",
            "--header=Select a file to open diff (Esc to cancel)",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        for f in files {
            let _ = writeln!(stdin, "{}", f);
        }
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        let sel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !sel.is_empty() {
            return Ok(Some(sel));
        }
    }
    if !matches!(output.status.code(), Some(0 | 1 | 130)) {
        anyhow::bail!("fzf exited with {}", output.status);
    }
    Ok(None)
}

pub fn search_diff_text_fzf(lines: &[String]) -> anyhow::Result<Option<String>> {
    let mut child = Command::new("fzf")
        .args([
            "--prompt=󰈞 Diff Text> ",
            "--layout=reverse",
            "--border=rounded",
            "--info=inline",
            "--header=Search changes across all diffs (Esc to cancel)",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        for l in lines {
            let _ = writeln!(stdin, "{}", l);
        }
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        let sel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !sel.is_empty() {
            return Ok(Some(sel));
        }
    }
    if !matches!(output.status.code(), Some(0 | 1 | 130)) {
        anyhow::bail!("fzf exited with {}", output.status);
    }
    Ok(None)
}
