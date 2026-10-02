use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    notification: Option<&str>,
    mode_label: &str,
    theme: &Theme,
) {
    let (badge_fg, badge_bg) = match mode_label {
        "VISUAL" => (Color::Rgb(15, 20, 25), Color::Rgb(215, 130, 255)),
        "TREE" => (Color::Rgb(15, 20, 25), theme.status_u),
        _ => (Color::Rgb(15, 20, 25), theme.header_fg),
    };

    let mode_pill = Span::styled(
        format!(" {} ", mode_label),
        Style::default()
            .fg(badge_fg)
            .bg(badge_bg)
            .add_modifier(Modifier::BOLD),
    );

    let content_line = if let Some(msg) = notification {
        Line::from(vec![
            mode_pill,
            Span::raw("  "),
            Span::styled(
                format!("🔔 {} ", msg),
                Style::default()
                    .fg(theme.key_fg)
                    .bg(theme.status_bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        let key_style = Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD);
        let text_style = Style::default().fg(theme.status_fg);
        let sep_style = Style::default().fg(theme.border);

        Line::from(vec![
            mode_pill,
            Span::raw("  "),
            Span::styled("s ", key_style),
            Span::styled("Stage  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("u ", key_style),
            Span::styled("Unstage  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("d ", key_style),
            Span::styled("Discard  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("v ", key_style),
            Span::styled("Visual  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("e ", key_style),
            Span::styled("Edit  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("c ", key_style),
            Span::styled("Copy  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("H ", key_style),
            Span::styled("History  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("? ", key_style),
            Span::styled("Help  ", text_style),
            Span::styled("│ ", sep_style),
            Span::styled("q ", key_style),
            Span::styled("Quit", text_style),
        ])
    };

    let block = Block::default().style(Style::default().bg(theme.status_bg));
    let paragraph = Paragraph::new(content_line).block(block);
    frame.render_widget(paragraph, area);
}
