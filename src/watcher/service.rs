use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};

pub enum WatchEvent {
    Scanning { scanned_dirs: usize },
    Ready { total_dirs: usize },
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
                let debouncer_tx = tx.clone();
                let mut debouncer = match new_debouncer(
                    Duration::from_millis(debounce_ms),
                    move |res: Result<Vec<DebouncedEvent>, _>| {
                        if let Ok(events) = res {
                            if is_relevant(&repo_root, events.iter().map(|e| e.path.as_path())) {
                                let _ = debouncer_tx.send(WatchEvent::ReloadRequested);
                            }
                        }
                    },
                ) {
                    Ok(d) => d,
                    Err(_) => return,
                };

                let _ = tx.send(WatchEvent::Scanning { scanned_dirs: 0 });
                setup_watches(&watch_path, debouncer.watcher(), &tx);

                // Keep debouncer alive until Stop signal is received when diffv exits
                let _ = stop_rx.recv();
            })?;

        Ok(Self {
            stop_tx: Some(stop_tx),
            _thread: Some(thread),
        })
    }
}

/// FSEvents (macOS) and ReadDirectoryChangesW (Windows) watch whole trees natively,
/// while notify restarts the FSEvents stream on every `watch` call, so registering
/// directories one by one is quadratic there. On those platforms the root gets one
/// recursive watch and ignored paths are filtered per event (`is_relevant`).
const NATIVE_RECURSIVE: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// Registers the watches: one recursive watch where the OS supports it, otherwise
/// every non-ignored directory (`.gitignore` aware), plus the git metadata.
fn setup_watches(watch_path: &Path, watcher: &mut dyn Watcher, tx: &Sender<WatchEvent>) {
    if !watch_path.is_dir() {
        let _ = watcher.watch(watch_path, RecursiveMode::NonRecursive);
        let _ = tx.send(WatchEvent::Ready { total_dirs: 1 });
        return;
    }
    if NATIVE_RECURSIVE {
        let _ = watcher.watch(watch_path, RecursiveMode::Recursive);
        // A linked worktree keeps its git dir elsewhere.
        if let Some(gd) = git_dir(watch_path).filter(|gd| !gd.starts_with(watch_path)) {
            watch_git_dir(&gd, watcher);
        }
        let _ = tx.send(WatchEvent::Ready { total_dirs: 1 });
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

    let mut dir_count = 0;
    for result in builder.build().flatten() {
        if result.file_type().is_some_and(|ft| ft.is_dir()) {
            let _ = watcher.watch(result.path(), RecursiveMode::NonRecursive);
            dir_count += 1;
            if dir_count % 35 == 0 {
                let _ = tx.send(WatchEvent::Scanning {
                    scanned_dirs: dir_count,
                });
            }
        }
    }

    // Watch .git metadata (HEAD, index, and refs) for staging, commits, and branch changes
    if let Some(gd) = git_dir(watch_path) {
        watch_git_dir(&gd, watcher);
    }

    let _ = tx.send(WatchEvent::Ready {
        total_dirs: dir_count,
    });
}

/// `.git`, or the directory a worktree's `.git` file points at.
fn git_dir(watch_path: &Path) -> Option<PathBuf> {
    let dot_git = watch_path.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    // Worktree gitdir pointer
    std::fs::read_to_string(&dot_git).ok().and_then(|c| {
        c.lines().next().and_then(|line| {
            line.strip_prefix("gitdir:")
                .map(|s| PathBuf::from(s.trim()))
        })
    })
}

fn watch_git_dir(gd: &Path, watcher: &mut dyn Watcher) {
    // Watch .git root non-recursively for HEAD and index
    let _ = watcher.watch(gd, RecursiveMode::NonRecursive);
    // Watch .git/refs recursively for branch/tag updates
    let refs = gd.join("refs");
    if refs.is_dir() {
        let _ = watcher.watch(&refs, RecursiveMode::Recursive);
    }
}

/// Paths under the directories the walker always prunes (dependencies, build
/// output); dropped before asking `git check-ignore`.
fn in_pruned_dir(repo_root: &Path, path: &Path) -> bool {
    // FSEvents reports resolved paths (`/private/var/…` for `/var/…`); only the part
    // inside the repository counts, never the folders above it.
    let relative = path.strip_prefix(repo_root).ok().or_else(|| {
        let root = repo_root.canonicalize().ok()?;
        path.strip_prefix(root).ok()
    });
    relative.is_some_and(|rel| {
        rel.components()
            .any(|c| COMMON_IGNORED_DIRS.iter().any(|&dir| c.as_os_str() == dir))
    })
}

/// Index/HEAD/ref updates (staging or committing elsewhere) count; other
/// `.git` internals and gitignored paths (build output, node_modules) don't.
fn is_relevant<'a>(repo_root: &Path, paths: impl Iterator<Item = &'a Path>) -> bool {
    let mut worktree_paths: Vec<PathBuf> = Vec::new();
    for path in paths {
        let in_git_dir = path.components().any(|c| c.as_os_str() == ".git");
        if in_git_dir {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let path_str = path.to_string_lossy();
            if name == "index" || name == "HEAD" || path_str.contains("/.git/refs/") {
                return true;
            }
        } else if !in_pruned_dir(repo_root, path) {
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
    let Ok(output) = child.wait_with_output() else {
        return false;
    };
    let ignored = output
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .count();
    ignored == paths.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_build_output_and_git_internals_but_not_index() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| {
            assert!(Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap()
                .status
                .success())
        };
        git(&["init"]);
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let target = root.join("target/debug/x");
        let source = root.join("src.rs");
        assert!(!is_relevant(root, [target.as_path()].into_iter()));
        assert!(!is_relevant(
            root,
            [root.join(".git/objects/ab").as_path()].into_iter()
        ));
        assert!(is_relevant(
            root,
            [root.join(".git/index").as_path()].into_iter()
        ));
        assert!(is_relevant(
            root,
            [target.as_path(), source.as_path()].into_iter()
        ));
        // Pruned dirs are dropped without asking git, even when not gitignored.
        assert!(!is_relevant(
            root,
            [root.join("node_modules/pkg/index.js").as_path()].into_iter()
        ));
        assert!(in_pruned_dir(root, &root.join("web/dist/app.js")));
        assert!(!in_pruned_dir(root, &root.join("src/distance.rs")));
        // Folders above the repository don't count.
        let nested = root.join("build/repo");
        assert!(!in_pruned_dir(&nested, &nested.join("src/main.rs")));
        assert!(in_pruned_dir(
            root,
            &root.canonicalize().unwrap().join("target/x")
        ));
    }

    #[test]
    fn edits_in_the_worktree_request_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap()
            .status
            .success());
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        let (tx, rx) = mpsc::channel();
        let _service = WatchService::start(root, 50, tx).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Ok(WatchEvent::Ready { .. })
        ) {
            assert!(
                std::time::Instant::now() < deadline,
                "watcher never got ready"
            );
        }
        // FSEvents may deliver events from just before the stream started.
        std::thread::sleep(Duration::from_millis(300));
        while rx.try_recv().is_ok() {}
        std::fs::write(root.join("src/deep/file.rs"), "fn main() {}\n").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(WatchEvent::ReloadRequested) => break,
                _ => assert!(
                    std::time::Instant::now() < deadline,
                    "no reload after editing a nested file"
                ),
            }
        }
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

    #[test]
    fn test_watch_service_emits_scanning_and_ready() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let _service = WatchService::start(dir.path(), 100, tx).unwrap();

        let first = rx.recv_timeout(Duration::from_secs(2));
        assert!(matches!(
            first,
            Ok(WatchEvent::Scanning { scanned_dirs: 0 })
        ));

        let second = rx.recv_timeout(Duration::from_secs(2));
        assert!(matches!(second, Ok(WatchEvent::Ready { .. })));
    }
}
