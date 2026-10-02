use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::core::models::{Language, WorktreeEntry};
use crate::ui::theme::Theme;

pub fn render_worktree_popup(
    frame: &mut Frame,
    area: Rect,
    worktrees: &[WorktreeEntry],
    selected_idx: usize,
    scroll_offset: usize,
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(80, 60, area);
    frame.render_widget(Clear, popup_area);

    let title = match language {
        Language::En => format!(" 󰹹 Git Worktrees ({} worktrees) ", worktrees.len()),
        Language::Pt => format!(" 󰹹 Worktrees Git ({} worktrees) ", worktrees.len()),
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.header_bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if worktrees.is_empty() {
        let msg = match language {
            Language::En => "  No git worktrees found.",
            Language::Pt => "  Nenhuma worktree git encontrada.",
        };
        let p = Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(p, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Worktree list
            Constraint::Length(1), // Footer shortcuts
        ])
        .split(inner);

    let list_area = chunks[0];
    let max_rows = list_area.height as usize;
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(worktrees.len());

    let mut lines = Vec::new();

    for idx in start_idx..end_idx {
        let wt = &worktrees[idx];
        let is_selected = idx == selected_idx;

        let base_style = if is_selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default().bg(theme.bg).fg(theme.fg)
        };

        let cursor_span = if is_selected {
            Span::styled("▎", Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ", base_style)
        };

        let current_pill = if wt.is_current {
            match language {
                Language::En => Span::styled(" [ACTIVE] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
                Language::Pt => Span::styled(" [ATIVO] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            }
        } else {
            Span::styled("          ", base_style)
        };

        let branch_name = wt.branch.as_deref().unwrap_or("detached");
        let path_str = wt.path.display().to_string();

        let line = Line::from(vec![
            cursor_span,
            Span::raw(" "),
            current_pill,
            Span::raw(" "),
            Span::styled("󰊢 ", Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg })),
            Span::styled(
                format!("{:<20} ", branch_name),
                if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style },
            ),
            Span::styled(
                path_str,
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ),
        ]);

        lines.push(line);
    }

    frame.render_widget(Paragraph::new(lines), list_area);

    // Footer shortcuts
    let footer_line = match language {
        Language::En => Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Switch Worktree    ", Style::default().fg(theme.fg)),
            Span::styled(" [j/k] ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Navigate    ", Style::default().fg(theme.line_num_fg)),
            Span::styled(" [Esc / W] ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Close Switcher", Style::default().fg(theme.line_num_fg)),
        ]),
        Language::Pt => Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Alternar Worktree    ", Style::default().fg(theme.fg)),
            Span::styled(" [j/k] ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Navegar    ", Style::default().fg(theme.line_num_fg)),
            Span::styled(" [Esc / W] ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(" Fechar", Style::default().fg(theme.line_num_fg)),
        ]),
    };
    frame.render_widget(Paragraph::new(footer_line), chunks[1]);
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
