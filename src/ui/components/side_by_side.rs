use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::core::models::{DiffKind, DiffLine, FileDiff, HighlightSpan};
use crate::core::syntax::SyntaxHighlighter;
use crate::ui::components::style::diff_pane_block;
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
    scroll_x: [usize; 2],
    wrap: bool,
    wrap_skip: usize,
    row_map: &mut Vec<usize>,
    selected_row: usize,
    visual_range: Option<(usize, usize)>,
    active_column: ColumnSide,
    is_focused: bool,
    syntax_enabled: bool,
    full_context: bool,
    theme: &Theme,
) {
    row_map.clear();
    let help: &[(&str, &str)] = if visual_range.is_some() {
        &[("s", "stage"), ("u", "unstage"), ("d", "discard"), ("esc", "exit")]
    } else {
        &[("x", if full_context { "collapse" } else { "full file" }), ("h/l", "scroll"), ("0/$", "start/end"), ("tab", "old/new"), ("r", "wrap")]
    };
    let block = diff_pane_block(
        file_diff,
        "side-by-side",
        visual_range.map(|(start, end)| end.saturating_sub(start) + 1),
        is_focused,
        wrap,
        full_context,
        help,
        format!("x:{} / {}", scroll_x[0], scroll_x[1]),
        area.width,
        theme,
    );

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if file_diff.is_none() || file_diff.unwrap().aligned_rows.is_empty() {
        let msg = if let Some(diff) = file_diff {
            if diff.is_binary {
                "  Binary file difference not shown in text viewer."
            } else {
                "  File has no differences."
            }
        } else {
            "  No file selected."
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
    let left_badge_text = if left_area.width < 28 {
        " ◄ OLD "
    } else {
        " ◄ ORIGINAL (HEAD) "
    };
    let (left_badge, left_style) = if active_column == ColumnSide::Left {
        (left_badge_text, Style::default().fg(theme.text_on(theme.header_fg)).bg(theme.header_fg).add_modifier(Modifier::BOLD))
    } else {
        (left_badge_text, Style::default().fg(theme.selected_fg).bg(theme.selected_bg))
    };
    let left_header = Line::from(vec![
        Span::styled(left_badge, left_style),
        Span::raw(" "),
        Span::styled(old_label, Style::default().fg(theme.line_num_fg)),
    ]);

    // Right Column Header
    let right_badge_text = if right_area.width < 28 {
        " ► NEW "
    } else {
        " ► MODIFIED (WORKING TREE) "
    };
    let (right_badge, right_style) = if active_column == ColumnSide::Right {
        (right_badge_text, Style::default().fg(theme.text_on(theme.key_fg)).bg(theme.key_fg).add_modifier(Modifier::BOLD))
    } else {
        (right_badge_text, Style::default().fg(theme.selected_fg).bg(theme.selected_bg))
    };
    let right_header = Line::from(vec![
        Span::styled(right_badge, right_style),
        Span::raw(" "),
        Span::styled(file.display_path(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]);

    let mut left_lines = vec![left_header];
    let mut right_lines = vec![right_header];
    row_map.push(usize::MAX);

    for idx in start_idx..end_idx {
        let row = &rows[idx];
        let is_cursor = idx == selected_row;
        let is_in_visual = visual_range.map(|(s, e)| idx >= s && idx <= e).unwrap_or(false);

        let left = render_side_line(
            row.left.as_ref(),
            is_cursor,
            is_in_visual,
            true,
            syntax_enabled,
            &file.new_path,
            theme,
            left_area.width as usize,
        );

        let right = render_side_line(
            row.right.as_ref(),
            is_cursor,
            is_in_visual,
            false,
            syntax_enabled,
            &file.new_path,
            theme,
            right_area.width as usize,
        );
        let mut left = if wrap { crate::ui::components::horizontal::wrap_line(left, 2, left_area.width as usize) }
            else { vec![crate::ui::components::horizontal::scroll_line(left, 2, scroll_x[0])] };
        let mut right = if wrap { crate::ui::components::horizontal::wrap_line(right, 2, right_area.width as usize) }
            else { vec![crate::ui::components::horizontal::scroll_line(right, 2, scroll_x[1])] };
        let height = left.len().max(right.len());
        left.resize(height, Line::default());
        right.resize(height, Line::default());
        let skip = if wrap && idx == start_idx { wrap_skip.min(height.saturating_sub(1)) } else { 0 };
        row_map.extend(std::iter::repeat(idx).take(height - skip));
        left_lines.extend(left.into_iter().skip(skip));
        right_lines.extend(right.into_iter().skip(skip));
        if left_lines.len() >= max_lines { break; }
    }

    frame.render_widget(Paragraph::new(left_lines), left_area);
    frame.render_widget(Paragraph::new(right_lines), right_area);
}

fn render_side_line<'a>(
    line_opt: Option<&'a DiffLine>,
    is_cursor: bool,
    is_in_visual: bool,
    is_left: bool,
    syntax_enabled: bool,
    path: &std::path::Path,
    theme: &Theme,
    _width: usize,
) -> Line<'a> {
    match line_opt {
        None => {
            let bg = if is_in_visual { theme.selected_bg } else { theme.virtual_bg };
            Line::from(vec![
                Span::styled("   · │", Style::default().fg(theme.virtual_fg).bg(bg)),
                Span::styled(" ·", Style::default().fg(theme.virtual_fg).bg(bg)),
            ])
        }
        Some(diff_line) => match diff_line.kind {
            DiffKind::Virtual => {
                let bg = if is_in_visual { theme.selected_bg } else { theme.virtual_bg };
                Line::from(vec![
                    Span::styled("   · │", Style::default().fg(theme.virtual_fg).bg(bg)),
                    Span::styled(" ·", Style::default().fg(theme.virtual_fg).bg(bg)),
                ])
            }
            DiffKind::Context => {
                let line_no = if is_left {
                    diff_line.old_line_no
                } else {
                    diff_line.new_line_no
                };

                let mut spans = Vec::new();
                let indicator = if is_cursor && is_in_visual {
                    Span::styled("█", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
                } else if is_cursor {
                    Span::styled("▎", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
                } else if is_in_visual {
                    Span::styled("▌", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled(" ", Style::default().fg(theme.line_num_fg))
                };
                spans.push(indicator);

                let num_style = if is_cursor && is_in_visual {
                    Style::default().fg(theme.selected_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD)
                } else if is_in_visual {
                    Style::default().fg(theme.selected_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD)
                } else if is_cursor {
                    Style::default().fg(theme.key_fg).bg(theme.line_num_bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.line_num_fg).bg(theme.line_num_bg)
                };
                spans.push(Span::styled(format_gutter_no(line_no, " │ "), num_style));

                let line_bg = if is_in_visual {
                    theme.selected_bg
                } else {
                    theme.bg
                };

                if syntax_enabled {
                    let tokens = SyntaxHighlighter::highlight_line(path, &diff_line.content);
                    if !tokens.is_empty() {
                        for token in tokens {
                            let text = &diff_line.content[token.start..token.end.min(diff_line.content.len())];
                            let fg = Color::Rgb(token.fg_color.0, token.fg_color.1, token.fg_color.2);
                            spans.push(Span::styled(text.to_string(), Style::default().fg(fg).bg(line_bg)));
                        }
                    } else {
                        spans.push(Span::styled(&diff_line.content, Style::default().fg(theme.fg).bg(line_bg)));
                    }
                } else {
                    spans.push(Span::styled(&diff_line.content, Style::default().fg(theme.fg).bg(line_bg)));
                }

                Line::from(spans)
            }
            DiffKind::Deletion => {
                let (bg_color, num_bg) = if is_in_visual {
                    (theme.selected_bg, theme.selected_bg)
                } else {
                    (theme.del_bg, theme.del_bg)
                };
                let fg_color = theme.del_fg;
                let intraline_bg = theme.del_intraline;

                let mut spans = Vec::new();
                let indicator = if is_cursor && is_in_visual {
                    Span::styled("█", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
                } else if is_cursor {
                    Span::styled("▎", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
                } else if is_in_visual {
                    Span::styled("▌", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled(" ", Style::default().fg(fg_color).bg(bg_color))
                };
                spans.push(indicator);

                let num_style = if is_in_visual {
                    Style::default().fg(theme.selected_fg).bg(num_bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg_color).bg(bg_color).add_modifier(Modifier::BOLD)
                };

                spans.push(Span::styled(
                    format_gutter_no(diff_line.old_line_no, " -│ "),
                    num_style,
                ));

                render_highlighted_spans(&diff_line.content, &diff_line.spans, fg_color, bg_color, intraline_bg, &mut spans);
                Line::from(spans)
            }
            DiffKind::Addition => {
                let (bg_color, num_bg) = if is_in_visual {
                    (theme.selected_bg, theme.selected_bg)
                } else {
                    (theme.add_bg, theme.add_bg)
                };
                let fg_color = theme.add_fg;
                let intraline_bg = theme.add_intraline;

                let mut spans = Vec::new();
                let indicator = if is_cursor && is_in_visual {
                    Span::styled("█", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
                } else if is_cursor {
                    Span::styled("▎", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
                } else if is_in_visual {
                    Span::styled("▌", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled(" ", Style::default().fg(fg_color).bg(bg_color))
                };
                spans.push(indicator);

                let num_style = if is_in_visual {
                    Style::default().fg(theme.selected_fg).bg(num_bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg_color).bg(bg_color).add_modifier(Modifier::BOLD)
                };

                spans.push(Span::styled(
                    format_gutter_no(diff_line.new_line_no, " +│ "),
                    num_style,
                ));

                render_highlighted_spans(&diff_line.content, &diff_line.spans, fg_color, bg_color, intraline_bg, &mut spans);
                Line::from(spans)
            }
        },
    }
}

#[inline(always)]
fn format_gutter_no(num: Option<usize>, suffix: &str) -> String {
    match num {
        Some(n) => format!("{:4}{}", n, suffix),
        None => format!("    {}", suffix),
    }
}

fn render_highlighted_spans<'a>(
    content: &'a str,
    spans: &[HighlightSpan],
    base_fg: Color,
    base_bg: Color,
    highlight_bg: Color,
    out: &mut Vec<Span<'a>>,
) {
    if spans.is_empty() {
        out.push(Span::styled(content, Style::default().fg(base_fg).bg(base_bg)));
        return;
    }

    let mut current_idx = 0;
    for span in spans {
        let span_start = span.start.min(content.len());
        let span_end = span.end.min(content.len());

        if span_start > current_idx {
            out.push(Span::styled(
                &content[current_idx..span_start],
                Style::default().fg(base_fg).bg(base_bg),
            ));
        }

        if span_end > span_start {
            out.push(Span::styled(
                &content[span_start..span_end],
                Style::default()
                    .fg(base_fg)
                    .bg(highlight_bg)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        current_idx = span_end;
    }

    if current_idx < content.len() {
        out.push(Span::styled(
            &content[current_idx..],
            Style::default().fg(base_fg).bg(base_bg),
        ));
    }
}
