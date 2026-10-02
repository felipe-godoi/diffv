use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::core::models::{Language, WorktreeEntry};
use crate::ui::theme::Theme;

#[derive(Debug, Clone)]
pub struct WorktreeCreationState {
    pub path_input: String,
    pub branch_input: String,
    pub active_field: usize, // 0 = Path, 1 = Branch
    pub error_msg: Option<String>,
}

impl Default for WorktreeCreationState {
    fn default() -> Self {
        Self {
            path_input: String::new(),
            branch_input: String::new(),
            active_field: 0,
            error_msg: None,
        }
    }
}

pub fn render_worktree_popup(
    frame: &mut Frame,
    area: Rect,
    worktrees: &[WorktreeEntry],
    selected_idx: usize,
    scroll_offset: usize,
    creation: Option<&WorktreeCreationState>,
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(82, 68, area);
    frame.render_widget(Clear, popup_area);

    if let Some(create_state) = creation {
        render_creation_form(frame, popup_area, create_state, language, theme);
        return;
    }

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
            Language::En => "  No git worktrees found. Press 'a' or 'n' to create one.",
            Language::Pt => "  Nenhuma worktree git encontrada. Pressione 'a' ou 'n' para criar uma.",
        };
        let p = Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(p, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),    // Worktree list
            Constraint::Length(3), // Full path detail overlay box
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

        let branch_pad = if list_area.width < 55 { 12 } else { 20 };
        let branch_truncated = if branch_name.len() > branch_pad {
            format!("{}…", &branch_name[..branch_pad.saturating_sub(1)])
        } else {
            branch_name.to_string()
        };

        let line = Line::from(vec![
            cursor_span,
            Span::raw(" "),
            current_pill,
            Span::raw(" "),
            Span::styled("󰊢 ", Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg })),
            Span::styled(
                format!("{:<width$} ", branch_truncated, width = branch_pad),
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

    // Full detail box of selected worktree (never truncated)
    if let Some(selected_wt) = worktrees.get(selected_idx) {
        let detail_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(Color::Rgb(24, 25, 36)));

        let detail_inner = detail_block.inner(chunks[1]);
        frame.render_widget(detail_block, chunks[1]);

        let branch_label = selected_wt.branch.as_deref().unwrap_or("detached");
        let head_short = if selected_wt.head.len() >= 7 { &selected_wt.head[..7] } else { &selected_wt.head };
        let full_path_line = Line::from(vec![
            Span::styled(" 󰉖 Path: ", Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
            Span::styled(selected_wt.path.display().to_string(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ·  󰊢 {}  ·   {}", branch_label, head_short), Style::default().fg(theme.line_num_fg)),
        ]);
        frame.render_widget(Paragraph::new(full_path_line), detail_inner);
    }

    // Footer shortcuts
    let footer_line = match language {
        Language::En => Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled("Switch  ", Style::default().fg(theme.fg)),
            Span::styled(" [a / n] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::styled("New Worktree  ", Style::default().fg(theme.fg)),
            Span::styled(" [j/k] ", Style::default().fg(theme.key_fg)),
            Span::styled("Nav  ", Style::default().fg(theme.line_num_fg)),
            Span::styled(" [Esc / W] ", Style::default().fg(theme.key_fg)),
            Span::styled("Close", Style::default().fg(theme.line_num_fg)),
        ]),
        Language::Pt => Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled("Alternar  ", Style::default().fg(theme.fg)),
            Span::styled(" [a / n] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::styled("Nova Worktree  ", Style::default().fg(theme.fg)),
            Span::styled(" [j/k] ", Style::default().fg(theme.key_fg)),
            Span::styled("Navegar  ", Style::default().fg(theme.line_num_fg)),
            Span::styled(" [Esc / W] ", Style::default().fg(theme.key_fg)),
            Span::styled("Fechar", Style::default().fg(theme.line_num_fg)),
        ]),
    };
    frame.render_widget(Paragraph::new(footer_line), chunks[2]);
}

fn render_creation_form(
    frame: &mut Frame,
    area: Rect,
    state: &WorktreeCreationState,
    language: Language,
    theme: &Theme,
) {
    let title = match language {
        Language::En => " 󰹹 Create New Git Worktree ",
        Language::Pt => " 󰹹 Criar Nova Worktree Git ",
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_a).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.header_bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Help text
            Constraint::Length(3), // Path Input box
            Constraint::Length(3), // Branch Input box
            Constraint::Length(2), // Error message if any
            Constraint::Length(1), // Footer shortcuts
        ])
        .split(inner);

    let desc = match language {
        Language::En => "Enter target folder path and optional branch name:",
        Language::Pt => "Informe o diretório de destino e nome opcional da branch:",
    };
    frame.render_widget(Paragraph::new(desc).style(Style::default().fg(theme.line_num_fg)), chunks[0]);

    // Path Box
    let path_border_style = if state.active_field == 0 {
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };
    let path_title = match language {
        Language::En => " 📁 Directory Path (e.g. ../feat-worktree) ",
        Language::Pt => " 📁 Diretório (ex: ../feat-worktree) ",
    };
    let path_block = Block::default()
        .title(path_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(path_border_style)
        .style(Style::default().bg(Color::Rgb(24, 25, 36)));
    let path_inner = path_block.inner(chunks[1]);
    frame.render_widget(path_block, chunks[1]);
    let path_cursor = if state.active_field == 0 { "█" } else { "" };
    let path_line = Line::from(vec![
        Span::styled(&state.path_input, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Span::styled(path_cursor, Style::default().fg(theme.key_fg)),
    ]);
    frame.render_widget(Paragraph::new(path_line), path_inner);

    // Branch Box
    let branch_border_style = if state.active_field == 1 {
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };
    let branch_title = match language {
        Language::En => " 󰊢 Branch (e.g. feature/my-branch - leave empty for default) ",
        Language::Pt => " 󰊢 Branch (ex: feature/minha-branch - deixe vazio para padrão) ",
    };
    let branch_block = Block::default()
        .title(branch_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(branch_border_style)
        .style(Style::default().bg(Color::Rgb(24, 25, 36)));
    let branch_inner = branch_block.inner(chunks[2]);
    frame.render_widget(branch_block, chunks[2]);
    let branch_cursor = if state.active_field == 1 { "█" } else { "" };
    let branch_line = Line::from(vec![
        Span::styled(&state.branch_input, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Span::styled(branch_cursor, Style::default().fg(theme.key_fg)),
    ]);
    frame.render_widget(Paragraph::new(branch_line), branch_inner);

    // Error display
    if let Some(err) = &state.error_msg {
        let err_line = Line::from(vec![
            Span::styled(format!(" ✖ {}", err), Style::default().fg(theme.del_fg).add_modifier(Modifier::BOLD)),
        ]);

        frame.render_widget(Paragraph::new(err_line), chunks[3]);
    }

    // Footer actions
    let footer_line = match language {
        Language::En => Line::from(vec![
            Span::styled(" [Tab] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled("Next Field  ", Style::default().fg(theme.fg)),
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::styled("Create & Switch  ", Style::default().fg(theme.fg)),
            Span::styled(" [Esc] ", Style::default().fg(theme.key_fg)),
            Span::styled("Cancel", Style::default().fg(theme.line_num_fg)),
        ]),
        Language::Pt => Line::from(vec![
            Span::styled(" [Tab] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::styled("Próximo Campo  ", Style::default().fg(theme.fg)),
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::styled("Criar e Alternar  ", Style::default().fg(theme.fg)),
            Span::styled(" [Esc] ", Style::default().fg(theme.key_fg)),
            Span::styled("Cancelar", Style::default().fg(theme.line_num_fg)),
        ]),
    };
    frame.render_widget(Paragraph::new(footer_line), chunks[4]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
