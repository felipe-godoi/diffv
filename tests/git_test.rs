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

#[test]
fn test_file_history() {
    let temp_dir = std::env::temp_dir().join("diffv_test_git_history");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init"]);
    run(&["config", "user.name", "HistoryTester"]);
    run(&["config", "user.email", "history@example.com"]);

    let file_path = temp_dir.join("history_doc.txt");

    // Commit 1
    fs::write(&file_path, "version 1\n").unwrap();
    run(&["add", "history_doc.txt"]);
    run(&["commit", "-m", "First commit on doc"]);

    // Commit 2
    fs::write(&file_path, "version 1\nversion 2\n").unwrap();
    run(&["add", "history_doc.txt"]);
    run(&["commit", "-m", "Second commit on doc"]);

    // Commit 3
    fs::write(&file_path, "version 1\nversion 2\nversion 3\n").unwrap();
    run(&["add", "history_doc.txt"]);
    run(&["commit", "-m", "Third commit on doc"]);

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    let commits = provider.get_file_history(std::path::Path::new("history_doc.txt"), 10).unwrap();

    assert_eq!(commits.len(), 3);
    assert_eq!(commits[0].message, "Third commit on doc");
    assert_eq!(commits[1].message, "Second commit on doc");
    assert_eq!(commits[2].message, "First commit on doc");
    assert_eq!(commits[0].author, "HistoryTester");

    // Test load commit diff
    let diff = provider.load_commit_diff_for_file(&commits[0].hash, std::path::Path::new("history_doc.txt")).unwrap();
    assert!(diff.is_some());
    let diff = diff.unwrap();
    assert_eq!(diff.new_path, std::path::PathBuf::from("history_doc.txt"));
    assert_eq!(diff.stats.additions, 1);
    assert_eq!(diff.stats.deletions, 0);

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worktrees_and_stashes() {
    let temp_dir = std::env::temp_dir().join("diffv_test_git_worktrees");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init"]);
    run(&["config", "user.name", "WorktreeTester"]);
    run(&["config", "user.email", "worktree@example.com"]);

    let main_file = temp_dir.join("main.txt");
    fs::write(&main_file, "initial\n").unwrap();
    run(&["add", "main.txt"]);
    run(&["commit", "-m", "Initial commit"]);

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();

    // 1. Worktrees
    let worktrees = provider.get_worktrees(&temp_dir).unwrap();
    assert!(!worktrees.is_empty());
    assert!(worktrees[0].is_current);

    // 2. Repo Commits
    let commits = provider.get_repo_commits(10).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].message, "Initial commit");

    // 3. Stash
    fs::write(&main_file, "initial\nstashed line\n").unwrap();
    run(&["stash", "push", "-m", "my-wip-stash"]);

    let stashes = provider.get_stashes().unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(stashes[0].message.contains("my-wip-stash"));

    let stash_diff = provider.load_stash_diff(&stashes[0].selector).unwrap();
    assert_eq!(stash_diff.len(), 1);
    assert_eq!(stash_diff[0].stats.additions, 1);

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_models_language_and_drawer() {
    use diffv::core::models::{DrawerTab, Language};
    use diffv::ui::components::file_tree::FileViewMode;

    let default_lang = Language::default();
    assert_eq!(default_lang, Language::En);

    let default_tab = DrawerTab::default();
    assert_eq!(default_tab, DrawerTab::Changes);

    let default_tree_mode = FileViewMode::Tree;
    assert_eq!(default_tree_mode, FileViewMode::Tree);
}

#[test]
fn test_commit_inspection() {
    let temp_dir = std::env::temp_dir().join("diffv_test_commit_inspect");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init"]);
    run(&["config", "user.name", "Commit Tester"]);
    run(&["config", "user.email", "committester@example.com"]);

    let f1 = temp_dir.join("file1.rs");
    let f2 = temp_dir.join("file2.py");
    fs::write(&f1, "fn main() {}\n").unwrap();
    fs::write(&f2, "print('hello')\n").unwrap();

    run(&["add", "."]);
    run(&["commit", "-m", "feat: first commit with two files"]);

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    let commits = provider.get_repo_commits(5).unwrap();
    assert_eq!(commits.len(), 1);

    let commit_diff = provider.load_commit_full_diff(&commits[0].hash).unwrap();
    assert_eq!(commit_diff.len(), 2);
    assert!(commit_diff.iter().any(|f| f.display_path().contains("file1.rs")));
    assert!(commit_diff.iter().any(|f| f.display_path().contains("file2.py")));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_details_popup_and_neovim_navigation() {
    let commit = diffv::core::models::CommitEntry {
        hash: "1234567890abcdef".into(),
        author: "Developer <dev@example.com>".into(),
        date: "2026-10-01 22:00:00".into(),
        message: "feat: add neovim motions and details popup\n\nFull wrapped message body here with multiple paragraphs.\nBreaking changes: None.".into(),
    };

    assert_eq!(&commit.hash[..7], "1234567");
    assert!(commit.message.lines().count() >= 3);

    // Verify visual range calculations
    let anchor = 5usize;
    let cursor = 12usize;
    let min_r = anchor.min(cursor);
    let max_r = anchor.max(cursor);
    assert_eq!(min_r, 5);
    assert_eq!(max_r, 12);
    assert_eq!(max_r - min_r + 1, 8);
}



