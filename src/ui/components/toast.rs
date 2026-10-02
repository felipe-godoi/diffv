use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::ui::theme::Theme;

pub const TOAST_DURATION: Duration = Duration::from_millis(1800);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    pub fn classify(message: &str) -> Self {
        let lower = message.to_lowercase();
        if ["error", "erro", "fail", "falh", "✖"]
            .iter()
            .any(|w| lower.contains(w))
        {
            Self::Error
        } else if lower.starts_with('✓')
            || [
                "staged", "copied", "copiado", "created", "criad", "switched", "alternad",
            ]
            .iter()
            .any(|w| lower.contains(w))
        {
            Self::Success
        } else if ["cancel", "reached", "no ", "nenhum", "⚠"]
            .iter()
            .any(|w| lower.contains(w))
        {
            Self::Warning
        } else {
            Self::Info
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Success => "",
            Self::Error => "",
            Self::Warning => "",
            Self::Info => "󰋽",
        }
    }

    fn accent(self, theme: &Theme) -> Color {
        match self {
            Self::Success => theme.status_a,
            Self::Error => theme.status_d,
            Self::Warning => theme.status_m,
            Self::Info => theme.status_u,
        }
    }
}

pub fn render_toast(frame: &mut Frame, area: Rect, notification: &str, theme: &Theme) {
    if notification.is_empty() || area.width < 25 || area.height < 6 {
        return;
    }

    let kind = ToastKind::classify(notification);
    let text = notification.trim_start_matches(['✓', '✖', '⚠', ' ']);
    let max_text = area.width.saturating_sub(10) as usize;
    let text: String = if text.width() > max_text {
        let mut out = String::new();
        for c in text.chars() {
            if out.width() + 2 > max_text {
                break;
            }
            out.push(c);
        }
        out + "…"
    } else {
        text.to_string()
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", kind.icon()),
            Style::default().fg(kind.accent(theme)),
        ),
        Span::styled(format!("{} ", text), Style::default().fg(theme.status_fg)),
    ]);
    let width = line.width() as u16;
    let toast_area = Rect {
        x: area.x + area.width.saturating_sub(width + 2),
        y: area.y + area.height.saturating_sub(3),
        width,
        height: 1,
    };
    frame.render_widget(Clear, toast_area);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme.status_bg)),
        toast_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_notifications() {
        assert_eq!(ToastKind::classify("✓ Hunk staged"), ToastKind::Success);
        assert_eq!(
            ToastKind::classify("Clipboard error: denied"),
            ToastKind::Error
        );
        assert_eq!(ToastKind::classify("Action cancelled"), ToastKind::Warning);
        assert_eq!(ToastKind::classify("Jumped to Hunk #2"), ToastKind::Info);
    }
}
