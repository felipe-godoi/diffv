use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};

pub enum WatchEvent {
    ReloadRequested,
}

pub struct WatchService {
    stop_tx: Option<Sender<()>>,
    _thread: Option<thread::JoinHandle<()>>,
}

impl Drop for WatchService {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }
}

const COMMON_IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".worktree",
    ".venv",
    "venv",
    "vendor",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    ".output",
];

impl WatchService {
    /// Starts the filesystem watcher in a dedicated background thread so the
    /// main thread is never blocked, ensuring immediate UI render and responsive inputs.
    /// Uses `.gitignore` rules (via `ignore::WalkBuilder`) to prune ignored directories.
    pub fn start(
        watch_path: &Path,
        debounce_ms: u64,
        tx: Sender<WatchEvent>,
    ) -> anyhow::Result<Self> {
        let watch_path = watch_path.to_path_buf();
        let (stop_tx, stop_rx) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("diffv-watcher".to_string())
            .spawn(move || {
                let repo_root = watch_path.clone();
                let tx_clone = tx.clone();

                let mut debouncer = match new_debouncer(
                    Duration::from_millis(debounce_ms),
                    move |res: Result<Vec<DebouncedEvent>, _>| {
                        if let Ok(events) = res {
                            if is_relevant(&repo_root, events.iter().map(|e| e.path.as_path())) {
                                let _ = tx_clone.send(WatchEvent::ReloadRequested);
                            }
                        }
                    },
                ) {
                    Ok(d) => d,
                    Err(_) => return,
                };

                setup_watches(&watch_path, debouncer.watcher());

                // Keep debouncer alive until Stop signal is received when diffv exits
                let _ = stop_rx.recv();
            })?;

        Ok(Self {
            stop_tx: Some(stop_tx),
            _thread: Some(thread),
        })
    }
}

/// Recursively registers non-ignored directories using `.gitignore` patterns.
fn setup_watches(watch_path: &Path, watcher: &mut dyn Watcher) {
    if !watch_path.is_dir() {
        let _ = watcher.watch(watch_path, RecursiveMode::NonRecursive);
        return;
    }

    // Build directory walker that strictly respects .gitignore and global/exclude git ignores
    let mut builder = ignore::WalkBuilder::new(watch_path);
    builder
        .hidden(false) // Don't skip dotfiles like .cursor or .github unless gitignored
        .git_ignore(true) // Respect .gitignore at all levels
        .git_global(true) // Respect global gitignore
        .git_exclude(true) // Respect .git/info/exclude
        .require_git(false)
        .filter_entry(|entry| {
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let name = entry.file_name().to_string_lossy();
                // Never descend into internal .git directory
                if name == ".git" {
                    return false;
                }
                // Prune known heavy directories even if not in .gitignore
                if COMMON_IGNORED_DIRS.iter().any(|&ign| name == ign) {
                    return false;
                }
            }
            true
        });

    for result in builder.build().flatten() {
        if result.file_type().is_some_and(|ft| ft.is_dir()) {
            let _ = watcher.watch(result.path(), RecursiveMode::NonRecursive);
        }
    }

    // Watch .git metadata (HEAD, index, and refs) for staging, commits, and branch changes
    let git_dir = if watch_path.join(".git").is_dir() {
        Some(watch_path.join(".git"))
    } else if watch_path.join(".git").is_file() {
        // Worktree gitdir pointer
        std::fs::read_to_string(watch_path.join(".git"))
            .ok()
            .and_then(|c| {
                c.lines().next().and_then(|line| {
                    line.strip_prefix("gitdir:")
                        .map(|s| PathBuf::from(s.trim()))
                })
            })
    } else {
        None
    };

    if let Some(gd) = git_dir {
        // Watch .git root non-recursively for HEAD and index
        let _ = watcher.watch(&gd, RecursiveMode::NonRecursive);
        // Watch .git/refs recursively for branch/tag updates
        let refs = gd.join("refs");
        if refs.is_dir() {
            let _ = watcher.watch(&refs, RecursiveMode::Recursive);
        }
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

    #[test]
    fn test_watch_service_nonblocking_start() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = mpsc::channel();
        let start = std::time::Instant::now();
        let service = WatchService::start(dir.path(), 100, tx);
        assert!(service.is_ok());
        // Must return almost instantaneously (under 50ms)
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
