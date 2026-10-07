//! Preview pane of the built-in picker: the diff around the candidate under the
//! cursor, drawn in-process with the unified diff pane's gutter and colors.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::core::models::{DiffKind, DiffLine, DiffSection, FileDiff, Language};
use crate::ui::app::FzfRequest;
use crate::ui::theme::Theme;

/// One row of the preview: a hunk header or a diff line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewRow {
    Header(String),
    Line {
        kind: DiffKind,
        old_no: Option<usize>,
        new_no: Option<usize>,
        text: String,
        /// The line the candidate points at.
        target: bool,
        /// Byte range of `text` matched by the query, highlighted.
        matched: Option<(usize, usize)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    /// No candidate under the cursor.
    NoSelection,
    /// The candidate's file is not among the searched diffs.
    NotFound(String),
    /// The file has no text diff (binary, mode change, …).
    NoDiff(String),
    Snippet {
        path: String,
        rows: Vec<PreviewRow>,
    },
}

/// Where a candidate points: file, line number (as `diff_text_lines` writes it:
/// the new line, or the old one for deletions), diff section and line kind.
struct Target<'a> {
    path: &'a str,
    line: Option<usize>,
    section: Option<DiffSection>,
    kind: Option<DiffKind>,
}

/// `path:line<TAB>±content[<TAB>staged]`.
fn parse_text_item(item: &str) -> Option<Target<'_>> {
    let mut fields = item.split('\t');
    let (path, line) = fields.next()?.rsplit_once(':')?;
    let kind = match fields.next().and_then(|c| c.chars().next()) {
        Some('+') => Some(DiffKind::Addition),
        Some('-') => Some(DiffKind::Deletion),
        Some(' ') => Some(DiffKind::Context),
        _ => None,
    };
    let section = if item.ends_with("\tstaged") {
        DiffSection::Staged
    } else {
        DiffSection::Changes
    };
    Some(Target {
        path,
        line: line.parse().ok().filter(|&n| n > 0),
        section: Some(section),
        kind,
    })
}

/// `path:N` in a file query points at line N.
fn query_line(query: &str) -> Option<usize> {
    query.split_whitespace().find_map(|term| {
        let (_, n) = term.rsplit_once(':')?;
        n.parse().ok().filter(|&n| n > 0)
    })
}

/// First query term found literally in `text` (smart case), as a byte range.
fn match_range(text: &str, query: &str) -> Option<(usize, usize)> {
    query.split_whitespace().find_map(|term| {
        if term.chars().any(char::is_uppercase) {
            text.find(term).map(|start| (start, start + term.len()))
        } else {
            let lower = text.to_lowercase();
            // Lowercasing can change byte lengths; only trust it when it does not.
            (lower.len() == text.len())
                .then(|| lower.find(&term.to_lowercase()))
                .flatten()
                .map(|start| (start, start + term.len()))
        }
    })
}

/// Builds the preview for `item` (the candidate under the cursor), showing at most
/// `height` rows around the line it points at.
pub fn build_preview(
    files: &[FileDiff],
    request: FzfRequest,
    item: Option<&str>,
    query: &str,
    height: usize,
) -> Preview {
    let Some(item) = item else {
        return Preview::NoSelection;
    };
    let wanted = match request {
        FzfRequest::Files => Target {
            path: item,
            line: query_line(query),
            section: None,
            kind: None,
        },
        FzfRequest::Text => match parse_text_item(item) {
            Some(target) => target,
            None => return Preview::NotFound(item.to_string()),
        },
    };
    let path = wanted.path;
    let in_section = |f: &&FileDiff| match wanted.section {
        Some(section) => f.section == section,
        None => true,
    };
    let file = files
        .iter()
        .filter(|f| f.display_path() == path)
        .find(in_section)
        .or_else(|| files.iter().find(|f| f.display_path() == path));
    let Some(file) = file else {
        return Preview::NotFound(path.to_string());
    };
    if file.hunks.is_empty() {
        return Preview::NoDiff(path.to_string());
    }

    let text_query = match request {
        FzfRequest::Text => query,
        FzfRequest::Files => "",
    };
    // Borrowed rows first (cheap), owned rows only for the visible window, so moving
    // the cursor over big diffs stays fast.
    let flat: Vec<Result<&str, &DiffLine>> = file
        .hunks
        .iter()
        .flat_map(|h| {
            std::iter::once(Ok(h.header.as_str())).chain(
                h.lines
                    .iter()
                    .filter(|l| l.kind != DiffKind::Virtual)
                    .map(Err),
            )
        })
        .collect();
    let is_target = |line: &DiffLine| {
        wanted.line.is_some()
            && line.new_line_no.or(line.old_line_no) == wanted.line
            && match wanted.kind {
                Some(kind) => kind == line.kind,
                None => true,
            }
    };
    let target = flat
        .iter()
        .position(|row| matches!(row, Err(line) if is_target(line)));
    // Without a line to point at, focus the first change of the first hunk.
    let focus = target.unwrap_or_else(|| {
        flat.iter()
            .position(|row| {
                matches!(row, Err(line) if matches!(line.kind, DiffKind::Addition | DiffKind::Deletion))
            })
            .unwrap_or(0)
    });
    let height = height.max(1);
    let start = focus
        .saturating_sub(height / 3)
        .min(flat.len().saturating_sub(height));
    let rows = flat
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(idx, row)| match row {
            Ok(header) => PreviewRow::Header(header.to_string()),
            Err(line) => {
                let hit = Some(idx) == target;
                PreviewRow::Line {
                    kind: line.kind,
                    old_no: line.old_line_no,
                    new_no: line.new_line_no,
                    text: line.content.trim_end_matches(['\r', '\n']).to_string(),
                    target: hit,
                    matched: if hit {
                        match_range(&line.content, text_query)
                    } else {
                        None
                    },
                }
            }
        })
        .collect();
    Preview::Snippet {
        path: path.to_string(),
        rows,
    }
}

pub fn render_preview(
    frame: &mut Frame,
    area: Rect,
    preview: &Preview,
    language: Language,
    theme: &Theme,
) {
    let title = match preview {
        Preview::Snippet { path, .. } | Preview::NoDiff(path) | Preview::NotFound(path) => {
            format!(" {} ", path)
        }
        Preview::NoSelection => String::from(" preview "),
    };
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme.line_num_fg)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let message = |en: &str, pt: &str| {
        let text = match language {
            Language::En => en,
            Language::Pt => pt,
        };
        Paragraph::new(format!(" {}", text)).style(Style::default().fg(theme.line_num_fg))
    };
    let rows = match preview {
        Preview::NoSelection => {
            frame.render_widget(message("Nothing selected.", "Nada selecionado."), inner);
            return;
        }
        Preview::NotFound(_) => {
            frame.render_widget(
                message(
                    "No diff loaded for this item.",
                    "Sem diff carregado para este item.",
                ),
                inner,
            );
            return;
        }
        Preview::NoDiff(_) => {
            frame.render_widget(
                message(
                    "No text diff for this file (binary or unchanged).",
                    "Sem diff de texto para este arquivo (binário ou sem mudanças).",
                ),
                inner,
            );
            return;
        }
        Preview::Snippet { rows, .. } => rows,
    };

    let lines: Vec<Line> = rows
        .iter()
        .map(|row| match row {
            PreviewRow::Header(header) => Line::from(Span::styled(
                format!(" @@ {} @@", header),
                Style::default().fg(theme.key_fg).bg(theme.selected_bg),
            )),
            PreviewRow::Line {
                kind,
                old_no,
                new_no,
                text,
                target,
                matched,
            } => {
                let (marker, fg, bg) = match kind {
                    DiffKind::Addition => ("+ ", theme.add_fg, theme.add_bg),
                    DiffKind::Deletion => ("- ", theme.del_fg, theme.del_bg),
                    _ => ("  ", theme.fg, theme.bg),
                };
                let num = |n: &Option<usize>| {
                    n.map(|n| format!("{:4}", n))
                        .unwrap_or_else(|| "    ".into())
                };
                let indicator = if *target {
                    Span::styled(
                        "▎",
                        Style::default()
                            .fg(theme.key_fg)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw(" ")
                };
                let mut spans = vec![
                    indicator,
                    Span::styled(
                        format!("{} {} │ ", num(old_no), num(new_no)),
                        Style::default().fg(theme.line_num_fg).bg(theme.line_num_bg),
                    ),
                    Span::styled(
                        marker,
                        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                ];
                let base = Style::default().fg(fg).bg(bg);
                match matched.filter(|(s, e)| {
                    s < e
                        && *e <= text.len()
                        && text.is_char_boundary(*s)
                        && text.is_char_boundary(*e)
                }) {
                    Some((s, e)) => {
                        let hl = match kind {
                            DiffKind::Addition => theme.add_intraline,
                            DiffKind::Deletion => theme.del_intraline,
                            _ => theme.selected_bg,
                        };
                        spans.push(Span::styled(text[..s].to_string(), base));
                        spans.push(Span::styled(
                            text[s..e].to_string(),
                            base.bg(hl)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                        ));
                        spans.push(Span::styled(text[e..].to_string(), base));
                    }
                    None => spans.push(Span::styled(text.clone(), base)),
                }
                // Fill the row so added/removed lines read as bands, like the diff pane.
                let used: usize = spans.iter().map(|s| s.content.width()).sum();
                let pad = (inner.width as usize).saturating_sub(used);
                spans.push(Span::styled(" ".repeat(pad), base));
                Line::from(spans)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::Hunk;
    use std::path::PathBuf;

    fn line(kind: DiffKind, old: Option<usize>, new: Option<usize>, text: &str) -> DiffLine {
        DiffLine {
            kind,
            content: text.into(),
            old_line_no: old,
            new_line_no: new,
            spans: Vec::new(),
        }
    }

    fn file(path: &str, hunks: Vec<Hunk>) -> FileDiff {
        FileDiff {
            old_path: None,
            new_path: PathBuf::from(path),
            status: crate::core::models::FileStatus::Modified,
            stage_status: crate::core::models::StageStatus::Unstaged,
            section: DiffSection::Changes,
            stats: Default::default(),
            hunks,
            aligned_rows: Vec::new(),
            is_binary: false,
        }
    }

    fn sample() -> Vec<FileDiff> {
        let mut lines: Vec<DiffLine> = (1..=20)
            .map(|n| line(DiffKind::Context, Some(n), Some(n), &format!("ctx {}", n)))
            .collect();
        lines[9] = line(DiffKind::Deletion, Some(10), None, "let total = 1;");
        lines.insert(
            10,
            line(
                DiffKind::Addition,
                None,
                Some(10),
                "let Total = sum(items);",
            ),
        );
        let hunk = Hunk {
            old_start: 1,
            old_lines: 20,
            new_start: 1,
            new_lines: 20,
            header: "-1,20 +1,20".into(),
            lines,
        };
        vec![
            file("src/main.rs", vec![hunk]),
            file("logo.png", Vec::new()),
        ]
    }

    fn target_of(preview: &Preview) -> Option<&PreviewRow> {
        match preview {
            Preview::Snippet { rows, .. } => rows
                .iter()
                .find(|r| matches!(r, PreviewRow::Line { target: true, .. })),
            _ => None,
        }
    }

    #[test]
    fn file_with_diff_previews_around_the_first_change() {
        let files = sample();
        let preview = build_preview(&files, FzfRequest::Files, Some("src/main.rs"), "main", 6);
        let Preview::Snippet { path, rows } = &preview else {
            panic!("expected a snippet: {preview:?}");
        };
        assert_eq!(path, "src/main.rs");
        assert_eq!(rows.len(), 6);
        // The first change (the deletion of line 10) is inside the window.
        assert!(rows.iter().any(|r| matches!(
            r,
            PreviewRow::Line {
                kind: DiffKind::Deletion,
                old_no: Some(10),
                ..
            }
        )));
        assert!(target_of(&preview).is_none(), "no line was pointed at");

        // A `path:N` query points the preview at line N.
        let at_18 = build_preview(
            &files,
            FzfRequest::Files,
            Some("src/main.rs"),
            "main.rs:18",
            5,
        );
        assert!(matches!(
            target_of(&at_18),
            Some(PreviewRow::Line {
                new_no: Some(18),
                ..
            })
        ));
    }

    #[test]
    fn text_match_previews_context_and_highlights_the_query() {
        let files = sample();
        let item = "src/main.rs:10\t+ let Total = sum(items);";
        let preview = build_preview(&files, FzfRequest::Text, Some(item), "sum", 7);
        let Some(PreviewRow::Line {
            kind,
            text,
            matched,
            ..
        }) = target_of(&preview)
        else {
            panic!("target line missing: {preview:?}");
        };
        assert_eq!(*kind, DiffKind::Addition);
        let (s, e) = matched.expect("query highlighted");
        assert_eq!(&text[s..e], "sum");
        // Context before and after the matched line is shown.
        let Preview::Snippet { rows, .. } = &preview else {
            unreachable!()
        };
        let pos = rows
            .iter()
            .position(|r| matches!(r, PreviewRow::Line { target: true, .. }))
            .unwrap();
        assert!(
            pos >= 2 && pos + 2 < rows.len(),
            "target at {pos} of {}",
            rows.len()
        );
        // Smart case: an uppercase term must match exactly.
        let exact = build_preview(&files, FzfRequest::Text, Some(item), "Total", 7);
        assert!(matches!(
            target_of(&exact),
            Some(PreviewRow::Line {
                matched: Some((4, 9)),
                ..
            })
        ));
    }

    #[test]
    fn deleted_line_matches_by_old_number_and_staged_section_is_respected() {
        let mut files = sample();
        let mut staged = files[0].clone();
        staged.section = DiffSection::Staged;
        staged.hunks[0].header = "staged hunk".into();
        files.push(staged);
        let item = "src/main.rs:10\t- let total = 1;\tstaged";
        let preview = build_preview(&files, FzfRequest::Text, Some(item), "total", 30);
        let Preview::Snippet { rows, .. } = &preview else {
            panic!("expected a snippet: {preview:?}");
        };
        assert_eq!(rows[0], PreviewRow::Header("staged hunk".into()));
        // Line 10 exists as a deletion (old 10) and an addition (new 10): the `-`
        // prefix picks the deletion.
        assert!(matches!(
            target_of(&preview),
            Some(PreviewRow::Line {
                kind: DiffKind::Deletion,
                old_no: Some(10),
                ..
            })
        ));
    }

    #[test]
    fn no_diff_missing_file_and_empty_selection_have_clear_states() {
        let files = sample();
        assert_eq!(
            build_preview(&files, FzfRequest::Files, None, "", 10),
            Preview::NoSelection
        );
        assert_eq!(
            build_preview(&files, FzfRequest::Files, Some("logo.png"), "", 10),
            Preview::NoDiff("logo.png".into())
        );
        assert_eq!(
            build_preview(&files, FzfRequest::Files, Some("gone.rs"), "", 10),
            Preview::NotFound("gone.rs".into())
        );
    }
}
