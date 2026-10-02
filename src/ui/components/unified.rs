use std::path::Path;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::core::models::{DiffKind, FileDiff};
use crate::core::syntax::SyntaxHighlighter;
use crate::ui::components::style::diff_pane_block;
use crate::ui::theme::Theme;

pub fn render_unified(
    frame: &mut Frame,
    area: Rect,
    file_diff: Option<&FileDiff>,
    scroll_y: usize,
    scroll_x: usize,
    wrap: bool,
    wrap_skip: usize,
    row_map: &mut Vec<usize>,
    selected_row: usize,
    visual_range: Option<(usize, usize)>,
    is_focused: bool,
    syntax_enabled: bool,
    full_context: bool,
    theme: &Theme,
) {
    row_map.clear();
    let help: &[(&str, &str)] = if visual_range.is_some() {
        &[("s", "stage"), ("u", "unstage"), ("d", "discard"), ("esc", "exit")]
    } else {
        &[("x", if full_context { "collapse" } else { "full file" }), ("h/l", "scroll"), ("0/$", "start/end"), ("r", "wrap")]
    };
    let block = diff_pane_block(
        file_diff,
        "unified",
        visual_range.map(|(start, end)| end.saturating_sub(start) + 1),
        is_focused,
        wrap,
        full_context,
        help,
        format!("x:{}", scroll_x),
        area.width,
        theme,
    );

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if file_diff.is_none() || file_diff.unwrap().hunks.is_empty() {
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
    let max_lines = inner_area.height as usize;
    if max_lines == 0 {
        return;
    }

    let start_idx = scroll_y;
    let end_idx = scroll_y + max_lines;

    let mut rendered_lines = Vec::with_capacity(max_lines);
    let mut current_idx = 0usize;

    'outer: for hunk in &file.hunks {
        if current_idx >= start_idx && current_idx < end_idx {
            row_map.push(current_idx);
            rendered_lines.push(Line::from(vec![
                Span::styled(
                    format!(" 󰦨 @@ {} @@ ", hunk.header),
                    Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD),
                )
            ]));
        }
        current_idx += 1;
        if current_idx >= end_idx {
            break 'outer;
        }

        for line in &hunk.lines {
            if current_idx >= start_idx && current_idx < end_idx {
                let is_cursor = current_idx == selected_row;
                let is_in_visual = visual_range.map(|(s, e)| current_idx >= s && current_idx <= e).unwrap_or(false);

                row_map.push(current_idx);
                rendered_lines.push(render_unified_line(
                    line.old_line_no,
                    line.new_line_no,
                    line.kind,
                    &line.content,
                    is_cursor,
                    is_in_visual,
                    syntax_enabled,
                    &file.new_path,
                    theme,
                ));
            }
            current_idx += 1;
            if current_idx >= end_idx {
                break 'outer;
            }
        }
    }

    let logical_rows = std::mem::take(row_map);
    let mut output = Vec::new();
    for (line, idx) in rendered_lines.into_iter().zip(logical_rows) {
        let gutter = if line.spans.len() >= 3 { 3 } else { 0 };
        let lines = if wrap { crate::ui::components::horizontal::wrap_line(line, gutter, inner_area.width as usize) }
            else { vec![crate::ui::components::horizontal::scroll_line(line, 3, scroll_x)] };
        let skip = if wrap && idx == start_idx { wrap_skip.min(lines.len().saturating_sub(1)) } else { 0 };
        row_map.extend(std::iter::repeat_n(idx, lines.len() - skip));
        output.extend(lines.into_iter().skip(skip));
        if output.len() >= max_lines { break; }
    }
    let paragraph = Paragraph::new(output);
    frame.render_widget(paragraph, inner_area);
}

fn render_unified_line<'a>(
    old_no: Option<usize>,
    new_no: Option<usize>,
    kind: DiffKind,
    content: &'a str,
    is_cursor: bool,
    is_in_visual: bool,
    syntax_enabled: bool,
    path: &Path,
    theme: &Theme,
) -> Line<'a> {
    let old_str = match old_no {
        Some(n) => format!("{:4}", n),
        None => "    ".to_string(),
    };
    let new_str = match new_no {
        Some(n) => format!("{:4}", n),
        None => "    ".to_string(),
    };

    let indicator = if is_cursor && is_in_visual {
        Span::styled("█", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
    } else if is_cursor {
        Span::styled("▎", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
    } else if is_in_visual {
        Span::styled("▌", Style::default().fg(theme.selected_fg).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" ", Style::default().fg(theme.line_num_fg))
    };

    let num_style = if is_in_visual {
        Style::default().fg(theme.selected_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD)
    } else if is_cursor {
        Style::default().fg(theme.key_fg).bg(theme.line_num_bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.line_num_fg).bg(theme.line_num_bg)
    };

    let num_span = Span::styled(format!("{} {} │ ", old_str, new_str), num_style);

    match kind {
        DiffKind::Context | DiffKind::Virtual => {
            let line_bg = if is_in_visual {
                theme.selected_bg
            } else {
                theme.bg
            };

            let mut spans = vec![indicator, num_span];
            spans.push(Span::styled("  ", Style::default().fg(theme.fg).bg(line_bg)));

            if syntax_enabled {
                let tokens = SyntaxHighlighter::highlight_line(path, content);
                if !tokens.is_empty() {
                    for token in tokens {
                        let text = &content[token.start..token.end.min(content.len())];
                        let fg = Color::Rgb(token.fg_color.0, token.fg_color.1, token.fg_color.2);
                        spans.push(Span::styled(text.to_string(), Style::default().fg(fg).bg(line_bg)));
                    }
                } else {
                    spans.push(Span::styled(content, Style::default().fg(theme.fg).bg(line_bg)));
                }
            } else {
                spans.push(Span::styled(content, Style::default().fg(theme.fg).bg(line_bg)));
            }

            Line::from(spans)
        }
        DiffKind::Deletion => {
            let (bg_color, num_bg) = if is_in_visual {
                (theme.selected_bg, theme.selected_bg)
            } else {
                (theme.del_bg, theme.del_bg)
            };
            let num_span_del = Span::styled(
                format!("{} {} │ ", old_str, new_str),
                if is_in_visual {
                    Style::default().fg(theme.selected_fg).bg(num_bg).add_modifier(Modifier::BOLD)
                } else {
                    num_style
                },
            );
            let spans = vec![
                indicator,
                num_span_del,
                Span::styled("- ", Style::default().fg(theme.del_fg).bg(bg_color).add_modifier(Modifier::BOLD)),
                Span::styled(content, Style::default().fg(theme.del_fg).bg(bg_color)),
            ];
            Line::from(spans)
        }
        DiffKind::Addition => {
            let (bg_color, num_bg) = if is_in_visual {
                (theme.selected_bg, theme.selected_bg)
            } else {
                (theme.add_bg, theme.add_bg)
            };
            let num_span_add = Span::styled(
                format!("{} {} │ ", old_str, new_str),
                if is_in_visual {
                    Style::default().fg(theme.selected_fg).bg(num_bg).add_modifier(Modifier::BOLD)
                } else {
                    num_style
                },
            );
            let spans = vec![
                indicator,
                num_span_add,
                Span::styled("+ ", Style::default().fg(theme.add_fg).bg(bg_color).add_modifier(Modifier::BOLD)),
                Span::styled(content, Style::default().fg(theme.add_fg).bg(bg_color)),
            ];
            Line::from(spans)
        }
    }
}
