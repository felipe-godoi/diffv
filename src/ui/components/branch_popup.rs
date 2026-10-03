use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::Language;
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchItem {
    pub branch: Option<String>,
    pub label: String,
    pub description: String,
    pub is_remote: bool,
    pub is_default: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct BranchSelectorState {
    pub branches: Vec<String>,
    pub filter: String,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub current_target: Option<String>,
}

impl BranchSelectorState {
    pub fn new(branches: Vec<String>, current_target: Option<String>) -> Self {
        Self {
            branches,
            filter: String::new(),
            selected_idx: 0,
            scroll_offset: 0,
            current_target,
        }
    }

    pub fn filtered_items(&self, language: Language) -> Vec<BranchItem> {
        let q = self.filter.trim().to_lowercase();
        let mut items = Vec::new();

        // 1. Default (HEAD) option
        let default_label = match language {
            Language::En => "Default (HEAD)".to_string(),
            Language::Pt => "Padrão (HEAD)".to_string(),
        };
        let default_desc = match language {
            Language::En => "Working tree vs repo HEAD (staged/changes)".to_string(),
            Language::Pt => {
                "Diff do worktree contra o HEAD do repositório (staged/mudanças)".to_string()
            }
        };

        let matches_default = q.is_empty()
            || "default".contains(&q)
            || "padrao".contains(&q)
            || "padrão".contains(&q)
            || "head".contains(&q)
            || default_label.to_lowercase().contains(&q);

        if matches_default {
            items.push(BranchItem {
                branch: None,
                label: default_label,
                description: default_desc,
                is_remote: false,
                is_default: true,
                is_active: self.current_target.is_none(),
            });
        }

        // 2. Branches (local branches first, then remote branches)
        let mut local_branches = Vec::new();
        let mut remote_branches = Vec::new();

        for b in &self.branches {
            if q.is_empty() || b.to_lowercase().contains(&q) {
                if b.starts_with("origin/") || b.contains('/') {
                    remote_branches.push(b.clone());
                } else {
                    local_branches.push(b.clone());
                }
            }
        }

        for b in local_branches {
            let is_active = self.current_target.as_deref() == Some(&b);
            let description = match language {
                Language::En => "Local branch".to_string(),
                Language::Pt => "Branch local".to_string(),
            };
            items.push(BranchItem {
                branch: Some(b.clone()),
                label: b,
                description,
                is_remote: false,
                is_default: false,
                is_active,
            });
        }

        for b in remote_branches {
            let is_active = self.current_target.as_deref() == Some(&b);
            let description = match language {
                Language::En => "Remote branch".to_string(),
                Language::Pt => "Branch remota".to_string(),
            };
            items.push(BranchItem {
                branch: Some(b.clone()),
                label: b,
                description,
                is_remote: true,
                is_default: false,
                is_active,
            });
        }

        items
    }

    pub fn select_current_target(&mut self, language: Language) {
        let items = self.filtered_items(language);
        if let Some(pos) = items.iter().position(|it| it.is_active) {
            self.selected_idx = pos;
            if self.selected_idx >= 8 {
                self.scroll_offset = self.selected_idx.saturating_sub(4);
            }
        }
    }

    pub fn selected_branch(&self, language: Language) -> Option<String> {
        let items = self.filtered_items(language);
        items
            .get(self.selected_idx)
            .and_then(|it| it.branch.clone())
    }

    pub fn update_filter(&mut self) {
        self.selected_idx = 0;
        self.scroll_offset = 0;
    }

    pub fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
            if self.selected_idx < self.scroll_offset {
                self.scroll_offset = self.selected_idx;
            }
        }
    }

    pub fn move_down(&mut self, total: usize) {
        if total > 0 && self.selected_idx + 1 < total {
            self.selected_idx += 1;
        }
    }

    pub fn move_up_by(&mut self, amount: usize) {
        self.selected_idx = self.selected_idx.saturating_sub(amount);
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        }
    }

    pub fn move_down_by(&mut self, amount: usize, total: usize) {
        if total > 0 {
            self.selected_idx = (self.selected_idx + amount).min(total.saturating_sub(1));
        }
    }
}

pub fn render_branch_popup(
    frame: &mut Frame,
    area: Rect,
    state: &BranchSelectorState,
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(76, 70, area);

    let title = match language {
        Language::En => "Compare Worktree with Branch",
        Language::Pt => "Comparar Worktree com Branch",
    };

    let inner = render_card(frame, popup_area, "󰊢", title, theme.header_fg, theme);
    if inner.height < 6 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search box
            Constraint::Min(4),    // Branch list
            Constraint::Length(1), // Footer shortcuts
        ])
        .split(inner);

    let search_area = chunks[0];
    let list_area = chunks[1];
    let footer_area = chunks[2];

    // 1. Search Box
    let search_title = match language {
        Language::En => "  Search / Filter Branch ",
        Language::Pt => "  Buscar / Filtrar Branch ",
    };
    let placeholder = match language {
        Language::En => "Type to filter branches (e.g. main, feat, origin)...",
        Language::Pt => "Digite para filtrar branches (ex: main, feat, origin)...",
    };

    let search_block = Block::default()
        .title(Span::styled(
            search_title,
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.header_fg));

    let search_text = if state.filter.is_empty() {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                placeholder,
                Style::default()
                    .fg(theme.line_num_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(" █", Style::default().fg(theme.header_fg)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                &state.filter,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(theme.header_fg)),
        ])
    };

    let search_paragraph = Paragraph::new(search_text).block(search_block);
    frame.render_widget(search_paragraph, search_area);

    // 2. Branch List
    let items = state.filtered_items(language);
    let max_rows = list_area.height as usize;
    let total_items = items.len();

    if total_items == 0 {
        let empty_msg = match language {
            Language::En => "  No branches match the filter.",
            Language::Pt => "  Nenhuma branch encontrada para o filtro.",
        };
        let p = Paragraph::new(empty_msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(p, list_area);
    } else {
        // Adjust scroll offset
        let scroll_offset = if state.selected_idx >= state.scroll_offset + max_rows {
            state.selected_idx.saturating_sub(max_rows - 1)
        } else if state.selected_idx < state.scroll_offset {
            state.selected_idx
        } else {
            state.scroll_offset
        };

        let start_idx = scroll_offset;
        let end_idx = (scroll_offset + max_rows).min(total_items);

        let mut lines = Vec::new();
        for idx in start_idx..end_idx {
            let item = &items[idx];
            let is_selected = idx == state.selected_idx;

            let cursor_span = if is_selected {
                Span::styled(
                    " ❯ ",
                    Style::default()
                        .fg(theme.key_fg)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("   ")
            };

            let (icon, icon_color) = if item.is_default {
                ("󰁨 ", theme.status_u)
            } else if item.is_remote {
                ("󰊢 ", theme.line_num_fg)
            } else {
                ("󰊢 ", theme.key_fg)
            };

            let icon_span = Span::styled(icon, Style::default().fg(icon_color));

            let (badge, badge_style) = if item.is_default {
                (
                    " [DEFAULT] ",
                    Style::default()
                        .fg(theme.status_u)
                        .add_modifier(Modifier::BOLD),
                )
            } else if item.is_remote {
                (" [REMOTE]  ", Style::default().fg(theme.line_num_fg))
            } else {
                (" [LOCAL]   ", Style::default().fg(theme.add_fg))
            };

            let name_style = if is_selected {
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };

            let active_indicator = if item.is_active {
                match language {
                    Language::En => Span::styled(
                        "✓ ",
                        Style::default()
                            .fg(theme.add_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Language::Pt => Span::styled(
                        "✓ ",
                        Style::default()
                            .fg(theme.add_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                }
            } else {
                Span::raw("  ")
            };

            let desc_span = Span::styled(
                format!(" · {}", item.description),
                Style::default()
                    .fg(theme.line_num_fg)
                    .add_modifier(Modifier::DIM),
            );

            let row_style = if is_selected {
                Style::default().bg(theme.header_bg)
            } else {
                Style::default()
            };

            lines.push(
                Line::from(vec![
                    cursor_span,
                    active_indicator,
                    icon_span,
                    Span::styled(badge, badge_style),
                    Span::styled(&item.label, name_style),
                    desc_span,
                ])
                .style(row_style),
            );
        }

        let p = Paragraph::new(lines);
        frame.render_widget(p, list_area);
    }

    // 3. Footer Help Bar
    let shortcuts = match language {
        Language::En => [
            ("Enter", "Compare Worktree"),
            ("↑/↓", "Navigate"),
            ("Esc", "Cancel"),
            ("Bksp", "Clear search"),
        ],
        Language::Pt => [
            ("Enter", "Comparar Worktree"),
            ("↑/↓", "Navegar"),
            ("Esc", "Cancelar"),
            ("Bksp", "Apagar busca"),
        ],
    };
    frame.render_widget(
        Paragraph::new(help_line(&shortcuts, footer_area.width, theme))
            .alignment(Alignment::Center),
        footer_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_selector_filtering_and_selection() {
        let branches = vec![
            "main".to_string(),
            "feature/auth".to_string(),
            "origin/main".to_string(),
            "origin/feature/auth".to_string(),
        ];

        let mut state = BranchSelectorState::new(branches, None);
        let items = state.filtered_items(Language::En);
        // Default (HEAD) + 2 local + 2 remote = 5 items
        assert_eq!(items.len(), 5);
        assert!(items[0].is_default);
        assert!(items[0].is_active);

        // Filter by "auth"
        state.filter = "auth".to_string();
        let filtered = state.filtered_items(Language::En);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].label, "feature/auth");
        assert_eq!(filtered[1].label, "origin/feature/auth");

        // Filter by "head"
        state.filter = "head".to_string();
        let filtered = state.filtered_items(Language::En);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].is_default);

        // Select branch
        state.filter = String::new();
        state.selected_idx = 1;
        assert_eq!(
            state.selected_branch(Language::En),
            Some("main".to_string())
        );
        state.selected_idx = 2;
        assert_eq!(
            state.selected_branch(Language::En),
            Some("feature/auth".to_string())
        );

        state.selected_idx = 0;
        assert_eq!(state.selected_branch(Language::En), None);
    }
}
