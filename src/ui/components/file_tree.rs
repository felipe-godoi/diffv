use std::collections::BTreeMap;
use std::path::PathBuf;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};
use ratatui::Frame;

use crate::core::models::{CommitEntry, DrawerTab, FileDiff, FileStatus, Language, StageStatus, StashEntry};
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

            if !is_collapsed {
                let (sub_add, sub_del) = flatten_node(
                    sub_node,
                    dir_path,
                    depth + 1,
                    files,
                    collapsed_dirs,
                    out,
                );
                out[insert_idx].additions = sub_add;
                out[insert_idx].deletions = sub_del;
                total_add += sub_add;
                total_del += sub_del;
            } else {
                fn calc_totals(n: &Node, files: &[FileDiff]) -> (usize, usize) {
                    let mut a = 0;
                    let mut d = 0;
                    for &f_idx in &n.files {
                        a += files[f_idx].stats.additions;
                        d += files[f_idx].stats.deletions;
                    }
                    for sub in n.dirs.values() {
                        let (sa, sd) = calc_totals(sub, files);
                        a += sa;
                        d += sd;
                    }
                    (a, d)
                }
                let (sub_add, sub_del) = calc_totals(sub_node, files);
                out[insert_idx].additions = sub_add;
                out[insert_idx].deletions = sub_del;
                total_add += sub_add;
                total_del += sub_del;
            }
        }

        for &file_idx in &node.files {
            let f = &files[file_idx];
            let name = f
                .new_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_else(|| f.display_path().leak())
                .to_string();

            out.push(TreeItem {
                name,
                path: f.new_path.clone(),
                is_dir: false,
                is_collapsed: false,
                file_index: Some(file_idx),
                status: Some(f.status),
                stage_status: Some(f.stage_status),
                additions: f.stats.additions,
                deletions: f.stats.deletions,
                depth,
            });
            total_add += f.stats.additions;
            total_del += f.stats.deletions;
        }

        (total_add, total_del)
    }

    let mut items = Vec::new();
    flatten_node(&root, PathBuf::new(), 0, files, collapsed_dirs, &mut items);
    items
}

fn file_icon(name: &str) -> (&'static str, Color) {
    let lower = name.to_lowercase();
    if lower.ends_with(".rs") {
        (" ", Color::Rgb(242, 143, 114)) // Soft Rust peach
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        (" ", Color::Rgb(137, 180, 250)) // Soft pastel blue
    } else if lower.ends_with(".toml") || lower.ends_with(".yaml") || lower.ends_with(".yml") || lower.ends_with(".json") {
        (" ", Color::Rgb(249, 226, 175)) // Soft pastel cream
    } else if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        (" ", Color::Rgb(137, 180, 250))
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") {
        (" ", Color::Rgb(249, 226, 175))
    } else if lower.ends_with(".py") {
        (" ", Color::Rgb(116, 199, 236)) // Soft sky
    } else if lower.ends_with(".sh") || lower.ends_with(".bash") || lower.ends_with(".zsh") {
        (" ", Color::Rgb(166, 227, 161)) // Soft mint
    } else if lower.ends_with(".html") || lower.ends_with(".htm") {
        (" ", Color::Rgb(242, 143, 114))
    } else if lower.ends_with(".css") || lower.ends_with(".scss") {
        (" ", Color::Rgb(180, 190, 254))
    } else if lower.ends_with(".lock") {
        ("󰌾 ", Color::Rgb(147, 153, 178))
    } else if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".svg") || lower.ends_with(".gif") {
        ("󰈟 ", Color::Rgb(203, 166, 247)) // Soft lavender
    } else {
        ("󰈚 ", Color::Rgb(186, 194, 222)) // Soft file
    }
}

pub fn render_drawer(
    frame: &mut Frame,
    area: Rect,
    active_tab: DrawerTab,
    items: &[TreeItem],
    selected_file_idx: usize,
    file_scroll: usize,
    commits: &[CommitEntry],
    selected_commit_idx: usize,
    commit_scroll: usize,
    stashes: &[StashEntry],
    selected_stash_idx: usize,
    stash_scroll: usize,
    is_focused: bool,
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    language: Language,
    theme: &Theme,
) {
    let tab_changes_title = match language {
        Language::En => "󰈚 Changes",
        Language::Pt => "󰈚 Mudanças",
    };
    let tab_commits_title = "󰜉 Commits";
    let tab_stashes_title = "󰮎 Stashes";

    let border_style = if filter_mode {
        Style::default().fg(theme.key_fg).add_modifier(Modifier::BOLD)
    } else if is_focused {
        Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    // Header title with 3 tabs
    let mut title_spans = Vec::new();
    let sel_tab_style = Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD);
    let norm_tab_style = Style::default().fg(theme.line_num_fg);

    title_spans.push(Span::raw(" "));
    if active_tab == DrawerTab::Changes {
        title_spans.push(Span::styled(format!(" [1] {} ", tab_changes_title), sel_tab_style));
    } else {
        title_spans.push(Span::styled(format!(" [1] {} ", tab_changes_title), norm_tab_style));
    }

    title_spans.push(Span::styled("│", Style::default().fg(theme.border)));

    if active_tab == DrawerTab::Commits {
        title_spans.push(Span::styled(format!(" [2] {} ", tab_commits_title), sel_tab_style));
    } else {
        title_spans.push(Span::styled(format!(" [2] {} ", tab_commits_title), norm_tab_style));
    }

    title_spans.push(Span::styled("│", Style::default().fg(theme.border)));

    if active_tab == DrawerTab::Stashes {
        title_spans.push(Span::styled(format!(" [3] {} ", tab_stashes_title), sel_tab_style));
    } else {
        title_spans.push(Span::styled(format!(" [3] {} ", tab_stashes_title), norm_tab_style));
    }
    title_spans.push(Span::raw(" "));

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner_area.height as usize;
    if max_rows == 0 {
        return;
    }

    match active_tab {
        DrawerTab::Changes => {
            render_changes_tab(
                frame,
                inner_area,
                items,
                selected_file_idx,
                file_scroll,
                filter_mode,
                filter_query,
                view_mode,
                language,
                theme,
            );
        }
        DrawerTab::Commits => {
            render_commits_tab(
                frame,
                inner_area,
                commits,
                selected_commit_idx,
                commit_scroll,
                language,
                theme,
            );
        }
        DrawerTab::Stashes => {
            render_stashes_tab(
                frame,
                inner_area,
                stashes,
                selected_stash_idx,
                stash_scroll,
                language,
                theme,
            );
        }
    }
}

fn render_changes_tab(
    frame: &mut Frame,
    area: Rect,
    items: &[TreeItem],
    selected_idx: usize,
    scroll_offset: usize,
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    language: Language,
    theme: &Theme,
) {
    if items.is_empty() {
        let msg = match language {
            Language::En => "  No matching files",
            Language::Pt => "  Nenhum arquivo correspondente",
        };
        let empty_p = Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(empty_p, area);
        return;
    }

    let mode_str = match (view_mode, language) {
        (FileViewMode::Tree, Language::En) => " Folders [t]",
        (FileViewMode::Tree, Language::Pt) => " Pastas [t]",
        (FileViewMode::Flat, Language::En) => "󰈚 Flat [t]",
        (FileViewMode::Flat, Language::Pt) => "󰈚 Lista [t]",
    };

    let sub_header = if filter_mode {
        match language {
            Language::En => format!(" 󰍉 Filter: {}_ ", filter_query),
            Language::Pt => format!(" 󰍉 Filtro: {}_ ", filter_query),
        }
    } else {
        format!(" Mode: {} · ({} files)", mode_str, items.len())
    };

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        sub_header,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    let max_rows = area.height.saturating_sub(1) as usize;
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

        let cursor_span = if is_selected {
            Span::styled("▎", Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ", base_style)
        };

        let indent = "  ".repeat(item.depth);

        if item.is_dir {
            let (dir_icon, dir_color) = if item.is_collapsed {
                (" ", theme.key_fg)
            } else {
                (" ", theme.key_fg)
            };
            let stats_str = format!(" +{} -{}", item.additions, item.deletions);

            let line = Line::from(vec![
                cursor_span,
                Span::styled(indent, base_style),
                Span::styled(dir_icon, Style::default().fg(dir_color).bg(if is_selected { theme.selected_bg } else { theme.bg })),
                Span::styled(
                    format!("{}/", item.name),
                    base_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(stats_str, Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg })),
            ]);
            lines.push(line);
        } else {
            let (status_badge, status_color) = match item.status.unwrap_or(FileStatus::Modified) {
                FileStatus::Modified => ("● ", theme.status_m),
                FileStatus::Added => ("✚ ", theme.status_a),
                FileStatus::Deleted => ("✖ ", theme.status_d),
                FileStatus::Untracked => ("? ", theme.status_u),
                FileStatus::Renamed => ("➜ ", theme.key_fg),
                FileStatus::Copied => ("✚ ", theme.status_a),
            };

            let stage_span = match item.stage_status.unwrap_or(StageStatus::Unstaged) {
                StageStatus::Staged => Span::styled(
                    "󰄬 ",
                    Style::default().fg(theme.status_a).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
                ),
                StageStatus::PartiallyStaged => Span::styled(
                    "± ",
                    Style::default().fg(theme.status_m).bg(if is_selected { theme.selected_bg } else { theme.bg }),
                ),
                StageStatus::Unstaged | StageStatus::Untracked => Span::styled(
                    "  ",
                    base_style,
                ),
            };

            let (icon_str, icon_color) = file_icon(&item.name);
            let stats_str = format!(" +{} -{}", item.additions, item.deletions);

            let name_style = if is_selected {
                base_style.add_modifier(Modifier::BOLD)
            } else {
                base_style
            };

            let line = Line::from(vec![
                cursor_span,
                Span::styled(indent, base_style),
                Span::styled(status_badge, Style::default().fg(status_color).bg(if is_selected { theme.selected_bg } else { theme.bg })),
                stage_span,
                Span::styled(icon_str, Style::default().fg(icon_color).bg(if is_selected { theme.selected_bg } else { theme.bg })),
                Span::styled(&item.name, name_style),
                Span::styled(stats_str, Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg })),
            ]);
            lines.push(line);
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn render_commits_tab(
    frame: &mut Frame,
    area: Rect,
    commits: &[CommitEntry],
    selected_idx: usize,
    scroll_offset: usize,
    language: Language,
    theme: &Theme,
) {
    if commits.is_empty() {
        let msg = match language {
            Language::En => "  No git commits found",
            Language::Pt => "  Nenhum commit encontrado",
        };
        let p = Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(p, area);
        return;
    }

    let mut lines = Vec::new();
    let header_msg = match language {
        Language::En => format!(" Recent Commits ({}) · [Enter] View", commits.len()),
        Language::Pt => format!(" Commits Recentes ({}) · [Enter] Ver", commits.len()),
    };
    lines.push(Line::from(Span::styled(
        header_msg,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    let max_rows = area.height.saturating_sub(1) as usize;
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(commits.len());

    for idx in start_idx..end_idx {
        let commit = &commits[idx];
        let is_selected = idx == selected_idx;

        let base_style = if is_selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default().bg(theme.bg).fg(theme.fg)
        };

        let cursor_span = if is_selected {
            Span::styled("▎", Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ", base_style)
        };

        let line = Line::from(vec![
            cursor_span,
            Span::styled(
                format!(" 󰜉 {:<7} ", commit.hash),
                Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &commit.message,
                if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style },
            ),
            Span::styled(
                format!(" ({})", commit.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ),
        ]);
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn render_stashes_tab(
    frame: &mut Frame,
    area: Rect,
    stashes: &[StashEntry],
    selected_idx: usize,
    scroll_offset: usize,
    language: Language,
    theme: &Theme,
) {
    if stashes.is_empty() {
        let msg = match language {
            Language::En => "  No git stashes found",
            Language::Pt => "  Nenhum stash encontrado",
        };
        let p = Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg));
        frame.render_widget(p, area);
        return;
    }

    let mut lines = Vec::new();
    let header_msg = match language {
        Language::En => format!(" Git Stashes ({}) · [Enter] View", stashes.len()),
        Language::Pt => format!(" Stashes Git ({}) · [Enter] Ver", stashes.len()),
    };
    lines.push(Line::from(Span::styled(
        header_msg,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    let max_rows = area.height.saturating_sub(1) as usize;
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(stashes.len());

    for idx in start_idx..end_idx {
        let stash = &stashes[idx];
        let is_selected = idx == selected_idx;

        let base_style = if is_selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default().bg(theme.bg).fg(theme.fg)
        };

        let cursor_span = if is_selected {
            Span::styled("▎", Style::default().fg(theme.key_fg).bg(theme.selected_bg).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ", base_style)
        };

        let line = Line::from(vec![
            cursor_span,
            Span::styled(
                format!(" 󰮎 {:<10} ", stash.selector),
                Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &stash.message,
                if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style },
            ),
            Span::styled(
                format!(" ({})", stash.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ),
        ]);
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
