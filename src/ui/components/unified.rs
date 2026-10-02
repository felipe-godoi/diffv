use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::{DiffKind, FileDiff};
use crate::core::syntax::SyntaxHighlighter;
use crate::ui::theme::Theme;

pub fn render_unified(
    frame: &mut Frame,
    area: Rect,
    file_diff: Option<&FileDiff>,
    scroll_y: usize,
    selected_row: usize,
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

    if file_diff.is_none() || file_diff.unwrap().hunks.is_empty() {
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

    // Flatten hunks into a unified list of lines
    let mut flat_lines = Vec::new();
    for hunk in &file.hunks {
        flat_lines.push((true, None, None, DiffKind::Context, hunk.header.clone()));
        for line in &hunk.lines {
            flat_lines.push((false, line.old_line_no, line.new_line_no, line.kind, line.content.clone()));
        }
    }

    let max_lines = inner_area.height as usize;
    if max_lines == 0 {
        return;
    }

    let start_idx = scroll_y;
    let end_idx = (scroll_y + max_lines).min(flat_lines.len());

    let mut rendered_lines = Vec::new();

    for idx in start_idx..end_idx {
        let (is_header, old_no, new_no, kind, content) = &flat_lines[idx];
        let is_cursor = idx == selected_row;

        if *is_header {
            rendered_lines.push(Line::from(vec![
                Span::styled(
                    format!("@@ {} @@", content),
                    Style::default().fg(theme.key_fg).bg(theme.header_bg).add_modifier(Modifier::BOLD),
                )
            ]));
            continue;
        }

        let old_str = match old_no {
            Some(n) => format!("{:4}", n),
            None => "    ".to_string(),
        };
        let new_str = match new_no {
            Some(n) => format!("{:4}", n),
            None => "    ".to_string(),
        };

        let num_style = if is_cursor {
            Style::default().fg(theme.key_fg).bg(theme.line_num_bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.line_num_fg).bg(theme.line_num_bg)
        };

        let num_span = Span::styled(format!("{} {} ", old_str, new_str), num_style);

        match kind {
            DiffKind::Addition => {
                rendered_lines.push(Line::from(vec![
                    num_span,
                    Span::styled("+ ", Style::default().fg(theme.add_fg).bg(theme.add_bg)),
                    Span::styled(content, Style::default().fg(theme.add_fg).bg(theme.add_bg)),
                ]));
            }
            DiffKind::Deletion => {
                rendered_lines.push(Line::from(vec![
                    num_span,
                    Span::styled("- ", Style::default().fg(theme.del_fg).bg(theme.del_bg)),
                    Span::styled(content, Style::default().fg(theme.del_fg).bg(theme.del_bg)),
                ]));
            }
            DiffKind::Context | DiffKind::Virtual => {
                let mut spans = vec![
                    num_span,
                    Span::styled("  ", Style::default().fg(theme.fg).bg(theme.bg)),
                ];

                if syntax_enabled {
                    let tokens = SyntaxHighlighter::highlight_line(&file.new_path, content);
                    if !tokens.is_empty() {
                        for token in tokens {
                            let text = &content[token.start..token.end.min(content.len())];
                            let fg = Color::Rgb(token.fg_color.0, token.fg_color.1, token.fg_color.2);
                            spans.push(Span::styled(text.to_string(), Style::default().fg(fg).bg(theme.bg)));
                        }
                    } else {
                        spans.push(Span::styled(content, Style::default().fg(theme.fg).bg(theme.bg)));
                    }
                } else {
                    spans.push(Span::styled(content, Style::default().fg(theme.fg).bg(theme.bg)));
                }

                rendered_lines.push(Line::from(spans));
            }
        }
    }

    frame.render_widget(Paragraph::new(rendered_lines), inner_area);
}
