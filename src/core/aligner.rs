use super::intraline::compute_intraline_spans;
use super::models::{AlignedRow, DiffKind, DiffLine, Hunk};

/// Aligns hunks into side-by-side rows with filler lines and intraline highlighting.
pub fn align_hunks_side_by_side(hunks: &[Hunk]) -> Vec<AlignedRow> {
    let mut aligned_rows = Vec::new();

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
                    });
                    i += 1;
                }
                DiffKind::Deletion | DiffKind::Addition => {
                    // Collect consecutive deletions and additions in this change block
                    let mut deletions = Vec::new();
                    let mut additions = Vec::new();

                    while i < lines.len() && (lines[i].kind == DiffKind::Deletion || lines[i].kind == DiffKind::Addition) {
                        if lines[i].kind == DiffKind::Deletion {
                            deletions.push(lines[i].clone());
                        } else {
                            additions.push(lines[i].clone());
                        }
                        i += 1;
                    }

                    let max_len = deletions.len().max(additions.len());
                    for row_idx in 0..max_len {
                        let mut left_line = if row_idx < deletions.len() {
                            Some(deletions[row_idx].clone())
                        } else {
                            Some(DiffLine::virtual_line())
                        };

                        let mut right_line = if row_idx < additions.len() {
                            Some(additions[row_idx].clone())
                        } else {
                            Some(DiffLine::virtual_line())
                        };

                        // Compute intra-line highlighting for paired modified lines
                        if row_idx < deletions.len() && row_idx < additions.len() {
                            let left_ref = left_line.as_mut().unwrap();
                            let right_ref = right_line.as_mut().unwrap();
                            let (left_spans, right_spans) = compute_intraline_spans(&left_ref.content, &right_ref.content);
                            left_ref.spans = left_spans;
                            right_ref.spans = right_spans;
                        }

                        aligned_rows.push(AlignedRow {
                            left: left_line,
                            right: right_line,
                            hunk_index: Some(hunk_idx),
                        });
                    }
                }
                DiffKind::Virtual => {
                    i += 1;
                }
            }
        }
    }

    aligned_rows
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
