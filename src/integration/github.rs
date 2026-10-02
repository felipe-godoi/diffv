use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `https://github.com/<owner>/<repo>` from an https or ssh GitHub remote.
pub fn parse_github_url(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let path = remote
        .strip_prefix("https://github.com/")
        .or_else(|| remote.strip_prefix("http://github.com/"))
        .or_else(|| remote.strip_prefix("git@github.com:"))
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("https://github.com/{}/{}", owner, repo))
}

pub fn github_repo_url(repo_root: &Path) -> Option<String> {
    let output = git(repo_root, &["remote", "get-url", "origin"])?;
    parse_github_url(&output)
}

/// Opens the pull request that introduced `commit` (falling back to the
/// commit page) in the browser. Runs in the background so the TUI never
/// blocks on the network.
pub fn open_commit_pr(repo_root: PathBuf, repo_url: String, commit: String) {
    std::thread::spawn(move || {
        let sha = git(&repo_root, &["rev-parse", &commit]).unwrap_or(commit);
        let url = find_pr_url(&repo_root, &sha).unwrap_or_else(|| format!("{}/commit/{}", repo_url, sha));
        open_url(&url);
    });
}

fn find_pr_url(repo_root: &Path, sha: &str) -> Option<String> {
    let output = Command::new("gh")
        .args(["api", &format!("repos/{{owner}}/{{repo}}/commits/{}/pulls", sha), "--jq", ".[0].html_url // empty"])
        .current_dir(repo_root)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && url.starts_with("https://")).then_some(url)
}

fn open_url(url: &str) {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = Command::new(opener).arg(url).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

fn git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).current_dir(repo_root).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remotes() {
        let expected = Some("https://github.com/felipe-godoi/diffv".to_string());
        assert_eq!(parse_github_url("https://github.com/felipe-godoi/diffv.git"), expected);
        assert_eq!(parse_github_url("git@github.com:felipe-godoi/diffv.git"), expected);
        assert_eq!(parse_github_url("ssh://git@github.com/felipe-godoi/diffv"), expected);
        assert_eq!(parse_github_url("https://gitlab.com/a/b.git"), None);
    }
}
