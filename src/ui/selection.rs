//! Mouse text selection over the diff panes.
//!
//! The renderers record, for every screen row of a diff pane, which characters of
//! the *source* line each cell shows (after wrapping / horizontal scrolling). The
//! selection is anchored to those source positions, so the copied text is the real
//! line content — never gutters, markers or expanded tabs.

use ratatui::layout::Rect;
use unicode_width::UnicodeWidthChar;

use crate::core::models::{DiffKind, DiffLine, FileDiff};

/// Which column of text the selection lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPane {
    Unified,
    Old,
    New,
}

/// One rendered screen row of a pane: cell `x + i` shows char `cells[i]` of `line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRow {
    pub y: u16,
    pub x: u16,
    pub line: usize,
    /// First char index shown on this row (used when the pointer is left of the text).
    pub start: usize,
    /// Char index right after the last char shown on this row.
    pub end: usize,
    pub cells: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct TextMap {
    pub panes: Vec<(SelectionPane, Rect, Vec<TextRow>)>,
}

/// A position between two chars of a source line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Caret {
    pub line: usize,
    pub ch: usize,
}

/// The cell under the pointer, as the carets right before and after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub line: usize,
    pub before: usize,
    pub after: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseSelection {
    pub pane: SelectionPane,
    pub anchor: Hit,
    pub head: Hit,
    pub dragging: bool,
}

impl MouseSelection {
    /// Half-open `[start, end)` range covering every cell from anchor to head.
    pub fn range(&self) -> (Caret, Caret) {
        selection_range(self.anchor, self.head)
    }
}

pub fn selection_range(anchor: Hit, head: Hit) -> (Caret, Caret) {
    let (first, last) = if (anchor.line, anchor.before) <= (head.line, head.before) {
        (anchor, head)
    } else {
        (head, anchor)
    };
    (
        Caret {
            line: first.line,
            ch: first.before,
        },
        Caret {
            line: last.line,
            ch: last.after,
        },
    )
}

pub fn is_selected(range: (Caret, Caret), line: usize, ch: usize) -> bool {
    let pos = Caret { line, ch };
    range.0 <= pos && pos < range.1
}

/// Lays out one source line the same way `horizontal::wrap_line` (when `wrap_width`
/// is set) or `horizontal::scroll_line` do, returning the char index shown by each
/// cell of each visual row, plus the `[start, end)` char span of every row.
pub fn layout_line(
    text: &str,
    wrap_width: Option<usize>,
    scroll_x: usize,
) -> Vec<(usize, usize, Vec<usize>)> {
    let char_count = text.chars().count();
    match wrap_width {
        Some(width) => {
            let available = width.max(1);
            let mut rows = Vec::new();
            let mut cells = Vec::new();
            let mut start = 0;
            let mut used = 0;
            for (idx, ch) in text.chars().enumerate() {
                let (count, cell_w) = if ch == '\t' {
                    (4, 1)
                } else {
                    (1, ch.width().unwrap_or(0))
                };
                for _ in 0..count {
                    if used + cell_w > available && used > 0 {
                        rows.push((start, idx, std::mem::take(&mut cells)));
                        start = idx;
                        used = 0;
                    }
                    cells.extend(std::iter::repeat_n(idx, cell_w));
                    used += cell_w;
                }
            }
            rows.push((start, char_count, cells));
            rows
        }
        None => {
            let mut offset = scroll_x;
            let mut cells = Vec::new();
            let mut start = None;
            for (idx, ch) in text.chars().enumerate() {
                let w = ch.width().unwrap_or(0);
                if offset >= w && offset > 0 {
                    offset -= w;
                    continue;
                }
                start.get_or_insert(idx);
                cells.extend(std::iter::repeat_n(idx, w - offset));
                offset = 0;
            }
            vec![(start.unwrap_or(char_count), char_count, cells)]
        }
    }
}

impl TextMap {
    pub fn clear(&mut self) {
        self.panes.clear();
    }

    pub fn pane_at(&self, col: u16, row: u16) -> Option<SelectionPane> {
        self.panes
            .iter()
            .find(|(_, area, rows)| !rows.is_empty() && area.contains((col, row).into()))
            .map(|(pane, _, _)| *pane)
    }

    /// Resolves a screen cell to source positions, clamping to the nearest rendered row
    /// so dragging past the pane edges keeps extending the selection.
    pub fn hit(&self, pane: SelectionPane, col: u16, row: u16) -> Option<Hit> {
        let (_, _, rows) = self.panes.iter().find(|(p, _, _)| *p == pane)?;
        let first = rows.first()?;
        let last = rows.last()?;
        if row < first.y {
            return Some(Hit {
                line: first.line,
                before: first.start,
                after: first.start,
            });
        }
        let text_row = rows.iter().rev().find(|r| r.y <= row).unwrap_or(last);
        if text_row.y != row {
            // Below the last rendered row or on a row without text: snap to its end.
            return Some(Hit {
                line: text_row.line,
                before: text_row.end,
                after: text_row.end,
            });
        }
        let caret = |ch| Hit {
            line: text_row.line,
            before: ch,
            after: ch,
        };
        if col < text_row.x {
            return Some(caret(text_row.start));
        }
        match text_row.cells.get((col - text_row.x) as usize) {
            Some(&ch) => Some(Hit {
                line: text_row.line,
                before: ch,
                after: ch + 1,
            }),
            None => Some(caret(text_row.end)),
        }
    }
}

/// Joins the selected part of each source line; lines without text (hunk headers,
/// filler rows of the other side) are skipped.
pub fn extract_text(lines: &[Option<&str>], range: (Caret, Caret)) -> String {
    let (start, end) = range;
    let mut parts = Vec::new();
    for line in start.line..=end.line.min(lines.len().saturating_sub(1)) {
        let Some(text) = lines.get(line).copied().flatten() else {
            continue;
        };
        let from = if line == start.line { start.ch } else { 0 };
        let to = if line == end.line { end.ch } else { usize::MAX };
        parts.push(
            text.chars()
                .skip(from)
                .take(to.saturating_sub(from))
                .collect::<String>(),
        );
    }
    parts.join("\n")
}

fn real_line(line: Option<&DiffLine>) -> Option<&str> {
    line.filter(|l| l.kind != DiffKind::Virtual)
        .map(|l| l.content.as_str())
}

/// Source text of every line id a pane uses, indexed like the renderers' logical rows.
pub fn pane_lines(file: &FileDiff, pane: SelectionPane) -> Vec<Option<&str>> {
    match pane {
        SelectionPane::Unified => file
            .hunks
            .iter()
            .flat_map(|h| std::iter::once(None).chain(h.lines.iter().map(|l| real_line(Some(l)))))
            .collect(),
        SelectionPane::Old => file
            .aligned_rows
            .iter()
            .map(|r| real_line(r.left.as_ref()))
            .collect(),
        SelectionPane::New => file
            .aligned_rows
            .iter()
            .map(|r| real_line(r.right.as_ref()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with(rows: Vec<TextRow>) -> TextMap {
        TextMap {
            panes: vec![(SelectionPane::Unified, Rect::new(0, 0, 40, 10), rows)],
        }
    }

    fn row(y: u16, line: usize, text: &str, wrap: Option<usize>, x: u16) -> Vec<TextRow> {
        layout_line(text, wrap, 0)
            .into_iter()
            .enumerate()
            .map(|(i, (start, end, cells))| TextRow {
                y: y + i as u16,
                x,
                line,
                start,
                end,
                cells,
            })
            .collect()
    }

    #[test]
    fn layout_wraps_like_wrap_line_and_maps_wide_chars_and_tabs() {
        let rows = layout_line("ab界\tx", Some(4), 0);
        // "ab界" fills 4 cells, the tab expands to 4 cells, then "x".
        assert_eq!(rows[0], (0, 3, vec![0, 1, 2, 2]));
        assert_eq!(rows[1], (3, 4, vec![3, 3, 3, 3]));
        assert_eq!(rows[2], (4, 5, vec![4]));
        assert_eq!(layout_line("", Some(4), 0), vec![(0, 0, vec![])]);
    }

    #[test]
    fn layout_scrolls_horizontally() {
        // Scrolling one cell into a wide char shows its remaining half.
        let rows = layout_line("a界bc", None, 2);
        assert_eq!(rows, vec![(1, 4, vec![1, 2, 3])]);
        assert_eq!(layout_line("ab", None, 5), vec![(2, 2, vec![])]);
    }

    #[test]
    fn range_from_screen_cells_is_ordered_and_inclusive() {
        let mut rows = row(0, 0, "hello world", None, 5);
        rows.extend(row(1, 1, "second line", None, 5));
        let map = map_with(rows);
        let anchor = map.hit(SelectionPane::Unified, 11, 0).unwrap(); // 'w'
        let head = map.hit(SelectionPane::Unified, 10, 1).unwrap(); // 'd'
        let range = selection_range(anchor, head);
        assert_eq!(range, (Caret { line: 0, ch: 6 }, Caret { line: 1, ch: 6 }));
        // Dragging backwards yields the same range.
        assert_eq!(selection_range(head, anchor), range);
        assert!(is_selected(range, 0, 6));
        assert!(is_selected(range, 1, 5));
        assert!(!is_selected(range, 1, 6));
        assert!(!is_selected(range, 0, 5));
    }

    #[test]
    fn hits_clamp_to_gutter_line_end_and_pane_edges() {
        let mut rows = row(2, 4, "abc", None, 5);
        rows.extend(row(3, 5, "de", None, 5));
        let map = map_with(rows);
        let gutter = map.hit(SelectionPane::Unified, 1, 2).unwrap();
        assert_eq!((gutter.before, gutter.after), (0, 0));
        let past_end = map.hit(SelectionPane::Unified, 30, 3).unwrap();
        assert_eq!((past_end.line, past_end.before), (5, 2));
        let above = map.hit(SelectionPane::Unified, 9, 0).unwrap();
        assert_eq!((above.line, above.before), (4, 0));
        let below = map.hit(SelectionPane::Unified, 9, 9).unwrap();
        assert_eq!((below.line, below.before), (5, 2));
        assert_eq!(map.pane_at(6, 3), Some(SelectionPane::Unified));
        assert_eq!(map.pane_at(60, 3), None);
    }

    #[test]
    fn selection_across_wrapped_rows_maps_back_to_source_text() {
        let map = map_with(row(0, 0, "abcdefgh", Some(3), 2));
        // Row 1 shows "def"; row 2 shows "gh".
        let anchor = map.hit(SelectionPane::Unified, 3, 1).unwrap(); // 'e'
        let head = map.hit(SelectionPane::Unified, 2, 2).unwrap(); // 'g'
        let lines = [Some("abcdefgh")];
        assert_eq!(extract_text(&lines, selection_range(anchor, head)), "efg");
        // Past the end of a wrapped segment stops at that segment, not the next char.
        let end = map.hit(SelectionPane::Unified, 9, 0).unwrap();
        let start = map.hit(SelectionPane::Unified, 2, 0).unwrap();
        assert_eq!(extract_text(&lines, selection_range(start, end)), "abc");
    }

    #[test]
    fn extract_spans_lines_and_skips_rows_without_text() {
        let lines = [Some("fn main() {"), None, Some("    let x = 1;"), Some("}")];
        let range = (Caret { line: 0, ch: 3 }, Caret { line: 3, ch: 1 });
        assert_eq!(extract_text(&lines, range), "main() {\n    let x = 1;\n}");
        let single = (Caret { line: 2, ch: 4 }, Caret { line: 2, ch: 7 });
        assert_eq!(extract_text(&lines, single), "let");
        let multibyte = [Some("olá, 界!")];
        let range = (Caret { line: 0, ch: 2 }, Caret { line: 0, ch: 6 });
        assert_eq!(extract_text(&multibyte, range), "á, 界");
    }
}
