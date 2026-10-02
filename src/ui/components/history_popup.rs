use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::core::models::CommitEntry;
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
    let popup_area = centered_rect(80, 75, area);
    frame.render_widget(Clear, popup_area);

    let title = format!(" File History: {} ({} commits) ", file_path, commits.len());
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if commits.is_empty() {
        let msg = Paragraph::new(" No git history found for this file.")
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

        let prefix = if is_selected { "▶ " } else { "  " };

        let line = Line::from(vec![
            Span::styled(prefix, base_style),
            Span::styled(
                format!("{:<8} ", commit.hash),
                Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<15} ", commit.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ),
            Span::styled(
                format!("{:<16} ", commit.author),
                Style::default().fg(theme.status_u).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ),
            Span::styled(
                &commit.message,
                base_style,
            ),
        ]);

        lines.push(line);
    }

    frame.render_widget(Paragraph::new(lines), list_area);

    // Footer shortcuts
    let footer_line = Line::from(vec![
        Span::styled(" [j/k]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(" Navigate  ", Style::default().fg(theme.line_num_fg)),
        Span::styled("[Enter]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(" View Commit Diff  ", Style::default().fg(theme.line_num_fg)),
        Span::styled("[Esc/H]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(" Return to Live Diff", Style::default().fg(theme.line_num_fg)),
    ]);
    frame.render_widget(Paragraph::new(footer_line), chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
