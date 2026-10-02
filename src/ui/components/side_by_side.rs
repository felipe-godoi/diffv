use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::{DiffKind, DiffLine, FileDiff, HighlightSpan};
use crate::core::syntax::SyntaxHighlighter;
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnSide {
    Left,
    Right,
}

pub fn render_side_by_side(
    frame: &mut Frame,
    area: Rect,
    file_diff: Option<&FileDiff>,
    scroll_y: usize,
    selected_row: usize,
    visual_range: Option<(usize, usize)>,
    active_column: ColumnSide,
    is_focused: bool,
    syntax_enabled: bool,
    theme: &Theme,
) {
    let border_style = if is_focused {
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if file_diff.is_none() || file_diff.unwrap().aligned_rows.is_empty() {
        let msg = if let Some(diff) = file_diff {
            if diff.is_binary {
                " Binary file difference not shown in text viewer."
            } else {
                " File has no differences."
            }
        } else {
            " No file selected."
        };
        let p = Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(p, inner_area);
        return;
    }

    let file = file_diff.unwrap();
    let rows = &file.aligned_rows;

    // Split inner area into Left column, vertical separator (1 col), Right column
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(inner_area);

    let left_area = columns[0];
    let sep_area = columns[1];
    let right_area = columns[2];

    // Render vertical separator
    let sep_text: Vec<Line> = (0..inner_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(theme.border))))
        .collect();
    frame.render_widget(Paragraph::new(sep_text), sep_area);

    let max_lines = inner_area.height as usize;
    if max_lines <= 1 {
        return;
    }

    let display_rows = max_lines - 1; // 1 row reserved for header
    let start_idx = scroll_y;
    let end_idx = (scroll_y + display_rows).min(rows.len());

    // Left Column Header
    let old_label = file
        .old_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.display_path());
    let left_prefix = if active_column == ColumnSide::Left { " OLD [◄]: " } else { " OLD: " };
    let left_header = Line::from(vec![
        Span::styled(left_prefix, Style::default().fg(if active_column == ColumnSide::Left { theme.key_fg } else { theme.line_num_fg }).add_modifier(Modifier::BOLD)),
        Span::styled(old_label, Style::default().fg(theme.fg)),
    ]);

    // Right Column Header
    let right_prefix = if active_column == ColumnSide::Right { " NEW [►]: " } else { " NEW: " };
    let right_header = Line::from(vec![
        Span::styled(right_prefix, Style::default().fg(if active_column == ColumnSide::Right { theme.key_fg } else { theme.line_num_fg }).add_modifier(Modifier::BOLD)),
        Span::styled(file.display_path(), Style::default().fg(theme.fg)),
    ]);

    let mut left_lines = vec![left_header];
    let mut right_lines = vec![right_header];

    for idx in start_idx..end_idx {
        let row = &rows[idx];
        let is_cursor = idx == selected_row;
        let is_in_visual = visual_range.map(|(s, e)| idx >= s && idx <= e).unwrap_or(false);

        left_lines.push(render_side_line(
            row.left.as_ref(),
            is_cursor || is_in_visual,
            true,
            syntax_enabled,
            &file.new_path,
            theme,
            left_area.width as usize,
        ));

        right_lines.push(render_side_line(
            row.right.as_ref(),
            is_cursor || is_in_visual,
            false,
            syntax_enabled,
            &file.new_path,
            theme,
            right_area.width as usize,
        ));
    }

    frame.render_widget(Paragraph::new(left_lines), left_area);
    frame.render_widget(Paragraph::new(right_lines), right_area);
}

fn render_side_line<'a>(
    line_opt: Option<&'a DiffLine>,
    is_cursor: bool,
    is_left: bool,
    syntax_enabled: bool,
    path: &std::path::Path,
    theme: &Theme,
    width: usize,
) -> Line<'a> {
    match line_opt {
        None => Line::from(Span::styled("~", Style::default().fg(theme.virtual_fg).bg(theme.virtual_bg))),
        Some(diff_line) => match diff_line.kind {
            DiffKind::Virtual => {
                let filler = "~".repeat(width.max(1));
                Line::from(Span::styled(
                    filler,
                    Style::default().fg(theme.virtual_fg).bg(theme.virtual_bg),
                ))
            }
            DiffKind::Context => {
                let line_no_str = if is_left {
                    format_line_no(diff_line.old_line_no)
                } else {
                    format_line_no(diff_line.new_line_no)
                };

                let mut spans = Vec::new();
                let num_style = if is_cursor {
                    Style::default().fg(theme.key_fg).bg(theme.line_num_bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.line_num_fg).bg(theme.line_num_bg)
                };
                spans.push(Span::styled(format!("{} │ ", line_no_str), num_style));

                if syntax_enabled {
                    let tokens = SyntaxHighlighter::highlight_line(path, &diff_line.content);
                    if !tokens.is_empty() {
                        for token in tokens {
                            let text = &diff_line.content[token.start..token.end.min(diff_line.content.len())];
                            let fg = Color::Rgb(token.fg_color.0, token.fg_color.1, token.fg_color.2);
                            spans.push(Span::styled(text.to_string(), Style::default().fg(fg).bg(theme.bg)));
                        }
                    } else {
                        spans.push(Span::styled(&diff_line.content, Style::default().fg(theme.fg).bg(theme.bg)));
                    }
                } else {
                    spans.push(Span::styled(&diff_line.content, Style::default().fg(theme.fg).bg(theme.bg)));
                }

                Line::from(spans)
            }
            DiffKind::Deletion => {
                let line_no_str = format_line_no(diff_line.old_line_no);
                let bg_color = theme.del_bg;
                let fg_color = theme.del_fg;
                let intraline_bg = theme.del_intraline;

                let mut spans = Vec::new();
                spans.push(Span::styled(
                    format!("{} -│ ", line_no_str),
                    Style::default().fg(fg_color).bg(bg_color),
                ));

                render_highlighted_spans(&diff_line.content, &diff_line.spans, fg_color, bg_color, intraline_bg, &mut spans);
                Line::from(spans)
            }
            DiffKind::Addition => {
                let line_no_str = format_line_no(diff_line.new_line_no);
                let bg_color = theme.add_bg;
                let fg_color = theme.add_fg;
                let intraline_bg = theme.add_intraline;

                let mut spans = Vec::new();
                spans.push(Span::styled(
                    format!("{} +│ ", line_no_str),
                    Style::default().fg(fg_color).bg(bg_color),
                ));

                render_highlighted_spans(&diff_line.content, &diff_line.spans, fg_color, bg_color, intraline_bg, &mut spans);
                Line::from(spans)
            }
        },
    }
}

fn render_highlighted_spans<'a>(
    content: &'a str,
    spans: &[HighlightSpan],
    fg: Color,
    bg: Color,
    intraline_bg: Color,
    out: &mut Vec<Span<'a>>,
) {
    if spans.is_empty() {
        out.push(Span::styled(content, Style::default().fg(fg).bg(bg)));
        return;
    }

    let mut last_idx = 0;
    for span in spans {
        if span.start > last_idx && span.start <= content.len() {
            out.push(Span::styled(
                &content[last_idx..span.start],
                Style::default().fg(fg).bg(bg),
            ));
        }

        let end = span.end.min(content.len());
        if span.start < end {
            out.push(Span::styled(
                &content[span.start..end],
                Style::default().fg(Color::Rgb(255, 255, 255)).bg(intraline_bg).add_modifier(Modifier::BOLD),
            ));
        }
        last_idx = end;
    }

    if last_idx < content.len() {
        out.push(Span::styled(
            &content[last_idx..],
            Style::default().fg(fg).bg(bg),
        ));
    }
}

fn format_line_no(num: Option<usize>) -> String {
    match num {
        Some(n) => format!("{:4}", n),
        None => "    ".to_string(),
    }
}
