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
