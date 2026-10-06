use diffv::git::actions::{stage_path, stage_paths, unstage_path, unstage_paths};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Temporary repository with one commit: `src/lib.rs`, `src/ui/app.rs`,
/// `docs/guide.md` and `top.txt`. Removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("diffv_stage_path_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = Self { dir };
        repo.git(&["init"]);
        repo.git(&["config", "user.name", "Path Tester"]);
        repo.git(&["config", "user.email", "path@example.com"]);
        repo.write("src/lib.rs", "lib\n");
        repo.write("src/ui/app.rs", "app\n");
        repo.write("docs/guide.md", "guide\n");
        repo.write("top.txt", "top\n");
        repo.git(&["add", "."]);
        repo.git(&["commit", "-m", "base"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn write(&self, rel: &str, content: &str) {
        let path = self.dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// Paths with staged changes, sorted.
    fn staged(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .git(&["diff", "--cached", "--name-only"])
            .lines()
            .map(str::to_string)
            .collect();
        names.sort();
        names
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn stage_path_on_a_file_stages_only_that_file() {
    let repo = Repo::new("file");
    repo.write("src/lib.rs", "lib changed\n");
    repo.write("top.txt", "top changed\n");

    stage_path(&repo.dir, Path::new("src/lib.rs")).unwrap();
    assert_eq!(repo.staged(), vec!["src/lib.rs"]);

    unstage_path(&repo.dir, Path::new("src/lib.rs")).unwrap();
    assert!(repo.staged().is_empty());
    // Unstaging keeps the working tree edit.
    assert_eq!(
        fs::read_to_string(repo.dir.join("src/lib.rs")).unwrap(),
        "lib changed\n"
    );
}

#[test]
fn stage_path_on_a_directory_is_recursive_and_adds_untracked_files() {
    let repo = Repo::new("dir");
    repo.write("src/lib.rs", "lib changed\n");
    repo.write("src/ui/app.rs", "app changed\n");
    repo.write("src/ui/widgets/new.rs", "brand new\n");
    repo.write("src/fresh.rs", "untracked\n");
    fs::remove_file(repo.dir.join("src/ui/app.rs")).unwrap();
    repo.write("docs/guide.md", "guide changed\n");

    stage_path(&repo.dir, Path::new("src")).unwrap();
    assert_eq!(
        repo.staged(),
        vec![
            "src/fresh.rs",
            "src/lib.rs",
            "src/ui/app.rs",
            "src/ui/widgets/new.rs"
        ],
        "everything under src/, deletion and untracked files included; docs/ untouched"
    );
    let status = repo.git(&["status", "--porcelain"]);
    assert!(status.contains("D  src/ui/app.rs"), "{}", status);
    assert!(status.contains("A  src/fresh.rs"), "{}", status);

    // Unstaging a nested directory only touches that subtree.
    unstage_path(&repo.dir, Path::new("src/ui")).unwrap();
    assert_eq!(repo.staged(), vec!["src/fresh.rs", "src/lib.rs"]);
    let status = repo.git(&["status", "--porcelain"]);
    assert!(
        status.contains("?? src/ui/widgets/"),
        "a never-committed file goes back to untracked: {}",
        status
    );

    unstage_path(&repo.dir, Path::new("src")).unwrap();
    assert!(repo.staged().is_empty());
}

#[test]
fn stage_paths_takes_several_paths_literally() {
    let repo = Repo::new("many");
    repo.write("src/lib.rs", "lib changed\n");
    repo.write("docs/guide.md", "guide changed\n");
    repo.write("top.txt", "top changed\n");
    repo.write("docs/*.md", "a file named like a glob\n");

    stage_paths(&repo.dir, &[PathBuf::from("src"), PathBuf::from("top.txt")]).unwrap();
    assert_eq!(repo.staged(), vec!["src/lib.rs", "top.txt"]);

    // `*` is not expanded: only the oddly named file is staged.
    stage_paths(&repo.dir, &[Path::new("docs/*.md")]).unwrap();
    assert_eq!(repo.staged(), vec!["docs/*.md", "src/lib.rs", "top.txt"]);

    unstage_paths(&repo.dir, &[Path::new("docs/*.md"), Path::new("top.txt")]).unwrap();
    assert_eq!(repo.staged(), vec!["src/lib.rs"]);

    // Nothing to do is not an error.
    stage_paths::<PathBuf>(&repo.dir, &[]).unwrap();
    unstage_paths::<PathBuf>(&repo.dir, &[]).unwrap();
    assert_eq!(repo.staged(), vec!["src/lib.rs"]);
}

#[test]
fn stage_path_reports_git_errors_instead_of_panicking() {
    let repo = Repo::new("error");
    let err = stage_path(&repo.dir, Path::new("does/not/exist")).unwrap_err();
    assert!(err.to_string().contains("Failed to stage"), "{}", err);
}

#[test]
fn s_and_u_in_the_changes_list_act_on_file_folder_and_section() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::core::models::{DiffSection, Language};
    use diffv::git::provider::GitProvider;
    use diffv::ui::app::{App, AppMode, Focus};
    use diffv::ui::components::file_tree::FileViewMode;

    let repo = Repo::new("keys");
    repo.write("src/lib.rs", "lib changed\n");
    repo.write("src/ui/app.rs", "app changed\n");
    repo.write("src/ui/new.rs", "untracked\n");
    repo.write("top.txt", "top changed\n");

    let provider = GitProvider::discover(Some(&repo.dir)).unwrap();
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
    app.reload_diffs();
    app.focus = Focus::FileTree;
    let press = |app: &mut App, c: char| {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    };
    let select = |app: &mut App, path: &str| {
        app.selected_tree_idx = app
            .tree_items
            .iter()
            .position(|i| i.path == Path::new(path))
            .unwrap_or_else(|| panic!("no tree item {}", path));
    };
    let select_file = |app: &mut App, path: &str, section: DiffSection| {
        app.selected_tree_idx = app
            .tree_items
            .iter()
            .position(|i| {
                i.file_index.is_some_and(|f| {
                    app.files[f].new_path == Path::new(path) && app.files[f].section == section
                })
            })
            .unwrap_or_else(|| panic!("no file item {} in {:?}", path, section));
    };
    let note = |app: &App| {
        app.notification
            .as_ref()
            .map(|(m, _)| m.clone())
            .unwrap_or_default()
    };

    // Folder (nothing staged yet, so no section prefix): recursive, untracked included.
    select(&mut app, "src");
    press(&mut app, 's');
    assert_eq!(
        repo.staged(),
        vec!["src/lib.rs", "src/ui/app.rs", "src/ui/new.rs"]
    );
    assert_eq!(note(&app), "✓ Staged 3 files under src/");
    assert_eq!(app.focus, Focus::FileTree);

    // Nested folder inside the Staged section.
    select(&mut app, ":staged/src/ui");
    press(&mut app, 'u');
    assert_eq!(repo.staged(), vec!["src/lib.rs"]);
    assert_eq!(note(&app), "✓ Unstaged 2 files under src/ui/");

    // File: the whole file, not a hunk.
    select_file(&mut app, "top.txt", DiffSection::Changes);
    press(&mut app, 's');
    assert_eq!(repo.staged(), vec!["src/lib.rs", "top.txt"]);
    assert_eq!(note(&app), "✓ Staged top.txt");

    // Nothing left to do: a notice, the index is unchanged.
    select_file(&mut app, "top.txt", DiffSection::Staged);
    press(&mut app, 's');
    assert_eq!(note(&app), "Nothing to stage in top.txt");
    select(&mut app, ":changes/src/ui");
    press(&mut app, 'u');
    assert_eq!(note(&app), "Nothing staged in src/ui/");
    assert_eq!(repo.staged(), vec!["src/lib.rs", "top.txt"]);

    // Section header: every file of that section.
    app.language = Language::Pt;
    app.update_filter();
    select(&mut app, ":changes");
    press(&mut app, 's');
    assert_eq!(
        repo.staged(),
        vec!["src/lib.rs", "src/ui/app.rs", "src/ui/new.rs", "top.txt"]
    );
    assert_eq!(note(&app), "✓ 2 arquivos preparados em Mudanças");
    select(&mut app, ":staged");
    press(&mut app, 'u');
    assert!(repo.staged().is_empty());
    assert_eq!(note(&app), "✓ 4 arquivos despreparados em Staged");
    app.language = Language::En;

    // Flat list: the selected file.
    app.file_view_mode = FileViewMode::Flat;
    app.update_filter();
    app.selected_filtered_idx = app
        .filtered_indices
        .iter()
        .position(|&i| app.files[i].new_path == Path::new("src/ui/app.rs"))
        .unwrap();
    press(&mut app, 's');
    assert_eq!(repo.staged(), vec!["src/ui/app.rs"]);
    assert_eq!(note(&app), "✓ Staged src/ui/app.rs");
    app.selected_filtered_idx = app
        .filtered_indices
        .iter()
        .position(|&i| app.files[i].new_path == Path::new("src/ui/app.rs"))
        .unwrap();
    press(&mut app, 'u');
    assert!(repo.staged().is_empty());

    // Uppercase S / U keep acting on the current file.
    app.file_view_mode = FileViewMode::Tree;
    app.update_filter();
    select(&mut app, "top.txt");
    press(&mut app, 'S');
    assert_eq!(repo.staged(), vec!["top.txt"]);
    select_file(&mut app, "top.txt", DiffSection::Staged);
    press(&mut app, 'U');
    assert!(repo.staged().is_empty());
}

#[test]
fn s_in_the_diff_pane_still_stages_only_the_hunk() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use diffv::core::models::DiffSection;
    use diffv::git::provider::GitProvider;
    use diffv::ui::app::{App, AppMode, Focus};

    let repo = Repo::new("hunk");
    let original: String = (1..=60).map(|i| format!("line {}\n", i)).collect();
    repo.write("src/lib.rs", &original);
    repo.git(&["commit", "-am", "longer"]);
    repo.write(
        "src/lib.rs",
        &original
            .replace("line 10\n", "line 10 a\n")
            .replace("line 50\n", "line 50 b\n"),
    );

    let provider = GitProvider::discover(Some(&repo.dir)).unwrap();
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
    assert!(app.jump_to_file_in("src/lib.rs", DiffSection::Changes));
    assert_eq!(app.current_file().unwrap().hunks.len(), 2);
    app.focus = Focus::DiffView;
    app.jump_to_line(10);
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    let cached = repo.git(&["diff", "--cached"]);
    assert!(
        cached.contains("+line 10 a") && !cached.contains("line 50 b"),
        "{}",
        cached
    );
}
