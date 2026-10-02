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

pub fn search_files_fzf(files: &[String], header: &str) -> anyhow::Result<Option<String>> {
    run_fzf(files, "󰈞 Files> ", header, &[])
}

/// Candidates are `path:line<TAB>content`; only the content is matched.
pub fn search_diff_text_fzf(lines: &[String], header: &str) -> anyhow::Result<Option<String>> {
    run_fzf(lines, "󰈞 Diff Text> ", header, &["--delimiter=\t", "--nth=2", "--with-nth=1,2,3", "--tabstop=2"])
}

fn run_fzf(items: &[String], prompt: &str, header: &str, extra: &[&str]) -> anyhow::Result<Option<String>> {
    let mut child = Command::new("fzf")
        .args(["--layout=reverse", "--border=rounded", "--info=inline"])
        .arg(format!("--prompt={}", prompt))
        .arg(format!("--header={}", header))
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            let _ = writeln!(stdin, "{}", item);
        }
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        let sel = String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']).to_string();
        if !sel.is_empty() {
            return Ok(Some(sel));
        }
    }
    if !matches!(output.status.code(), Some(0 | 1 | 130)) {
        anyhow::bail!("fzf exited with {}", output.status);
    }
    Ok(None)
}
