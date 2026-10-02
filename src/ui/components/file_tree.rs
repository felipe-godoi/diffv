use std::collections::BTreeMap;
use std::path::PathBuf;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::{FileDiff, FileStatus, StageStatus};
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileViewMode {
    Flat,
    Tree,
}

#[derive(Debug, Clone)]
pub struct TreeItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_collapsed: bool,
    pub file_index: Option<usize>,
    pub status: Option<FileStatus>,
    pub stage_status: Option<StageStatus>,
    pub additions: usize,
    pub deletions: usize,
    pub depth: usize,
}

pub fn build_tree_items(
    files: &[FileDiff],
    filtered_indices: &[usize],
    collapsed_dirs: &std::collections::HashSet<PathBuf>,
    view_mode: FileViewMode,
) -> Vec<TreeItem> {
    if view_mode == FileViewMode::Flat {
        return filtered_indices
            .iter()
            .map(|&idx| {
                let f = &files[idx];
                TreeItem {
                    name: f.display_path(),
                    path: f.new_path.clone(),
                    is_dir: false,
                    is_collapsed: false,
                    file_index: Some(idx),
                    status: Some(f.status),
                    stage_status: Some(f.stage_status),
                    additions: f.stats.additions,
                    deletions: f.stats.deletions,
                    depth: 0,
                }
            })
            .collect();
    }

    // Build hierarchical tree
    #[derive(Default)]
    struct Node {
        files: Vec<usize>,
        dirs: BTreeMap<String, Node>,
    }

    let mut root = Node::default();
    for &idx in filtered_indices {
        let path = &files[idx].new_path;
        let mut curr = &mut root;
        let components: Vec<&str> = path.iter().filter_map(|c| c.to_str()).collect();

        if components.is_empty() {
            continue;
        }

        for dir_name in &components[..components.len() - 1] {
            curr = curr.dirs.entry((*dir_name).to_string()).or_default();
        }
        curr.files.push(idx);
    }

    fn flatten_node(
        node: &Node,
        curr_path: PathBuf,
        depth: usize,
        files: &[FileDiff],
        collapsed_dirs: &std::collections::HashSet<PathBuf>,
        out: &mut Vec<TreeItem>,
    ) -> (usize, usize) {
        let mut total_add = 0;
        let mut total_del = 0;

        for (dir_name, sub_node) in &node.dirs {
            let dir_path = curr_path.join(dir_name);
            let is_collapsed = collapsed_dirs.contains(&dir_path);

            let insert_idx = out.len();
            // Placeholder for directory item
            out.push(TreeItem {
                name: dir_name.clone(),
                path: dir_path.clone(),
                is_dir: true,
                is_collapsed,
                file_index: None,
                status: None,
                stage_status: None,
                additions: 0,
                deletions: 0,
                depth,
            });

            let mut sub_out = Vec::new();
            let (sub_add, sub_del) = flatten_node(
                sub_node,
                dir_path.clone(),
                depth + 1,
                files,
                collapsed_dirs,
                &mut sub_out,
            );

            // Update dir stats
            out[insert_idx].additions = sub_add;
            out[insert_idx].deletions = sub_del;
            total_add += sub_add;
            total_del += sub_del;

            if !is_collapsed {
                out.extend(sub_out);
            }
        }

        for &idx in &node.files {
            let f = &files[idx];
            let name = f.new_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            total_add += f.stats.additions;
            total_del += f.stats.deletions;

            out.push(TreeItem {
                name,
                path: f.new_path.clone(),
                is_dir: false,
                is_collapsed: false,
                file_index: Some(idx),
                status: Some(f.status),
                stage_status: Some(f.stage_status),
                additions: f.stats.additions,
                deletions: f.stats.deletions,
                depth,
            });
        }

        (total_add, total_del)
    }

    let mut items = Vec::new();
    flatten_node(&root, PathBuf::new(), 0, files, collapsed_dirs, &mut items);
    items
}

pub fn render_file_tree(
    frame: &mut Frame,
    area: Rect,
    items: &[TreeItem],
    selected_idx: usize,
    scroll_offset: usize,
    is_focused: bool,
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    theme: &Theme,
) {
    let mode_indicator = if view_mode == FileViewMode::Tree { "[Tree: t]" } else { "[Flat: t]" };
    let title = if filter_mode {
        format!(" FILTER: {}_ ", filter_query)
    } else if !filter_query.is_empty() {
        format!(" FILES ({}) {} ", items.len(), mode_indicator)
    } else {
        format!(" FILES ({}) {} ", items.len(), mode_indicator)
    };

    let border_style = if is_focused {
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::RIGHT | Borders::BOTTOM)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner_area.height as usize;
    if max_rows == 0 || items.is_empty() {
        if items.is_empty() {
            let empty_msg = Paragraph::new(" No matching files")
                .style(Style::default().fg(theme.line_num_fg));
            frame.render_widget(empty_msg, inner_area);
        }
        return;
    }

    let mut lines = Vec::new();
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(items.len());

    for idx in start_idx..end_idx {
        let item = &items[idx];
        let is_selected = idx == selected_idx;

        let base_style = if is_selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default().bg(theme.bg).fg(theme.fg)
        };

        let prefix = if is_selected { "> " } else { "  " };
        let indent = "  ".repeat(item.depth);

        if item.is_dir {
            let dir_icon = if item.is_collapsed { "▸ " } else { "▾ " };
            let stats_str = format!(" +{} -{}", item.additions, item.deletions);

            let line = Line::from(vec![
                Span::styled(prefix, base_style),
                Span::styled(indent, base_style),
                Span::styled(dir_icon, Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}/", item.name), Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
                Span::styled(stats_str, Style::default().fg(theme.line_num_fg)),
            ]);
            lines.push(line);
        } else {
            let status_color = match item.status.unwrap_or(FileStatus::Modified) {
                FileStatus::Modified => theme.status_m,
                FileStatus::Added => theme.status_a,
                FileStatus::Deleted => theme.status_d,
                FileStatus::Untracked => theme.status_u,
                FileStatus::Renamed => theme.key_fg,
                FileStatus::Copied => theme.status_a,
            };

            let stage_str = match item.stage_status.unwrap_or(StageStatus::Unstaged) {
                StageStatus::Staged => "S",
                StageStatus::Unstaged => "U",
                StageStatus::PartiallyStaged => "±",
                StageStatus::Untracked => "?",
            };

            let status_badge = format!("{} ", item.status.unwrap_or(FileStatus::Modified).code());
            let stage_badge = format!("[{}] ", stage_str);
            let stats_str = format!(" +{} -{}", item.additions, item.deletions);

            let line = Line::from(vec![
                Span::styled(prefix, base_style),
                Span::styled(indent, base_style),
                Span::styled(status_badge, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Span::styled(stage_badge, Style::default().fg(theme.line_num_fg)),
                Span::styled(&item.name, base_style),
                Span::styled(stats_str, Style::default().fg(theme.line_num_fg)),
            ]);
            lines.push(line);
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner_area);
}
