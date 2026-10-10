//! Repository search runs on a worker; symlinks and expensive/hidden trees are skipped.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::provider::GitProvider;

#[derive(Default)]
pub struct SearchResult {
    pub repositories: Vec<PathBuf>,
    pub unreadable_dirs: usize,
}

pub fn search_repositories(
    root: &Path,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(usize),
) -> SearchResult {
    let mut result = SearchResult::default();
    let mut pending = vec![root.to_path_buf()];
    let mut scanned = 0;
    while let Some(dir) = pending.pop() {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        scanned += 1;
        progress(scanned);
        if dir.join(".git").exists() {
            if let Ok(provider) = GitProvider::discover(Some(&dir)) {
                result.repositories.push(provider.repo_root);
            }
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                result.unreadable_dirs += 1;
                continue;
            }
        };
        for entry in entries {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            let Ok(entry) = entry else {
                result.unreadable_dirs += 1;
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || matches!(name.as_ref(), "node_modules" | "target") {
                continue;
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(entry.path());
            }
        }
    }
    result.repositories.sort();
    result.repositories.dedup();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init(path: &Path) {
        std::fs::create_dir_all(path).unwrap();
        assert!(Command::new("git")
            .args(["init", "--quiet"])
            .arg(path)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn finds_zero_one_and_multiple_nested_repositories() {
        let root = tempfile::tempdir().unwrap();
        let search =
            || search_repositories(root.path(), &AtomicBool::new(false), |_| {}).repositories;
        assert!(search().is_empty());
        let first = root.path().join("projects/first");
        init(&first);
        let first = std::fs::canonicalize(first).unwrap();
        assert_eq!(search(), vec![first.clone()]);
        let nested = first.join("nested/repo");
        let second = root.path().join("other/repo");
        init(&nested);
        init(&second);
        let mut expected = vec![
            first,
            std::fs::canonicalize(nested).unwrap(),
            std::fs::canonicalize(second).unwrap(),
        ];
        expected.sort();
        assert_eq!(search(), expected);
    }

    #[test]
    fn skips_hidden_heavy_and_symlink_directories_and_can_cancel() {
        let root = tempfile::tempdir().unwrap();
        for name in [".hidden", "node_modules", "target", ".git"] {
            init(&root.path().join(name).join("repo"));
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path(), root.path().join("loop")).unwrap();
        assert!(
            search_repositories(root.path(), &AtomicBool::new(false), |_| {})
                .repositories
                .is_empty()
        );
        let cancel = AtomicBool::new(false);
        let mut count = 0;
        search_repositories(root.path(), &cancel, |_| {
            count += 1;
            cancel.store(true, Ordering::Relaxed);
        });
        assert_eq!(count, 1);
    }
}
