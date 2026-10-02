use diffv::core::models::{DiffKind, FileStatus};
use diffv::git::patch::{generate_hunk_patch, parse_unified_diff};
use std::path::PathBuf;

#[test]
fn test_parse_multi_hunk_patch() {
    let diff_text = r#"diff --git a/lib/calc.py b/lib/calc.py
index 1111111..2222222 100644
--- a/lib/calc.py
+++ b/lib/calc.py
@@ -5,4 +5,5 @@ def add(a, b):
     # simple add
-    return a - b
+    return a + b
+    # fixed bug
 
@@ -20,3 +21,3 @@ def sub(a, b):
-    return a + b
+    return a - b
     # sub
"#;

    let diffs = parse_unified_diff(diff_text);
    assert_eq!(diffs.len(), 1);
    let diff = &diffs[0];
    assert_eq!(diff.new_path, PathBuf::from("lib/calc.py"));
    assert_eq!(diff.hunks.len(), 2);

    let hunk1 = &diff.hunks[0];
    assert_eq!(hunk1.old_start, 5);
    assert_eq!(hunk1.new_start, 5);
    assert_eq!(hunk1.lines[1].kind, DiffKind::Deletion);
    assert_eq!(hunk1.lines[2].kind, DiffKind::Addition);

    let hunk2 = &diff.hunks[1];
    assert_eq!(hunk2.old_start, 20);
    assert_eq!(hunk2.new_start, 21);

    // Test patch generation for hunk 1
    let generated_patch = generate_hunk_patch(&diff.new_path, hunk1);
    assert!(generated_patch.contains("--- a/lib/calc.py"));
    assert!(generated_patch.contains("+++ b/lib/calc.py"));
    assert!(generated_patch.contains("-    return a - b"));
    assert!(generated_patch.contains("+    return a + b"));
}

#[test]
fn test_parse_new_and_deleted_files() {
    let diff_text = r#"diff --git a/new.rs b/new.rs
new file mode 100644
--- /dev/null
+++ b/new.rs
@@ -0,0 +1,2 @@
+fn hello() {}
+fn world() {}
diff --git a/old.rs b/old.rs
deleted file mode 100644
--- a/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn obsolete() {}
-fn unused() {}
"#;

    let diffs = parse_unified_diff(diff_text);
    assert_eq!(diffs.len(), 2);

    assert_eq!(diffs[0].status, FileStatus::Added);
    assert_eq!(diffs[0].stats.additions, 2);
    assert_eq!(diffs[0].stats.deletions, 0);

    assert_eq!(diffs[1].status, FileStatus::Deleted);
    assert_eq!(diffs[1].stats.additions, 0);
    assert_eq!(diffs[1].stats.deletions, 2);
}

#[test]
fn test_generate_partial_hunk_patch() {
    let diff_text = r#"diff --git a/test.rs b/test.rs
--- a/test.rs
+++ b/test.rs
@@ -1,5 +1,6 @@
 context 1
-del 1
-del 2
+add 1
+add 2
 context 2
"#;
    let diffs = parse_unified_diff(diff_text);
    let hunk = &diffs[0].hunks[0];

    // Select only 'del 1' (index 1) and 'add 1' (index 3)
    let partial_patch =
        diffv::git::patch::generate_partial_hunk_patch(&diffs[0].new_path, hunk, &[1, 3]);

    assert!(partial_patch.contains("-del 1"));
    assert!(partial_patch.contains(" del 2")); // Unselected deletion turned into context
    assert!(partial_patch.contains("+add 1"));
    assert!(!partial_patch.contains("+add 2")); // Unselected addition omitted
}
