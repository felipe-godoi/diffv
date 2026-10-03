use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::core::models::Language;
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

type Section<'a> = (&'a str, Vec<(&'a str, &'a str)>);

pub fn render_help_popup(frame: &mut Frame, area: Rect, language: Language, theme: &Theme) {
    let popup_area = centered_rect(88, 86, area);

    let (title, footer) = match language {
        Language::En => (
            "Keyboard Shortcuts & Neovim Motions",
            [("esc / ? / q", "close")],
        ),
        Language::Pt => (
            "Atalhos de Teclado & Comandos Neovim",
            [("esc / ? / q", "fechar")],
        ),
    };

    let card = render_card(frame, popup_area, "󰋖", title, theme.header_fg, theme);
    if card.height < 3 {
        return;
    }
    let [body, footer_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(card);
    frame.render_widget(
        Paragraph::new(help_line(&footer, footer_area.width, theme)).alignment(Alignment::Center),
        footer_area,
    );
    let inner = Rect {
        y: body.y + 1,
        height: body.height.saturating_sub(1),
        ..body
    };

    let (left_sections, right_sections): (Vec<Section>, Vec<Section>) = match language {
        Language::En => (
            vec![
                (
                    "󰌌  Neovim Motions & Diff Navigation",
                    vec![
                        ("j / k or ↓ / ↑", "Move down / up line by line"),
                        ("Ctrl+e / Ctrl+y", "Scroll viewport down / up 1 line (Vim)"),
                        ("Ctrl+d / Ctrl+u", "Move half page down / up"),
                        ("PageDown / Ctrl+b", "Move full page down / up (PageUp)"),
                        ("gg / G", "Jump to top / bottom of diff or tree"),
                        ("]c / [c (or ] / [)", "Jump to next / previous hunk"),
                        ("n / N (or p)", "Next / previous hunk"),
                        ("zz / zt / zb", "Center cursor / top / bottom of screen"),
                        ("H / M / L", "Jump cursor to High / Middle / Low of screen"),
                        (
                            "Tab / 1,2,3",
                            "Cycle drawer tabs: Changes ↔ Commits ↔ Stashes",
                        ),
                        ("Enter / Esc", "Focus Diff View / Return to Drawer"),
                        ("Mouse Drag", "Resize panes divider / click to select"),
                    ],
                ),
                (
                    "󰦨  Staging & Git Operations",
                    vec![
                        ("s", "Stage hunk / Visual lines (file under Changes)"),
                        ("u", "Unstage hunk / Visual lines (file under Staged)"),
                        ("d", "Discard hunk in Changes (with prompt)"),
                        ("S / U / D", "Stage / Unstage / Discard entire file"),
                        ("v", "Toggle Visual Mode (line-by-line selection)"),
                        ("H / gh", "View commit History for active file"),
                        ("o", "Open the commit's GitHub PR (commits / history)"),
                        ("B", "Branch compare selector (worktree vs branch)"),
                        ("W", "Worktrees modal (filter & switch, Ctrl+n to create)"),
                    ],
                ),
            ],
            vec![
                (
                    "󰈚  View, Drawer & Fuzzy Search",
                    vec![
                        (
                            "Ctrl+p",
                            "Find file in current scope: changes / commit / stash",
                        ),
                        ("Ctrl+f", "Search text in file / commit / changes (fzf)"),
                        ("b", "Toggle File Drawer sidebar visible ↔ hidden"),
                        ("t", "Toggle Folders (Tree) ↔ Flat List view"),
                        ("< / > or , / .", "Resize File Drawer sidebar width"),
                        ("/", "Inline filter files by path"),
                        ("i", "Open verbose Details Popup (Commit / File / Stash)"),
                        ("Tab", "In diff: switch OLD / NEW column"),
                        ("h/l · ←/→", "Scroll text horizontally (gutters stay fixed)"),
                        ("0 / $", "Horizontal start / end"),
                        ("r", "Toggle line wrap (default ON)"),
                        ("x", "Expand full file ↔ changes only"),
                        ("m", "Toggle Side-by-Side ↔ Unified diff mode"),
                        ("w", "Toggle AI live file watching (auto-reload)"),
                    ],
                ),
                (
                    "󰒅  Integrations & System",
                    vec![
                        ("e", "Open in Neovim / $EDITOR at cursor line (+line)"),
                        ("c", "Copy hunk to system clipboard as Markdown"),
                        ("C", "Open Settings (Auto-update, Beta channel, Theme...)"),
                        ("L / F2", "Toggle Language (English ↔ Português)"),
                        ("? ", "Show / hide this shortcuts cheat-sheet"),
                        ("Esc", "Return / close modal / cancel (never quits)"),
                        ("q", "Return to Changes / close modal / quit at root"),
                        ("Shift+Q", "Force quit immediately from anywhere"),
                    ],
                ),
            ],
        ),
        Language::Pt => (
            vec![
                (
                    "󰌌  Movimentação Neovim & Navegação",
                    vec![
                        ("j / k ou ↓ / ↑", "Mover linha por linha para baixo / cima"),
                        (
                            "Ctrl+e / Ctrl+y",
                            "Rolar viewport 1 linha abaixo / acima (Vim)",
                        ),
                        ("Ctrl+d / Ctrl+u", "Meia página para baixo / cima"),
                        (
                            "PageDown / Ctrl+b",
                            "Página inteira para baixo / cima (PageUp)",
                        ),
                        ("gg / G", "Saltar para início / fim do diff ou lista"),
                        ("]c / [c (ou ] / [)", "Ir para próximo / anterior hunk"),
                        ("n / N (ou p)", "Próximo / anterior hunk"),
                        ("zz / zt / zb", "Centralizar cursor / topo / base da tela"),
                        ("H / M / L", "Mover cursor para Topo / Meio / Base da tela"),
                        ("Tab / 1,2,3", "Ciclar abas: Mudanças ↔ Commits ↔ Stashes"),
                        ("Enter / Esc", "Focar Diff / Retornar ao painel lateral"),
                        (
                            "Arrastar Mouse",
                            "Redimensionar divisor / clique para selecionar",
                        ),
                    ],
                ),
                (
                    "󰦨  Staging & Operações Git",
                    vec![
                        ("s", "Stage do hunk / linhas Visual (arquivo em Mudanças)"),
                        ("u", "Unstage do hunk / linhas Visual (arquivo em Staged)"),
                        ("d", "Descartar hunk em Mudanças (com confirmação)"),
                        (
                            "S / U / D",
                            "Preparar / Despreparar / Descartar arquivo inteiro",
                        ),
                        ("v", "Alternar Modo Visual para seleção linha a linha"),
                        ("H / gh", "Ver Histórico de commits do arquivo ativo"),
                        ("o", "Abrir o PR do commit no GitHub (commits / histórico)"),
                        ("B", "Seletor de branch (comparar worktree com branch)"),
                        (
                            "W",
                            "Modal de Worktrees (filtrar e alternar, Ctrl+n para criar)",
                        ),
                    ],
                ),
            ],
            vec![
                (
                    "󰈚  Visualização, Painel & Busca Fuzzy",
                    vec![
                        (
                            "Ctrl+p",
                            "Buscar arquivo no escopo: mudanças / commit / stash",
                        ),
                        (
                            "Ctrl+f",
                            "Buscar texto no arquivo / commit / mudanças (fzf)",
                        ),
                        ("b", "Exibir ↔ ocultar painel lateral (sidebar)"),
                        ("t", "Alternar entre Pastas (Tree) ↔ Lista Plana"),
                        ("< / > ou , / .", "Redimensionar largura do painel lateral"),
                        ("/", "Filtro rápido de arquivos por caminho"),
                        ("i", "Abrir Popup de Detalhes (Commit / Arquivo / Stash)"),
                        ("Tab", "No diff: alternar coluna OLD / NEW"),
                        ("h/l · ←/→", "Rolar texto horizontalmente"),
                        ("0 / $", "Início / fim horizontal"),
                        ("r", "Alternar quebra de linha (padrão ligado)"),
                        ("x", "Expandir arquivo inteiro ↔ só mudanças"),
                        ("m", "Alternar modo Side-by-Side ↔ Unificado"),
                        ("w", "Alternar modo Watch ao vivo para agentes IA"),
                    ],
                ),
                (
                    "󰒅  Integrações & Sistema",
                    vec![
                        ("e", "Abrir arquivo no Neovim / $EDITOR na linha (+line)"),
                        ("c", "Copiar hunk em Markdown para clipboard"),
                        ("C", "Configurações (Auto-update, Canal Beta, Tema...)"),
                        ("L / F2", "Alternar Idioma (English ↔ Português)"),
                        ("? ", "Exibir / ocultar esta ajuda de atalhos"),
                        ("Esc", "Retornar / fechar modal / cancelar (nunca sai)"),
                        ("q", "Voltar às mudanças / fechar modal / sair na raiz"),
                        ("Shift+Q", "Sair imediatamente de qualquer tela"),
                    ],
                ),
            ],
        ),
    };

    if inner.width >= 86 {
        // Dual column layout
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        let left_lines = build_section_lines(&left_sections, theme);
        frame.render_widget(Paragraph::new(left_lines), cols[0]);

        let right_lines = build_section_lines(&right_sections, theme);
        frame.render_widget(Paragraph::new(right_lines), cols[1]);
    } else {
        // Single column layout
        let mut combined = left_sections;
        combined.extend(right_sections);
        let lines = build_section_lines(&combined, theme);
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn build_section_lines(sections: &[Section], theme: &Theme) -> Vec<Line<'static>> {
    let key_pad = sections
        .iter()
        .flat_map(|(_, bindings)| bindings.iter().map(|(key, _)| key.width()))
        .max()
        .unwrap_or(0)
        + 2;
    let mut lines = Vec::new();
    for (sec_title, bindings) in sections {
        lines.push(Line::from(vec![Span::styled(
            format!("{}  ", sec_title),
            Style::default()
                .fg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        )]));

        for (key, desc) in bindings {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}{}", key, " ".repeat(key_pad - key.width())),
                    Style::default()
                        .fg(theme.key_fg)
                        .add_modifier(Modifier::BOLD),
                ),
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
    let title = match language {
        Language::En => "Confirm Action",
        Language::Pt => "Confirmar Ação",
    };
    let inner = render_card(frame, popup_area, "", title, theme.del_fg, theme);

    let (confirm_btn, cancel_btn) = match language {
        Language::En => ("  y  Confirm  ", "  n / esc  Cancel  "),
        Language::Pt => ("  y  Confirmar  ", "  n / esc  Cancelar  "),
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                confirm_btn,
                Style::default()
                    .fg(theme.text_on(theme.del_fg))
                    .bg(theme.del_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                cancel_btn,
                Style::default().fg(theme.fg).bg(theme.selected_bg),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}
