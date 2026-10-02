use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::RepoStats;
use crate::ui::theme::Theme;

pub fn render_header(
    frame: &mut Frame,
    area: Rect,
    repo_stats: &RepoStats,
    is_unified: bool,
    watch_mode: bool,
    theme: &Theme,
) {
    let mode_str = if is_unified { " Unified " } else { " Side-by-Side " };

    let mut spans = vec![
        // App brand pill
        Span::styled(
            " ⚡ diffv ",
            Style::default()
                .fg(Color::Rgb(15, 20, 25))
                .bg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        // Repo and branch
        Span::styled(
            format!(" ⎇ {} ", repo_stats.repo_name),
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("on ", Style::default().fg(theme.line_num_fg)),
        Span::styled(
            format!("{} ", repo_stats.branch),
            Style::default().fg(theme.status_u).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme.border)),
        // Files count
        Span::styled(
            format!("{} files ", repo_stats.file_count),
            Style::default().fg(theme.fg),
        ),
        // Additions pill
        Span::styled(
            format!(" +{} ", repo_stats.total_additions),
            Style::default()
                .fg(theme.add_fg)
                .bg(theme.add_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        // Deletions pill
        Span::styled(
            format!(" -{} ", repo_stats.total_deletions),
            Style::default()
                .fg(theme.del_fg)
                .bg(theme.del_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("│ ", Style::default().fg(theme.border)),
        // Mode badge
        Span::styled("Mode: ", Style::default().fg(theme.line_num_fg)),
        Span::styled(
            mode_str,
            Style::default()
                .fg(theme.header_fg)
                .bg(theme.selected_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" [m]  ", Style::default().fg(theme.line_num_fg)),
        // Watch indicator pill
    ];

    if watch_mode {
        spans.push(Span::styled(
            " ● LIVE ",
            Style::default()
                .fg(Color::Rgb(20, 200, 100))
                .bg(Color::Rgb(15, 45, 25))
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            " ○ PAUSED ",
            Style::default()
                .fg(theme.line_num_fg)
                .add_modifier(Modifier::DIM),
        ));
    }
    spans.push(Span::styled(" [w]  ", Style::default().fg(theme.line_num_fg)));

    // Right-aligned / trailing help hint
    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
    spans.push(Span::styled("? ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)));
    spans.push(Span::styled("Help  ", Style::default().fg(theme.line_num_fg)));
    spans.push(Span::styled("Tab ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)));
    spans.push(Span::styled("Switch", Style::default().fg(theme.line_num_fg)));

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.header_bg));

    let paragraph = Paragraph::new(Line::from(spans)).block(block);
    frame.render_widget(paragraph, area);
}
