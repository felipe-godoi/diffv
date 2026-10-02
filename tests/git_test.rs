use std::fs;
use std::process::Command;
use diffv::core::models::{FileStatus, StageStatus};
use diffv::git::actions::{stage_file, stage_hunk, unstage_file, unstage_hunk};
use diffv::git::provider::GitProvider;

#[test]
fn test_git_provider_lifecycle() {
    let temp_dir = std::env::temp_dir().join("diffv_test_git_repo");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Initialize git
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init"]);
    run(&["config", "user.name", "Tester"]);
    run(&["config", "user.email", "test@example.com"]);

    let test_file = temp_dir.join("test.txt");
    fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

    run(&["add", "test.txt"]);
    run(&["commit", "-m", "Initial commit"]);

    // Modify file
    fs::write(&test_file, "line 1\nline 2 modified\nline 3\nline 4 added\n").unwrap();

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    let (files, stats) = provider.load_diffs(None, false, true, false).unwrap();

    assert_eq!(files.len(), 1);
    let f = &files[0];
    assert_eq!(f.status, FileStatus::Modified);
    assert_eq!(f.stage_status, StageStatus::Unstaged);
    assert_eq!(f.stats.additions, 2);
    assert_eq!(f.stats.deletions, 1);
    assert_eq!(stats.total_additions, 2);
    assert_eq!(stats.total_deletions, 1);

    // Test stage hunk
    let hunk = &f.hunks[0];
    stage_hunk(&temp_dir, &f.new_path, hunk).unwrap();

    // Verify file is now staged
    let (files_staged, _) = provider.load_diffs(None, true, true, false).unwrap();
    assert_eq!(files_staged.len(), 1);
    assert_eq!(files_staged[0].stage_status, StageStatus::Staged);

    // Test unstage hunk
    unstage_hunk(&temp_dir, &f.new_path, hunk).unwrap();
    let (files_unstaged, _) = provider.load_diffs(None, true, true, false).unwrap();
    assert!(files_unstaged.is_empty());

    // Test stage entire file
    stage_file(&temp_dir, &f.new_path).unwrap();
    let (files_staged2, _) = provider.load_diffs(None, true, true, false).unwrap();
    assert_eq!(files_staged2.len(), 1);

    // Test unstage entire file
    unstage_file(&temp_dir, &f.new_path).unwrap();
    let (files_unstaged2, _) = provider.load_diffs(None, true, true, false).unwrap();
    assert!(files_unstaged2.is_empty());

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}
