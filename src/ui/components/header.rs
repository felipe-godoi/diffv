use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::path::Path;

use crate::core::models::{Language, RepoStats, WatcherScanState};
use crate::ui::theme::Theme;

/// Formats a folder path starting from the git repository base (e.g. `repo_name` or `repo_name/subpath`)
/// and truncates to `max_chars` with an ellipsis (`…`) if it exceeds the limit.
pub fn format_folder_display(path: &Path, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let resolved = if path.as_os_str().is_empty() || path == Path::new(".") {
        std::env::current_dir().unwrap_or_else(|_| path.to_path_buf())
    } else if path.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };

    let base_name = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");

    let cwd = std::env::current_dir().unwrap_or_else(|_| resolved.clone());

    let canon_resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    let canon_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());

    let path_str = if let Ok(rel) = canon_cwd.strip_prefix(&canon_resolved) {
        if rel.as_os_str().is_empty() {
            base_name.to_string()
        } else {
            format!("{}/{}", base_name, rel.to_string_lossy().replace('\\', "/"))
        }
    } else {
        base_name.to_string()
    };

    let count = path_str.chars().count();
    if count > max_chars {
        let take_chars = max_chars.saturating_sub(1);
        let prefix: String = path_str.chars().take(take_chars).collect();
        format!("{}…", prefix)
    } else {
        path_str
    }
}

pub fn render_header(
    frame: &mut Frame,
    area: Rect,
    repo_stats: &RepoStats,
    is_unified: bool,
    watch_mode: bool,
    watcher_state: WatcherScanState,
    spinner_idx: usize,
    worktree_name: Option<&str>,
    comparison_branch: Option<&str>,
    language: Language,
    theme: &Theme,
) {
    let files_label = match language {
        Language::En => "files",
        Language::Pt => "arquivos",
    };
    let mode_label = match language {
        Language::En => "Mode:",
        Language::Pt => "Modo:",
    };

    let width = area.width;

    let mut spans = vec![
        // App brand pill
        Span::styled(
            " ⚡ diffv ",
            Style::default()
                .fg(theme.text_on(theme.header_fg))
                .bg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    // Branch (with optional comparison target and [B] keybinding hint)
    let branch_display = if let Some(target) = comparison_branch {
        if width < 65 {
            format!(
                " 󰊢 {}↔{} ",
                &repo_stats.branch[..5.min(repo_stats.branch.len())],
                &target[..5.min(target.len())]
            )
        } else if width < 85 {
            format!(" 󰊢 {} ↔ {} ", repo_stats.branch, target)
        } else {
            format!(" 󰊢 {} ↔ {} [B] ", repo_stats.branch, target)
        }
    } else if width < 60 && repo_stats.branch.len() > 10 {
        format!(" 󰊢 {}… ", &repo_stats.branch[..8])
    } else if width >= 80 {
        format!(" 󰊢 {} [B] ", repo_stats.branch)
    } else {
        format!(" 󰊢 {} ", repo_stats.branch)
    };
    spans.push(Span::styled(
        branch_display,
        Style::default()
            .fg(theme.key_fg)
            .add_modifier(Modifier::BOLD),
    ));

    // Optional worktree pill
    if let Some(wt) = worktree_name {
        if width >= 75 {
            spans.push(Span::styled("· ", Style::default().fg(theme.border)));
            let wt_str = if width < 90 && wt.len() > 10 {
                format!("󰹹 {}… ", &wt[..8])
            } else {
                format!("󰹹 {} [W] ", wt)
            };
            spans.push(Span::styled(
                wt_str,
                Style::default()
                    .fg(theme.status_u)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Current working directory pill (with character limit and ellipsis when overflowing)
    let folder_budget = if width >= 140 {
        Some(32)
    } else if width >= 115 {
        Some(25)
    } else if width >= 90 {
        Some(18)
    } else if width >= 68 {
        Some(12)
    } else {
        None
    };

    if let Some(max_len) = folder_budget {
        let folder_str = format_folder_display(&repo_stats.root_dir, max_len);
        if !folder_str.is_empty() {
            spans.push(Span::styled("· ", Style::default().fg(theme.border)));
            spans.push(Span::styled(
                " ",
                Style::default()
                    .fg(theme.key_fg)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!("{} ", folder_str),
                Style::default().fg(theme.fg),
            ));
        }
    }

    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));

    // Files count (if not ultra-compact)
    if width >= 80 {
        spans.push(Span::styled(
            format!("{} {} ", repo_stats.file_count, files_label),
            Style::default().fg(theme.fg),
        ));
    }

    // Additions pill
    spans.push(Span::styled(
        format!("+{} ", repo_stats.total_additions),
        Style::default()
            .fg(theme.add_fg)
            .add_modifier(Modifier::BOLD),
    ));

    // Deletions pill
    spans.push(Span::styled(
        format!("-{} ", repo_stats.total_deletions),
        Style::default()
            .fg(theme.del_fg)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));

    // Mode badge
    if width >= 95 {
        spans.push(Span::styled(
            format!("{} ", mode_label),
            Style::default().fg(theme.line_num_fg),
        ));
    }
    let mode_str = if is_unified {
        if width < 80 {
            "󰤈 Uni [m] "
        } else {
            " 󰤈 Unified [m] "
        }
    } else {
        if width < 80 {
            "󰤉 SbS [m] "
        } else {
            " 󰤉 Side-by-Side [m] "
        }
    };
    spans.push(Span::styled(
        mode_str,
        Style::default()
            .fg(theme.header_fg)
            .add_modifier(Modifier::BOLD),
    ));

    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

    // Watch indicator pill (if space permits)
    if width >= 80 {
        spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
        if watch_mode {
            match watcher_state {
                WatcherScanState::Scanning { scanned_dirs } => {
                    let spinner_char = SPINNER[spinner_idx % SPINNER.len()];
                    let count_str = if scanned_dirs >= 1000 {
                        format!("{:.1}k", scanned_dirs as f64 / 1000.0)
                    } else {
                        scanned_dirs.to_string()
                    };
                    let (scan_text, scan_short) = match language {
                        Language::En => (
                            format!(" {} scan ({} dirs) ", spinner_char, count_str),
                            format!(" {} scan ", spinner_char),
                        ),
                        Language::Pt => (
                            format!(" {} varrendo ({} pastas) ", spinner_char, count_str),
                            format!(" {} varrendo ", spinner_char),
                        ),
                    };
                    let text = if width >= 100 { scan_text } else { scan_short };
                    spans.push(Span::styled(
                        text,
                        Style::default()
                            .fg(theme.key_fg)
                            .add_modifier(Modifier::DIM),
                    ));
                }
                WatcherScanState::Ready { .. } | WatcherScanState::Idle => {
                    spans.push(Span::styled(
                        " 󰐥 LIVE ",
                        Style::default()
                            .fg(theme.text_on(theme.status_a))
                            .bg(theme.status_a)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        } else {
            spans.push(Span::styled(
                " 󰏤 PAUSED ",
                Style::default()
                    .fg(theme.line_num_fg)
                    .add_modifier(Modifier::DIM),
            ));
        }
    }

    // Language switcher badge
    spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
    let lang_str = match language {
        Language::En => "󰗊 en [L]",
        Language::Pt => "󰗊 pt [L]",
    };
    spans.push(Span::styled(
        format!(" {} ", lang_str),
        Style::default()
            .fg(theme.key_fg)
            .add_modifier(Modifier::BOLD),
    ));

    // Right-aligned settings and help hints
    if width >= 105 {
        spans.push(Span::styled("│ ", Style::default().fg(theme.border)));
        spans.push(Span::styled(
            "󰒓 ",
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            match language {
                Language::En => "Config [C]  ",
                Language::Pt => "Config [C]  ",
            },
            Style::default().fg(theme.line_num_fg),
        ));
    }
    if width >= 125 {
        spans.push(Span::styled(
            "󰋖 ",
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            match language {
                Language::En => "Help [?]  ",
                Language::Pt => "Ajuda [?]  ",
            },
            Style::default().fg(theme.line_num_fg),
        ));
        spans.push(Span::styled(
            "󰌌 ",
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            match language {
                Language::En => "Focus [Tab]",
                Language::Pt => "Foco [Tab]",
            },
            Style::default().fg(theme.line_num_fg),
        ));
    }

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.header_bg));

    let paragraph = Paragraph::new(Line::from(spans)).block(block);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_folder_display_truncation() {
        let repo_path =
            PathBuf::from("/some/deep/and/long/directory/structure/myproject-with-very-long-name");
        // Limit shorter than repo base name -> should truncate and end with ellipsis '…'
        let formatted = format_folder_display(&repo_path, 10);
        assert_eq!(formatted.chars().count(), 10);
        assert!(formatted.ends_with('…'));

        // Zero limit -> returns empty string
        assert_eq!(format_folder_display(&repo_path, 0), "");

        // Limit larger than base name -> no truncation or ellipsis
        let short_path = PathBuf::from("/tmp/myproj");
        let formatted_short = format_folder_display(&short_path, 20);
        assert_eq!(formatted_short, "myproj");
        assert!(!formatted_short.contains('…'));
    }

    #[test]
    fn test_format_folder_display_repo_base() {
        let path = PathBuf::from("/home/user/code/diffv");
        let formatted = format_folder_display(&path, 30);
        assert!(formatted.starts_with("diffv"));
    }
}
