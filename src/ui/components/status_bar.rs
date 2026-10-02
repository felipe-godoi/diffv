use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::core::models::Language;
use crate::ui::theme::Theme;

pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    notification: Option<&str>,
    mode_label: &str,
    language: Language,
    theme: &Theme,
) {
    let (badge_fg, badge_bg) = match mode_label {
        "VISUAL" => (Color::Rgb(15, 20, 25), Color::Rgb(203, 166, 247)), // Soft Lavender
        "TREE" => (Color::Rgb(15, 20, 25), theme.status_u),              // Soft Blue
        _ => (Color::Rgb(15, 20, 25), theme.header_fg),
    };

    let mode_pill = Span::styled(
        format!(" {} ", mode_label),
        Style::default()
            .fg(badge_fg)
            .bg(badge_bg)
            .add_modifier(Modifier::BOLD),
    );

    let content_line = if let Some(msg) = notification {
        Line::from(vec![
            mode_pill,
            Span::raw("  "),
            Span::styled(
                format!("󰂚 {} ", msg),
                Style::default()
                    .fg(theme.key_fg)
                    .bg(theme.status_bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        let key_style = Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD);
        let text_style = Style::default().fg(theme.status_fg);
        let sep_style = Style::default().fg(theme.border);

        let (stage_txt, unstage_txt, discard_txt, visual_txt, edit_txt, copy_txt, hist_txt, help_txt, quit_txt) = match language {
            Language::En => ("Stage ", "Unstage ", "Discard ", "Visual ", "Edit ", "Copy ", "History ", "Help ", "Quit"),
            Language::Pt => ("Preparar ", "Despreparar ", "Descartar ", "Visual ", "Editar ", "Copiar ", "Histórico ", "Ajuda ", "Sair"),
        };

        let width = area.width;

        let mut spans = vec![
            mode_pill,
            Span::raw("  "),
            Span::styled("1/2/3 ", key_style),
            Span::styled(match language { Language::En => "Tabs ", Language::Pt => "Abas " }, text_style),
            Span::styled("│ ", sep_style),
        ];

        if width < 75 {
            // Very narrow terminal (< 75 cols)
            spans.push(Span::styled("s/u ", key_style));
            spans.push(Span::styled("Stage ", text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("b ", key_style));
            spans.push(Span::styled("Sidebar ", text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("? ", key_style));
            spans.push(Span::styled(help_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("q ", key_style));
            spans.push(Span::styled(quit_txt, text_style));
        } else if width < 110 {
            // Half-screen / medium terminal (75 - 110 cols)
            spans.push(Span::styled("s ", key_style));
            spans.push(Span::styled(stage_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("v ", key_style));
            spans.push(Span::styled(visual_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("b ", key_style));
            spans.push(Span::styled(match language { Language::En => "Side ", Language::Pt => "Painel " }, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("W ", key_style));
            spans.push(Span::styled(match language { Language::En => "Trees ", Language::Pt => "Trees " }, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("L ", key_style));
            spans.push(Span::styled(match language { Language::En => "Lang ", Language::Pt => "Idioma " }, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("? ", key_style));
            spans.push(Span::styled(help_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("q ", key_style));
            spans.push(Span::styled(quit_txt, text_style));
        } else {
            // Wide terminal (>= 110 cols)
            spans.push(Span::styled("s ", key_style));
            spans.push(Span::styled(stage_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("u ", key_style));
            spans.push(Span::styled(unstage_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("d ", key_style));
            spans.push(Span::styled(discard_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("v ", key_style));
            spans.push(Span::styled(visual_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("b ", key_style));
            spans.push(Span::styled(match language { Language::En => "Sidebar ", Language::Pt => "Painel " }, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("e ", key_style));
            spans.push(Span::styled(edit_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("c ", key_style));
            spans.push(Span::styled(copy_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("H ", key_style));
            spans.push(Span::styled(hist_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("W ", key_style));
            spans.push(Span::styled("Worktrees ", text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("L ", key_style));
            spans.push(Span::styled("Lang ", text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("? ", key_style));
            spans.push(Span::styled(help_txt, text_style));
            spans.push(Span::styled("│ ", sep_style));
            spans.push(Span::styled("q ", key_style));
            spans.push(Span::styled(quit_txt, text_style));
        }

        Line::from(spans)
    };

    let block = Block::default().style(Style::default().bg(theme.status_bg));
    let paragraph = Paragraph::new(content_line).block(block);
    frame.render_widget(paragraph, area);
}
