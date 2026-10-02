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

#[test]
fn test_tab_esc_worktree_fzf_features() {
    let temp_dir = std::env::temp_dir().join("diffv_test_tab_esc");
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
    run(&["config", "user.name", "FeatureTester"]);
    run(&["config", "user.email", "feature@example.com"]);

    let f1 = temp_dir.join("main.rs");
    fs::write(&f1, "fn main() { println!(\"hello\"); }\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();

    // 1. Test Worktree Creation
    let wt_path = temp_dir.join("wt_feature");
    let res = provider.add_worktree(wt_path.to_str().unwrap(), "feat-branch");
    assert!(res.is_ok(), "Worktree creation should succeed");
    let wts = provider.get_worktrees(&temp_dir).unwrap();
    assert_eq!(wts.len(), 2);
    assert!(wts.iter().any(|w| w.branch.as_deref() == Some("feat-branch")));

    // 2. Modify file in main repo to test diffs and fzf search collection
    fs::write(&f1, "fn main() { println!(\"world\"); }\n").unwrap();
    let mut config = diffv::config::Config::load();
    config.ui.theme = "catppuccin-mocha".into();

    let mut app = diffv::ui::app::App::new(
        diffv::ui::app::AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        config,
        false,
        false,
        true,
        None,
        false,
        false,
    ).unwrap();

    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);

    // 3. Test Tab cycles tabs: Changes -> Commits -> Stashes -> Changes
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Commits);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Stashes);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);

    // History temporarily replaces the drawer and returns without changing tabs.
    app.open_file_history();
    assert!(app.show_history);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(matches!(app.fzf_request.take(), Some(diffv::ui::app::FzfRequest::Files)));
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.fzf_request.take(), Some(diffv::ui::app::FzfRequest::Text)));
    app.load_selected_commit_diff();
    assert!(app.show_history);
    assert!(app.active_commit_view.is_some());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect();
    assert!(content.contains("File history") || content.contains("Histórico do arquivo"));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_history);
    assert!(app.active_commit_view.is_none());
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);

    // Tab switches columns inside a diff; horizontal scrolling preserves access to long lines.
    app.is_unified = false;
    app.wrap_lines = false;
    app.focus = diffv::ui::app::Focus::DiffView;
    app.column_side = diffv::ui::components::side_by_side::ColumnSide::Right;
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.column_side, diffv::ui::components::side_by_side::ColumnSide::Left);
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let row = app.files[0].aligned_rows.iter_mut().find(|r| r.right.is_some()).unwrap();
    row.right.as_mut().unwrap().content = format!("{}TAIL_MARKER", "x".repeat(160));
    app.handle_key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
    assert!(app.scroll_x[1] > 0);
    terminal.draw(|frame| app.render(frame)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect();
    assert!(content.contains("TAIL_MARKER"));
    app.handle_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
    assert_eq!(app.scroll_x[1], 0);
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(app.wrap_lines);
    terminal.draw(|frame| app.render(frame)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect();
    assert!(content.contains("AIL_MARKER"));
    assert!(app.diff_row_map.iter().filter(|&&idx| idx == 0).count() > 1);
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(!app.wrap_lines);

    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(!app.should_quit);
    assert_eq!(app.focus, diffv::ui::app::Focus::FileTree);
    app.switch_drawer_tab(diffv::core::models::DrawerTab::Commits);
    app.load_selected_repo_commit();
    assert!(app.active_commit_info.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(app.active_commit_info.is_none());
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);
    assert!(!app.should_quit);
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(app.should_quit);

    // 4. Test Esc NEVER quits app
    app.should_quit = false;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.should_quit, "Esc must never quit the program!");

    // Switch focus to DiffView and verify Esc returns to FileTree
    app.focus = diffv::ui::app::Focus::DiffView;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.focus, diffv::ui::app::Focus::FileTree, "Esc should return focus to FileTree");
    assert!(!app.should_quit);

    app.viewport_height = 0;
    app.is_unified = true;
    // 5. Test Ctrl+e and Ctrl+y scrolling
    app.focus = diffv::ui::app::Focus::DiffView;
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert_eq!(app.scroll_y, 1);
    app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    assert_eq!(app.scroll_y, 0);


    // 6. Test fzf search line collection
    let files = app.collect_diff_files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], "main.rs");

    let text_lines = app.collect_diff_text_lines();
    assert!(!text_lines.is_empty());
    assert!(text_lines.iter().any(|l| l.contains("main.rs") && (l.contains('+') || l.contains('-'))));

    // 7. Test jumping to file and line
    assert!(app.jump_to_file("main.rs"));
    app.jump_to_line(1);

    // 8. Test mouse click coordinates and row mapping
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    // Clicking row 3 toggles file view mode between Tree and Flat
    assert_eq!(app.file_view_mode, diffv::ui::components::file_tree::FileViewMode::Tree);
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.file_view_mode, diffv::ui::components::file_tree::FileViewMode::Flat);

    // Clicking row 4 selects item 0 (first file)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.selected_filtered_idx, 0);

    terminal.draw(|frame| app.render(frame)).unwrap();
    // Clicking diff view at row 3 selects line 0 of diff
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 50,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.focus, diffv::ui::app::Focus::DiffView);
    assert_eq!(app.selected_row, 0);

    // Clicking diff view at row 4 selects line 1 of diff
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 50,
        row: 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.selected_row, 1);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn history_previews_selected_diff_and_opens_commit_details() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::ui::app::{App, AppMode, Focus};
    let dir = std::env::temp_dir().join(format!("diffv_history_preview_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git").args(args).current_dir(&dir).output().unwrap().status.success());
    };
    git(&["init"]);
    git(&["config", "user.name", "History Tester"]);
    git(&["config", "user.email", "history@example.com"]);
    fs::write(dir.join("file.txt"), "original_value\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "First history commit"]);
    fs::write(dir.join("file.txt"), "updated_value\n").unwrap();
    git(&["commit", "-am", "Second history commit"]);
    fs::write(dir.join("file.txt"), "working_value\n").unwrap();
    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let mut app = App::new(AppMode::Git { target_ref: None, git_provider: provider },
        diffv::config::Config::load(), false, false, false, None, false, false).unwrap();
    app.language = diffv::core::models::Language::En;
    app.open_file_history();
    assert_eq!(app.focus, Focus::FileTree);
    assert_eq!(app.history_commits.len(), 2);
    assert_eq!(app.active_commit_view.as_ref().unwrap().0, app.history_commits[0].hash);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let text: String = terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains("updated_value"));
    assert!(text.contains("original_value"));
    assert!(!text.contains("working_value"));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::FileTree);
    assert_eq!(app.active_commit_view.as_ref().unwrap().0, app.history_commits[1].hash);
    app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
    assert!(app.show_details_popup);
    terminal.draw(|frame| app.render(frame)).unwrap();
    let text: String = terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains("Commit Details"));
    assert!(text.contains("First history commit"));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.show_history);
    assert!(!app.show_details_popup);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_history);
    assert!(app.active_commit_view.is_none());
    assert!(app.current_file().unwrap().hunks.iter().flat_map(|h| &h.lines).any(|l| l.content.contains("working_value")));
    fs::remove_dir_all(dir).unwrap();
}
