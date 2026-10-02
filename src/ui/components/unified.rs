use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};
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
    visual_range: Option<(usize, usize)>,
    is_focused: bool,
    syntax_enabled: bool,
    theme: &Theme,
) {
    let title = if let Some((start, end)) = visual_range {
        let count = end.saturating_sub(start) + 1;
        format!(" 󰒅 [VISUAL MODE: {} lines selected (s: stage, u: unstage, d: discard, Esc: exit)] ", count)
    } else if let Some(diff) = file_diff {
        format!(" 󰈚 {} (+{}, -{}) [Unified] ", diff.display_path(), diff.stats.additions, diff.stats.deletions)
    } else {
        " Diff View [Unified] ".to_string()
    };

    let border_style = if visual_range.is_some() {
        Style::default().fg(Color::Rgb(215, 130, 255)).add_modifier(Modifier::BOLD)
    } else if is_focused {
        Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg));

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
        let is_in_visual = visual_range.map(|(s, e)| idx >= s && idx <= e).unwrap_or(false);

        if *is_header {
            rendered_lines.push(Line::from(vec![
                Span::styled(
                    format!(" 󰦨 @@ {} @@ ", content),
                    Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD),
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

        let indicator = if is_cursor && is_in_visual {
            Span::styled("█", Style::default().fg(Color::Rgb(203, 166, 247)).add_modifier(Modifier::BOLD))
        } else if is_cursor {
            Span::styled("▎", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        } else if is_in_visual {
            Span::styled("▌", Style::default().fg(Color::Rgb(180, 140, 220)).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ", Style::default().fg(theme.line_num_fg))
        };

        let num_style = if is_cursor && is_in_visual {
            Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(75, 58, 100)).add_modifier(Modifier::BOLD)
        } else if is_in_visual {
            Style::default().fg(Color::Rgb(220, 205, 250)).bg(Color::Rgb(58, 48, 80)).add_modifier(Modifier::BOLD)
        } else if is_cursor {
            Style::default().fg(theme.key_fg).bg(theme.line_num_bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.line_num_fg).bg(theme.line_num_bg)
        };

        let num_span = Span::styled(format!("{} {} │ ", old_str, new_str), num_style);

        match kind {
            DiffKind::Context | DiffKind::Virtual => {
                let line_bg = if is_in_visual {
                    Color::Rgb(48, 42, 68)
                } else {
                    theme.bg
                };

                let mut spans = vec![indicator, num_span];
                spans.push(Span::styled("  ", Style::default().fg(theme.fg).bg(line_bg)));

                if syntax_enabled {
                    let tokens = SyntaxHighlighter::highlight_line(&file.new_path, content);
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

                rendered_lines.push(Line::from(spans));
            }
            DiffKind::Deletion => {
                let (bg_color, num_bg) = if is_in_visual {
                    (Color::Rgb(78, 32, 50), Color::Rgb(98, 38, 62))
                } else {
                    (theme.del_bg, theme.del_bg)
                };
                let num_span_del = Span::styled(
                    format!("{} {} │ ", old_str, new_str),
                    if is_in_visual {
                        Style::default().fg(Color::Rgb(255, 230, 240)).bg(num_bg).add_modifier(Modifier::BOLD)
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
                rendered_lines.push(Line::from(spans));
            }
            DiffKind::Addition => {
                let (bg_color, num_bg) = if is_in_visual {
                    (Color::Rgb(32, 72, 52), Color::Rgb(40, 92, 65))
                } else {
                    (theme.add_bg, theme.add_bg)
                };
                let num_span_add = Span::styled(
                    format!("{} {} │ ", old_str, new_str),
                    if is_in_visual {
                        Style::default().fg(Color::Rgb(230, 255, 240)).bg(num_bg).add_modifier(Modifier::BOLD)
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
                rendered_lines.push(Line::from(spans));
            }
        }
    }

    let paragraph = Paragraph::new(rendered_lines);
    frame.render_widget(paragraph, inner_area);
}
