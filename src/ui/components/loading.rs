use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::ui::components::header::SPINNER;
use crate::ui::theme::Theme;

// First frame drawn while the initial git/diff collection runs in the background.
// The UI language is only known once the app is loaded, so both are shown.
pub fn render_loading(frame: &mut Frame, spinner_idx: usize, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    let [_, middle, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .areas(area);

    let spinner = SPINNER[spinner_idx % SPINNER.len()];
    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", spinner),
                Style::default()
                    .fg(theme.key_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Loading diff…",
                Style::default()
                    .fg(theme.header_fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "Carregando diff…",
            Style::default().fg(theme.line_num_fg),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), middle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn loading_frame_shows_spinner_and_both_languages() {
        let mut terminal = Terminal::new(TestBackend::new(60, 10)).unwrap();
        let theme = Theme::from_name("vscode-dark");
        terminal.draw(|f| render_loading(f, 1, &theme)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains(SPINNER[1]));
        assert!(text.contains("Loading diff…"));
        assert!(text.contains("Carregando diff…"));
    }
}
