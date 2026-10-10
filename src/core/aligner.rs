use rayon::prelude::*;

use super::intraline::compute_intraline_spans;
use super::models::{AlignedRow, DiffKind, DiffLine, FileDiff, Hunk};

/// Aligns hunks into side-by-side rows with filler lines and intraline highlighting.
pub fn align_hunks_side_by_side(hunks: &[Hunk]) -> Vec<AlignedRow> {
    let total: usize = hunks.iter().map(|h| h.lines.len()).sum();
    let mut aligned_rows = Vec::with_capacity(total);

    for (hunk_idx, hunk) in hunks.iter().enumerate() {
        let mut i = 0;
        let lines = &hunk.lines;

        while i < lines.len() {
            match lines[i].kind {
                DiffKind::Context => {
                    aligned_rows.push(AlignedRow {
                        left: Some(lines[i].clone()),
                        right: Some(lines[i].clone()),
                        hunk_index: Some(hunk_idx),
                        left_line_idx: Some(i),
                        right_line_idx: Some(i),
                    });
                    i += 1;
                }
                DiffKind::Deletion | DiffKind::Addition => {
                    // Indices of the consecutive deletions and additions of this block;
                    // each line is cloned once, straight into its row.
                    let mut del_indices = Vec::new();
                    let mut add_indices = Vec::new();
                    while i < lines.len()
                        && matches!(lines[i].kind, DiffKind::Deletion | DiffKind::Addition)
                    {
                        if lines[i].kind == DiffKind::Deletion {
                            del_indices.push(i);
                        } else {
                            add_indices.push(i);
                        }
                        i += 1;
                    }

                    let max_len = del_indices.len().max(add_indices.len());
                    for row_idx in 0..max_len {
                        let left_line_idx = del_indices.get(row_idx).copied();
                        let right_line_idx = add_indices.get(row_idx).copied();
                        let left_line = left_line_idx
                            .map(|idx| lines[idx].clone())
                            .unwrap_or_else(DiffLine::virtual_line);
                        let right_line = right_line_idx
                            .map(|idx| lines[idx].clone())
                            .unwrap_or_else(DiffLine::virtual_line);

                        aligned_rows.push(AlignedRow {
                            left: Some(left_line),
                            right: Some(right_line),
                            hunk_index: Some(hunk_idx),
                            left_line_idx,
                            right_line_idx,
                        });
                    }
                }
                DiffKind::Virtual => {
                    i += 1;
                }
            }
        }
    }

    // Intra-line highlighting for paired modified lines. It is the costly part
    // (a token diff per pair), so a file with many edits spreads it over all cores.
    aligned_rows
        .par_iter_mut()
        .with_min_len(512)
        .for_each(|row| {
            if let (Some(left), Some(right)) = (&mut row.left, &mut row.right) {
                if left.kind == DiffKind::Deletion && right.kind == DiffKind::Addition {
                    let (left_spans, right_spans) =
                        compute_intraline_spans(&left.content, &right.content);
                    left.spans = left_spans;
                    right.spans = right_spans;
                }
            }
        });

    aligned_rows
}

/// Fills `aligned_rows` for every file, spreading the files over all cores.
pub fn align_files(files: &mut [FileDiff]) {
    files
        .par_iter_mut()
        .for_each(|file| file.aligned_rows = align_hunks_side_by_side(&file.hunks));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_by_side_alignment_with_fillers() {
        let hunk = Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 4,
            header: "@@ -1,3 +1,4 @@".to_string(),
            lines: vec![
                DiffLine {
                    kind: DiffKind::Context,
                    content: "line 1".to_string(),
                    old_line_no: Some(1),
                    new_line_no: Some(1),
                    spans: vec![],
                },
                DiffLine {
                    kind: DiffKind::Deletion,
                    content: "line 2 old".to_string(),
                    old_line_no: Some(2),
                    new_line_no: None,
                    spans: vec![],
                },
                DiffLine {
                    kind: DiffKind::Addition,
                    content: "line 2 new".to_string(),
                    old_line_no: None,
                    new_line_no: Some(2),
                    spans: vec![],
                },
                DiffLine {
                    kind: DiffKind::Addition,
                    content: "line 2.5 new".to_string(),
                    old_line_no: None,
                    new_line_no: Some(3),
                    spans: vec![],
                },
                DiffLine {
                    kind: DiffKind::Context,
                    content: "line 3".to_string(),
                    old_line_no: Some(3),
                    new_line_no: Some(4),
                    spans: vec![],
                },
            ],
        };

        let rows = align_hunks_side_by_side(&[hunk]);
        // Row 0: context "line 1"
        // Row 1: "line 2 old" vs "line 2 new" (with intraline highlight)
        // Row 2: virtual line vs "line 2.5 new"
        // Row 3: context "line 3"
        assert_eq!(rows.len(), 4);

        assert_eq!(rows[0].left.as_ref().unwrap().kind, DiffKind::Context);
        assert_eq!(rows[0].right.as_ref().unwrap().kind, DiffKind::Context);

        assert_eq!(rows[1].left.as_ref().unwrap().kind, DiffKind::Deletion);
        assert_eq!(rows[1].right.as_ref().unwrap().kind, DiffKind::Addition);
        assert!(!rows[1].left.as_ref().unwrap().spans.is_empty());
        assert!(!rows[1].right.as_ref().unwrap().spans.is_empty());

        assert_eq!(rows[2].left.as_ref().unwrap().kind, DiffKind::Virtual);
        assert_eq!(rows[2].right.as_ref().unwrap().kind, DiffKind::Addition);

        assert_eq!(rows[3].left.as_ref().unwrap().kind, DiffKind::Context);
        assert_eq!(rows[3].right.as_ref().unwrap().kind, DiffKind::Context);
    }
}
