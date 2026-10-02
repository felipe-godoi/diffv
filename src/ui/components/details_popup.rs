use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::core::models::{CommitEntry, FileDiff, FileStatus, Language, StashEntry};
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

fn file_status_char(file: &FileDiff) -> &'static str {
    match file.status {
        FileStatus::Modified => "M",
        FileStatus::Added => "A",
        FileStatus::Deleted => "D",
        FileStatus::Untracked => "?",
        FileStatus::Renamed => "R",
        FileStatus::Copied => "C",
    }
}

pub enum DetailsContent<'a> {
    Commit(&'a CommitEntry, &'a [FileDiff]),
    File(&'a FileDiff),
    Stash(&'a StashEntry, &'a [FileDiff]),
}

pub fn render_details_popup(
    frame: &mut Frame,
    area: Rect,
    content: DetailsContent,
    scroll: usize,
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(82, 80, area);

    match content {
        DetailsContent::Commit(commit, files) => {
            render_commit_details(frame, popup_area, commit, files, scroll, language, theme);
        }
        DetailsContent::File(file) => {
            render_file_details(frame, popup_area, file, scroll, language, theme);
        }
        DetailsContent::Stash(stash, files) => {
            render_stash_details(frame, popup_area, stash, files, scroll, language, theme);
        }
    }
}

fn render_commit_details(
    frame: &mut Frame,
    area: Rect,
    commit: &CommitEntry,
    files: &[FileDiff],
    scroll: usize,
    language: Language,
    theme: &Theme,
) {
    let title = match language {
        Language::En => format!("Commit Details · {}", &commit.hash[..7.min(commit.hash.len())]),
        Language::Pt => format!("Detalhes do Commit · {}", &commit.hash[..7.min(commit.hash.len())]),
    };

    let inner = render_card(frame, area, "󰜉", &title, theme.header_fg, theme);

    if inner.height < 6 || inner.width < 20 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Metadata
            Constraint::Length(1),  // Separator
            Constraint::Min(4),     // Scrollable message & files
            Constraint::Length(1),  // Footer
        ])
        .split(inner);

    // 1. Metadata lines
    let mut meta = Vec::new();
    meta.push(Line::from(vec![
        Span::styled(" Commit SHA:  ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(&commit.hash, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));
    meta.push(Line::from(vec![
        Span::styled(" Author:      ", Style::default().fg(theme.line_num_fg)),
        Span::styled(&commit.author, Style::default().fg(theme.status_u).add_modifier(Modifier::BOLD)),
    ]));
    meta.push(Line::from(vec![
        Span::styled(" Date:        ", Style::default().fg(theme.line_num_fg)),
        Span::styled(&commit.date, Style::default().fg(theme.fg)),
    ]));

    let total_adds: usize = files.iter().map(|f| f.stats.additions).sum();
    let total_dels: usize = files.iter().map(|f| f.stats.deletions).sum();
    let stats_label = match language {
        Language::En => format!(" Summary:     󰈚 {} files changed (", files.len()),
        Language::Pt => format!(" Resumo:      󰈚 {} arquivos modificados (", files.len()),
    };
    meta.push(Line::from(vec![
        Span::styled(stats_label, Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("+{}", total_adds), Style::default().fg(theme.status_a).add_modifier(Modifier::BOLD)),
        Span::styled(", ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("-{}", total_dels), Style::default().fg(theme.status_d).add_modifier(Modifier::BOLD)),
        Span::styled(")", Style::default().fg(theme.line_num_fg)),
    ]));

    frame.render_widget(Paragraph::new(meta), chunks[0]);

    // 2. Separator
    let sep = "─".repeat(chunks[1].width as usize);
    frame.render_widget(Paragraph::new(Span::styled(sep, Style::default().fg(theme.border))), chunks[1]);

    // 3. Scrollable content: Commit message (wrapped) followed by changed files list
    let mut body = Vec::new();
    body.push(Line::from(Span::styled(
        match language {
            Language::En => "─── Commit Message ───────────────────────────────────────────",
            Language::Pt => "─── Mensagem do Commit ───────────────────────────────────────",
        },
        Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD),
    )));
    body.push(Line::from(""));

    let msg_lines: Vec<&str> = commit.message.lines().collect();
    for (i, line) in msg_lines.iter().enumerate() {
        if i == 0 {
            body.push(Line::from(Span::styled(
                *line,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            )));
        } else {
            body.push(Line::from(Span::styled(
                *line,
                Style::default().fg(theme.fg),
            )));
        }
    }

    if !files.is_empty() {
        body.push(Line::from(""));
        body.push(Line::from(Span::styled(
            match language {
                Language::En => "─── Changed Files ────────────────────────────────────────────",
                Language::Pt => "─── Arquivos Alterados ───────────────────────────────────────",
            },
            Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD),
        )));
        body.push(Line::from(""));

        for file in files {
            body.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", file_status_char(file)),
                    Style::default().fg(theme.status_m).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<40}", file.display_path()),
                    Style::default().fg(theme.fg),
                ),
                Span::styled(
                    format!(" +{:<4}", file.stats.additions),
                    Style::default().fg(theme.status_a),
                ),
                Span::styled(
                    format!(" -{:<4}", file.stats.deletions),
                    Style::default().fg(theme.status_d),
                ),
            ]));
        }
    }

    let p = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));
    frame.render_widget(p, chunks[2]);

    // 4. Footer controls
    let footer_items: &[(&str, &str)] = match language {
        Language::En => &[("esc / q / i", "close"), ("j/k", "scroll message")],
        Language::Pt => &[("esc / q / i", "fechar"), ("j/k", "rolar mensagem")],
    };
    frame.render_widget(Paragraph::new(help_line(footer_items, chunks[3].width, theme)).alignment(Alignment::Center), chunks[3]);
}

fn render_file_details(
    frame: &mut Frame,
    area: Rect,
    file: &FileDiff,
    _scroll: usize,
    language: Language,
    theme: &Theme,
) {
    let title = match language {
        Language::En => format!("File Details · {}", file.display_path()),
        Language::Pt => format!("Detalhes do Arquivo · {}", file.display_path()),
    };

    let inner = render_card(frame, area, "󰈚", &title, theme.key_fg, theme);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(inner);

    let mut lines = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" File Path:       ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(file.display_path(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Git Status:      ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("{:?}", file.status), Style::default().fg(theme.status_m).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Stage Status:    ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("{:?}", file.stage_status), Style::default().fg(theme.status_a).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Total Hunks:     ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("{}", file.hunks.len()), Style::default().fg(theme.fg)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Changes:         ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("+{} additions", file.stats.additions), Style::default().fg(theme.status_a).add_modifier(Modifier::BOLD)),
        Span::styled(", ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("-{} deletions", file.stats.deletions), Style::default().fg(theme.status_d).add_modifier(Modifier::BOLD)),
    ]));
    if file.is_binary {
        lines.push(Line::from(vec![
            Span::styled(" Type:            ", Style::default().fg(theme.line_num_fg)),
            Span::styled("Binary file difference", Style::default().fg(theme.status_d)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), chunks[0]);

    let footer_items: &[(&str, &str)] = match language {
        Language::En => &[("esc / q / i", "close")],
        Language::Pt => &[("esc / q / i", "fechar")],
    };
    frame.render_widget(Paragraph::new(help_line(footer_items, chunks[1].width, theme)).alignment(Alignment::Center), chunks[1]);
}

fn render_stash_details(
    frame: &mut Frame,
    area: Rect,
    stash: &StashEntry,
    files: &[FileDiff],
    scroll: usize,
    language: Language,
    theme: &Theme,
) {
    let title = match language {
        Language::En => format!("Stash Details · {}", stash.selector),
        Language::Pt => format!("Detalhes do Stash · {}", stash.selector),
    };

    let inner = render_card(frame, area, "󰮎", &title, theme.key_fg, theme);

    if inner.height < 6 || inner.width < 20 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let mut meta = Vec::new();
    meta.push(Line::from(vec![
        Span::styled(" Stash ID:    ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::styled(&stash.selector, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));
    meta.push(Line::from(vec![
        Span::styled(" Date:        ", Style::default().fg(theme.line_num_fg)),
        Span::styled(&stash.date, Style::default().fg(theme.fg)),
    ]));
    meta.push(Line::from(vec![
        Span::styled(" Files:       ", Style::default().fg(theme.line_num_fg)),
        Span::styled(format!("󰈚 {} modified", files.len()), Style::default().fg(theme.status_a)),
    ]));

    frame.render_widget(Paragraph::new(meta), chunks[0]);

    let sep = "─".repeat(chunks[1].width as usize);
    frame.render_widget(Paragraph::new(Span::styled(sep, Style::default().fg(theme.border))), chunks[1]);

    let mut body = Vec::new();
    body.push(Line::from(Span::styled(
        match language {
            Language::En => "─── Stash Message ────────────────────────────────────────────",
            Language::Pt => "─── Mensagem do Stash ────────────────────────────────────────",
        },
        Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD),
    )));
    body.push(Line::from(""));
    body.push(Line::from(Span::styled(&stash.message, Style::default().fg(theme.fg))));

    if !files.is_empty() {
        body.push(Line::from(""));
        body.push(Line::from(Span::styled(
            match language {
                Language::En => "─── Changed Files ────────────────────────────────────────────",
                Language::Pt => "─── Arquivos Alterados ───────────────────────────────────────",
            },
            Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD),
        )));
        body.push(Line::from(""));
        for f in files {
            body.push(Line::from(vec![
                Span::styled(format!("  {} ", file_status_char(f)), Style::default().fg(theme.status_m).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<40}", f.display_path()), Style::default().fg(theme.fg)),
                Span::styled(format!(" +{:<4}", f.stats.additions), Style::default().fg(theme.status_a)),
                Span::styled(format!(" -{:<4}", f.stats.deletions), Style::default().fg(theme.status_d)),
            ]));
        }
    }

    let p = Paragraph::new(body).wrap(Wrap { trim: false }).scroll((scroll as u16, 0));
    frame.render_widget(p, chunks[2]);

    let footer_items: &[(&str, &str)] = match language {
        Language::En => &[("esc / q / i", "close"), ("j/k", "scroll")],
        Language::Pt => &[("esc / q / i", "fechar"), ("j/k", "rolar")],
    };
    frame.render_widget(Paragraph::new(help_line(footer_items, chunks[3].width, theme)).alignment(Alignment::Center), chunks[3]);
}
