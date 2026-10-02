use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::core::models::CommitEntry;
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

pub fn render_history_popup(
    frame: &mut Frame,
    area: Rect,
    file_path: &str,
    commits: &[CommitEntry],
    selected_idx: usize,
    scroll_offset: usize,
    theme: &Theme,
) {
    let popup_area = centered_rect(85, 80, area);
    let title = format!("File History · {} · {} commits", file_path, commits.len());
    let inner = render_card(frame, popup_area, "󰜉", &title, theme.header_fg, theme);

    if commits.is_empty() {
        let msg = Paragraph::new("  No git commit history found for this file.")
            .style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(msg, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Commits list
            Constraint::Length(1), // Footer shortcuts
        ])
        .split(inner);

    let list_area = chunks[0];
    let max_rows = list_area.height as usize;
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(commits.len());

    let mut lines = Vec::new();

    for idx in start_idx..end_idx {
        let commit = &commits[idx];
        let is_selected = idx == selected_idx;

        let base_style = if is_selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default().bg(theme.bg).fg(theme.fg)
        };

        let cursor_span = if is_selected {
            Span::styled("▎", Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ", base_style)
        };

        let mut spans = vec![
            cursor_span,
            Span::styled(
                format!(" 󰜉 {:<7} ", commit.hash),
                Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
            ),
            Span::styled("● ", Style::default().fg(theme.status_a).bg(if is_selected { theme.selected_bg } else { theme.bg })),
        ];

        let msg_len = if list_area.width < 60 { 24 } else { 38 };
        let msg_truncated = if commit.message.len() > msg_len {
            format!("{}…", &commit.message[..msg_len.saturating_sub(1)])
        } else {
            commit.message.clone()
        };

        spans.push(Span::styled(
            format!("{:<width$} ", msg_truncated, width = msg_len),
            if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style },
        ));

        if list_area.width >= 55 {
            spans.push(Span::styled(
                format!("· {:<12} ", commit.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ));
        }

        if list_area.width >= 75 {
            spans.push(Span::styled(
                format!("· {}", commit.author),
                Style::default().fg(theme.status_u).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ));
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), list_area);

    let footer_line = help_line(&[("enter", "view commit diff"), ("esc / H", "close"), ("i", "details"), ("j/k", "navigate")], chunks[1].width, theme);
    frame.render_widget(Paragraph::new(footer_line), chunks[1]);
}
