use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

pub fn render_help_popup(frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(80, 85, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" 󰋖 Keyboard Shortcuts  ·  [Esc] or [?] to close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.header_bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("󰌌  Navigation & Scrolling", vec![
            ("j / k or ↓ / ↑", "Scroll lines down / up"),
            ("J / K or Ctrl+d / Ctrl+u", "Scroll half page down / up"),
            ("] or n  /  [ or p", "Jump to Next / Previous hunk"),
            ("Tab", "Switch focus between File Tree and Diff View"),
            ("h / l or ← / →", "Diff: switch column (Old/New) · Tree: collapse/expand"),
        ]),
        ("󰦨  Staging & Git Operations", vec![
            ("s", "Stage hunk under cursor (or selected lines in Visual mode)"),
            ("u", "Unstage hunk under cursor"),
            ("d", "Discard hunk under cursor (with confirmation)"),
            ("S / U / D", "Stage / Unstage / Discard entire file"),
            ("v", "Toggle Visual Mode for line-by-line partial staging"),
            ("H", "View commit History for active file"),
        ]),
        ("󰈚  View & File Tree", vec![
            ("t", "Toggle Flat List ↔ Collapsible Directory Tree"),
            ("/", "Filter files by path or extension (fuzzy search)"),
            ("m", "Toggle Side-by-Side ↔ Unified view mode"),
            ("w", "Toggle AI live file watching mode (auto-reload)"),
        ]),
        ("󰒅  Integrations & System", vec![
            ("Enter / e", "Open file in Neovim / $EDITOR at cursor line (+line)"),
            ("c", "Copy hunk to system clipboard as Markdown"),
            ("? ", "Show / hide this help modal"),
            ("q / Esc", "Quit diffv (or dismiss modal / visual mode)"),
        ]),
    ];

    let mut lines = Vec::new();

    for (sec_title, bindings) in sections {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}  ", sec_title),
                Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD),
            ),
        ]));

        for (key, desc) in bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:26}", key), Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
                Span::styled(desc, Style::default().fg(theme.fg)),
            ]));
        }
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

pub fn render_confirm_popup(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    theme: &Theme,
) {
    let popup_area = centered_rect(55, 25, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" ⚠ Confirm Action ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(245, 110, 120)).add_modifier(Modifier::BOLD))
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
            Span::styled(" [y] Confirm ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::raw("    "),
            Span::styled(" [n / Esc] Cancel ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_d).add_modifier(Modifier::BOLD)),
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
