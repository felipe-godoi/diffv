use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Uninstalls specified targets and optional configuration directory.
/// Returns the number of binary files or symlinks successfully removed.
pub fn uninstall_targets(targets: &[PathBuf], config_dir: Option<&Path>) -> Result<usize> {
    let mut removed_count = 0;
    let mut seen = HashSet::new();

    for target in targets {
        // Normalize or check uniqueness
        if !seen.insert(target.clone()) {
            continue;
        }

        // Use symlink_metadata so broken symlinks are also detected and removed
        if let Ok(meta) = fs::symlink_metadata(target) {
            if !meta.is_dir() {
                // If there's an update lock file in the same directory, remove it
                if let Some(parent) = target.parent() {
                    let lock_file = parent.join(".diffv-update.lock");
                    if lock_file.exists() {
                        let _ = fs::remove_file(lock_file);
                    }
                }

                match fs::remove_file(target) {
                    Ok(_) => {
                        println!("\x1b[32m✓\x1b[0m Removed {}", target.display());
                        removed_count += 1;
                    }
                    Err(err) => {
                        eprintln!("\x1b[31m✗ Error removing {}:\x1b[0m {}", target.display(), err);
                    }
                }
            }
        }
    }

    if let Some(dir) = config_dir {
        if dir.exists() {
            match fs::remove_dir_all(dir) {
                Ok(_) => {
                    println!("\x1b[32m✓\x1b[0m Removed configuration directory {}", dir.display());
                }
                Err(err) => {
                    eprintln!("\x1b[33m!\x1b[0m Note: Could not remove config directory {}: {}", dir.display(), err);
                }
            }
        }
    }

    Ok(removed_count)
}

/// Executes the full uninstallation flow for diffv.
pub fn run() -> Result<()> {
    println!("\x1b[36m==>\x1b[0m \x1b[1mUninstalling diffv...\x1b[0m");

    let mut targets = Vec::new();

    // 1. Current executable (and canonical path)
    if let Ok(current_exe) = std::env::current_exe() {
        if let Ok(canonical) = current_exe.canonicalize() {
            targets.push(canonical);
        }
        targets.push(current_exe);
    }

    // 2. Standard installation paths
    if let Some(home) = dirs::home_dir() {
        targets.push(home.join(".local/bin/diffv"));
        targets.push(home.join(".cargo/bin/diffv"));
    }
    targets.push(PathBuf::from("/usr/local/bin/diffv"));

    let config_dir = dirs::config_dir().map(|p| p.join("diffv"));

    let removed = uninstall_targets(&targets, config_dir.as_deref())?;

    if removed > 0 {
        println!("\x1b[32m✓\x1b[0m diffv successfully uninstalled.");
    } else {
        println!("\x1b[33m!\x1b[0m No diffv binary found to remove.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_uninstall_removes_files_and_config() {
        let temp = tempdir().unwrap();
        let bin1 = temp.path().join("diffv-bin1");
        let bin2 = temp.path().join("diffv-bin2");
        let config = temp.path().join("config_dir");

        fs::write(&bin1, "binary content").unwrap();
        fs::write(&bin2, "binary content 2").unwrap();
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("config.toml"), "theme = 'tokyonight'").unwrap();

        assert!(bin1.exists());
        assert!(bin2.exists());
        assert!(config.exists());

        let removed = uninstall_targets(&[bin1.clone(), bin2.clone()], Some(&config)).unwrap();
        assert_eq!(removed, 2);
        assert!(!bin1.exists());
        assert!(!bin2.exists());
        assert!(!config.exists());
    }

    #[test]
    fn test_uninstall_nonexistent_target_returns_zero() {
        let temp = tempdir().unwrap();
        let missing = temp.path().join("nonexistent_diffv");
        let removed = uninstall_targets(&[missing], None).unwrap();
        assert_eq!(removed, 0);
    }
}
