use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

const POPUP_SESSION_PREFIX: &str = "diffv-popup-";

pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

pub fn get_tmux_popup_snippet() -> &'static str {
    r##"# Add to your ~/.tmux.conf:
# Prefix + d shows/hides diffv in a floating popup (state is kept while hidden):
bind-key d run-shell -b "diffv --tmux-toggle '#{client_name}' '#{session_name}' '#{pane_current_path}'"
"##
}

/// One background session per project; tmux session names cannot contain `.` or `:`.
pub fn popup_session_name(path: &Path) -> String {
    let slug: String = path
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    format!("{}{}", POPUP_SESSION_PREFIX, slug.trim_matches('_'))
}

/// Pressed inside the popup: detach its client, which hides the popup while
/// diffv keeps running. Pressed elsewhere: start (or reuse) the project's
/// session and attach to it in a popup.
pub fn toggle_popup(client: &str, session: &str, path: &Path) -> Result<()> {
    if session.starts_with(POPUP_SESSION_PREFIX) {
        return tmux(&["detach-client", "-t", client]);
    }

    let name = popup_session_name(path);
    let target = format!("={}", name);
    let exists = Command::new("tmux")
        .args(["has-session", "-t", &target])
        .output()
        .context("Failed to run tmux")?
        .status
        .success();
    if !exists {
        let exe = std::env::current_exe().context("Cannot locate the diffv executable")?;
        let command = format!("{} --watch --wait-on-error", shell_quote(&exe.to_string_lossy()));
        let dir = path.to_string_lossy();
        tmux(&["new-session", "-d", "-s", &name, "-c", &dir, &command])?;
        tmux(&["set-option", "-t", &name, "status", "off"])?;
    }

    let socket = std::env::var("TMUX").ok().and_then(|v| v.split(',').next().map(str::to_string));
    let socket_arg = socket.map(|s| format!(" -S {}", shell_quote(&s))).unwrap_or_default();
    let attach = format!("TMUX= tmux{} attach-session -t {}", socket_arg, shell_quote(&target));
    tmux(&["display-popup", "-c", client, "-w", "92%", "-h", "90%", "-E", &attach])
}

fn tmux(args: &[&str]) -> Result<()> {
    let output = Command::new("tmux").args(args).output().context("Failed to run tmux")?;
    if !output.status.success() {
        bail!("tmux {} failed: {}", args[0], String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_names_are_tmux_safe() {
        assert_eq!(popup_session_name(Path::new("/Users/me/repo.rs:x")), "diffv-popup-Users_me_repo_rs_x");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
