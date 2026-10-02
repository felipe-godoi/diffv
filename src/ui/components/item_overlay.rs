use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::core::models::{CommitEntry, DrawerTab, FileDiff, Language, StageStatus, StashEntry};
use crate::ui::components::file_tree::{FileViewMode, TreeItem};
use crate::ui::theme::Theme;

pub fn render_item_overlay(
    frame: &mut Frame,
    container_area: Rect,
    drawer_tab: DrawerTab,
    tree_items: &[TreeItem],
    selected_tree_idx: usize,
    files: &[FileDiff],
    filtered_indices: &[usize],
    view_mode: FileViewMode,
    commits: &[CommitEntry],
    selected_commit_idx: usize,
    stashes: &[StashEntry],
    selected_stash_idx: usize,
    is_inspecting_snapshot: bool,
    language: Language,
    theme: &Theme,
) {
    if container_area.width < 25 || container_area.height < 6 {
        return;
    }

    // Determine what item is currently focused and its full untruncated details
    let (badge, title, subtitle) = if is_inspecting_snapshot || drawer_tab == DrawerTab::Changes {
        if view_mode == FileViewMode::Tree {
            if let Some(item) = tree_items.get(selected_tree_idx) {
                if item.is_dir {
                    let dir_badge = match language {
                        Language::En => "📁 DIRECTORY",
                        Language::Pt => "📁 DIRETÓRIO",
                    };
                    (
                        dir_badge.to_string(),
                        item.path.display().to_string(),
                        format!("+{} -{} (subtree)", item.additions, item.deletions),
                    )
                } else if let Some(idx) = item.file_index {
                    if let Some(f) = files.get(idx) {
                        let stage_label = match (f.stage_status, language) {
                            (StageStatus::Staged, Language::En) => "STAGED",
                            (StageStatus::Staged, Language::Pt) => "PREPARADO",
                            (StageStatus::Unstaged, Language::En) => "MODIFIED",
                            (StageStatus::Unstaged, Language::Pt) => "MODIFICADO",
                            (StageStatus::Untracked, Language::En) => "UNTRACKED",
                            (StageStatus::Untracked, Language::Pt) => "NÃO RASTREADO",
                            (StageStatus::PartiallyStaged, Language::En) => "PARTIAL",
                            (StageStatus::PartiallyStaged, Language::Pt) => "PARCIAL",
                        };
                        (
                            format!("📄 [{}]", stage_label),
                            f.display_path(),
                            format!("+{} -{} · {}", f.stats.additions, f.stats.deletions, f.status.code()),
                        )
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            } else {
                return;
            }
        } else if let Some(&real_idx) = filtered_indices.get(selected_tree_idx) {
            if let Some(f) = files.get(real_idx) {
                let stage_label = match (f.stage_status, language) {
                    (StageStatus::Staged, Language::En) => "STAGED",
                    (StageStatus::Staged, Language::Pt) => "PREPARADO",
                    (StageStatus::Unstaged, Language::En) => "MODIFIED",
                    (StageStatus::Unstaged, Language::Pt) => "MODIFICADO",
                    (StageStatus::Untracked, Language::En) => "UNTRACKED",
                    (StageStatus::Untracked, Language::Pt) => "NÃO RASTREADO",
                    (StageStatus::PartiallyStaged, Language::En) => "PARTIAL",
                    (StageStatus::PartiallyStaged, Language::Pt) => "PARCIAL",
                };
                (
                    format!("📄 [{}]", stage_label),
                    f.display_path(),
                    format!("+{} -{} · {}", f.stats.additions, f.stats.deletions, f.status.code()),
                )
            } else {
                return;
            }
        } else {
            return;
        }
    } else if drawer_tab == DrawerTab::Commits {
        if let Some(c) = commits.get(selected_commit_idx) {
            let short_h = &c.hash[..7.min(c.hash.len())];
            let first_line = c.message.lines().next().unwrap_or("").trim();
            (
                format!(" COMMIT {}", short_h),
                first_line.to_string(),
                format!("👤 {} · {}", c.author, c.date),
            )
        } else {
            return;
        }
    } else if drawer_tab == DrawerTab::Stashes {
        if let Some(s) = stashes.get(selected_stash_idx) {
            (
                format!("󰛢 {}", s.selector),
                s.message.clone(),
                s.date.clone(),
            )

        } else {
            return;
        }
    } else {
        return;
    };

    let title_len = title.len();
    let subtitle_len = subtitle.len();
    let badge_len = badge.len();
    let max_text_len = title_len.max(subtitle_len + badge_len + 4);

    let max_avail_w = container_area.width.saturating_sub(4);
    let card_width = ((max_text_len as u16) + 4).clamp(32, max_avail_w);
    let card_height = 3;

    // Anchor overlay at the top-left of the diff viewport
    let card_x = container_area.x + 2;
    let card_y = container_area.y + 1;

    let overlay_rect = Rect {
        x: card_x,
        y: card_y,
        width: card_width,
        height: card_height,
    };

    frame.render_widget(Clear, overlay_rect);

    let title_line = Line::from(vec![
        Span::styled(format!(" {} ", badge), Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(subtitle, Style::default().fg(theme.line_num_fg)),
    ]);

    let block = Block::default()
        .title(title_line)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Rgb(24, 25, 36)));

    let inner = block.inner(overlay_rect);
    frame.render_widget(block, overlay_rect);

    let text_line = Line::from(vec![
        Span::styled(title, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(Paragraph::new(text_line), inner);
}
