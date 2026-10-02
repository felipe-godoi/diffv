use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::core::models::FileDiff;
use crate::ui::theme::Theme;

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let px = if r.width < 90 { 96 } else { percent_x };
    let py = if r.height < 30 { 92 } else { percent_y };
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - py) / 2),
            Constraint::Percentage(py),
            Constraint::Percentage((100 - py) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - px) / 2),
            Constraint::Percentage(px),
            Constraint::Percentage((100 - px) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Title rendered as a solid accent "pill" sitting on the top border.
pub fn pill_title(icon: &str, title: &str, accent: Color, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!(" {} {} ", icon, title),
            Style::default()
                .fg(theme.text_on(accent))
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

pub fn card_block(title: Line<'static>, accent: Color, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.header_bg))
}

/// Modal card: dims the backdrop, casts a drop shadow and draws a rounded,
/// padded block with a pill title. Returns the inner content area.
pub fn render_card(
    frame: &mut Frame,
    area: Rect,
    icon: &str,
    title: &str,
    accent: Color,
    theme: &Theme,
) -> Rect {
    let full = frame.area();
    dim_backdrop(frame.buffer_mut(), full, area);
    render_shadow(frame.buffer_mut(), full, area, theme);
    frame.render_widget(Clear, area);
    let block = card_block(pill_title(icon, title, accent, theme), accent, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

fn dim_backdrop(buf: &mut Buffer, full: Rect, popup: Rect) {
    for y in full.top()..full.bottom() {
        for x in full.left()..full.right() {
            if !popup.contains((x, y).into()) {
                let cell = &mut buf[(x, y)];
                cell.set_style(cell.style().add_modifier(Modifier::DIM));
            }
        }
    }
}

pub fn shadow_color(theme: &Theme) -> Color {
    if theme.is_light() {
        Color::Rgb(188, 192, 202)
    } else {
        Color::Rgb(8, 8, 12)
    }
}

pub fn render_shadow(buf: &mut Buffer, full: Rect, popup: Rect, theme: &Theme) {
    let shadow = Rect {
        x: popup.x.saturating_add(2),
        y: popup.y.saturating_add(1),
        ..popup
    }
    .intersection(full);
    let color = shadow_color(theme);
    for y in shadow.top()..shadow.bottom() {
        for x in shadow.left()..shadow.right() {
            if !popup.contains((x, y).into()) {
                buf[(x, y)].set_bg(color);
            }
        }
    }
}

/// `bubbles/help`-style short help: `key desc • key desc`, truncated with `…`
/// when the line would overflow `width`.
pub fn help_line(items: &[(&str, &str)], width: u16, theme: &Theme) -> Line<'static> {
    let key = Style::default()
        .fg(theme.key_fg)
        .add_modifier(Modifier::BOLD);
    let desc = Style::default().fg(theme.line_num_fg);
    let sep = Style::default().fg(theme.border);
    let budget = width as usize;
    let mut used = 0;
    let mut spans = Vec::new();
    for (i, (k, d)) in items.iter().enumerate() {
        let prefix = if i == 0 { "" } else { " • " };
        let item_width = prefix.width() + k.width() + 1 + d.width();
        if used + item_width > budget {
            if used + 2 <= budget {
                spans.push(Span::styled(" …", sep));
            }
            break;
        }
        if i > 0 {
            spans.push(Span::styled(prefix, sep));
        }
        spans.push(Span::styled(k.to_string(), key));
        spans.push(Span::styled(format!(" {}", d), desc));
        used += item_width;
    }
    Line::from(spans)
}

/// Rounded diff pane with a pill title, colored stats, a right-aligned
/// mode/wrap tag and short help along the bottom border.
pub fn diff_pane_block(
    file_diff: Option<&FileDiff>,
    mode: &str,
    visual_lines: Option<usize>,
    focused: bool,
    wrap: bool,
    full_context: bool,
    help: &[(&str, &str)],
    position: String,
    width: u16,
    theme: &Theme,
) -> Block<'static> {
    let muted = Style::default().fg(theme.line_num_fg);
    let (accent, border) = match (visual_lines, focused) {
        (Some(_), _) => (theme.key_fg, Style::default().fg(theme.key_fg)),
        (None, true) => (theme.header_fg, Style::default().fg(theme.header_fg)),
        (None, false) => (theme.line_num_fg, Style::default().fg(theme.border)),
    };
    let title = match (visual_lines, file_diff) {
        (Some(count), _) => pill_title("󰒅", &format!("VISUAL · {} lines", count), accent, theme),
        (None, Some(diff)) => {
            let mut line = if focused {
                pill_title("󰈚", &diff.display_path(), accent, theme)
            } else {
                Line::from(Span::styled(
                    format!(" 󰈚 {} ", diff.display_path()),
                    muted.add_modifier(Modifier::BOLD),
                ))
            };
            line.spans.push(Span::styled(
                format!("+{}", diff.stats.additions),
                Style::default()
                    .fg(theme.status_a)
                    .add_modifier(Modifier::BOLD),
            ));
            line.spans.push(Span::raw(" "));
            line.spans.push(Span::styled(
                format!("-{} ", diff.stats.deletions),
                Style::default()
                    .fg(theme.status_d)
                    .add_modifier(Modifier::BOLD),
            ));
            line
        }
        (None, None) => Line::from(Span::styled(" Diff View ", muted)),
    };
    let wrap_span = if wrap {
        Span::styled("wrap on", Style::default().fg(theme.status_a))
    } else {
        Span::styled("wrap off", muted)
    };
    let context_span = if full_context {
        Span::styled(
            "full file",
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("hunks", muted)
    };
    let tag = Line::from(vec![
        Span::raw(" "),
        context_span,
        Span::styled(" • ", Style::default().fg(theme.border)),
        Span::styled(mode.to_string(), muted),
        Span::styled(" • ", Style::default().fg(theme.border)),
        wrap_span,
        Span::raw(" "),
    ])
    .right_aligned();
    let position_width = if position.is_empty() {
        0
    } else {
        position.width() as u16 + 2
    };
    let mut help = help_line(help, width.saturating_sub(position_width + 6), theme);
    help.spans.insert(0, Span::raw(" "));
    help.spans.push(Span::raw(" "));
    let mut block = Block::default().title(title).title(tag).title_bottom(help);
    if !position.is_empty() {
        block = block.title_bottom(
            Line::from(Span::styled(format!(" {} ", position), muted)).right_aligned(),
        );
    }
    block
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .style(Style::default().bg(theme.bg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_line_truncates_with_ellipsis() {
        let theme = Theme::catppuccin();
        let items = [("enter", "open"), ("j/k", "navigate"), ("esc", "close")];
        let full: String = help_line(&items, 80, &theme)
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert_eq!(full, "enter open • j/k navigate • esc close");
        let short: String = help_line(&items, 20, &theme)
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert_eq!(short, "enter open …");
    }

    #[test]
    fn card_dims_backdrop_and_casts_shadow() {
        let backend = ratatui::backend::TestBackend::new(40, 12);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let theme = Theme::tokyonight();
        let popup = Rect::new(5, 2, 20, 6);
        terminal
            .draw(|frame| {
                render_card(frame, popup, "*", "Title", theme.header_fg, &theme);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        assert!(buf[(0, 0)].modifier.contains(Modifier::DIM));
        assert_eq!(buf[(26, 5)].bg, shadow_color(&theme));
        assert!(!buf[(6, 3)].modifier.contains(Modifier::DIM));
    }
}
