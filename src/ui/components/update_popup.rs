use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::cli::VERSION;
use crate::config::UpdateChannel;
use crate::core::models::Language;
use crate::ui::components::style::{help_line, render_card};
use crate::ui::theme::Theme;
use crate::update::UpdateEvent;

fn channel_name(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => "Stable",
        UpdateChannel::Beta => "Beta",
        UpdateChannel::Nightly => "Nightly",
    }
}

/// Result of the startup update check. The new binary replaces the one on disk but
/// this process keeps running the old build, so the popup says it applies next start.
pub fn render_update_popup(
    frame: &mut Frame,
    area: Rect,
    event: &UpdateEvent,
    language: Language,
    theme: &Theme,
) {
    let en = language == Language::En;
    let (icon, title, accent, channel, target) = match event {
        UpdateEvent::Downloading(build) => (
            "󰇚",
            if en {
                "Updating diffv"
            } else {
                "Atualizando o diffv"
            },
            theme.header_fg,
            build.channel,
            Some(build),
        ),
        UpdateEvent::Installed(build) => (
            "󰚰",
            if en {
                "diffv updated"
            } else {
                "diffv atualizado"
            },
            theme.add_fg,
            build.channel,
            Some(build),
        ),
        UpdateEvent::Failed {
            channel, target, ..
        } => (
            "",
            if en {
                "diffv update failed"
            } else {
                "Falha ao atualizar o diffv"
            },
            theme.del_fg,
            *channel,
            target.as_ref(),
        ),
    };

    let strong = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let label = Style::default().fg(theme.line_num_fg);
    let value = Style::default().fg(theme.fg);
    let row = |name: &str, text: String| {
        Line::from(vec![
            Span::styled(format!("{:<14}", name), label),
            Span::styled(text, value),
        ])
    };

    let headline = match event {
        UpdateEvent::Downloading(_) => {
            if en {
                "A new build was found — downloading and verifying it (SHA-256)…".to_string()
            } else {
                "Um build novo foi encontrado — baixando e verificando (SHA-256)…".to_string()
            }
        }
        UpdateEvent::Installed(_) => {
            if en {
                "✓ New build downloaded, SHA-256 verified and installed.".to_string()
            } else {
                "✓ Build novo baixado, SHA-256 verificado e instalado.".to_string()
            }
        }
        UpdateEvent::Failed {
            failure, channel, ..
        } => format!("✗ {}", failure.describe(*channel, language)),
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(headline, strong.fg(accent))),
        Line::from(""),
        row(
            if en { "Channel" } else { "Canal" },
            channel_name(channel).to_string(),
        ),
    ];
    if let Some(build) = target {
        lines.push(row("Build", build.label()));
    }
    lines.push(row(
        if en { "Running now" } else { "Em execução" },
        VERSION.to_string(),
    ));
    lines.push(Line::from(""));

    let notes: &[&str] = match (event, en) {
        (UpdateEvent::Downloading(_), true) => {
            &["You can keep working; the result will show here when it finishes."]
        }
        (UpdateEvent::Downloading(_), false) => {
            &["Pode continuar usando; o resultado aparece aqui ao terminar."]
        }
        (UpdateEvent::Installed(_), true) => &[
            "Takes effect the next time you open diffv.",
            "This window is still running the previous build (\"Running now\").",
        ],
        (UpdateEvent::Installed(_), false) => &[
            "Passa a valer na próxima abertura do diffv.",
            "Esta janela ainda roda o build anterior (\"Em execução\").",
        ],
        (UpdateEvent::Failed { .. }, true) => &[
            "Nothing was replaced: diffv keeps the build it has.",
            "Run `diffv update` to retry and see the details.",
        ],
        (UpdateEvent::Failed { .. }, false) => &[
            "Nada foi substituído: o diffv mantém o build atual.",
            "Rode `diffv update` para tentar de novo e ver os detalhes.",
        ],
    };
    lines.extend(
        notes
            .iter()
            .map(|note| Line::from(Span::styled(*note, value))),
    );

    let width = area.width.saturating_sub(4).min(76);
    // Borders, footer and one spare row for a wrapped line
    let height = (lines.len() as u16 + 5).min(area.height.saturating_sub(2));
    let popup_area = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let card = render_card(frame, popup_area, icon, title, accent, theme);
    if card.height < 3 {
        return;
    }
    let [body, footer_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(card);
    let footer = [("esc / enter / q", if en { "close" } else { "fechar" })];
    frame.render_widget(
        Paragraph::new(help_line(&footer, footer_area.width, theme)).alignment(Alignment::Center),
        footer_area,
    );

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{BuildInfo, UpdateFailure};

    fn build() -> BuildInfo {
        BuildInfo {
            channel: UpdateChannel::Nightly,
            tag: "nightly".into(),
            commit: Some("b97105d".into()),
            built_at: Some("2026-10-06 21:21 UTC".into()),
        }
    }

    fn render(event: &UpdateEvent, language: Language) -> String {
        let backend = ratatui::backend::TestBackend::new(100, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let theme = Theme::vscode_dark();
        terminal
            .draw(|frame| render_update_popup(frame, frame.area(), event, language, &theme))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn installed_popup_names_channel_build_and_when_it_applies() {
        let en = render(&UpdateEvent::Installed(build()), Language::En);
        assert!(en.contains("diffv updated"));
        assert!(en.contains("Nightly"));
        assert!(en.contains("nightly · b97105d · 2026-10-06 21:21 UTC"));
        assert!(en.contains(VERSION));
        assert!(en.contains("Takes effect the next time you open diffv"));

        let pt = render(&UpdateEvent::Installed(build()), Language::Pt);
        assert!(pt.contains("diffv atualizado"));
        assert!(pt.contains("Passa a valer na próxima abertura"));
    }

    #[test]
    fn downloading_popup_says_it_is_updating() {
        assert!(render(&UpdateEvent::Downloading(build()), Language::En).contains("Updating diffv"));
        assert!(render(&UpdateEvent::Downloading(build()), Language::Pt)
            .contains("Atualizando o diffv"));
    }

    #[test]
    fn failed_popup_shows_the_reason() {
        let failed = |failure| UpdateEvent::Failed {
            channel: UpdateChannel::Beta,
            failure,
            target: None,
        };
        let en = render(&failed(UpdateFailure::Offline), Language::En);
        assert!(en.contains("diffv update failed"));
        assert!(en.contains("Could not reach GitHub"));
        assert!(en.contains("Beta"));
        assert!(en.contains("Nothing was replaced"));

        let pt = render(
            &failed(UpdateFailure::NotWritable("/usr/local/bin".into())),
            Language::Pt,
        );
        assert!(pt.contains("Falha ao atualizar o diffv"));
        assert!(pt.contains("Sem permissão de escrita em /usr/local/bin"));

        let checksum = render(
            &UpdateEvent::Failed {
                channel: UpdateChannel::Stable,
                failure: UpdateFailure::ChecksumMismatch,
                target: Some(BuildInfo {
                    channel: UpdateChannel::Stable,
                    tag: "v0.6.0".into(),
                    commit: None,
                    built_at: None,
                }),
            },
            Language::En,
        );
        assert!(checksum.contains("failed SHA-256 verification"));
        assert!(checksum.contains("v0.6.0"));
    }
}
