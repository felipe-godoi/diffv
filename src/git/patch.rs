use std::path::{Path, PathBuf};
use crate::core::models::{ChangeStats, DiffKind, DiffLine, FileDiff, FileStatus, Hunk, StageStatus};

/// Parses unified diff text into a vector of FileDiff structs.
pub fn parse_unified_diff(diff_text: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let lines: Vec<&str> = diff_text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("diff --git ") {
            let (file_diff, next_idx) = parse_file_diff(&lines, i);
            files.push(file_diff);
            i = next_idx;
        } else if line.starts_with("--- ") && i + 1 < lines.len() && lines[i + 1].starts_with("+++ ") {
            let (file_diff, next_idx) = parse_file_diff(&lines, i);
            files.push(file_diff);
            i = next_idx;
        } else {
            i += 1;
        }
    }

    files
}

fn parse_file_diff(lines: &[&str], start_idx: usize) -> (FileDiff, usize) {
    let mut old_path = None;
    let mut new_path = PathBuf::new();
    let mut status = FileStatus::Modified;
    let mut hunks = Vec::new();
    let mut i = start_idx;
    let mut is_binary = false;

    // Check git header
    if lines[i].starts_with("diff --git ") {
        let parts: Vec<&str> = lines[i]["diff --git ".len()..].split_whitespace().collect();
        if parts.len() >= 2 {
            let a = parts[0].strip_prefix("a/").unwrap_or(parts[0]);
            let b = parts[1].strip_prefix("b/").unwrap_or(parts[1]);
            old_path = Some(PathBuf::from(a));
            new_path = PathBuf::from(b);
        }
        i += 1;
    }

    while i < lines.len() && !lines[i].starts_with("diff --git ") {
        let line = lines[i];

        if line.starts_with("new file mode") {
            status = FileStatus::Added;
            i += 1;
        } else if line.starts_with("deleted file mode") {
            status = FileStatus::Deleted;
            i += 1;
        } else if line.starts_with("similarity index") || line.starts_with("rename from") || line.starts_with("rename to") {
            status = FileStatus::Renamed;
            i += 1;
        } else if line.starts_with("Binary files ") {
            is_binary = true;
            i += 1;
        } else if line.starts_with("--- ") {
            let raw_path = &line[4..].trim();
            if *raw_path == "/dev/null" {
                status = FileStatus::Added;
                old_path = None;
            } else {
                let clean = raw_path.strip_prefix("a/").unwrap_or(raw_path);
                old_path = Some(PathBuf::from(clean));
            }
            i += 1;
        } else if line.starts_with("+++ ") {
            let raw_path = &line[4..].trim();
            if *raw_path == "/dev/null" {
                status = FileStatus::Deleted;
            } else {
                let clean = raw_path.strip_prefix("b/").unwrap_or(raw_path);
                new_path = PathBuf::from(clean);
            }
            i += 1;
        } else if line.starts_with("@@ ") {
            let (hunk, next_idx) = parse_hunk(lines, i);
            hunks.push(hunk);
            i = next_idx;
        } else {
            i += 1;
        }
    }

    // Compute stats
    let mut stats = ChangeStats::default();
    for h in &hunks {
        for l in &h.lines {
            match l.kind {
                DiffKind::Addition => stats.additions += 1,
                DiffKind::Deletion => stats.deletions += 1,
                _ => {}
            }
        }
    }

    let file_diff = FileDiff {
        old_path,
        new_path,
        status,
        stage_status: StageStatus::Unstaged,
        stats,
        aligned_rows: Vec::new(),
        hunks,
        is_binary,
    };

    (file_diff, i)
}

fn parse_hunk(lines: &[&str], start_idx: usize) -> (Hunk, usize) {
    let header = lines[start_idx].to_string();
    let (old_start, old_lines, new_start, new_lines) = parse_hunk_header(&header);

    let mut hunk_lines = Vec::new();
    let mut curr_old = old_start;
    let mut curr_new = new_start;
    let mut i = start_idx + 1;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("diff --git ") || line.starts_with("@@ ") {
            break;
        }

        if line.starts_with('\\') {
            // \ No newline at end of file
            i += 1;
            continue;
        }

        if let Some(rest) = line.strip_prefix('+') {
            hunk_lines.push(DiffLine {
                kind: DiffKind::Addition,
                content: rest.to_string(),
                old_line_no: None,
                new_line_no: Some(curr_new),
                spans: Vec::new(),
            });
            curr_new += 1;
        } else if let Some(rest) = line.strip_prefix('-') {
            hunk_lines.push(DiffLine {
                kind: DiffKind::Deletion,
                content: rest.to_string(),
                old_line_no: Some(curr_old),
                new_line_no: None,
                spans: Vec::new(),
            });
            curr_old += 1;
        } else {
            // Context line: can start with space ' ' or be empty line
            let content = if let Some(rest) = line.strip_prefix(' ') {
                rest.to_string()
            } else {
                line.to_string()
            };
            hunk_lines.push(DiffLine {
                kind: DiffKind::Context,
                content,
                old_line_no: Some(curr_old),
                new_line_no: Some(curr_new),
                spans: Vec::new(),
            });
            curr_old += 1;
            curr_new += 1;
        }

        i += 1;
    }

    let hunk = Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        header,
        lines: hunk_lines,
    };

    (hunk, i)
}

fn parse_hunk_header(header: &str) -> (usize, usize, usize, usize) {
    // format: @@ -old_start,old_lines +new_start,new_lines @@
    let mut old_start = 1;
    let mut old_lines = 1;
    let mut new_start = 1;
    let mut new_lines = 1;

    if let Some(start) = header.find("@@ -") {
        let rest = &header[start + 4..];
        if let Some(end) = rest.find(" @@") {
            let range_part = &rest[..end];
            let parts: Vec<&str> = range_part.split(' ').collect();
            if parts.len() >= 2 {
                // -old
                let old_part = parts[0];
                let old_sub: Vec<&str> = old_part.split(',').collect();
                if let Ok(val) = old_sub[0].parse::<usize>() {
                    old_start = val;
                }
                if old_sub.len() > 1 {
                    if let Ok(val) = old_sub[1].parse::<usize>() {
                        old_lines = val;
                    }
                }

                // +new
                let new_part = parts[1].strip_prefix('+').unwrap_or(parts[1]);
                let new_sub: Vec<&str> = new_part.split(',').collect();
                if let Ok(val) = new_sub[0].parse::<usize>() {
                    new_start = val;
                }
                if new_sub.len() > 1 {
                    if let Ok(val) = new_sub[1].parse::<usize>() {
                        new_lines = val;
                    }
                }
            }
        }
    }

    (old_start, old_lines, new_start, new_lines)
}

/// Generates a unified patch string for a single hunk of a file.
pub fn generate_hunk_patch(file_path: &Path, hunk: &Hunk) -> String {
    let path_str = file_path.to_string_lossy();
    let old_header = if hunk.old_lines == 0 {
        "--- /dev/null".to_string()
    } else {
        format!("--- a/{}", path_str)
    };
    let new_header = if hunk.new_lines == 0 {
        "+++ /dev/null".to_string()
    } else {
        format!("+++ b/{}", path_str)
    };

    let mut patch = format!("{}\n{}\n{}\n", old_header, new_header, hunk.header);

    for line in &hunk.lines {
        match line.kind {
            DiffKind::Context => {
                patch.push(' ');
                patch.push_str(&line.content);
                patch.push('\n');
            }
            DiffKind::Addition => {
                patch.push('+');
                patch.push_str(&line.content);
                patch.push('\n');
            }
            DiffKind::Deletion => {
                patch.push('-');
                patch.push_str(&line.content);
                patch.push('\n');
            }
            DiffKind::Virtual => {}
        }
    }

    patch
}

/// Generates a partial unified patch string containing only selected lines within a hunk.
pub fn generate_partial_hunk_patch(
    file_path: &Path,
    hunk: &Hunk,
    selected_indices: &[usize],
) -> String {
    let path_str = file_path.to_string_lossy();
    let old_header = if hunk.old_lines == 0 {
        "--- /dev/null".to_string()
    } else {
        format!("--- a/{}", path_str)
    };
    let new_header = if hunk.new_lines == 0 {
        "+++ /dev/null".to_string()
    } else {
        format!("+++ b/{}", path_str)
    };

    let mut old_count = 0;
    let mut new_count = 0;
    let mut body = String::new();

    for (idx, line) in hunk.lines.iter().enumerate() {
        let is_selected = selected_indices.contains(&idx);

        match line.kind {
            DiffKind::Context => {
                body.push(' ');
                body.push_str(&line.content);
                body.push('\n');
                old_count += 1;
                new_count += 1;
            }
            DiffKind::Deletion => {
                if is_selected {
                    body.push('-');
                    body.push_str(&line.content);
                    body.push('\n');
                    old_count += 1;
                } else {
                    // Unselected deletion remains context in working tree
                    body.push(' ');
                    body.push_str(&line.content);
                    body.push('\n');
                    old_count += 1;
                    new_count += 1;
                }
            }
            DiffKind::Addition => {
                if is_selected {
                    body.push('+');
                    body.push_str(&line.content);
                    body.push('\n');
                    new_count += 1;
                }
                // Unselected addition is omitted from staged patch
            }
            DiffKind::Virtual => {}
        }
    }

    let header = format!(
        "@@ -{},{} +{},{} @@",
        hunk.old_start, old_count, hunk.new_start, new_count
    );

    format!("{}\n{}\n{}\n{}", old_header, new_header, header, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unified_diff() {
        let sample = r#"diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    let x = 1;
+    let x = 2;
+    let y = 3;
 }
"#;

        let diffs = parse_unified_diff(sample);
        assert_eq!(diffs.len(), 1);
        let f = &diffs[0];
        assert_eq!(f.new_path, PathBuf::from("src/main.rs"));
        assert_eq!(f.stats.additions, 2);
        assert_eq!(f.stats.deletions, 1);
        assert_eq!(f.hunks.len(), 1);
        let h = &f.hunks[0];
        assert_eq!(h.lines.len(), 5);
        assert_eq!(h.lines[1].kind, DiffKind::Deletion);
        assert_eq!(h.lines[2].kind, DiffKind::Addition);
        assert_eq!(h.lines[3].kind, DiffKind::Addition);
    }
}
