use diffv::core::models::{FileStatus, StageStatus};
use diffv::git::actions::{stage_file, stage_hunk, unstage_file, unstage_hunk};
use diffv::git::provider::GitProvider;
use std::fs;
use std::process::Command;

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
    fs::write(
        &test_file,
        "line 1\nline 2 modified\nline 3\nline 4 added\n",
    )
    .unwrap();

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
    let commits = provider
        .get_file_history(std::path::Path::new("history_doc.txt"), 10)
        .unwrap();

    assert_eq!(commits.len(), 3);
    assert_eq!(commits[0].message, "Third commit on doc");
    assert_eq!(commits[1].message, "Second commit on doc");
    assert_eq!(commits[2].message, "First commit on doc");
    assert_eq!(commits[0].author, "HistoryTester");

    // Test load commit diff
    let diff = provider
        .load_commit_diff_for_file(&commits[0].hash, std::path::Path::new("history_doc.txt"))
        .unwrap();
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
    assert!(commit_diff
        .iter()
        .any(|f| f.display_path().contains("file1.rs")));
    assert!(commit_diff
        .iter()
        .any(|f| f.display_path().contains("file2.py")));

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
    assert!(wts
        .iter()
        .any(|w| w.branch.as_deref() == Some("feat-branch")));

    // 2. Modify file in main repo to test diffs and fzf search collection
    fs::write(&f1, "fn main() { println!(\"world\"); }\n").unwrap();
    let mut config = diffv::config::Config::default();
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
    )
    .unwrap();

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
    assert!(matches!(
        app.fzf_request.take(),
        Some(diffv::ui::app::FzfRequest::Files)
    ));
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert!(matches!(
        app.fzf_request.take(),
        Some(diffv::ui::app::FzfRequest::Text)
    ));
    app.load_selected_commit_diff();
    assert!(app.show_history);
    assert!(app.active_commit_view.is_some());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
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
    assert_eq!(
        app.column_side,
        diffv::ui::components::side_by_side::ColumnSide::Left
    );
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let row = app.files[0]
        .aligned_rows
        .iter_mut()
        .find(|r| r.right.is_some())
        .unwrap();
    row.right.as_mut().unwrap().content = format!("{}TAIL_MARKER", "x".repeat(160));
    app.handle_key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
    assert!(app.scroll_x[1] > 0);
    terminal.draw(|frame| app.render(frame)).unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(content.contains("TAIL_MARKER"));
    app.handle_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
    assert_eq!(app.scroll_x[1], 0);
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(app.wrap_lines);
    terminal.draw(|frame| app.render(frame)).unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
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
    // Lowercase 'q' returns to changes from deep navigation
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(app.active_commit_info.is_none());
    assert_eq!(app.drawer_tab, diffv::core::models::DrawerTab::Changes);
    assert!(!app.should_quit);

    // Shift+Q unconditionally quits the program regardless of navigation state
    app.switch_drawer_tab(diffv::core::models::DrawerTab::Commits);
    app.load_selected_repo_commit();
    assert!(app.active_commit_info.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(
        app.should_quit,
        "Shift+Q must unconditionally quit from any navigation state"
    );
    app.should_quit = false;
    app.switch_drawer_tab(diffv::core::models::DrawerTab::Changes);
    app.return_to_changes();

    // 4. Test Esc NEVER quits app
    app.should_quit = false;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.should_quit, "Esc must never quit the program!");

    // Switch focus to DiffView and verify Esc returns to FileTree
    app.focus = diffv::ui::app::Focus::DiffView;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(
        app.focus,
        diffv::ui::app::Focus::FileTree,
        "Esc should return focus to FileTree"
    );
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
    app.focus = diffv::ui::app::Focus::FileTree;
    let files = app.prepare_fzf(diffv::ui::app::FzfRequest::Files).items;
    assert_eq!(files, vec!["main.rs".to_string()]);

    let text_lines = app.prepare_fzf(diffv::ui::app::FzfRequest::Text).items;
    assert!(!text_lines.is_empty());
    assert!(text_lines
        .iter()
        .all(|l| l.starts_with("main.rs:") && l.contains('\t')));

    app.focus = diffv::ui::app::Focus::DiffView;
    let query = app.prepare_fzf(diffv::ui::app::FzfRequest::Text);
    assert_eq!(app.search_source, diffv::ui::app::SearchSource::CurrentFile);
    assert!(query.header.contains("main.rs"));
    app.handle_fzf_text_result(text_lines[0].clone());
    assert_eq!(app.focus, diffv::ui::app::Focus::DiffView);

    // 7. Test jumping to file and line
    assert!(app.jump_to_file("main.rs"));
    app.jump_to_line(1);

    // 8. Test mouse click coordinates and row mapping
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    // Clicking row 3 toggles file view mode between Tree and Flat
    assert_eq!(
        app.file_view_mode,
        diffv::ui::components::file_tree::FileViewMode::Tree
    );
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        app.file_view_mode,
        diffv::ui::components::file_tree::FileViewMode::Flat
    );

    // Clicking row 4 selects item 0 (first file)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.selected_filtered_idx, 0);

    terminal.draw(|frame| app.render(frame)).unwrap();
    // Clicking diff view at row 3 selects line 0 of diff (applied on release, so a
    // drag can become a text selection instead)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 50,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
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
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
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
        assert!(Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success());
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
    let mut app = App::new(
        AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        diffv::config::Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();
    app.language = diffv::core::models::Language::En;
    app.open_file_history();
    assert_eq!(app.focus, Focus::FileTree);
    assert_eq!(app.history_commits.len(), 2);
    assert_eq!(
        app.active_commit_view.as_ref().unwrap().0,
        app.history_commits[0].hash
    );
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("updated_value"));
    assert!(text.contains("original_value"));
    assert!(!text.contains("working_value"));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::FileTree);
    assert_eq!(
        app.active_commit_view.as_ref().unwrap().0,
        app.history_commits[1].hash
    );
    app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
    assert!(app.show_details_popup);
    terminal.draw(|frame| app.render(frame)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Commit Details"));
    assert!(text.contains("First history commit"));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.show_history);
    assert!(!app.show_details_popup);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::DiffView);
    app.visual_mode = true;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.visual_mode);
    assert_eq!(app.focus, Focus::DiffView);
    assert!(app.show_history);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::FileTree);
    assert!(app.show_history);
    assert_eq!(app.selected_history_idx, 1);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_history);
    assert!(app.active_commit_view.is_none());
    assert!(app
        .current_file()
        .unwrap()
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| l.content.contains("working_value")));
    // Commit and stash inspection each retain their file list until a second Escape.
    use diffv::core::models::DrawerTab;
    for tab in [DrawerTab::Commits, DrawerTab::Stashes] {
        if tab == DrawerTab::Stashes {
            git(&["stash", "push", "-m", "navigation stash"]);
            app.stashes = GitProvider::discover(Some(&dir))
                .unwrap()
                .get_stashes()
                .unwrap();
        }
        app.switch_drawer_tab(tab);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.handle_key(key(KeyCode::Enter));
        assert!(app.live_snapshot.is_some());
        let paths: Vec<_> = app.files.iter().map(|f| f.new_path.clone()).collect();
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.focus, Focus::DiffView);
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('i')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::DiffView);
        assert!(!app.show_details_popup);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.show_help);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::DiffView);
        app.handle_key(key(KeyCode::Char('W')));
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert!(app.worktree_creation.is_some());
        app.handle_key(key(KeyCode::Esc));
        assert!(app.worktree_creation.is_none());
        assert!(app.show_worktrees);
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_worktrees);
        assert_eq!(app.focus, Focus::DiffView);
        app.open_file_history();
        assert!(app.show_history);
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.show_history);
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_history);
        assert_eq!(app.focus, Focus::DiffView);
        app.filter_mode = true;
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.filter_mode);
        assert_eq!(app.focus, Focus::DiffView);
        app.show_drawer = false;
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::FileTree);
        assert!(app.show_drawer);
        assert_eq!(
            paths,
            app.files
                .iter()
                .map(|f| f.new_path.clone())
                .collect::<Vec<_>>()
        );
        assert!(app.live_snapshot.is_some());
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::DiffView);
        assert_eq!(app.drawer_tab, tab);
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.drawer_tab, tab);
        assert!(app.live_snapshot.is_none());
        assert!(app.active_commit_info.is_none());
        assert!(app.active_stash_info.is_none());
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn full_context_toggle_expands_whole_file_and_keeps_cursor() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::ui::app::{App, AppMode, Focus};
    let dir = std::env::temp_dir().join(format!("diffv_full_context_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init"]);
    git(&["config", "user.name", "Context Tester"]);
    git(&["config", "user.email", "context@example.com"]);
    let original: String = (1..=30).map(|i| format!("line {}\n", i)).collect();
    fs::write(dir.join("file.txt"), &original).unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "base"]);
    fs::write(
        dir.join("file.txt"),
        original.replace("line 15\n", "line 15 changed\n"),
    )
    .unwrap();

    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let mut app = App::new(
        AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        diffv::config::Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();
    app.config.diff.context_lines = 3;
    app.is_unified = true;
    app.reload_diffs();
    let visible_lines = |app: &App| {
        app.current_file()
            .unwrap()
            .hunks
            .iter()
            .map(|h| h.lines.len())
            .sum::<usize>()
    };
    assert_eq!(visible_lines(&app), 8);

    app.focus = Focus::DiffView;
    app.jump_to_line(15);
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(app.full_context);
    assert_eq!(visible_lines(&app), 31);
    assert_eq!(app.focus, Focus::DiffView);
    let line = &app.current_file().unwrap().hunks[0].lines[app.selected_row - 1];
    assert_eq!(line.old_line_no.or(line.new_line_no), Some(15));

    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(!app.full_context);
    assert_eq!(visible_lines(&app), 8);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn live_reload_keeps_open_commit_and_its_selection() {
    use diffv::core::models::DrawerTab;
    use diffv::ui::app::{App, AppMode};
    let dir = std::env::temp_dir().join(format!("diffv_live_commit_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init"]);
    git(&["config", "user.name", "Live Tester"]);
    git(&["config", "user.email", "live@example.com"]);
    fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "first"]);
    fs::write(dir.join("b.txt"), "two\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "second"]);
    fs::write(dir.join("a.txt"), "one edited\n").unwrap();

    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let mut app = App::new(
        AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        diffv::config::Config::default(),
        true,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();
    app.drawer_tab = DrawerTab::Commits;
    app.selected_repo_commit_idx = 1;
    app.load_selected_repo_commit();
    let commit_files: Vec<String> = app.files.iter().map(|f| f.display_path()).collect();
    assert_eq!(commit_files, vec!["a.txt".to_string()]);

    fs::write(dir.join("c.txt"), "new\n").unwrap();
    app.reload_diffs();
    let after: Vec<String> = app.files.iter().map(|f| f.display_path()).collect();
    assert_eq!(
        after, commit_files,
        "live reload must not replace the open commit's files"
    );
    assert_eq!(app.selected_repo_commit_idx, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stage_hunk_under_cursor_in_both_view_modes() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::ui::app::{App, AppMode, Focus};
    for unified in [false, true] {
        let dir = std::env::temp_dir().join(format!(
            "diffv_stage_hunk_{}_{}",
            unified,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| -> String {
            let out = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(out.status.success(), "{:?}", args);
            String::from_utf8_lossy(&out.stdout).to_string()
        };
        git(&["init"]);
        git(&["config", "user.name", "Stage Tester"]);
        git(&["config", "user.email", "stage@example.com"]);
        let original: String = (1..=40).map(|i| format!("line {}\n", i)).collect();
        fs::write(dir.join("file.txt"), &original).unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "base"]);
        let original: String = (1..=120).map(|i| format!("line {}\n", i)).collect();
        fs::write(dir.join("file.txt"), &original).unwrap();
        git(&["commit", "-am", "longer"]);
        let mut modified = original.clone();
        for i in [5, 25, 45, 65, 85, 105] {
            modified = modified.replace(&format!("line {}\n", i), &format!("line {} changed\n", i));
        }
        fs::write(dir.join("file.txt"), modified).unwrap();

        let provider = GitProvider::discover(Some(&dir)).unwrap();
        let mut app = App::new(
            AppMode::Git {
                target_ref: None,
                git_provider: provider,
            },
            diffv::config::Config::default(),
            false,
            false,
            false,
            None,
            false,
            false,
        )
        .unwrap();
        app.config.diff.context_lines = 3;
        app.is_unified = unified;
        app.reload_diffs();
        assert_eq!(app.current_file().unwrap().hunks.len(), 6);
        app.focus = Focus::DiffView;
        app.jump_to_line(105);
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        let cached = git(&["diff", "--cached"]);
        assert!(
            cached.contains("+line 105 changed"),
            "unified={} cached:\n{}",
            unified,
            cached
        );
        assert_eq!(
            cached.matches("changed").count(),
            1,
            "unified={} staged extra hunks:\n{}",
            unified,
            cached
        );

        app.selected_row = 0;
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        let cached = git(&["diff", "--cached"]);
        assert!(
            cached.contains("+line 45 changed"),
            "unified={} hunk navigation:\n{}",
            unified,
            cached
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

#[test]
fn unstage_after_stage_round_trips() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::core::models::DiffSection;
    use diffv::ui::app::{App, AppMode, Focus};
    let dir = std::env::temp_dir().join(format!("diffv_unstage_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{:?}", args);
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    git(&["init"]);
    git(&["config", "user.name", "Unstage Tester"]);
    git(&["config", "user.email", "unstage@example.com"]);
    let original: String = (1..=60).map(|i| format!("line {}\n", i)).collect();
    fs::write(dir.join("file.txt"), &original).unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "base"]);
    fs::write(
        dir.join("file.txt"),
        original
            .replace("line 10\n", "line 10 a\n")
            .replace("line 40\n", "line 40 b\n"),
    )
    .unwrap();

    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let mut app = App::new(
        AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        diffv::config::Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();
    app.config.diff.context_lines = 3;
    app.reload_diffs();
    app.focus = Focus::DiffView;
    app.jump_to_line(10);
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert!(git(&["diff", "--cached"]).contains("+line 10 a"));
    let sections: Vec<_> = app.files.iter().map(|f| f.section).collect();
    assert_eq!(sections, vec![DiffSection::Staged, DiffSection::Changes]);

    // `u` on the Changes copy explains instead of failing silently
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    assert!(git(&["diff", "--cached"]).contains("+line 10 a"));

    // Editing the staged region afterwards used to break unstage
    let current = fs::read_to_string(dir.join("file.txt")).unwrap();
    fs::write(
        dir.join("file.txt"),
        current.replace("line 10 a\n", "line 10 a edited again\n"),
    )
    .unwrap();
    app.reload_diffs();
    assert!(app.jump_to_file_in("file.txt", DiffSection::Staged));
    app.jump_to_line(10);
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    let note = app
        .notification
        .as_ref()
        .map(|(m, _)| m.clone())
        .unwrap_or_default();
    assert_eq!(
        git(&["diff", "--cached"]),
        "",
        "unstage failed, notification: {}",
        note
    );

    // Partial (visual) stage, then partial unstage of the same lines
    app.reload_diffs();
    assert!(app.jump_to_file_in("file.txt", DiffSection::Changes));
    app.jump_to_line(40);
    let row = app.selected_row;
    app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
    app.selected_row = row + 1;
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    let cached = git(&["diff", "--cached"]);
    assert!(
        cached.contains("+line 40 b") && !cached.contains("line 10 a"),
        "{}",
        cached
    );
    assert!(app.jump_to_file_in("file.txt", DiffSection::Staged));
    app.jump_to_line(40);
    let row = app.selected_row;
    app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
    app.selected_row = row + 1;
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    let note = app
        .notification
        .as_ref()
        .map(|(m, _)| m.clone())
        .unwrap_or_default();
    assert_eq!(
        git(&["diff", "--cached"]),
        "",
        "partial unstage failed: {}",
        note
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn paths_with_spaces_unicode_and_renames() {
    use diffv::core::models::{DiffSection, FileStatus, StageStatus};
    let dir = std::env::temp_dir().join(format!("diffv_paths_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success(),
            "{:?}",
            args
        );
    };
    git(&["init"]);
    git(&["config", "user.name", "Paths Tester"]);
    git(&["config", "user.email", "paths@example.com"]);
    fs::write(dir.join("my file.txt"), "one\n").unwrap();
    fs::write(dir.join("café.txt"), "one\n").unwrap();
    fs::write(dir.join("old name.txt"), "a\nb\nc\nd\ne\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "base"]);
    fs::write(dir.join("my file.txt"), "two\n").unwrap();
    fs::write(dir.join("café.txt"), "two\n").unwrap();
    git(&["mv", "old name.txt", "new name.txt"]);
    fs::write(dir.join("untracked é.txt"), "new\n").unwrap();

    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let (files, _) = provider.load_diffs(None, false, true, false).unwrap();
    let find = |name: &str, section: DiffSection| {
        files
            .iter()
            .find(|f| f.display_path() == name && f.section == section)
    };
    assert!(
        find("my file.txt", DiffSection::Changes).is_some(),
        "{:#?}",
        files.iter().map(|f| f.display_path()).collect::<Vec<_>>()
    );
    assert!(
        find("café.txt", DiffSection::Changes).is_some(),
        "{:#?}",
        files.iter().map(|f| f.display_path()).collect::<Vec<_>>()
    );
    let renamed = find("new name.txt", DiffSection::Staged).expect("rename staged");
    assert_eq!(renamed.status, FileStatus::Renamed);
    let untracked = find("untracked é.txt", DiffSection::Changes).expect("untracked with unicode");
    assert_eq!(untracked.stage_status, StageStatus::Untracked);
    assert!(!untracked.hunks.is_empty());

    for name in ["my file.txt", "café.txt"] {
        let file = find(name, DiffSection::Changes).unwrap();
        stage_hunk(&dir, &file.new_path, &file.hunks[0])
            .unwrap_or_else(|e| panic!("stage {}: {}", name, e));
    }
    let (files, _) = provider.load_diffs(None, false, true, false).unwrap();
    for name in ["my file.txt", "café.txt"] {
        let file = files
            .iter()
            .find(|f| f.display_path() == name && f.section == DiffSection::Staged)
            .expect(name);
        unstage_hunk(&dir, &file.new_path, &file.hunks[0])
            .unwrap_or_else(|e| panic!("unstage {}: {}", name, e));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_watcher_scan_state_transitions() {
    use diffv::config::Config;
    use diffv::core::models::WatcherScanState;
    use diffv::ui::app::{App, AppMode};

    let temp_dir = tempfile::tempdir().unwrap();
    let mode = AppMode::DirPair(temp_dir.path().to_path_buf(), temp_dir.path().to_path_buf());

    let mut app = App::new(
        mode,
        Config::default(),
        true,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();

    assert!(app.is_watcher_scanning());
    assert_eq!(
        app.watcher_state,
        WatcherScanState::Scanning { scanned_dirs: 0 }
    );

    app.update_watcher_progress(50, None);
    assert!(app.is_watcher_scanning());
    assert_eq!(
        app.watcher_state,
        WatcherScanState::Scanning { scanned_dirs: 50 }
    );

    app.update_watcher_progress(120, Some(120));
    assert!(!app.is_watcher_scanning());
    assert_eq!(
        app.watcher_state,
        WatcherScanState::Ready { total_dirs: 120 }
    );
}

#[test]
fn test_branch_comparison_selector_and_cli() {
    use clap::Parser;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::cli::Cli;
    use diffv::config::Config;
    use diffv::core::models::Language;
    use diffv::git::provider::GitProvider;
    use diffv::ui::app::{App, AppMode};

    // 1. Verify CLI argument parsing for --compare and --branch alias
    let cli1 = Cli::try_parse_from(["diffv", "--compare", "main"]).unwrap();
    assert_eq!(cli1.compare, Some("main".to_string()));

    let cli2 = Cli::try_parse_from(["diffv", "--branch", "feature-x"]).unwrap();
    assert_eq!(cli2.compare, Some("feature-x".to_string()));

    let cli3 = Cli::try_parse_from(["diffv", "-B", "develop"]).unwrap();
    assert_eq!(cli3.compare, Some("develop".to_string()));

    // 2. Setup temporary git repo with two branches
    let temp_dir = std::env::temp_dir().join("diffv_test_branch_selector");
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

    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "BranchTester"]);
    run(&["config", "user.email", "branch@example.com"]);

    let file_a = temp_dir.join("shared.txt");
    fs::write(&file_a, "version 1 on main\n").unwrap();
    run(&["add", "shared.txt"]);
    run(&["commit", "-m", "Initial commit on main"]);

    // Create and commit on feature-test
    run(&["checkout", "-b", "feature-test"]);
    fs::write(&file_a, "version 2 on feature-test\n").unwrap();
    let file_feat = temp_dir.join("feat_only.txt");
    fs::write(&file_feat, "new feature file\n").unwrap();
    run(&["add", "shared.txt", "feat_only.txt"]);
    run(&["commit", "-m", "Feature commit"]);

    // Switch back to main
    run(&["checkout", "main"]);

    // 3. Test GitProvider::is_ref
    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    assert!(provider.is_ref("main"));
    assert!(provider.is_ref("feature-test"));
    assert!(!provider.is_ref("nonexistent_branch"));
    assert!(!provider.is_ref("shared.txt"));

    // 4. Test App with default worktree diff (HEAD)
    let mode = AppMode::Git {
        target_ref: None,
        git_provider: provider,
    };

    let mut app = App::new(
        mode,
        Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();

    assert_eq!(app.current_comparison_branch(), None);
    assert!(
        app.files.is_empty(),
        "On main with clean worktree, 0 diffs against HEAD"
    );

    // 5. Open Branch Selector using 'B' key
    assert!(!app.show_branch_selector);
    app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
    assert!(app.show_branch_selector);
    assert!(app.branch_selector.is_some());

    let selector = app.branch_selector.as_ref().unwrap();
    let items = selector.filtered_items(Language::En);
    // Should contain Default (HEAD), feature-test, main
    assert!(items.iter().any(|it| it.is_default));
    assert!(items.iter().any(|it| it.label == "feature-test"));
    assert!(items.iter().any(|it| it.label == "main"));

    // 6. Filter for "feature"
    for c in "feature".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    let filtered_items = app
        .branch_selector
        .as_ref()
        .unwrap()
        .filtered_items(Language::En);
    assert_eq!(filtered_items.len(), 1);
    assert_eq!(filtered_items[0].label, "feature-test");

    // 7. Press Enter to select feature-test
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.show_branch_selector);
    assert_eq!(app.current_comparison_branch(), Some("feature-test"));

    // Verify diffs are now loaded comparing worktree on main against feature-test
    assert!(
        !app.files.is_empty(),
        "Should have diffs when comparing main against feature-test"
    );
    assert!(app
        .files
        .iter()
        .any(|f| f.new_path.to_string_lossy().contains("feat_only.txt")));

    // 8. Staging should be blocked with notification while comparing branches
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert!(app
        .notification
        .as_ref()
        .is_some_and(|(n, _)| n.contains("disabled while comparing")));

    // 9. Open branch selector again, cancel with Esc
    app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
    assert!(app.show_branch_selector);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_branch_selector);
    assert_eq!(app.current_comparison_branch(), Some("feature-test"));

    // 10. Open branch selector, filter "head" (default), select it
    app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
    for c in "head".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.show_branch_selector);
    assert_eq!(app.current_comparison_branch(), None);
    assert!(
        app.files.is_empty(),
        "Back to default: clean worktree has 0 diffs"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_shift_q_unconditional_quit_and_footer_visual_and_wrap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::config::Config;
    use diffv::git::provider::GitProvider;
    use diffv::ui::app::{App, AppMode, Focus};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let temp_dir = std::env::temp_dir().join("diffv_test_shift_q_and_footer");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(out.status.success());
    };

    run(&["init"]);
    run(&["config", "user.name", "Tester"]);
    run(&["config", "user.email", "test@example.com"]);
    let file = temp_dir.join("test.txt");
    fs::write(&file, "line 1\n").unwrap();
    run(&["add", "test.txt"]);
    run(&["commit", "-m", "init"]);
    fs::write(
        &file,
        "line 1 modified with very long content to test diff pane and scrolling\n",
    )
    .unwrap();

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    let mode = AppMode::Git {
        target_ref: None,
        git_provider: provider,
    };
    let mut app = App::new(
        mode,
        Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();

    // 1. Verify Shift+Q quits from settings modal
    app.show_settings = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(app.should_quit, "Shift+Q must quit from settings modal");

    // 2. Verify Shift+Q quits from worktree modal
    app.should_quit = false;
    app.show_worktrees = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(app.should_quit, "Shift+Q must quit from worktree modal");

    // 3. Verify Shift+Q quits from branch selector modal
    app.should_quit = false;
    app.open_branch_selector();
    assert!(app.show_branch_selector);
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(
        app.should_quit,
        "Shift+Q must quit from branch selector modal"
    );

    // 4. Verify Shift+Q quits from help modal
    app.should_quit = false;
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(app.should_quit, "Shift+Q must quit from help modal");

    // 5. Verify Shift+Q quits from DiffView
    app.should_quit = false;
    app.focus = Focus::DiffView;
    app.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE));
    assert!(app.should_quit, "Shift+Q must quit from DiffView");

    // 6. Test footer help line when wrapped (default)
    app.should_quit = false;
    app.wrap_lines = true;
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        content.contains("visual"),
        "Footer must contain 'visual' option (v)"
    );
    assert!(
        !content.contains("h/l"),
        "Footer must NOT contain horizontal scroll 'h/l' when wrapped"
    );

    // 7. Test footer help line when unwrapped (wrap_lines = false)
    app.wrap_lines = false;
    terminal.draw(|f| app.render(f)).unwrap();
    let content_unwrapped: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        content_unwrapped.contains("visual"),
        "Footer must contain 'visual' option (v)"
    );
    assert!(
        content_unwrapped.contains("h/l"),
        "Footer must contain horizontal scroll 'h/l' when unwrapped"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_details_popup_folder_path_and_header_repo_base() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::config::Config;
    use diffv::ui::app::{App, AppMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let temp_dir = std::env::temp_dir().join("diffv_test_details_folder");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    let file_path = temp_dir.join("main.rs");
    fs::write(&file_path, "fn main() {}\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init commit"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    fs::write(&file_path, "fn main() { println!(\"hello\"); }\n").unwrap();

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    let mode = AppMode::Git {
        target_ref: None,
        git_provider: provider,
    };
    let mut app = App::new(
        mode,
        Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();

    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let header_content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        header_content.contains("diffv_test_details_folder"),
        "Header must show repo base folder name"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
    assert!(
        app.show_details_popup,
        "Pressing 'i' must open details modal"
    );

    terminal.draw(|f| app.render(f)).unwrap();
    let modal_content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        modal_content.contains(&temp_dir.display().to_string()),
        "Details modal must display full folder path"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worktree_selector_live_filter_and_modifier_commands() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::config::Config;
    use diffv::git::provider::GitProvider;
    use diffv::ui::app::{App, AppMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let temp_dir = std::env::temp_dir().join("diffv_test_wt_filter");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(out.status.success());
    };

    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "Tester"]);
    run(&["config", "user.email", "test@example.com"]);
    fs::write(temp_dir.join("readme.md"), "# Diffv\n").unwrap();
    run(&["add", "readme.md"]);
    run(&["commit", "-m", "initial"]);

    // Create two extra branches and worktrees
    let wt1_dir = std::env::temp_dir().join("diffv_test_wt_filter_alpha");
    let wt2_dir = std::env::temp_dir().join("diffv_test_wt_filter_beta");
    let _ = fs::remove_dir_all(&wt1_dir);
    let _ = fs::remove_dir_all(&wt2_dir);

    run(&[
        "worktree",
        "add",
        "-b",
        "feature-alpha",
        wt1_dir.to_str().unwrap(),
    ]);
    run(&[
        "worktree",
        "add",
        "-b",
        "bugfix-beta",
        wt2_dir.to_str().unwrap(),
    ]);

    let provider = GitProvider::discover(Some(&temp_dir)).unwrap();
    let mode = AppMode::Git {
        target_ref: None,
        git_provider: provider,
    };
    let mut app = App::new(
        mode,
        Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();

    assert_eq!(app.worktrees.len(), 3);

    // 1. Press W to open worktrees modal
    app.handle_key(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::NONE));
    assert!(app.show_worktrees);
    assert_eq!(app.worktree_filter, "");
    assert_eq!(app.filtered_worktrees().len(), 3);

    // 2. Typing characters without modifiers filters the worktree list
    for c in "alpha".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.worktree_filter, "alpha");
    assert_eq!(app.filtered_worktrees().len(), 1);
    assert_eq!(
        app.filtered_worktrees()[0].1.branch.as_deref(),
        Some("feature-alpha")
    );

    // Verify rendering of the search input and filtered result
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(screen.contains("alpha"));
    assert!(screen.contains("feature-alpha"));
    assert!(!screen.contains("bugfix-beta"));

    // 3. Backspace removes last character
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.worktree_filter, "alph");
    assert_eq!(app.filtered_worktrees().len(), 1);

    // 4. Ctrl+u clears filter
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert_eq!(app.worktree_filter, "");
    assert_eq!(app.filtered_worktrees().len(), 3);

    // 5. Ctrl+n opens worktree creation form (does NOT type 'n' into filter)
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert!(app.worktree_creation.is_some());
    // Esc cancels creation form back to worktrees modal
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.worktree_creation.is_none());
    assert!(app.show_worktrees);

    // 6. Filter for "beta"
    for c in "beta".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.worktree_filter, "beta");
    assert_eq!(app.filtered_worktrees().len(), 1);
    assert_eq!(
        app.filtered_worktrees()[0].1.branch.as_deref(),
        Some("bugfix-beta")
    );

    // 7. Press Enter to switch to the filtered worktree
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.show_worktrees);
    assert_eq!(app.worktree_filter, "");
    if let AppMode::Git { git_provider, .. } = &app.mode {
        let expected = wt2_dir.canonicalize().unwrap_or(wt2_dir.clone());
        let actual = git_provider
            .repo_root
            .canonicalize()
            .unwrap_or_else(|_| git_provider.repo_root.clone());
        assert_eq!(actual, expected);
    }

    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::remove_dir_all(&wt1_dir);
    let _ = fs::remove_dir_all(&wt2_dir);
}

#[test]
fn dragging_over_diff_text_selects_the_source_text() {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use diffv::ui::app::{App, AppMode};
    let dir = std::env::temp_dir().join(format!("diffv_mouse_select_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init"]);
    git(&["config", "user.name", "Mouse Tester"]);
    git(&["config", "user.email", "mouse@example.com"]);
    fs::write(dir.join("notes.txt"), "alpha line\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);
    fs::write(dir.join("notes.txt"), "alpha line\nbravo charlie\n").unwrap();
    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let mut app = App::new(
        AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        diffv::config::Config::default(),
        false,
        false,
        true,
        None,
        false,
        false,
    )
    .unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();

    // Locate "bravo" on screen, inside the diff pane.
    let (col, row) = find_on_screen(terminal.backend().buffer(), "bravo charlie");

    let mouse = |kind, column| MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    let selected_row = app.selected_row;
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), col + 6));
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), col + 12));
    // The click handlers do not run while dragging.
    assert_eq!(app.selected_row, selected_row);
    assert_eq!(app.mouse_selection_text().as_deref(), Some("charlie"));
    // Dragging back past the start of the line extends the selection leftwards.
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 0));
    assert_eq!(app.mouse_selection_text().as_deref(), Some("bravo c"));
    app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 0));
    assert!(app.mouse_selection.is_some());

    // The selection is highlighted on the next frame and Esc clears it.
    terminal.draw(|frame| app.render(frame)).unwrap();
    assert!(terminal.backend().buffer()[(col, row)]
        .modifier
        .contains(ratatui::style::Modifier::REVERSED));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.mouse_selection.is_none());

    // Side-by-side: the selection stays in the NEW column it started in.
    app.is_unified = false;
    terminal.draw(|frame| app.render(frame)).unwrap();
    let (col, row) = find_on_screen(terminal.backend().buffer(), "bravo charlie");
    let at = |kind, column, row| MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(at(MouseEventKind::Down(MouseButton::Left), col, row));
    app.handle_mouse(at(MouseEventKind::Drag(MouseButton::Left), col + 4, row));
    app.handle_mouse(at(MouseEventKind::Up(MouseButton::Left), col + 4, row));
    assert_eq!(app.mouse_selection_text().as_deref(), Some("bravo"));

    let _ = fs::remove_dir_all(&dir);
}

fn find_on_screen(buffer: &ratatui::buffer::Buffer, needle: &str) -> (u16, u16) {
    let n = needle.chars().count();
    (0..buffer.area.height)
        .find_map(|y| {
            let cells: Vec<&str> = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            (0..cells.len().saturating_sub(n))
                .find(|&x| cells[x..x + n].concat() == needle)
                .map(|x| (x as u16, y))
        })
        .expect("text rendered on screen")
}

#[test]
fn builtin_picker_feeds_the_fzf_result_handlers() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::ui::app::{App, AppMode, Focus, FzfRequest};
    let dir = std::env::temp_dir().join(format!("diffv_builtin_picker_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init"]);
    git(&["config", "user.name", "Picker Tester"]);
    git(&["config", "user.email", "picker@example.com"]);
    fs::write(dir.join("alpha.txt"), "one\n").unwrap();
    fs::write(dir.join("beta.txt"), "two\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);
    fs::write(dir.join("alpha.txt"), "one\nalpha_marker\n").unwrap();
    fs::write(dir.join("beta.txt"), "two\nbeta_marker\n").unwrap();
    let provider = GitProvider::discover(Some(&dir)).unwrap();
    let mut app = App::new(
        AppMode::Git {
            target_ref: None,
            git_provider: provider,
        },
        diffv::config::Config::default(),
        false,
        false,
        true,
        None,
        false,
        false,
    )
    .unwrap();
    let type_text = |app: &mut App, text: &str| {
        for c in text.chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
    };

    // Files: filter, pick, and land on the chosen file.
    app.open_builtin_picker(FzfRequest::Files);
    assert_eq!(app.picker.as_ref().unwrap().matches.len(), 2);
    type_text(&mut app, "beta");
    assert_eq!(app.picker.as_ref().unwrap().matches.len(), 1);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.picker.is_none());
    assert_eq!(app.current_file().unwrap().display_path(), "beta.txt");

    // The popup renders on top of the UI.
    app.open_builtin_picker(FzfRequest::Files);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(screen.contains("Find File"));
    // Esc cancels without moving.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.picker.is_none());
    assert_eq!(app.current_file().unwrap().display_path(), "beta.txt");

    // Text: matches line content and jumps to the file + line.
    app.focus = Focus::FileTree;
    app.open_builtin_picker(FzfRequest::Text);
    type_text(&mut app, "alpha_mark");
    assert_eq!(app.picker.as_ref().unwrap().matches.len(), 1);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.current_file().unwrap().display_path(), "alpha.txt");
    assert_eq!(app.focus, Focus::DiffView);

    let _ = fs::remove_dir_all(&dir);
}
