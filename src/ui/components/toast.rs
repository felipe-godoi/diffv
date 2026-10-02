use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

pub fn render_toast(
    frame: &mut Frame,
    area: Rect,
    notification: &str,
    theme: &Theme,
) {
    if notification.is_empty() || area.width < 25 || area.height < 6 {
        return;
    }

    let toast_text = format!(" 󰂚 {} ", notification);
    let width = (toast_text.len() as u16 + 4).min(area.width.saturating_sub(4)).max(22);
    let height = 3;

    // Position at bottom-right, just above status bar
    let x = area.width.saturating_sub(width + 2);
    let y = area.height.saturating_sub(height + 2);

    let toast_area = Rect {
        x,
        y,
        width,
        height,
    };

    frame.render_widget(Clear, toast_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Rgb(26, 27, 38)));

    let inner = block.inner(toast_area);
    frame.render_widget(block, toast_area);

    let p = Paragraph::new(Line::from(vec![
        Span::styled(
            toast_text,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(p, inner);
}
