use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

pub fn render_help_popup(frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(75, 80, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Keyboard Shortcuts (Press Esc or ? to close) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let keybindings = vec![
        ("j / k or ↓ / ↑", "Scroll lines down / up"),
        ("J / K or Ctrl+d / Ctrl+u", "Scroll half page down / up"),
        ("] or n", "Jump to NEXT hunk"),
        ("[ or p", "Jump to PREVIOUS hunk"),
        ("Tab", "Switch focus between File Tree and Diff View"),
        ("Enter", "In File Tree: open diff; In Diff: open file in Neovim"),
        ("e", "Open file in Neovim at current cursor line (nvim +line)"),
        ("s", "STAGE current hunk"),
        ("u", "UNSTAGE current hunk"),
        ("d", "DISCARD current hunk (with confirmation)"),
        ("S", "STAGE entire file"),
        ("U", "UNSTAGE entire file"),
        ("D", "DISCARD entire file (with confirmation)"),
        ("c", "COPY hunk as Markdown to clipboard"),
        ("m", "TOGGLE Side-by-Side vs Unified view mode"),
        ("w", "TOGGLE Live Watch mode"),
        ("/", "Filter files in File Tree (fuzzy search)"),
        ("? ", "Show / hide this help modal"),
        ("q / Esc", "Quit diffv"),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (key, desc) in keybindings {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:26}", key), Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(desc, Style::default().fg(theme.fg)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Tmux Popup integration: ", Style::default().fg(theme.status_u).add_modifier(Modifier::BOLD)),
        Span::styled("bind-key d display-popup -E -w 92% -h 90% \"diffv --watch\"", Style::default().fg(theme.line_num_fg)),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

pub fn render_confirm_popup(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    theme: &Theme,
) {
    let popup_area = centered_rect(50, 25, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Confirm Action ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(240, 100, 100)).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.header_bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(theme.line_num_fg)),
            Span::styled("[y]", Style::default().fg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::styled(" to confirm, ", Style::default().fg(theme.line_num_fg)),
            Span::styled("[n]", Style::default().fg(theme.status_d).add_modifier(Modifier::BOLD)),
            Span::styled(" or [Esc] to cancel", Style::default().fg(theme.line_num_fg)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
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
