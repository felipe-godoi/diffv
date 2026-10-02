use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    notification: Option<&str>,
    theme: &Theme,
) {
    let block = Block::default().style(Style::default().bg(theme.status_bg));

    let content_line = if let Some(msg) = notification {
        Line::from(vec![
            Span::styled(" ", Style::default().bg(theme.status_bg)),
            Span::styled(
                msg,
                Style::default()
                    .fg(theme.key_fg)
                    .bg(theme.status_bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(" ", Style::default().bg(theme.status_bg)),
            Span::styled("[j/k]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Scroll ", Style::default().fg(theme.status_fg)),
            Span::styled("[n/p]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Hunk ", Style::default().fg(theme.status_fg)),
            Span::styled("[Tab]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Focus ", Style::default().fg(theme.status_fg)),
            Span::styled("[s]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Stage ", Style::default().fg(theme.status_fg)),
            Span::styled("[u]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Unstage ", Style::default().fg(theme.status_fg)),
            Span::styled("[d]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Discard ", Style::default().fg(theme.status_fg)),
            Span::styled("[e]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Edit ", Style::default().fg(theme.status_fg)),
            Span::styled("[c]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Copy ", Style::default().fg(theme.status_fg)),
            Span::styled("[m]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Mode ", Style::default().fg(theme.status_fg)),
            Span::styled("[w]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Watch ", Style::default().fg(theme.status_fg)),
            Span::styled("[?]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Help ", Style::default().fg(theme.status_fg)),
            Span::styled("[q]", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Quit", Style::default().fg(theme.status_fg)),
        ])
    };

    let paragraph = Paragraph::new(content_line).block(block);
    frame.render_widget(paragraph, area);
}
