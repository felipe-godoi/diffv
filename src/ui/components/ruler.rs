use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::core::models::{DiffKind, FileDiff};
use crate::ui::theme::Theme;

pub fn render_ruler(
    frame: &mut Frame,
    area: Rect,
    file_diff: Option<&FileDiff>,
    scroll_y: usize,
    viewport_height: usize,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let ruler_height = area.height as usize;
    let total_rows = file_diff
        .map(|f| f.aligned_rows.len())
        .unwrap_or(0);

    let mut lines = Vec::with_capacity(ruler_height);

    if total_rows == 0 {
        for _ in 0..ruler_height {
            lines.push(Line::from(Span::styled(" ", Style::default().bg(theme.ruler_bg))));
        }
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }

    let rows = &file_diff.unwrap().aligned_rows;

    // Determine viewport start and end in ruler coordinates
    let vp_start = (scroll_y * ruler_height) / total_rows;
    let vp_end = ((scroll_y + viewport_height).min(total_rows) * ruler_height) / total_rows;

    for r_idx in 0..ruler_height {
        let is_in_viewport = r_idx >= vp_start && r_idx <= vp_end.max(vp_start);

        // Map ruler row to range of aligned rows
        let row_start = (r_idx * total_rows) / ruler_height;
        let row_end = (((r_idx + 1) * total_rows) / ruler_height).min(total_rows);

        let mut has_add = false;
        let mut has_del = false;

        for i in row_start..row_end {
            let row = &rows[i];
            if let Some(r) = &row.right {
                if r.kind == DiffKind::Addition {
                    has_add = true;
                }
            }
            if let Some(l) = &row.left {
                if l.kind == DiffKind::Deletion {
                    has_del = true;
                }
            }
        }

        let symbol = if is_in_viewport { "█" } else if has_add || has_del { "■" } else { " " };
        let char_style = match (has_add, has_del) {
            (true, true) => Style::default().fg(theme.key_fg).bg(theme.ruler_bg),
            (true, false) => Style::default().fg(theme.add_fg).bg(theme.ruler_bg),
            (false, true) => Style::default().fg(theme.del_fg).bg(theme.ruler_bg),
            (false, false) => {
                if is_in_viewport {
                    Style::default().fg(theme.ruler_viewport).bg(theme.ruler_bg)
                } else {
                    Style::default().bg(theme.ruler_bg)
                }
            }
        };

        lines.push(Line::from(Span::styled(symbol, char_style)));
    }

    frame.render_widget(Paragraph::new(lines), area);
}
