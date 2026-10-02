use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::core::models::Language;
use crate::ui::theme::Theme;

pub fn render_help_popup(frame: &mut Frame, area: Rect, language: Language, theme: &Theme) {
    let popup_area = centered_rect(80, 85, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let title = match language {
        Language::En => " 󰋖 Keyboard Shortcuts  ·  [Esc] or [?] to close ",
        Language::Pt => " 󰋖 Atalhos de Teclado  ·  [Esc] ou [?] para fechar ",
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

    let sections: Vec<(&str, Vec<(&str, &str)>)> = match language {
        Language::En => vec![
            ("󰌌  Navigation & Scrolling", vec![
                ("j / k or ↓ / ↑", "Scroll lines down / up"),
                ("J / K or Ctrl+d / Ctrl+u", "Scroll half page down / up"),
                ("] or n  /  [ or p", "Jump to Next / Previous hunk"),
                ("Tab", "Switch focus between File Tree and Diff View"),
                ("h / l or ← / →", "Diff: switch column (Old/New) · Tree: collapse/expand"),
                ("Mouse Wheel / Click", "Smooth scroll and select files / rows / tabs"),
            ]),
            ("󰦨  Staging & Git Operations", vec![
                ("s", "Stage hunk under cursor (or selected lines in Visual mode)"),
                ("u", "Unstage hunk under cursor"),
                ("d", "Discard hunk under cursor (with confirmation)"),
                ("S / U / D", "Stage / Unstage / Discard entire file"),
                ("v", "Toggle Visual Mode for line-by-line partial staging"),
                ("H", "View commit History for active file"),
                ("W", "Open Git Worktrees Switcher modal"),
            ]),
            ("󰈚  View & File Tree", vec![
                ("1 / 2 / 3", "Switch Drawer Tab: [1] Changes  [2] Commits  [3] Stashes"),
                ("t", "Toggle Folders (Tree) ↔ Flat List"),
                ("< / > or , / .", "Resize File Drawer width"),
                ("/", "Filter files by path or extension (fuzzy search)"),
                ("m", "Toggle Side-by-Side ↔ Unified view mode"),
                ("w", "Toggle AI live file watching mode (auto-reload)"),
            ]),
            ("󰒅  Integrations & System", vec![
                ("Enter / e", "Open file in Neovim / $EDITOR at cursor line (+line)"),
                ("c", "Copy hunk to system clipboard as Markdown"),
                ("L", "Toggle Language (English ↔ Português)"),
                ("? ", "Show / hide this help modal"),
                ("q / Esc", "Quit diffv (or dismiss modal / visual mode)"),
            ]),
        ],
        Language::Pt => vec![
            ("󰌌  Navegação & Rolagem", vec![
                ("j / k ou ↓ / ↑", "Rolar linhas para baixo / cima"),
                ("J / K ou Ctrl+d / Ctrl+u", "Rolar meia página para baixo / cima"),
                ("] ou n  /  [ ou p", "Ir para Próximo / Anterior hunk"),
                ("Tab", "Alternar foco entre Árvore de Arquivos e Diff"),
                ("h / l ou ← / →", "Diff: focar coluna (Old/New) · Árvore: fechar/abrir"),
                ("Roda do Mouse / Clique", "Rolagem suave e seleção de arquivos / linhas / abas"),
            ]),
            ("󰦨  Staging & Operações Git", vec![
                ("s", "Preparar (stage) hunk atual (ou linhas no modo Visual)"),
                ("u", "Despreparar (unstage) hunk atual"),
                ("d", "Descartar alterações do hunk atual (com confirmação)"),
                ("S / U / D", "Preparar / Despreparar / Descartar arquivo inteiro"),
                ("v", "Alternar Modo Visual para staging parcial linha por linha"),
                ("H", "Ver Histórico de commits do arquivo ativo"),
                ("W", "Abrir Alternador de Worktrees Git"),
            ]),
            ("󰈚  Visualização & Árvore", vec![
                ("1 / 2 / 3", "Alternar Abas: [1] Mudanças  [2] Commits  [3] Stashes"),
                ("t", "Alternar entre Pastas (Tree) ↔ Lista Plana"),
                ("< / > ou , / .", "Redimensionar largura do painel lateral"),
                ("/", "Filtrar arquivos por caminho ou extensão (busca rápida)"),
                ("m", "Alternar modo Side-by-Side ↔ Unificado"),
                ("w", "Alternar modo Watch ao vivo para agentes de IA"),
            ]),
            ("󰒅  Integrações & Sistema", vec![
                ("Enter / e", "Abrir arquivo no Neovim / $EDITOR na linha (+line)"),
                ("c", "Copiar hunk em Markdown para a área de transferência"),
                ("L", "Alternar Idioma (English ↔ Português)"),
                ("? ", "Exibir / ocultar este modal de ajuda"),
                ("q / Esc", "Sair do diffv (ou fechar modal / modo visual)"),
            ]),
        ],
    };

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
