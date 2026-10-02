use std::fs;
use diffv::core::engine::DiffEngine;
use diffv::core::models::FileStatus;

#[test]
fn test_compare_files() {
    let temp_dir = std::env::temp_dir().join("diffv_test_files");
    let _ = fs::create_dir_all(&temp_dir);

    let file_a = temp_dir.join("file_a.txt");
    let file_b = temp_dir.join("file_b.txt");

    fs::write(&file_a, "alpha\nbeta\ngamma\n").unwrap();
    fs::write(&file_b, "alpha\nbeta mod\ngamma\ndelta\n").unwrap();

    let engine = DiffEngine::default();
    let diff = engine.compare_files(&file_a, &file_b).unwrap();

    assert_eq!(diff.status, FileStatus::Modified);
    assert_eq!(diff.stats.additions, 2);
    assert_eq!(diff.stats.deletions, 1);
    assert!(!diff.aligned_rows.is_empty());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_compare_directories() {
    let temp_dir = std::env::temp_dir().join("diffv_test_dirs");
    let dir_a = temp_dir.join("dir_a");
    let dir_b = temp_dir.join("dir_b");

    let _ = fs::create_dir_all(&dir_a);
    let _ = fs::create_dir_all(&dir_b);

    // Modified file
    fs::write(dir_a.join("common.txt"), "hello world\n").unwrap();
    fs::write(dir_b.join("common.txt"), "hello rust world\n").unwrap();

    // Deleted file
    fs::write(dir_a.join("deleted.txt"), "bye\n").unwrap();

    // Added file
    fs::write(dir_b.join("added.txt"), "welcome\n").unwrap();

    let engine = DiffEngine::default();
    let diffs = engine.compare_directories(&dir_a, &dir_b).unwrap();

    assert_eq!(diffs.len(), 3);

    let added = diffs.iter().find(|d| d.status == FileStatus::Added);
    assert!(added.is_some());
    assert_eq!(added.unwrap().stats.additions, 1);

    let deleted = diffs.iter().find(|d| d.status == FileStatus::Deleted);
    assert!(deleted.is_some());
    assert_eq!(deleted.unwrap().stats.deletions, 1);

    let modified = diffs.iter().find(|d| d.status == FileStatus::Modified);
    assert!(modified.is_some());

    let _ = fs::remove_dir_all(&temp_dir);
}
