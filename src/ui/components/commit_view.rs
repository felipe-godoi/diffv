use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Wrap};
use ratatui::Frame;

use crate::core::models::{CommitEntry, Language, StashEntry};
use crate::ui::theme::Theme;

pub fn render_commit_overview(
    frame: &mut Frame,
    area: Rect,
    commit: &CommitEntry,
    language: Language,
    theme: &Theme,
) {
    let title = match language {
        Language::En => format!(" 󰜉 Commit Details: {} ", &commit.hash[..7.min(commit.hash.len())]),
        Language::Pt => format!(" 󰜉 Detalhes do Commit: {} ", &commit.hash[..7.min(commit.hash.len())]),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Metadata
            Constraint::Length(1),  // Separator
            Constraint::Min(4),     // Commit message
            Constraint::Length(3),  // Action buttons footer
        ])
        .split(inner);

    // 1. Metadata lines
    let mut meta_lines = Vec::new();
    meta_lines.push(Line::from(vec![
        Span::styled(" Commit:  ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(&commit.hash, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));
    meta_lines.push(Line::from(vec![
        Span::styled(" Author:  ", Style::default().fg(theme.line_num_fg)),
        Span::styled(&commit.author, Style::default().fg(theme.status_u)),
    ]));
    meta_lines.push(Line::from(vec![
        Span::styled(" Date:    ", Style::default().fg(theme.line_num_fg)),
        Span::styled(&commit.date, Style::default().fg(theme.line_num_fg)),
    ]));

    frame.render_widget(Paragraph::new(meta_lines), chunks[0]);

    // 2. Separator
    let sep = "─".repeat(chunks[1].width as usize);
    frame.render_widget(Paragraph::new(Span::styled(sep, Style::default().fg(theme.border))), chunks[1]);

    // 3. Commit Message
    let mut msg_lines = Vec::new();
    msg_lines.push(Line::from(vec![
        Span::styled(" Message:", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
    ]));
    msg_lines.push(Line::from(""));

    for (i, line) in commit.message.lines().enumerate() {
        if i == 0 {
            msg_lines.push(Line::from(Span::styled(
                format!("   {}", line),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            )));
        } else {
            msg_lines.push(Line::from(Span::styled(
                format!("   {}", line),
                Style::default().fg(theme.fg),
            )));
        }
    }

    let msg_p = Paragraph::new(msg_lines).wrap(Wrap { trim: false });
    frame.render_widget(msg_p, chunks[2]);

    // 4. Action buttons
    let (inspect_btn, inspect_label, details_label, nav_label, back_label) = match language {
        Language::En => (
            " [Enter] ",
            "Inspect files",
            " [i] Details ",
            "[j/k] Navigate",
            "[1] Live diff",
        ),
        Language::Pt => (
            " [Enter] ",
            "Inspecionar",
            " [i] Detalhes ",
            "[j/k] Navegar",
            "[1] Diff local",
        ),
    };

    let actions = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(inspect_btn, Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}   ", inspect_label), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(details_label, Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled("   ", Style::default()),
            Span::styled(format!("{}   ", nav_label), Style::default().fg(theme.line_num_fg)),
            Span::styled(back_label, Style::default().fg(theme.line_num_fg)),
        ]),
    ];

    frame.render_widget(Paragraph::new(actions), chunks[3]);
}

pub fn render_stash_overview(
    frame: &mut Frame,
    area: Rect,
    stash: &StashEntry,
    language: Language,
    theme: &Theme,
) {
    let title = match language {
        Language::En => format!(" 󰮎 Stash Details: {} ", stash.selector),
        Language::Pt => format!(" 󰮎 Detalhes do Stash: {} ", stash.selector),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),  // Metadata
            Constraint::Length(1),  // Separator
            Constraint::Min(4),     // Stash message
            Constraint::Length(3),  // Action buttons footer
        ])
        .split(inner);

    // 1. Metadata
    let mut meta_lines = Vec::new();
    meta_lines.push(Line::from(vec![
        Span::styled(" Stash:   ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(&stash.selector, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));
    meta_lines.push(Line::from(vec![
        Span::styled(" Date:    ", Style::default().fg(theme.line_num_fg)),
        Span::styled(&stash.date, Style::default().fg(theme.line_num_fg)),
    ]));

    frame.render_widget(Paragraph::new(meta_lines), chunks[0]);

    // 2. Separator
    let sep = "─".repeat(chunks[1].width as usize);
    frame.render_widget(Paragraph::new(Span::styled(sep, Style::default().fg(theme.border))), chunks[1]);

    // 3. Stash Message
    let mut msg_lines = Vec::new();
    msg_lines.push(Line::from(vec![
        Span::styled(" Description:", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
    ]));
    msg_lines.push(Line::from(""));
    msg_lines.push(Line::from(Span::styled(
        format!("   {}", stash.message),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )));

    let msg_p = Paragraph::new(msg_lines).wrap(Wrap { trim: false });
    frame.render_widget(msg_p, chunks[2]);

    // 4. Action buttons
    let (inspect_btn, inspect_label, nav_label, back_label) = match language {
        Language::En => (
            " [Enter] ",
            "Inspect files & diffs of this stash",
            "[j/k] Navigate stashes",
            "[1] Return to live diff",
        ),
        Language::Pt => (
            " [Enter] ",
            "Inspecionar arquivos e alterações deste stash",
            "[j/k] Navegar stashes",
            "[1] Retornar ao diff local",
        ),
    };

    let actions = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(inspect_btn, Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}    ", inspect_label), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}    ", nav_label), Style::default().fg(theme.line_num_fg)),
            Span::styled(back_label, Style::default().fg(theme.line_num_fg)),
        ]),
    ];

    frame.render_widget(Paragraph::new(actions), chunks[3]);
}
