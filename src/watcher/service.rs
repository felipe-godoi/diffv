use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Duration;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};

pub enum WatchEvent {
    ReloadRequested,
}

pub struct WatchService {
    // Keep debouncer alive
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

impl WatchService {
    pub fn start(
        watch_path: &Path,
        debounce_ms: u64,
        tx: Sender<WatchEvent>,
    ) -> anyhow::Result<Self> {
        let repo_root = watch_path.to_path_buf();
        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            move |res: Result<Vec<DebouncedEvent>, _>| {
                // Errors are dropped: printing would corrupt the TUI.
                if let Ok(events) = res {
                    if is_relevant(&repo_root, events.iter().map(|e| e.path.as_path())) {
                        let _ = tx.send(WatchEvent::ReloadRequested);
                    }
                }
            },
        )?;

        debouncer.watcher().watch(watch_path, RecursiveMode::Recursive)?;

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}

/// Index/HEAD/ref updates (staging or committing elsewhere) count; other
/// `.git` internals and gitignored paths (build output, node_modules) don't.
fn is_relevant<'a>(repo_root: &Path, paths: impl Iterator<Item = &'a Path>) -> bool {
    let mut worktree_paths: Vec<PathBuf> = Vec::new();
    for path in paths {
        let in_git_dir = path.components().any(|c| c.as_os_str() == ".git");
        if in_git_dir {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let path_str = path.to_string_lossy();
            if name == "index" || name == "HEAD" || path_str.contains("/.git/refs/") {
                return true;
            }
        } else {
            worktree_paths.push(path.to_path_buf());
        }
    }
    if worktree_paths.is_empty() {
        return false;
    }
    !all_ignored(repo_root, &worktree_paths)
}

fn all_ignored(repo_root: &Path, paths: &[PathBuf]) -> bool {
    let Ok(mut child) = Command::new("git")
        .args(["check-ignore", "-z", "--stdin"])
        .current_dir(repo_root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        for path in paths {
            let _ = stdin.write_all(path.to_string_lossy().as_bytes());
            let _ = stdin.write_all(b"\0");
        }
    }
    let Ok(output) = child.wait_with_output() else { return false; };
    let ignored = output.stdout.split(|&b| b == 0).filter(|s| !s.is_empty()).count();
    ignored == paths.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_build_output_and_git_internals_but_not_index() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(root).output().unwrap().status.success());
        git(&["init"]);
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let target = root.join("target/debug/x");
        let source = root.join("src.rs");
        assert!(!is_relevant(root, [target.as_path()].into_iter()));
        assert!(!is_relevant(root, [root.join(".git/objects/ab").as_path()].into_iter()));
        assert!(is_relevant(root, [root.join(".git/index").as_path()].into_iter()));
        assert!(is_relevant(root, [target.as_path(), source.as_path()].into_iter()));
    }
}
