use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
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
    let mode_str = if is_unified { "UNIFIED [m]" } else { "SIDE-BY-SIDE [m]" };
    let watch_str = if watch_mode { "⚡ LIVE [w]" } else { "WATCH: OFF [w]" };

    let header_line = Line::from(vec![
        Span::styled(
            format!(" [REPO: {}] ", repo_stats.repo_name),
            Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("branch: {}  ", repo_stats.branch),
            Style::default().fg(theme.header_fg),
        ),
        Span::styled(
            format!("Files: {} ", repo_stats.file_count),
            Style::default().fg(theme.fg),
        ),
        Span::styled(
            format!("(+{}, -{})  ", repo_stats.total_additions, repo_stats.total_deletions),
            Style::default().fg(theme.add_fg),
        ),
        Span::styled(
            format!("Mode: {}  ", mode_str),
            Style::default().fg(theme.status_u),
        ),
        Span::styled(
            format!("{}  ", watch_str),
            Style::default().fg(if watch_mode { theme.status_a } else { theme.line_num_fg }),
        ),
        Span::styled(
            "[Tab] Focus  [?] Help ",
            Style::default().fg(theme.line_num_fg),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.header_bg));

    let paragraph = Paragraph::new(header_line).block(block);
    frame.render_widget(paragraph, area);
}
