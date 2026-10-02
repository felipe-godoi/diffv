use std::collections::BTreeMap;
use std::path::PathBuf;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
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

pub fn file_icon(name: &str) -> (&'static str, Color) {
    let lower = name.to_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);

    // 1. Exact / Special filename matches
    match basename {
        "dockerfile" | "dockerfile.dev" | "dockerfile.prod" | "containerfile" => {
            return ("󰡨 ", Color::Rgb(116, 199, 236));
        }
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
            return ("󰡨 ", Color::Rgb(116, 199, 236));
        }
        "makefile" | "gnumakefile" => {
            return (" ", Color::Rgb(249, 226, 175));
        }
        "justfile" | "taskfile.yml" | "taskfile.yaml" => {
            return ("󰑮 ", Color::Rgb(242, 205, 172));
        }
        "cmakelists.txt" => {
            return (" ", Color::Rgb(137, 180, 250));
        }
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitconfig" => {
            return ("󰊢 ", Color::Rgb(243, 139, 168));
        }
        ".env" | ".env.local" | ".env.example" | ".env.development" | ".env.production" | ".env.test" => {
            return (" ", Color::Rgb(249, 226, 175));
        }
        "cargo.toml" | "cargo.lock" => {
            return (" ", Color::Rgb(242, 143, 114));
        }
        "package.json" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "bun.lockb" => {
            return (" ", Color::Rgb(166, 227, 161));
        }
        "tsconfig.json" | "jsconfig.json" => {
            return (" ", Color::Rgb(137, 180, 250));
        }
        "go.mod" | "go.sum" | "go.work" => {
            return (" ", Color::Rgb(116, 199, 236));
        }
        "gemfile" | "gemfile.lock" | "rakefile" => {
            return (" ", Color::Rgb(243, 139, 168));
        }
        "readme" | "readme.md" | "readme.txt" | "readme.markdown" => {
            return (" ", Color::Rgb(137, 180, 250));
        }
        "license" | "license.md" | "license.txt" | "licence" | "copying" => {
            return ("󰌆 ", Color::Rgb(249, 226, 175));
        }
        "changelog" | "changelog.md" | "history.md" => {
            return ("󰮏 ", Color::Rgb(166, 227, 161));
        }
        _ => {}
    }

    // 2. Extension matches
    let ext = basename.rsplit('.').next().unwrap_or("");

    match ext {
        // Rust
        "rs" => (" ", Color::Rgb(242, 143, 114)),
        // Go
        "go" => (" ", Color::Rgb(116, 199, 236)),
        // Python
        "py" | "pyi" | "pyw" | "ipynb" => (" ", Color::Rgb(116, 199, 236)),
        // JavaScript / Node
        "js" | "mjs" | "cjs" => (" ", Color::Rgb(249, 226, 175)),
        // TypeScript
        "ts" | "mts" | "cts" => (" ", Color::Rgb(137, 180, 250)),
        // React
        "jsx" => (" ", Color::Rgb(116, 199, 236)),
        "tsx" => (" ", Color::Rgb(137, 180, 250)),
        // Vue / Svelte
        "vue" => (" ", Color::Rgb(166, 227, 161)),
        "svelte" => (" ", Color::Rgb(243, 139, 168)),
        // C / C++
        "c" | "h" => (" ", Color::Rgb(137, 180, 250)),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => (" ", Color::Rgb(137, 180, 250)),
        // C# / .NET
        "cs" | "csx" => ("󰌛 ", Color::Rgb(203, 166, 247)),
        "fs" | "fsi" | "fsx" => (" ", Color::Rgb(116, 199, 236)),
        // Java / JVM
        "java" | "class" | "jar" => (" ", Color::Rgb(242, 143, 114)),
        "kt" | "kts" => (" ", Color::Rgb(203, 166, 247)),
        "scala" | "sc" => (" ", Color::Rgb(243, 139, 168)),
        "clj" | "cljs" | "cljc" | "edn" => (" ", Color::Rgb(166, 227, 161)),
        // Swift
        "swift" => (" ", Color::Rgb(242, 143, 114)),
        // PHP
        "php" => (" ", Color::Rgb(180, 190, 254)),
        // Ruby
        "rb" | "erb" | "gemspec" => (" ", Color::Rgb(243, 139, 168)),
        // Lua
        "lua" => (" ", Color::Rgb(137, 180, 250)),
        // Zig
        "zig" => (" ", Color::Rgb(249, 226, 175)),
        // Dart / Flutter
        "dart" => (" ", Color::Rgb(116, 199, 236)),
        // Elixir / Erlang
        "ex" | "exs" => (" ", Color::Rgb(203, 166, 247)),
        "erl" | "hrl" => (" ", Color::Rgb(243, 139, 168)),
        // Haskell
        "hs" | "lhs" => (" ", Color::Rgb(203, 166, 247)),
        // R / Julia
        "r" | "rmd" => ("󰟔 ", Color::Rgb(137, 180, 250)),
        "jl" => (" ", Color::Rgb(203, 166, 247)),
        // OCaml
        "ml" | "mli" => (" ", Color::Rgb(242, 143, 114)),
        // Perl
        "pl" | "pm" => (" ", Color::Rgb(137, 180, 250)),
        // Shell & Terminal
        "sh" | "bash" | "zsh" => (" ", Color::Rgb(166, 227, 161)),
        "fish" => ("󰈺 ", Color::Rgb(249, 226, 175)),
        "ps1" | "psm1" | "psd1" => ("󰨊 ", Color::Rgb(137, 180, 250)),
        "bat" | "cmd" => (" ", Color::Rgb(147, 153, 178)),
        // Databases & Queries
        "sql" | "pgsql" | "mysql" | "plsql" => (" ", Color::Rgb(249, 226, 175)),
        "graphql" | "gql" => ("󰡪 ", Color::Rgb(243, 139, 168)),
        "proto" => ("󰒍 ", Color::Rgb(116, 199, 236)),
        // Low-level & Assembly
        "asm" | "s" => ("󰒍 ", Color::Rgb(147, 153, 178)),
        "wasm" | "wat" => (" ", Color::Rgb(203, 166, 247)),
        // Cloud & Infra
        "nix" => (" ", Color::Rgb(116, 199, 236)),
        "tf" | "tfvars" | "hcl" => ("󱁢 ", Color::Rgb(203, 166, 247)),
        // Web & Markup
        "html" | "htm" | "xhtml" => (" ", Color::Rgb(242, 143, 114)),
        "css" => (" ", Color::Rgb(180, 190, 254)),
        "scss" | "sass" => (" ", Color::Rgb(243, 139, 168)),
        "less" => (" ", Color::Rgb(137, 180, 250)),
        "styl" => (" ", Color::Rgb(166, 227, 161)),
        // Config & Data Serialization
        "json" | "jsonc" | "json5" => (" ", Color::Rgb(249, 226, 175)),
        "yaml" | "yml" => (" ", Color::Rgb(243, 139, 168)),
        "toml" => (" ", Color::Rgb(242, 205, 172)),
        "ini" | "conf" | "cfg" | "properties" => (" ", Color::Rgb(147, 153, 178)),
        "xml" | "plist" => ("󰗀 ", Color::Rgb(249, 226, 175)),
        "csv" | "tsv" => ("󰈙 ", Color::Rgb(166, 227, 161)),
        // Documentation & Writing
        "md" | "markdown" | "mdx" => (" ", Color::Rgb(137, 180, 250)),
        "txt" | "text" => ("󰈙 ", Color::Rgb(186, 194, 222)),
        "pdf" => ("󰈦 ", Color::Rgb(243, 139, 168)),
        "tex" | "latex" | "bib" => ("󰙩 ", Color::Rgb(116, 199, 236)),
        "org" => (" ", Color::Rgb(116, 199, 236)),
        "rst" | "adoc" => ("󰈙 ", Color::Rgb(186, 194, 222)),
        // Images & Media
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => ("󰈟 ", Color::Rgb(203, 166, 247)),
        "svg" => ("󰈟 ", Color::Rgb(249, 226, 175)),
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => ("󰎈 ", Color::Rgb(249, 226, 175)),
        "mp4" | "mkv" | "webm" | "avi" | "mov" => ("󰕧 ", Color::Rgb(243, 139, 168)),
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => (" ", Color::Rgb(243, 139, 168)),
        // Archives & Compressed
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => (" ", Color::Rgb(249, 226, 175)),
        // Lockfiles
        "lock" => ("󰌾 ", Color::Rgb(147, 153, 178)),
        // Binaries & Executables
        "bin" | "exe" | "dll" | "so" | "dylib" | "o" | "a" => ("󰜎 ", Color::Rgb(147, 153, 178)),
        // Default
        _ => ("󰈚 ", Color::Rgb(186, 194, 222)),
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
    active_commit_info: Option<&CommitEntry>,
    active_stash_info: Option<&StashEntry>,
    is_focused: bool,
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    language: Language,
    theme: &Theme,
) {
    let is_compact = area.width < 34;

    let (tab_changes_title, tab_commits_title, tab_stashes_title) = if is_compact {
        match language {
            Language::En => ("󰈚 Chg", "󰜉 Cmt", "󰮎 Stsh"),
            Language::Pt => ("󰈚 Mud", "󰜉 Cmt", "󰮎 Stsh"),
        }
    } else {
        match language {
            Language::En => ("󰈚 Changes", "󰜉 Commits", "󰮎 Stashes"),
            Language::Pt => ("󰈚 Mudanças", "󰜉 Commits", "󰮎 Stashes"),
        }
    };

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
            if let Some(commit) = active_commit_info {
                render_commit_files_drawer(
                    frame,
                    inner_area,
                    commit,
                    items,
                    selected_file_idx,
                    file_scroll,
                    filter_mode,
                    filter_query,
                    view_mode,
                    language,
                    theme,
                );
            } else {
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
        }
        DrawerTab::Stashes => {
            if let Some(stash) = active_stash_info {
                render_stash_files_drawer(
                    frame,
                    inner_area,
                    stash,
                    items,
                    selected_file_idx,
                    file_scroll,
                    filter_mode,
                    filter_query,
                    view_mode,
                    language,
                    theme,
                );
            } else {
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
}

fn render_commit_files_drawer(
    frame: &mut Frame,
    area: Rect,
    commit: &CommitEntry,
    items: &[TreeItem],
    selected_file_idx: usize,
    file_scroll: usize,
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    language: Language,
    theme: &Theme,
) {
    let header_height = if area.height < 14 { 3 } else { 4 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(3),
        ])
        .split(area);

    let short_hash = &commit.hash[..7.min(commit.hash.len())];
    let mut header_lines = Vec::new();

    let msg_max_w = (chunks[0].width as usize).saturating_sub(12).max(8);
    let msg_trunc = if commit.message.len() > msg_max_w {
        format!("{}…", &commit.message[..msg_max_w.saturating_sub(1)])
    } else {
        commit.message.clone()
    };

    header_lines.push(Line::from(vec![
        Span::styled(format!(" 󰜉 {} ", short_hash), Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.header_fg).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(msg_trunc, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));

    if header_height >= 4 {
        let auth_max_w = (chunks[0].width as usize).saturating_sub(16).max(8);
        let auth_trunc = if commit.author.len() > auth_max_w {
            format!("{}…", &commit.author[..auth_max_w.saturating_sub(1)])
        } else {
            commit.author.clone()
        };
        header_lines.push(Line::from(vec![
            Span::styled(format!(" 👤 {} · {}", auth_trunc, commit.date), Style::default().fg(theme.line_num_fg)),
        ]));
    }

    let back_hint = match language {
        Language::En => format!(" 󰈚 {} files · [Esc / 2] ← Back", items.len()),
        Language::Pt => format!(" 󰈚 {} arquivos · [Esc / 2] ← Voltar", items.len()),
    };
    header_lines.push(Line::from(Span::styled(
        back_hint,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    frame.render_widget(Paragraph::new(header_lines), chunks[0]);

    render_changes_tab(
        frame,
        chunks[1],
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

fn render_stash_files_drawer(
    frame: &mut Frame,
    area: Rect,
    stash: &StashEntry,
    items: &[TreeItem],
    selected_file_idx: usize,
    file_scroll: usize,
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    language: Language,
    theme: &Theme,
) {
    let header_height = if area.height < 14 { 2 } else { 3 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(3),
        ])
        .split(area);

    let mut header_lines = Vec::new();
    let msg_max_w = (chunks[0].width as usize).saturating_sub(14).max(8);
    let msg_trunc = if stash.message.len() > msg_max_w {
        format!("{}…", &stash.message[..msg_max_w.saturating_sub(1)])
    } else {
        stash.message.clone()
    };

    header_lines.push(Line::from(vec![
        Span::styled(format!(" 󰮎 {} ", stash.selector), Style::default().fg(Color::Rgb(15, 20, 25)).bg(theme.key_fg).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(msg_trunc, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ]));

    let back_hint = match language {
        Language::En => format!(" 󰈚 {} files · [Esc / 3] ← Back", items.len()),
        Language::Pt => format!(" 󰈚 {} arquivos · [Esc / 3] ← Voltar", items.len()),
    };
    header_lines.push(Line::from(Span::styled(
        back_hint,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    frame.render_widget(Paragraph::new(header_lines), chunks[0]);

    render_changes_tab(
        frame,
        chunks[1],
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

    let show_stats = area.width >= 28;

    let sub_header = if filter_mode {
        match language {
            Language::En => format!(" 󰍉 Filter: {}_ ", filter_query),
            Language::Pt => format!(" 󰍉 Filtro: {}_ ", filter_query),
        }
    } else if area.width < 28 {
        format!(" {} ({})", mode_str, items.len())
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

            let mut spans = vec![
                cursor_span,
                Span::styled(indent, base_style),
                Span::styled(dir_icon, Style::default().fg(dir_color).bg(if is_selected { theme.selected_bg } else { theme.bg })),
                Span::styled(
                    format!("{}/", item.name),
                    base_style.add_modifier(Modifier::BOLD),
                ),
            ];
            if show_stats {
                spans.push(Span::styled(stats_str, Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg })));
            }
            lines.push(Line::from(spans));
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

            let mut spans = vec![
                cursor_span,
                Span::styled(indent, base_style),
                Span::styled(status_badge, Style::default().fg(status_color).bg(if is_selected { theme.selected_bg } else { theme.bg })),
                stage_span,
                Span::styled(icon_str, Style::default().fg(icon_color).bg(if is_selected { theme.selected_bg } else { theme.bg })),
                Span::styled(&item.name, name_style),
            ];
            if show_stats {
                spans.push(Span::styled(stats_str, Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg })));
            }
            lines.push(Line::from(spans));
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
        Language::En => if area.width < 28 { format!(" Commits ({})", commits.len()) } else { format!(" Recent Commits ({}) · [Enter] View", commits.len()) },
        Language::Pt => if area.width < 28 { format!(" Commits ({})", commits.len()) } else { format!(" Commits Recentes ({}) · [Enter] Ver", commits.len()) },
    };
    lines.push(Line::from(Span::styled(
        header_msg,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    let max_rows = area.height.saturating_sub(1) as usize;
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(commits.len());
    let show_date = area.width >= 32;

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

        let short_hash = &commit.hash[..7.min(commit.hash.len())];
        let mut spans = vec![
            cursor_span,
            Span::styled(
                format!(" 󰜉 {:<7} ", short_hash),
                Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &commit.message,
                if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style },
            ),
        ];

        if show_date {
            spans.push(Span::styled(
                format!(" ({})", commit.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ));
        }

        lines.push(Line::from(spans));
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
        Language::En => if area.width < 28 { format!(" Stashes ({})", stashes.len()) } else { format!(" Git Stashes ({}) · [Enter] View", stashes.len()) },
        Language::Pt => if area.width < 28 { format!(" Stashes ({})", stashes.len()) } else { format!(" Stashes Git ({}) · [Enter] Ver", stashes.len()) },
    };
    lines.push(Line::from(Span::styled(
        header_msg,
        Style::default().fg(theme.key_fg).add_modifier(Modifier::DIM),
    )));

    let max_rows = area.height.saturating_sub(1) as usize;
    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + max_rows).min(stashes.len());
    let show_date = area.width >= 32;

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

        let mut spans = vec![
            cursor_span,
            Span::styled(
                format!(" 󰮎 {:<9} ", stash.selector),
                Style::default().fg(theme.key_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &stash.message,
                if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style },
            ),
        ];

        if show_date {
            spans.push(Span::styled(
                format!(" ({})", stash.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected { theme.selected_bg } else { theme.bg }),
            ));
        }

        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_icon_catalog() {
        assert_eq!(file_icon("Dockerfile").0, "󰡨 ");
        assert_eq!(file_icon("docker-compose.yml").0, "󰡨 ");
        assert_eq!(file_icon("Cargo.toml").0, " ");
        assert_eq!(file_icon("package.json").0, " ");
        assert_eq!(file_icon("tsconfig.json").0, " ");
        assert_eq!(file_icon("go.mod").0, " ");
        assert_eq!(file_icon(".gitignore").0, "󰊢 ");
        assert_eq!(file_icon(".env.local").0, " ");
        assert_eq!(file_icon("README.md").0, " ");
        assert_eq!(file_icon("LICENSE").0, "󰌆 ");

        // Extensions
        assert_eq!(file_icon("main.rs").0, " ");
        assert_eq!(file_icon("server.go").0, " ");
        assert_eq!(file_icon("script.py").0, " ");
        assert_eq!(file_icon("index.js").0, " ");
        assert_eq!(file_icon("types.ts").0, " ");
        assert_eq!(file_icon("App.tsx").0, " ");
        assert_eq!(file_icon("Component.vue").0, " ");
        assert_eq!(file_icon("main.cpp").0, " ");
        assert_eq!(file_icon("Program.cs").0, "󰌛 ");
        assert_eq!(file_icon("App.java").0, " ");
        assert_eq!(file_icon("App.kt").0, " ");
        assert_eq!(file_icon("Main.swift").0, " ");
        assert_eq!(file_icon("run.sh").0, " ");
        assert_eq!(file_icon("schema.sql").0, " ");
        assert_eq!(file_icon("styles.css").0, " ");
        assert_eq!(file_icon("infra.tf").0, "󱁢 ");
        assert_eq!(file_icon("photo.png").0, "󰈟 ");
        assert_eq!(file_icon("archive.zip").0, " ");
    }
}

