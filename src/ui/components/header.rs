use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::{Language, RepoStats};
use crate::ui::theme::Theme;

pub fn render_header(
    frame: &mut Frame,
    area: Rect,
    repo_stats: &RepoStats,
    is_unified: bool,
    watch_mode: bool,
    worktree_name: Option<&str>,
    language: Language,
    theme: &Theme,
) {
    let files_label = match language {
        Language::En => "files",
        Language::Pt => "arquivos",
    };
    let mode_label = match language {
        Language::En => "Mode:",
        Language::Pt => "Modo:",
    };

    let width = area.width;

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
    ];

    // Branch
    let branch_display = if width < 60 && repo_stats.branch.len() > 10 {
        format!(" 󰊢 {}… ", &repo_stats.branch[..8])
    } else {
        format!(" 󰊢 {} ", repo_stats.branch)
    };
    spans.push(Span::styled(
        branch_display,
        Style::default()
            .fg(theme.key_fg)
            .add_modifier(Modifier::BOLD),
    ));

    // Optional worktree pill
    if let Some(wt) = worktree_name {
        if width >= 75 {
            spans.push(Span::styled("· ", Style::default().fg(theme.border)));
            let wt_str = if width < 90 && wt.len() > 10 {
                format!("󰹹 {}… ", &wt[..8])
            } else {
                format!("󰹹 {} [W] ", wt)
            };
            spans.push(Span::styled(
                wt_str,
                Style::default().fg(theme.status_u).add_modifier(Modifier::BOLD),
            ));
        }
    }

    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));

    // Files count (if not ultra-compact)
    if width >= 80 {
        spans.push(Span::styled(
            format!("{} {} ", repo_stats.file_count, files_label),
            Style::default().fg(theme.fg),
        ));
    }

    // Additions pill
    spans.push(Span::styled(
        format!("+{} ", repo_stats.total_additions),
        Style::default()
            .fg(theme.add_fg)
            .add_modifier(Modifier::BOLD),
    ));

    // Deletions pill
    spans.push(Span::styled(
        format!("-{} ", repo_stats.total_deletions),
        Style::default()
            .fg(theme.del_fg)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));

    // Mode badge
    if width >= 95 {
        spans.push(Span::styled(format!("{} ", mode_label), Style::default().fg(theme.line_num_fg)));
    }
    let mode_str = if is_unified {
        if width < 80 { "󰤈 Uni [m] " } else { " 󰤈 Unified [m] " }
    } else {
        if width < 80 { "󰤉 SbS [m] " } else { " 󰤉 Side-by-Side [m] " }
    };
    spans.push(Span::styled(
        mode_str,
        Style::default()
            .fg(theme.header_fg)
            .add_modifier(Modifier::BOLD),
    ));

    // Watch indicator pill (if space permits)
    if width >= 85 {
        spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
        if watch_mode {
            spans.push(Span::styled(
                " 󰐥 LIVE ",
                Style::default()
                    .fg(Color::Rgb(15, 20, 25))
                    .bg(theme.status_a)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                " 󰏤 PAUSED ",
                Style::default()
                    .fg(theme.line_num_fg)
                    .add_modifier(Modifier::DIM),
            ));
        }
    }

    // Language switcher badge
    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
    let lang_str = match language {
        Language::En => "󰗊 en [L]",
        Language::Pt => "󰗊 pt [L]",
    };
    spans.push(Span::styled(
        format!(" {} ", lang_str),
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD),
    ));

    // Right-aligned help hint (only if screen has plenty of width)
    if width >= 110 {
        spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
        spans.push(Span::styled("󰋖 ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(
            match language { Language::En => "Help [?]  ", Language::Pt => "Ajuda [?]  " },
            Style::default().fg(theme.line_num_fg),
        ));
        spans.push(Span::styled("󰌌 ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(
            match language { Language::En => "Focus [Tab]", Language::Pt => "Foco [Tab]" },
            Style::default().fg(theme.line_num_fg),
        ));
    }

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.header_bg));

    let paragraph = Paragraph::new(Line::from(spans)).block(block);
    frame.render_widget(paragraph, area);
}
