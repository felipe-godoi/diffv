use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Wrap};
use ratatui::Frame;

use crate::config::{Config, UpdateChannel};
use crate::core::models::Language;
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    AutoUpdate,
    UpdateChannel,
    Theme,
    DefaultView,
    LineNumbers,
    OverviewRuler,
    TabWidth,
    DiffAlgorithm,
    IgnoreWhitespace,
    WatcherEnabled,
}

pub const SETTING_ITEMS: &[SettingItem] = &[
    SettingItem::AutoUpdate,
    SettingItem::UpdateChannel,
    SettingItem::Theme,
    SettingItem::DefaultView,
    SettingItem::LineNumbers,
    SettingItem::OverviewRuler,
    SettingItem::TabWidth,
    SettingItem::DiffAlgorithm,
    SettingItem::IgnoreWhitespace,
    SettingItem::WatcherEnabled,
];

pub fn render_settings_popup(
    frame: &mut Frame,
    area: Rect,
    config: &Config,
    selected_idx: usize,
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(86, 84, area);

    let title = match language {
        Language::En => "Project & User Settings (config.toml)",
        Language::Pt => "Configurações do Projeto & Usuário (config.toml)",
    };

    let card = render_card(frame, popup_area, "󰒓", title, theme.header_fg, theme);
    if card.height < 6 {
        return;
    }

    let [header_area, list_area, desc_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(5),
        Constraint::Length(1),
    ])
    .areas(card);

    // 1. Header info banner
    let config_file_path = Config::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/diffv/config.toml".to_string());

    let header_line = Line::from(vec![
        Span::styled(" 󰈚 ", Style::default().fg(theme.key_fg)),
        Span::styled(
            match language {
                Language::En => format!("Config file: {}  (Changes persist automatically)", config_file_path),
                Language::Pt => format!("Arquivo de config: {}  (Alterações persistem automaticamente)", config_file_path),
            },
            Style::default().fg(theme.line_num_fg).add_modifier(Modifier::DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(header_line), header_area);

    // 2. Settings list
    let selected_item = SETTING_ITEMS.get(selected_idx).copied().unwrap_or(SettingItem::AutoUpdate);
    let mut list_lines = Vec::new();

    for (idx, &item) in SETTING_ITEMS.iter().enumerate() {
        let is_selected = idx == selected_idx;
        let (icon, label, value_str, category_badge) = match item {
            SettingItem::AutoUpdate => {
                let badge = match language { Language::En => "UPDATE", Language::Pt => "ATUALIZAÇÃO" };
                let (val, style_fg) = if config.update.auto_update {
                    (match language { Language::En => "[ ✓ Enabled ]", Language::Pt => "[ ✓ Ativado ]" }, theme.add_fg)
                } else {
                    (match language { Language::En => "[ ✗ Disabled ]", Language::Pt => "[ ✗ Desativado ]" }, theme.del_fg)
                };
                ("󰚰", match language { Language::En => "Automatic Updates (Startup check)", Language::Pt => "Atualizações Automáticas (Auto-Update)" }, val, (badge, style_fg))
            }
            SettingItem::UpdateChannel => {
                let badge = match language { Language::En => "CHANNEL", Language::Pt => "CANAL" };
                let (val, style_fg) = match config.update.channel {
                    UpdateChannel::Stable => (
                        match language { Language::En => "[ Stable (releases) ]", Language::Pt => "[ Stable (estável) ]" },
                        theme.status_a,
                    ),
                    UpdateChannel::Beta => (
                        match language { Language::En => "[ Beta (latest main build) ]", Language::Pt => "[ Beta (última build da main) ]" },
                        theme.header_fg,
                    ),
                };
                ("󰏤", match language { Language::En => "Update Channel (Opt-in Beta)", Language::Pt => "Canal de Atualização (Opt-in Beta)" }, val, (badge, style_fg))
            }
            SettingItem::Theme => {
                ("󰔎", match language { Language::En => "Color Theme", Language::Pt => "Tema de Cores" }, config.ui.theme.as_str(), ("UI", theme.key_fg))
            }
            SettingItem::DefaultView => {
                let val = match config.ui.default_view.as_str() {
                    "unified" => match language { Language::En => "[ Unified ]", Language::Pt => "[ Unificado ]" },
                    _ => match language { Language::En => "[ Side-by-Side ]", Language::Pt => "[ Lado a Lado ]" },
                };
                ("󰤈", match language { Language::En => "Default View Mode", Language::Pt => "Modo de Exibição Inicial" }, val, ("UI", theme.key_fg))
            }
            SettingItem::LineNumbers => {
                let val = if config.ui.show_line_numbers {
                    match language { Language::En => "[ ✓ Yes ]", Language::Pt => "[ ✓ Sim ]" }
                } else {
                    match language { Language::En => "[ ✗ No ]", Language::Pt => "[ ✗ Não ]" }
                };
                ("󰞋", match language { Language::En => "Show Line Numbers", Language::Pt => "Exibir Números de Linha" }, val, ("UI", theme.line_num_fg))
            }
            SettingItem::OverviewRuler => {
                let val = if config.ui.overview_ruler {
                    match language { Language::En => "[ ✓ Yes ]", Language::Pt => "[ ✓ Sim ]" }
                } else {
                    match language { Language::En => "[ ✗ No ]", Language::Pt => "[ ✗ Não ]" }
                };
                ("󰍉", match language { Language::En => "Overview Ruler Minimap", Language::Pt => "Régua Lateral de Visão Geral" }, val, ("UI", theme.line_num_fg))
            }
            SettingItem::TabWidth => {
                let val = match config.ui.tab_width {
                    2 => "[ 2 spaces ]",
                    8 => "[ 8 spaces ]",
                    _ => "[ 4 spaces ]",
                };
                ("󰌒", match language { Language::En => "Tab Width", Language::Pt => "Largura de Tabulação" }, val, ("UI", theme.key_fg))
            }
            SettingItem::DiffAlgorithm => {
                let val = match config.diff.algorithm.as_str() {
                    "myers" => "[ Myers ]",
                    _ => "[ Patience ]",
                };
                ("󰊢", match language { Language::En => "Diff Algorithm", Language::Pt => "Algoritmo de Diferenças" }, val, ("DIFF", theme.key_fg))
            }
            SettingItem::IgnoreWhitespace => {
                let val = if config.diff.ignore_whitespace {
                    match language { Language::En => "[ ✓ Yes ]", Language::Pt => "[ ✓ Sim ]" }
                } else {
                    match language { Language::En => "[ ✗ No ]", Language::Pt => "[ ✗ Não ]" }
                };
                ("󱁐", match language { Language::En => "Ignore Whitespace by Default", Language::Pt => "Ignorar Espaços por Padrão" }, val, ("DIFF", theme.line_num_fg))
            }
            SettingItem::WatcherEnabled => {
                let val = if config.watcher.enabled {
                    match language { Language::En => "[ ✓ Yes ]", Language::Pt => "[ ✓ Sim ]" }
                } else {
                    match language { Language::En => "[ ✗ No ]", Language::Pt => "[ ✗ Não ]" }
                };
                ("󰐥", match language { Language::En => "Auto Watch Git Repositories", Language::Pt => "File Watcher Automático no Git" }, val, ("WATCH", theme.key_fg))
            }
        };

        let pointer = if is_selected { " ▸ " } else { "   " };
        let pointer_style = if is_selected {
            Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.line_num_fg)
        };

        let label_style = if is_selected {
            Style::default().fg(theme.text_on(theme.selected_bg)).bg(theme.selected_bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };

        let value_style = Style::default().fg(category_badge.1).add_modifier(Modifier::BOLD);

        let row = Line::from(vec![
            Span::styled(pointer, pointer_style),
            Span::styled(format!("{} ", icon), Style::default().fg(theme.key_fg)),
            Span::styled(format!("{:<38}", label), label_style),
            Span::raw(" "),
            Span::styled(value_str, value_style),
        ]);

        list_lines.push(row);
    }

    frame.render_widget(Paragraph::new(list_lines), list_area);

    // 3. Description box for selected item
    let (desc_text, tip_text) = match selected_item {
        SettingItem::AutoUpdate => (
            match language {
                Language::En => "Checks for updates automatically on startup. Opt-out at any time to keep running this exact version offline or undisturbed.",
                Language::Pt => "Verifica e instala atualizações automaticamente ao iniciar o diffv. Desative a qualquer momento para manter esta versão sem verificações de rede.",
            },
            match language {
                Language::En => "Notice: You can also override with 'diffv -n' or 'diffv --auto-update false'.",
                Language::Pt => "Aviso: Você também pode usar 'diffv -n' ou 'diffv --auto-update false' no terminal.",
            }
        ),
        SettingItem::UpdateChannel => (
            match language {
                Language::En => "Update channel. 'Stable' receives validated GitHub releases. 'Beta' automatically fetches the latest development build compiled from the main branch.",
                Language::Pt => "Canal de atualizações. 'Stable' recebe releases oficiais estáveis. 'Beta' recebe a versão mais recente compilada diretamente da branch main.",
            },
            match language {
                Language::En => "Notice: You can also opt-in via CLI with 'diffv -b' or 'diffv --channel beta'.",
                Language::Pt => "Aviso: Você também pode ativar via CLI com 'diffv -b' ou 'diffv --channel beta'.",
            }
        ),
        SettingItem::Theme => (
            match language {
                Language::En => "Color palette and highlight theme (auto, terminal, vscode-dark, tokyonight, catppuccin, gruvbox).",
                Language::Pt => "Tema de cores e esquema visual (auto, terminal, vscode-dark, tokyonight, catppuccin, gruvbox).",
            },
            match language {
                Language::En => "Override in terminal with 'diffv -t <theme>'.",
                Language::Pt => "Sobrescreva no terminal com 'diffv -t <tema>'.",
            }
        ),
        SettingItem::DefaultView => (
            match language {
                Language::En => "Initial view mode when comparing files (Side-by-Side dual column or Unified inline).",
                Language::Pt => "Modo de visualização inicial ao abrir diferenças (Lado a Lado ou Unificado em linha).",
            },
            match language {
                Language::En => "Toggle during viewing anytime with key 'm'.",
                Language::Pt => "Alterne a qualquer momento visualizando um diff com a tecla 'm'.",
            }
        ),
        SettingItem::LineNumbers => (
            match language {
                Language::En => "Display gutter with original line numbers.",
                Language::Pt => "Exibir calha com numeração das linhas originais e modificadas.",
            },
            ""
        ),
        SettingItem::OverviewRuler => (
            match language {
                Language::En => "Mini overview bar on the right margin showing relative positions of diff hunks.",
                Language::Pt => "Barra minimapa na margem direita destacando a posição de todas as adições e deleções.",
            },
            ""
        ),
        SettingItem::TabWidth => (
            match language {
                Language::En => "Number of columns to advance for horizontal tab characters.",
                Language::Pt => "Número de colunas equivalentes para o caractere de tabulação.",
            },
            ""
        ),
        SettingItem::DiffAlgorithm => (
            match language {
                Language::En => "Algorithm used to generate diffs. 'Patience' produces more human-friendly diffs on large refactors.",
                Language::Pt => "Algoritmo de comparação. 'Patience' gera diffs mais limpos em refatorações; 'Myers' é clássico.",
            },
            ""
        ),
        SettingItem::IgnoreWhitespace => (
            match language {
                Language::En => "Ignore all trailing and space-only modifications.",
                Language::Pt => "Ignora alterações que consistem exclusivamente de espaçamento em branco.",
            },
            match language {
                Language::En => "Toggle in terminal with 'diffv -i' or 'diffv -W'.",
                Language::Pt => "Ative no terminal com 'diffv -i' ou 'diffv -W'.",
            }
        ),
        SettingItem::WatcherEnabled => (
            match language {
                Language::En => "When true, diffv automatically starts in live auto-reload watch mode inside Git repos.",
                Language::Pt => "Quando ativo, o diffv inicia automaticamente em modo live watch ao rodar em repositórios Git.",
            },
            match language {
                Language::En => "Toggle live watch mode anytime with key 'w'.",
                Language::Pt => "Alterne o modo live watch a qualquer momento com a tecla 'w'.",
            }
        ),
    };

    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.header_bg));

    let desc_lines = vec![
        Line::from(vec![
            Span::styled(" 󰋖 ", Style::default().fg(theme.header_fg)),
            Span::styled(desc_text, Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(tip_text, Style::default().fg(theme.key_fg).add_modifier(Modifier::ITALIC)),
        ]),
    ];

    let desc_widget = Paragraph::new(desc_lines)
        .block(desc_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(desc_widget, desc_area);

    // 4. Footer shortcuts helper
    let shortcuts = match language {
        Language::En => [
            ("↑/↓ or j/k", "navigate"),
            ("←/→/Enter/Space", "change value"),
            ("s", "save to disk"),
            ("Esc/C", "close"),
        ],
        Language::Pt => [
            ("↑/↓ ou j/k", "navegar"),
            ("←/→/Enter/Espaço", "alterar"),
            ("s", "salvar no disco"),
            ("Esc/C", "fechar"),
        ],
    };

    frame.render_widget(
        Paragraph::new(help_line(&shortcuts, footer_area.width, theme)).alignment(Alignment::Center),
        footer_area,
    );
}
