use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::core::models::Language;
use crate::ui::theme::Theme;

pub fn render_help_popup(frame: &mut Frame, area: Rect, language: Language, theme: &Theme) {
    let popup_area = centered_rect(88, 86, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let title = match language {
        Language::En => " 󰋖 Keyboard Shortcuts & Neovim Motions  ·  [Esc] or [?] to close ",
        Language::Pt => " 󰋖 Atalhos de Teclado & Comandos Neovim  ·  [Esc] ou [?] para fechar ",
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.header_bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let (left_sections, right_sections): (Vec<(&str, Vec<(&str, &str)>)>, Vec<(&str, Vec<(&str, &str)>)>) = match language {
        Language::En => (
            vec![
                ("󰌌  Neovim Motions & Diff Navigation", vec![
                    ("j / k or ↓ / ↑", "Move down / up line by line"),
                    ("Ctrl+d / Ctrl+u", "Move half page down / up"),
                    ("Ctrl+f / Ctrl+b", "Move full page down / up (PageDown/Up)"),
                    ("gg / G", "Jump to top / bottom of diff or tree"),
                    ("]c / [c (or ] / [)", "Jump to next / previous hunk"),
                    ("n / N (or p)", "Next / previous hunk"),
                    ("zz / zt / zb", "Center cursor / top / bottom of screen"),
                    ("H / M / L", "Jump cursor to High / Middle / Low of screen"),
                    ("Tab", "Switch focus: File Tree ↔ Diff View"),
                    ("Mouse Wheel / Click", "Smooth scroll and select files / rows"),
                ]),
                ("󰦨  Staging & Git Operations", vec![
                    ("s", "Stage hunk (or selected lines in Visual mode)"),
                    ("u", "Unstage hunk under cursor"),
                    ("d", "Discard hunk under cursor (with prompt)"),
                    ("S / U / D", "Stage / Unstage / Discard entire file"),
                    ("v", "Toggle Visual Mode (line-by-line selection)"),
                    ("H / gh", "View commit History for active file"),
                    ("W", "Open Git Worktrees Switcher modal"),
                ]),
            ],
            vec![
                ("󰈚  View, Drawer & Details", vec![
                    ("1 / 2 / 3", "Switch Drawer Tab: [1] Changes [2] Commits [3] Stashes"),
                    ("b", "Toggle File Drawer sidebar visible ↔ hidden"),
                    ("o", "Toggle Folders (Tree) ↔ Flat List view"),
                    ("< / > or , / .", "Resize File Drawer sidebar width"),
                    ("/", "Filter files by path or extension (fuzzy search)"),
                    ("i", "Open verbose Details Popup (Commit / File / Stash)"),
                    ("m", "Toggle Side-by-Side ↔ Unified diff mode"),
                    ("w", "Toggle AI live file watching (auto-reload)"),
                ]),
                ("󰒅  Integrations & System", vec![
                    ("Enter / e", "Open in Neovim / $EDITOR at cursor line (+line)"),
                    ("c", "Copy hunk to system clipboard as Markdown"),
                    ("L", "Toggle Language (English ↔ Português)"),
                    ("? ", "Show / hide this shortcuts cheat-sheet"),
                    ("q / Esc", "Quit diffv (or dismiss modal / visual mode)"),
                ]),
            ],
        ),
        Language::Pt => (
            vec![
                ("󰌌  Movimentação Neovim & Navegação", vec![
                    ("j / k ou ↓ / ↑", "Mover linha por linha para baixo / cima"),
                    ("Ctrl+d / Ctrl+u", "Meia página para baixo / cima"),
                    ("Ctrl+f / Ctrl+b", "Página inteira para baixo / cima (PageDown/Up)"),
                    ("gg / G", "Saltar para início / fim do diff ou lista"),
                    ("]c / [c (ou ] / [)", "Ir para próximo / anterior hunk"),
                    ("n / N (ou p)", "Próximo / anterior hunk"),
                    ("zz / zt / zb", "Centralizar cursor / topo / base da tela"),
                    ("H / M / L", "Mover cursor para Topo / Meio / Base da tela"),
                    ("Tab", "Alternar foco: Árvore de Arquivos ↔ Diff"),
                    ("Roda do Mouse / Clique", "Rolagem suave e seleção de arquivos / linhas"),
                ]),
                ("󰦨  Staging & Operações Git", vec![
                    ("s", "Preparar (stage) hunk ou linhas no modo Visual"),
                    ("u", "Despreparar (unstage) hunk atual"),
                    ("d", "Descartar alterações do hunk atual (com confirmação)"),
                    ("S / U / D", "Preparar / Despreparar / Descartar arquivo inteiro"),
                    ("v", "Alternar Modo Visual para seleção linha a linha"),
                    ("H / gh", "Ver Histórico de commits do arquivo ativo"),
                    ("W", "Abrir Alternador de Worktrees Git"),
                ]),
            ],
            vec![
                ("󰈚  Visualização, Painel & Detalhes", vec![
                    ("1 / 2 / 3", "Alternar Abas: [1] Mudanças [2] Commits [3] Stashes"),
                    ("b", "Exibir ↔ ocultar painel lateral (sidebar)"),
                    ("o", "Alternar entre Pastas (Tree) ↔ Lista Plana"),
                    ("< / > ou , / .", "Redimensionar largura do painel lateral"),
                    ("/", "Filtrar arquivos por caminho ou extensão"),
                    ("i", "Abrir Popup de Detalhes (Commit / Arquivo / Stash)"),
                    ("m", "Alternar modo Side-by-Side ↔ Unificado"),
                    ("w", "Alternar modo Watch ao vivo para agentes IA"),
                ]),
                ("󰒅  Integrações & Sistema", vec![
                    ("Enter / e", "Abrir arquivo no Neovim / $EDITOR na linha (+line)"),
                    ("c", "Copiar hunk em Markdown para clipboard"),
                    ("L", "Alternar Idioma (English ↔ Português)"),
                    ("? ", "Exibir / ocultar esta ajuda de atalhos"),
                    ("q / Esc", "Sair do diffv (ou fechar modal / modo visual)"),
                ]),
            ],
        ),
    };

    if inner.width >= 86 {
        // Dual column layout
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(inner);

        let left_lines = build_section_lines(&left_sections, theme, 22);
        frame.render_widget(Paragraph::new(left_lines), cols[0]);

        let right_lines = build_section_lines(&right_sections, theme, 20);
        frame.render_widget(Paragraph::new(right_lines), cols[1]);
    } else {
        // Single column layout
        let mut combined = left_sections;
        combined.extend(right_sections);
        let lines = build_section_lines(&combined, theme, 18);
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn build_section_lines(
    sections: &[(&str, Vec<(&str, &str)>)],
    theme: &Theme,
    key_pad: usize,
) -> Vec<Line<'static>> {
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
                Span::styled(format!("    {:<width$}", key, width = key_pad), Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
                Span::styled(desc.to_string(), Style::default().fg(theme.fg)),
            ]));
        }
        lines.push(Line::from(""));
    }
    lines
}

pub fn render_confirm_popup(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(55, 25, area);
    frame.render_widget(Clear, popup_area);

    let title = match language {
        Language::En => " ⚠ Confirm Action ",
        Language::Pt => " ⚠ Confirmar Ação ",
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.del_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.header_bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let (confirm_btn, cancel_btn) = match language {
        Language::En => (" [y] Confirm ", " [n / Esc] Cancel "),
        Language::Pt => (" [y] Confirmar ", " [n / Esc] Cancelar "),
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(confirm_btn, Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_a).add_modifier(Modifier::BOLD)),
            Span::raw("    "),
            Span::styled(cancel_btn, Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.status_d).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
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
