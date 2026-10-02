use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};
use ratatui::Frame;

use crate::core::models::{Language, WorktreeEntry};
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

#[derive(Debug, Clone)]
pub struct WorktreeCreationState {
    pub path_input: String,
    pub branch_input: String,
    pub active_field: usize, // 0 = Path, 1 = Branch
    pub error_msg: Option<String>,
    pub base: String,
    pub branches: Vec<String>,
    pub selected_candidate: usize,
    pub path_manual: bool,
}

impl WorktreeCreationState {
    pub fn new(base: String, branches: Vec<String>) -> Self {
        Self { path_input: base.clone(), branch_input: String::new(), active_field: 1,
            error_msg: None, base, branches, selected_candidate: 0, path_manual: false }
    }

    pub fn candidates(&self) -> Vec<&str> {
        self.branches.iter().filter(|b| b.to_lowercase().contains(&self.branch_input.to_lowercase()))
            .map(String::as_str).collect()
    }

    pub fn update_branch(&mut self) {
        self.selected_candidate = 0;
        self.error_msg = None;
        if !self.path_manual {
            self.path_input = format!("{}{}", self.base, self.branch_input.replace('/', "-"));
        }
    }

    pub fn complete_branch(&mut self) {
        if let Some(branch) = self.candidates().get(self.selected_candidate) {
            self.branch_input = branch.to_string();
            self.update_branch();
        }
    }
}

impl Default for WorktreeCreationState {
    fn default() -> Self { Self::new(String::new(), Vec::new()) }
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

    if let Some(create_state) = creation {
        render_creation_form(frame, popup_area, create_state, language, theme);
        return;
    }

    let title = match language {
        Language::En => format!("Git Worktrees · {}", worktrees.len()),
        Language::Pt => format!("Worktrees Git · {}", worktrees.len()),
    };

    let inner = render_card(frame, popup_area, "󰹹", &title, theme.header_fg, theme);

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
                Language::En => Span::styled(" [ACTIVE] ", Style::default().fg(theme.text_on(theme.status_a)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
                Language::Pt => Span::styled(" [ATIVO] ", Style::default().fg(theme.text_on(theme.status_a)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            }
        } else {
            Span::styled("          ", base_style)
        };

        let branch_name = wt.branch.as_deref().unwrap_or("detached");
        let path_str = wt.path.display().to_string();

        let branch_pad = if list_area.width < 55 { 12 } else { 20 };
        let branch_truncated = if branch_name.len() > branch_pad {
            format!("{}…", branch_name.chars().take(branch_pad.saturating_sub(1)).collect::<String>())
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
            .style(Style::default().bg(theme.bg));

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

    let footer_items: &[(&str, &str)] = match language {
        Language::En => &[("enter", "switch"), ("a / n", "new worktree"), ("esc / W", "close"), ("j/k", "navigate")],
        Language::Pt => &[("enter", "alternar"), ("a / n", "nova worktree"), ("esc / W", "fechar"), ("j/k", "navegar")],
    };
    let footer_line = help_line(footer_items, chunks[2].width, theme);
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
        Language::En => "Create New Git Worktree",
        Language::Pt => "Criar Nova Worktree Git",
    };

    let inner = render_card(frame, area, "󰹹", title, theme.status_a, theme);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Help text
            Constraint::Length(3), // Path Input box
            Constraint::Length(3), // Branch Input box
            Constraint::Min(1), // Branch suggestions
            Constraint::Length(2), // Error message if any
            Constraint::Length(1), // Footer shortcuts
        ])
        .split(inner);

    let desc = match language {
        Language::En => "Choose an existing branch or type a new name.",
        Language::Pt => "Escolha uma branch existente ou digite um novo nome.",
    };
    frame.render_widget(Paragraph::new(desc).style(Style::default().fg(theme.line_num_fg)), chunks[0]);

    // Path Box
    let path_border_style = if state.active_field == 0 {
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };
    let path_title = match language {
        Language::En => " Directory ",
        Language::Pt => " Diretório ",
    };
    let path_block = Block::default()
        .title(path_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(path_border_style)
        .style(Style::default().bg(theme.bg));
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
        Language::En => " Branch ",
        Language::Pt => " Branch ",
    };
    let branch_block = Block::default()
        .title(branch_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(branch_border_style)
        .style(Style::default().bg(theme.bg));
    let branch_inner = branch_block.inner(chunks[2]);
    frame.render_widget(branch_block, chunks[2]);
    let branch_cursor = if state.active_field == 1 { "█" } else { "" };
    let branch_line = Line::from(vec![
        Span::styled(&state.branch_input, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Span::styled(branch_cursor, Style::default().fg(theme.key_fg)),
    ]);
    frame.render_widget(Paragraph::new(branch_line), branch_inner);

    let candidates = state.candidates();
    let height = chunks[3].height as usize;
    let start = state.selected_candidate.saturating_sub(height.saturating_sub(1));
    let mut suggestions: Vec<Line> = candidates.iter().enumerate().skip(start).take(height)
        .map(|(idx, branch)| Line::styled(format!(" {} {}", if idx == state.selected_candidate { "›" } else { " " }, branch),
            if idx == state.selected_candidate && state.active_field == 1 {
                Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
            } else { Style::default().fg(theme.line_num_fg) })).collect();
    if !state.branch_input.is_empty() && !state.branches.contains(&state.branch_input) && suggestions.len() < height {
        suggestions.push(Line::styled(match language {
            Language::En => format!(" + New branch: {}", state.branch_input),
            Language::Pt => format!(" + Nova branch: {}", state.branch_input),
        }, Style::default().fg(theme.status_a)));
    }
    frame.render_widget(Paragraph::new(suggestions), chunks[3]);

    // Error display
    if let Some(err) = &state.error_msg {
        let err_line = Line::from(vec![
            Span::styled(format!(" ✖ {}", err), Style::default().fg(theme.del_fg).add_modifier(Modifier::BOLD)),
        ]);

        frame.render_widget(Paragraph::new(err_line), chunks[4]);
    }

    let footer_items: &[(&str, &str)] = match language {
        Language::En => &[("enter", "create & switch"), ("esc", "cancel"), ("tab", "complete / field"), ("↑↓", "select")],
        Language::Pt => &[("enter", "criar e alternar"), ("esc", "cancelar"), ("tab", "completar / campo"), ("↑↓", "seleção")],
    };
    let footer_line = help_line(footer_items, chunks[5].width, theme);
    frame.render_widget(Paragraph::new(footer_line), chunks[5]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autocomplete_preserves_manual_path() {
        let mut state = WorktreeCreationState::new(".worktree/".into(), vec!["main".into(), "feature/ui".into()]);
        state.branch_input = "ui".into();
        state.update_branch();
        assert_eq!(state.candidates(), vec!["feature/ui"]);
        state.complete_branch();
        assert_eq!(state.path_input, ".worktree/feature-ui");
        state.path_manual = true;
        state.path_input = "../custom".into();
        state.branch_input = "new-branch".into();
        state.update_branch();
        assert!(state.candidates().is_empty());
        assert_eq!(state.path_input, "../custom");
    }

    #[test]
    fn creation_renders_in_small_terminal() {
        let backend = ratatui::backend::TestBackend::new(60, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = WorktreeCreationState::new(".worktree/".into(), vec!["main".into()]);
        let theme = crate::ui::theme::Theme::vscode_dark();
        terminal.draw(|frame| render_worktree_popup(frame, frame.area(), &[], 0, 0, Some(&state), Language::Pt, &theme)).unwrap();
        let content: String = terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect();
        assert!(content.contains("main"));
        assert!(content.contains(".worktree/"));
        assert!(content.contains("cancelar"));
    }
}
