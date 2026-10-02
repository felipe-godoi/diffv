use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::core::models::{DrawerTab, Language};
use crate::ui::app::Focus;
use crate::ui::theme::Theme;

pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    focus: Focus,
    drawer_tab: DrawerTab,
    visual_mode: bool,
    has_active_commit: bool,
    has_active_stash: bool,
    language: Language,
    theme: &Theme,
) {
    let mode_label = if visual_mode {
        "VISUAL"
    } else if focus == Focus::DiffView {
        "DIFF"
    } else {
        match drawer_tab {
            DrawerTab::Changes => "TREE",
            DrawerTab::Commits => {
                if has_active_commit {
                    "COMMIT FILES"
                } else {
                    "COMMITS"
                }
            }
            DrawerTab::Stashes => {
                if has_active_stash {
                    "STASH FILES"
                } else {
                    "STASHES"
                }
            }
        }
    };

    let (badge_fg, badge_bg) = match mode_label {
        "VISUAL" => (Color::Rgb(15, 20, 25), Color::Rgb(203, 166, 247)), // Soft Lavender
        "DIFF" => (Color::Rgb(15, 20, 25), theme.header_fg),              // Accent
        "TREE" => (Color::Rgb(15, 20, 25), theme.status_u),              // Soft Blue
        "COMMITS" | "COMMIT FILES" => (Color::Rgb(15, 20, 25), Color::Rgb(180, 140, 240)), // Violet
        "STASHES" | "STASH FILES" => (Color::Rgb(15, 20, 25), Color::Rgb(249, 175, 120)),  // Soft Amber
        _ => (Color::Rgb(15, 20, 25), theme.header_fg),
    };

    let mode_pill = Span::styled(
        format!(" {} ", mode_label),
        Style::default()
            .fg(badge_fg)
            .bg(badge_bg)
            .add_modifier(Modifier::BOLD),
    );

    let key_style = Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(theme.status_fg);
    let sep_style = Style::default().fg(theme.border);

    let mut spans = vec![
        mode_pill,
        Span::raw(" "),
    ];

    let width = area.width;

    if visual_mode {
        // Visual mode commands
        let commands: &[(&str, &str)] = match language {
            Language::En => &[
                ("j/k", "Select lines"),
                ("s", "Stage"),
                ("u", "Unstage"),
                ("d", "Discard"),
                ("c", "Copy"),
                ("v/Esc", "Exit"),
            ],
            Language::Pt => &[
                ("j/k", "Selecionar"),
                ("s", "Preparar"),
                ("u", "Despreparar"),
                ("d", "Descartar"),
                ("c", "Copiar"),
                ("v/Esc", "Sair"),
            ],
        };

        for (i, (key, desc)) in commands.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("│ ", sep_style));
            }
            spans.push(Span::styled(format!("{} ", key), key_style));
            spans.push(Span::styled(format!("{} ", desc), text_style));
        }
    } else if focus == Focus::DiffView {
        // Diff view commands
        if width < 80 {
            let commands: &[(&str, &str)] = match language {
                Language::En => &[
                    ("j/k", "Scroll"),
                    ("]c/[c", "Hunk"),
                    ("s", "Stage"),
                    ("v", "Vis"),
                    ("i", "Info"),
                    ("?", "Help"),
                ],
                Language::Pt => &[
                    ("j/k", "Rolar"),
                    ("]c/[c", "Hunk"),
                    ("s", "Prep"),
                    ("v", "Vis"),
                    ("i", "Info"),
                    ("?", "Ajuda"),
                ],
            };
            for (i, (key, desc)) in commands.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled("│ ", sep_style));
                }
                spans.push(Span::styled(format!("{} ", key), key_style));
                spans.push(Span::styled(format!("{} ", desc), text_style));
            }
        } else if width < 115 {
            let commands: &[(&str, &str)] = match language {
                Language::En => &[
                    ("j/k", "Scroll"),
                    ("Ctrl-d/u", "Half"),
                    ("]c/[c", "Hunk"),
                    ("s/u", "Stage"),
                    ("v", "Visual"),
                    ("d", "Discard"),
                    ("i", "Info"),
                    ("?", "Help"),
                ],
                Language::Pt => &[
                    ("j/k", "Rolar"),
                    ("Ctrl-d/u", "Meia"),
                    ("]c/[c", "Hunk"),
                    ("s/u", "Prep"),
                    ("v", "Visual"),
                    ("d", "Descartar"),
                    ("i", "Info"),
                    ("?", "Ajuda"),
                ],
            };
            for (i, (key, desc)) in commands.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled("│ ", sep_style));
                }
                spans.push(Span::styled(format!("{} ", key), key_style));
                spans.push(Span::styled(format!("{} ", desc), text_style));
            }
        } else {
            let commands: &[(&str, &str)] = match language {
                Language::En => &[
                    ("j/k", "Scroll"),
                    ("Ctrl-d/u", "Half"),
                    ("gg/G", "Top/Bot"),
                    ("]c/[c", "Hunk"),
                    ("s/u", "Stage/Unstage"),
                    ("v", "Visual"),
                    ("d", "Discard"),
                    ("e", "Edit"),
                    ("i", "Info"),
                    ("?", "Help"),
                ],
                Language::Pt => &[
                    ("j/k", "Rolar"),
                    ("Ctrl-d/u", "Meia"),
                    ("gg/G", "Início/Fim"),
                    ("]c/[c", "Hunk"),
                    ("s/u", "Preparar/Desp"),
                    ("v", "Visual"),
                    ("d", "Descartar"),
                    ("e", "Editar"),
                    ("i", "Info"),
                    ("?", "Ajuda"),
                ],
            };
            for (i, (key, desc)) in commands.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled("│ ", sep_style));
                }
                spans.push(Span::styled(format!("{} ", key), key_style));
                spans.push(Span::styled(format!("{} ", desc), text_style));
            }
        }
    } else {
        // Focus::FileTree commands
        match drawer_tab {
            DrawerTab::Changes => {
                if width < 80 {
                    let commands: &[(&str, &str)] = match language {
                        Language::En => &[
                            ("j/k", "Files"),
                            ("Enter", "Diff"),
                            ("s/u", "Stage"),
                            ("1/2/3", "Tabs"),
                            ("?", "Help"),
                        ],
                        Language::Pt => &[
                            ("j/k", "Arquivos"),
                            ("Enter", "Diff"),
                            ("s/u", "Prep"),
                            ("1/2/3", "Abas"),
                            ("?", "Ajuda"),
                        ],
                    };
                    for (i, (key, desc)) in commands.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled("│ ", sep_style));
                        }
                        spans.push(Span::styled(format!("{} ", key), key_style));
                        spans.push(Span::styled(format!("{} ", desc), text_style));
                    }
                } else {
                    let commands: &[(&str, &str)] = match language {
                        Language::En => &[
                            ("j/k", "Select"),
                            ("Enter", "Diff"),
                            ("o", "Tree/List"),
                            ("s/u", "Stage/Unstage"),
                            ("1/2/3", "Tabs"),
                            ("i", "File Info"),
                            ("?", "Help"),
                        ],
                        Language::Pt => &[
                            ("j/k", "Selecionar"),
                            ("Enter", "Diff"),
                            ("o", "Árvore/Lista"),
                            ("s/u", "Preparar/Desp"),
                            ("1/2/3", "Abas"),
                            ("i", "Info"),
                            ("?", "Ajuda"),
                        ],
                    };
                    for (i, (key, desc)) in commands.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled("│ ", sep_style));
                        }
                        spans.push(Span::styled(format!("{} ", key), key_style));
                        spans.push(Span::styled(format!("{} ", desc), text_style));
                    }
                }
            }
            DrawerTab::Commits => {
                if has_active_commit {
                    let commands: &[(&str, &str)] = match language {
                        Language::En => &[
                            ("j/k", "Select File"),
                            ("Enter", "Diff"),
                            ("i", "Commit Details"),
                            ("Esc/2", "Back"),
                            ("1", "Live Diff"),
                        ],
                        Language::Pt => &[
                            ("j/k", "Selecionar Arquivo"),
                            ("Enter", "Diff"),
                            ("i", "Detalhes do Commit"),
                            ("Esc/2", "Voltar"),
                            ("1", "Diff ao vivo"),
                        ],
                    };
                    for (i, (key, desc)) in commands.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled("│ ", sep_style));
                        }
                        spans.push(Span::styled(format!("{} ", key), key_style));
                        spans.push(Span::styled(format!("{} ", desc), text_style));
                    }
                } else {
                    let commands: &[(&str, &str)] = match language {
                        Language::En => &[
                            ("j/k", "Select Commit"),
                            ("Enter", "Inspect Files"),
                            ("i", "Commit Details"),
                            ("1", "Live Diff"),
                            ("?", "Help"),
                        ],
                        Language::Pt => &[
                            ("j/k", "Selecionar Commit"),
                            ("Enter", "Inspecionar"),
                            ("i", "Detalhes do Commit"),
                            ("1", "Diff ao vivo"),
                            ("?", "Ajuda"),
                        ],
                    };
                    for (i, (key, desc)) in commands.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled("│ ", sep_style));
                        }
                        spans.push(Span::styled(format!("{} ", key), key_style));
                        spans.push(Span::styled(format!("{} ", desc), text_style));
                    }
                }
            }
            DrawerTab::Stashes => {
                if has_active_stash {
                    let commands: &[(&str, &str)] = match language {
                        Language::En => &[
                            ("j/k", "Select File"),
                            ("Enter", "Diff"),
                            ("i", "Stash Details"),
                            ("Esc/3", "Back"),
                            ("1", "Live Diff"),
                        ],
                        Language::Pt => &[
                            ("j/k", "Selecionar Arquivo"),
                            ("Enter", "Diff"),
                            ("i", "Detalhes do Stash"),
                            ("Esc/3", "Voltar"),
                            ("1", "Diff ao vivo"),
                        ],
                    };
                    for (i, (key, desc)) in commands.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled("│ ", sep_style));
                        }
                        spans.push(Span::styled(format!("{} ", key), key_style));
                        spans.push(Span::styled(format!("{} ", desc), text_style));
                    }
                } else {
                    let commands: &[(&str, &str)] = match language {
                        Language::En => &[
                            ("j/k", "Select Stash"),
                            ("Enter", "Inspect Files"),
                            ("i", "Stash Details"),
                            ("1", "Live Diff"),
                            ("?", "Help"),
                        ],
                        Language::Pt => &[
                            ("j/k", "Selecionar Stash"),
                            ("Enter", "Inspecionar"),
                            ("i", "Detalhes do Stash"),
                            ("1", "Diff ao vivo"),
                            ("?", "Ajuda"),
                        ],
                    };
                    for (i, (key, desc)) in commands.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled("│ ", sep_style));
                        }
                        spans.push(Span::styled(format!("{} ", key), key_style));
                        spans.push(Span::styled(format!("{} ", desc), text_style));
                    }
                }
            }
        }
    }

    let block = Block::default().style(Style::default().bg(theme.status_bg));
    let paragraph = Paragraph::new(Line::from(spans)).block(block);
    frame.render_widget(paragraph, area);
}
