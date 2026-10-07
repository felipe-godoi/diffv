use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::core::models::{
    CommitEntry, DiffSection, DrawerTab, FileDiff, FileStatus, Language, StageStatus, StashEntry,
};
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
    pub is_section: bool,
}

pub fn section_key(section: DiffSection) -> PathBuf {
    PathBuf::from(match section {
        DiffSection::Staged => ":staged",
        DiffSection::Changes => ":changes",
    })
}

/// What `s` / `u` act on when pressed on a file-list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageScope {
    File(PathBuf),
    /// Repo-relative directory, without the section prefix.
    Dir(PathBuf),
    Section(DiffSection),
}

/// A file-list item resolved to the visible files it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTarget {
    pub scope: StageScope,
    /// Repo-relative paths, sorted and deduplicated.
    pub paths: Vec<PathBuf>,
}

/// Resolves a tree item to the files under it: the file itself, every visible
/// file below a directory (recursive, collapsed or not), or every visible file
/// of a section. Directories inside a section only cover that section's files.
pub fn resolve_stage_target(
    item: &TreeItem,
    files: &[FileDiff],
    filtered_indices: &[usize],
) -> Option<StageTarget> {
    let collect = |keep: &dyn Fn(&FileDiff) -> bool| -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = filtered_indices
            .iter()
            .filter_map(|&i| files.get(i))
            .filter(|f| keep(f))
            .map(|f| f.new_path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    };

    if let Some(idx) = item.file_index {
        let file = files.get(idx)?;
        return Some(StageTarget {
            scope: StageScope::File(file.new_path.clone()),
            paths: vec![file.new_path.clone()],
        });
    }
    if !item.is_dir {
        return None;
    }

    let section = [DiffSection::Staged, DiffSection::Changes]
        .into_iter()
        .find(|&s| item.path.starts_with(section_key(s)));
    if item.is_section {
        let section = section?;
        return Some(StageTarget {
            scope: StageScope::Section(section),
            paths: collect(&|f| f.section == section),
        });
    }

    let dir = match section {
        Some(s) => item.path.strip_prefix(section_key(s)).ok()?.to_path_buf(),
        None => item.path.clone(),
    };
    let paths =
        collect(&|f| section.is_none_or(|s| f.section == s) && f.new_path.starts_with(&dir));
    Some(StageTarget {
        scope: StageScope::Dir(dir),
        paths,
    })
}

/// Files that `s` (wanted = Changes) or `u` (wanted = Staged) would really
/// change among `paths`: those with an entry in that section.
pub fn files_pending_in(files: &[FileDiff], paths: &[PathBuf], wanted: DiffSection) -> Vec<usize> {
    files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.section == wanted && paths.contains(&f.new_path))
        .map(|(i, _)| i)
        .collect()
}

pub fn build_tree_items(
    files: &[FileDiff],
    filtered_indices: &[usize],
    collapsed_dirs: &std::collections::HashSet<PathBuf>,
    view_mode: FileViewMode,
    language: Language,
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
                    is_section: false,
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

    fn build_node(files: &[FileDiff], indices: &[usize]) -> Node {
        let mut root = Node::default();
        for &idx in indices {
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
        root
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
                is_section: false,
            });

            if !is_collapsed {
                let (sub_add, sub_del) =
                    flatten_node(sub_node, dir_path, depth + 1, files, collapsed_dirs, out);
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
                .map(str::to_string)
                .unwrap_or_else(|| f.display_path());

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
                is_section: false,
            });
            total_add += f.stats.additions;
            total_del += f.stats.deletions;
        }

        (total_add, total_del)
    }

    let mut items = Vec::new();
    if !filtered_indices
        .iter()
        .any(|&i| files[i].section == DiffSection::Staged)
    {
        flatten_node(
            &build_node(files, filtered_indices),
            PathBuf::new(),
            0,
            files,
            collapsed_dirs,
            &mut items,
        );
        return items;
    }

    for section in [DiffSection::Staged, DiffSection::Changes] {
        let indices: Vec<usize> = filtered_indices
            .iter()
            .copied()
            .filter(|&i| files[i].section == section)
            .collect();
        if indices.is_empty() {
            continue;
        }
        let key = section_key(section);
        let is_collapsed = collapsed_dirs.contains(&key);
        let name = match (section, language) {
            (DiffSection::Staged, Language::En) => "Staged",
            (DiffSection::Staged, Language::Pt) => "Staged",
            (DiffSection::Changes, Language::En) => "Changes",
            (DiffSection::Changes, Language::Pt) => "Mudanças",
        };
        items.push(TreeItem {
            name: format!("{} ({})", name, indices.len()),
            path: key.clone(),
            is_dir: true,
            is_collapsed,
            file_index: None,
            status: None,
            stage_status: None,
            additions: indices.iter().map(|&i| files[i].stats.additions).sum(),
            deletions: indices.iter().map(|&i| files[i].stats.deletions).sum(),
            depth: 0,
            is_section: true,
        });
        if !is_collapsed {
            flatten_node(
                &build_node(files, &indices),
                key,
                1,
                files,
                collapsed_dirs,
                &mut items,
            );
        }
    }
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
        ".env" | ".env.local" | ".env.example" | ".env.development" | ".env.production"
        | ".env.test" => {
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
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => {
            ("󰈟 ", Color::Rgb(203, 166, 247))
        }
        "svg" => ("󰈟 ", Color::Rgb(249, 226, 175)),
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => ("󰎈 ", Color::Rgb(249, 226, 175)),
        "mp4" | "mkv" | "webm" | "avi" | "mov" => ("󰕧 ", Color::Rgb(243, 139, 168)),
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => (" ", Color::Rgb(243, 139, 168)),
        // Archives & Compressed
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => {
            (" ", Color::Rgb(249, 226, 175))
        }
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
    transient_title: Option<&str>,
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
        Style::default()
            .fg(theme.key_fg)
            .add_modifier(Modifier::BOLD)
    } else if is_focused {
        Style::default()
            .fg(theme.header_fg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    // Header title with 3 tabs
    let mut title_spans = Vec::new();
    let sel_tab_style = Style::default()
        .fg(theme.text_on(theme.header_fg))
        .bg(theme.header_fg)
        .add_modifier(Modifier::BOLD);
    let norm_tab_style = Style::default().fg(theme.line_num_fg);

    title_spans.push(Span::raw(" "));
    if active_tab == DrawerTab::Changes {
        title_spans.push(Span::styled(
            format!(" [1] {} ", tab_changes_title),
            sel_tab_style,
        ));
    } else {
        title_spans.push(Span::styled(
            format!(" [1] {} ", tab_changes_title),
            norm_tab_style,
        ));
    }

    title_spans.push(Span::styled("│", Style::default().fg(theme.border)));

    if active_tab == DrawerTab::Commits {
        title_spans.push(Span::styled(
            format!(" [2] {} ", tab_commits_title),
            sel_tab_style,
        ));
    } else {
        title_spans.push(Span::styled(
            format!(" [2] {} ", tab_commits_title),
            norm_tab_style,
        ));
    }

    title_spans.push(Span::styled("│", Style::default().fg(theme.border)));

    if active_tab == DrawerTab::Stashes {
        title_spans.push(Span::styled(
            format!(" [3] {} ", tab_stashes_title),
            sel_tab_style,
        ));
    } else {
        title_spans.push(Span::styled(
            format!(" [3] {} ", tab_stashes_title),
            norm_tab_style,
        ));
    }
    title_spans.push(Span::raw(" "));

    if let Some(title) = transient_title {
        title_spans = vec![Span::styled(format!(" {} · Esc ", title), sel_tab_style)];
    } else if let Some(commit) = active_commit_info {
        title_spans = vec![Span::styled(
            format!(
                " Commit {} · Esc ",
                commit.hash.chars().take(7).collect::<String>()
            ),
            sel_tab_style,
        )];
    }

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
                true,
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
    let header_height = if area.height < 16 { 3 } else { 4 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_height), Constraint::Min(3)])
        .split(area);

    let short_hash = &commit.hash[..7.min(commit.hash.len())];
    let mut header_lines = Vec::new();

    let (details_label, back_label) = match language {
        Language::En => ("[i] Details · [o] PR", "[Esc/2] Back"),
        Language::Pt => ("[i] Detalhes · [o] PR", "[Esc/2] Voltar"),
    };

    header_lines.push(Line::from(vec![
        Span::styled(
            format!(" 󰜉 {} ", short_hash),
            Style::default()
                .fg(theme.text_on(theme.header_fg))
                .bg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("👤 {} · {}", commit.author, commit.date),
            Style::default().fg(theme.line_num_fg),
        ),
    ]));

    // Commit subject (wrapped naturally without cutting off)
    let first_line = commit.message.lines().next().unwrap_or("");
    header_lines.push(Line::from(vec![Span::styled(
        format!(" 󰈚 {}", first_line),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )]));

    if header_height >= 4 {
        header_lines.push(Line::from(vec![Span::styled(
            format!(
                " 󰈚 {} files · {} · {}",
                items.len(),
                details_label,
                back_label
            ),
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::DIM),
        )]));
    }

    frame.render_widget(
        Paragraph::new(header_lines).wrap(Wrap { trim: true }),
        chunks[0],
    );

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
        false,
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
    let header_height = if area.height < 16 { 3 } else { 4 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_height), Constraint::Min(3)])
        .split(area);

    let mut header_lines = Vec::new();
    let (details_label, back_label) = match language {
        Language::En => ("[i] Details", "[Esc/3] Back"),
        Language::Pt => ("[i] Detalhes", "[Esc/3] Voltar"),
    };

    header_lines.push(Line::from(vec![
        Span::styled(
            format!(" 󰮎 {} ", stash.selector),
            Style::default()
                .fg(theme.text_on(theme.key_fg))
                .bg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(&stash.date, Style::default().fg(theme.line_num_fg)),
    ]));

    header_lines.push(Line::from(vec![Span::styled(
        format!(" 󰈚 {}", stash.message),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )]));

    if header_height >= 4 {
        header_lines.push(Line::from(vec![Span::styled(
            format!(
                " 󰈚 {} files · {} · {}",
                items.len(),
                details_label,
                back_label
            ),
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::DIM),
        )]));
    }

    frame.render_widget(
        Paragraph::new(header_lines).wrap(Wrap { trim: true }),
        chunks[0],
    );

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
        false,
        theme,
    );
}

/// Smallest Changes-area height (header row included) in which the header may
/// wrap onto a second row.
///
/// A wrapped header takes one more row away from the file list, so the rule is
/// "a two-row header must still leave at least five list rows": 2 header + 5
/// list = 7. In shorter areas the header stays on one row and is reduced
/// instead, so a short pane never trades its list for a second header row.
pub const CHANGES_WRAP_MIN_HEIGHT: u16 = 7;

/// How many rows the Changes header may use in an area of `height` rows.
fn changes_header_budget(height: u16, allow_wrap: bool) -> u16 {
    if allow_wrap && height >= CHANGES_WRAP_MIN_HEIGHT {
        2
    } else {
        1
    }
}

/// Rows the Changes header occupies in the drawer `drawer` (borders included),
/// so mouse hit-testing and the selection overlay agree with what is drawn.
pub fn changes_header_rows(
    drawer: Rect,
    items: &[TreeItem],
    filter_mode: bool,
    filter_query: &str,
    view_mode: FileViewMode,
    language: Language,
) -> u16 {
    if items.is_empty() {
        // "No matching files" is drawn instead of a header.
        return 1;
    }
    let inner = Block::default().borders(Borders::ALL).inner(drawer);
    let file_count = items
        .iter()
        .filter(|item| item.file_index.is_some())
        .count();
    changes_header(
        inner.width,
        changes_header_budget(inner.height, true),
        file_count,
        view_mode,
        language,
        filter_mode.then_some(filter_query),
    )
    .len() as u16
}

/// Lay out the Changes header as one row when it fits, or as two rows
/// (`max_rows >= 2`) keeping the same information. Only when even two rows do
/// not fit is the text reduced, step by step: drop the `Mode:` label, shorten
/// `(N files)` to `(N)`, drop the mode icon, drop the `[t]` hint, then drop the
/// mode itself. In filter mode the query is cut last, and the file count is
/// always the final thing left: it is shown whole or not at all, never partial.
/// Always returns at least one (possibly empty) row.
fn changes_header(
    width: u16,
    max_rows: u16,
    file_count: usize,
    view_mode: FileViewMode,
    language: Language,
    filter_query: Option<&str>,
) -> Vec<String> {
    let max_width = usize::from(width);
    let fits = |text: &str| Line::from(text).width() <= max_width;
    let (icon, mode) = match (view_mode, language) {
        (FileViewMode::Tree, Language::En) => ("", "Folders"),
        (FileViewMode::Tree, Language::Pt) => ("", "Pastas"),
        (FileViewMode::Flat, Language::En) => ("󰈚", "Flat"),
        (FileViewMode::Flat, Language::Pt) => ("󰈚", "Lista"),
    };
    let (label, files) = match language {
        Language::En => ("Mode", "files"),
        Language::Pt => ("Modo", "arquivos"),
    };
    let full_count = format!("({file_count} {files})");
    let compact_count = format!("({file_count})");

    // Place `lead` and `count` on one row, else lead on row 1 and count on row 2.
    let place = |lead: &str, count: &str| -> Option<Vec<String>> {
        let joined = format!("{lead} · {count}");
        if fits(&joined) {
            return Some(vec![joined]);
        }
        let second = format!(" {count}");
        (max_rows >= 2 && fits(lead) && fits(&second)).then(|| vec![lead.to_string(), second])
    };

    // (lead, count) pairs, from the most to the least informative.
    let steps: Vec<(String, &str)> = if let Some(query) = filter_query {
        let filter = match language {
            Language::En => "Filter",
            Language::Pt => "Filtro",
        };
        vec![
            (format!(" 󰍉 {filter}: {query}_"), full_count.as_str()),
            (format!(" 󰍉 {filter}: {query}_"), compact_count.as_str()),
            (format!(" 󰍉 {query}_"), compact_count.as_str()),
        ]
    } else {
        vec![
            (format!(" {label}: {icon} {mode} [t]"), full_count.as_str()),
            (format!(" {icon} {mode} [t]"), full_count.as_str()),
            (format!(" {icon} {mode} [t]"), compact_count.as_str()),
            (format!(" {mode} [t]"), compact_count.as_str()),
            (format!(" {mode}"), compact_count.as_str()),
        ]
    };
    for (lead, count) in &steps {
        if let Some(rows) = place(lead, count) {
            return rows;
        }
    }

    if let Some(query) = filter_query {
        let filter = match language {
            Language::En => "Filter",
            Language::Pt => "Filtro",
        };
        // The query is the last thing cut. On two rows it gets a whole row of its own.
        if max_rows >= 2 {
            let second = [&full_count, &compact_count]
                .into_iter()
                .map(|count| format!(" {count}"))
                .find(|text| fits(text));
            if let (Some(second), Some(first)) = (
                second,
                abbreviate_query(&format!(" 󰍉 {filter}: "), query, max_width),
            ) {
                return vec![first, second];
            }
        }
        // On one row the count stays in front of the abbreviated query.
        let prefix = format!(" {compact_count} · 󰍉 {filter}: ");
        if let Some(row) = abbreviate_query(&prefix, query, max_width) {
            return vec![row];
        }
    }

    [
        format!(" {full_count}"),
        compact_count,
        file_count.to_string(),
    ]
    .into_iter()
    .find(|text| fits(text))
    .map(|text| vec![text])
    // Below the width of the number itself, show nothing rather than a partial number.
    .unwrap_or_else(|| vec![String::new()])
}

/// `prefix` + the longest start of `query` that fits `max_width`, followed by
/// the `_` cursor; `None` when not even one character of the query fits.
fn abbreviate_query(prefix: &str, query: &str, max_width: usize) -> Option<String> {
    let mut abbreviated = String::new();
    for ch in query.chars() {
        let next = format!("{prefix}{abbreviated}{ch}_");
        if Line::from(next.as_str()).width() > max_width {
            break;
        }
        abbreviated.push(ch);
    }
    (!abbreviated.is_empty()).then(|| format!("{prefix}{abbreviated}_"))
}

/// `allow_wrap`: whether the header may take a second row. Only the main
/// Changes tab does; the commit / stash file lists keep their one-row header
/// because their click mapping assumes it.
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
    allow_wrap: bool,
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

    let show_stats = area.width >= 28;
    let file_count = items
        .iter()
        .filter(|item| item.file_index.is_some())
        .count();
    let sub_header = changes_header(
        area.width,
        changes_header_budget(area.height, allow_wrap),
        file_count,
        view_mode,
        language,
        filter_mode.then_some(filter_query),
    );

    let header_style = Style::default()
        .fg(theme.key_fg)
        .add_modifier(Modifier::DIM);
    let header_rows = sub_header.len();
    let mut lines: Vec<Line> = sub_header
        .into_iter()
        .map(|row| Line::from(Span::styled(row, header_style)))
        .collect();

    let max_rows = (area.height as usize).saturating_sub(header_rows);
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
            Span::styled(
                "▎",
                Style::default()
                    .fg(theme.key_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" ", base_style)
        };

        let indent = "  ".repeat(item.depth);

        if item.is_section {
            let bg = if is_selected {
                theme.selected_bg
            } else {
                theme.bg
            };
            let mut spans = vec![
                cursor_span,
                Span::styled(
                    if item.is_collapsed { "▸ " } else { "▾ " },
                    Style::default().fg(theme.line_num_fg).bg(bg),
                ),
                Span::styled(
                    item.name.clone(),
                    Style::default()
                        .fg(theme.header_fg)
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            if show_stats {
                spans.push(Span::styled(
                    format!(" +{} -{}", item.additions, item.deletions),
                    Style::default().fg(theme.line_num_fg).bg(bg),
                ));
            }
            lines.push(Line::from(spans));
        } else if item.is_dir {
            let (dir_icon, dir_color) = if item.is_collapsed {
                (" ", theme.key_fg)
            } else {
                (" ", theme.key_fg)
            };
            let stats_str = format!(" +{} -{}", item.additions, item.deletions);

            let mut spans = vec![
                cursor_span,
                Span::styled(indent, base_style),
                Span::styled(
                    dir_icon,
                    Style::default().fg(dir_color).bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    }),
                ),
                Span::styled(
                    format!("{}/", item.name),
                    base_style.add_modifier(Modifier::BOLD),
                ),
            ];
            if show_stats {
                spans.push(Span::styled(
                    stats_str,
                    Style::default().fg(theme.line_num_fg).bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    }),
                ));
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
                    Style::default()
                        .fg(theme.status_a)
                        .bg(if is_selected {
                            theme.selected_bg
                        } else {
                            theme.bg
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                StageStatus::PartiallyStaged => Span::styled(
                    "± ",
                    Style::default().fg(theme.status_m).bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    }),
                ),
                StageStatus::Unstaged | StageStatus::Untracked => Span::styled("  ", base_style),
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
                Span::styled(
                    status_badge,
                    Style::default().fg(status_color).bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    }),
                ),
                stage_span,
                Span::styled(
                    icon_str,
                    Style::default().fg(icon_color).bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    }),
                ),
                Span::styled(&item.name, name_style),
            ];
            if show_stats {
                spans.push(Span::styled(
                    stats_str,
                    Style::default().fg(theme.line_num_fg).bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    }),
                ));
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
        Language::En => {
            if area.width < 28 {
                format!(" Commits ({})", commits.len())
            } else {
                format!(
                    " Recent Commits ({}) · [Enter] View · [o] PR",
                    commits.len()
                )
            }
        }
        Language::Pt => {
            if area.width < 28 {
                format!(" Commits ({})", commits.len())
            } else {
                format!(
                    " Commits Recentes ({}) · [Enter] Ver · [o] PR",
                    commits.len()
                )
            }
        }
    };
    lines.push(Line::from(Span::styled(
        header_msg,
        Style::default()
            .fg(theme.key_fg)
            .add_modifier(Modifier::DIM),
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
            Span::styled(
                "▎",
                Style::default()
                    .fg(theme.key_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" ", base_style)
        };

        let short_hash = &commit.hash[..7.min(commit.hash.len())];
        let mut spans = vec![
            cursor_span,
            Span::styled(
                format!(" 󰜉 {:<7} ", short_hash),
                Style::default()
                    .fg(theme.key_fg)
                    .bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &commit.message,
                if is_selected {
                    base_style.add_modifier(Modifier::BOLD)
                } else {
                    base_style
                },
            ),
        ];

        if show_date {
            spans.push(Span::styled(
                format!(" ({})", commit.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected {
                    theme.selected_bg
                } else {
                    theme.bg
                }),
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
        Language::En => {
            if area.width < 28 {
                format!(" Stashes ({})", stashes.len())
            } else {
                format!(" Git Stashes ({}) · [Enter] View", stashes.len())
            }
        }
        Language::Pt => {
            if area.width < 28 {
                format!(" Stashes ({})", stashes.len())
            } else {
                format!(" Stashes Git ({}) · [Enter] Ver", stashes.len())
            }
        }
    };
    lines.push(Line::from(Span::styled(
        header_msg,
        Style::default()
            .fg(theme.key_fg)
            .add_modifier(Modifier::DIM),
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
            Span::styled(
                "▎",
                Style::default()
                    .fg(theme.key_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" ", base_style)
        };

        let mut spans = vec![
            cursor_span,
            Span::styled(
                format!(" 󰮎 {:<9} ", stash.selector),
                Style::default()
                    .fg(theme.key_fg)
                    .bg(if is_selected {
                        theme.selected_bg
                    } else {
                        theme.bg
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &stash.message,
                if is_selected {
                    base_style.add_modifier(Modifier::BOLD)
                } else {
                    base_style
                },
            ),
        ];

        if show_date {
            spans.push(Span::styled(
                format!(" ({})", stash.date),
                Style::default().fg(theme.line_num_fg).bg(if is_selected {
                    theme.selected_bg
                } else {
                    theme.bg
                }),
            ));
        }

        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

pub fn render_drawer_line_overlay(
    frame: &mut Frame,
    file_tree_area: Rect,
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
    changes_header_rows: u16,
    theme: &Theme,
) {
    if file_tree_area.width == 0 || file_tree_area.height < 4 {
        return;
    }

    let avail_w = file_tree_area.width.saturating_sub(2) as usize;

    let (spans, row_y) = match active_tab {
        DrawerTab::Changes => {
            if items.is_empty() || selected_file_idx >= items.len() {
                return;
            }
            let item = &items[selected_file_idx];
            let indent = "  ".repeat(item.depth);

            let mut s = vec![
                Span::styled(
                    "▎",
                    Style::default()
                        .fg(theme.key_fg)
                        .bg(theme.selected_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(indent, Style::default().bg(theme.selected_bg)),
            ];

            if item.is_dir {
                let dir_icon = if item.is_collapsed { " " } else { " " };
                let path_text = format!("{}/", item.name);
                let stats = format!(" +{} -{}", item.additions, item.deletions);
                s.push(Span::styled(
                    dir_icon,
                    Style::default().fg(theme.key_fg).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    path_text,
                    Style::default()
                        .fg(theme.selected_fg)
                        .bg(theme.selected_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                s.push(Span::styled(
                    stats,
                    Style::default().fg(theme.line_num_fg).bg(theme.selected_bg),
                ));
            } else {
                let (status_badge, status_color) = match item.status.unwrap_or(FileStatus::Modified)
                {
                    FileStatus::Modified => ("● ", theme.status_m),
                    FileStatus::Added => ("✚ ", theme.status_a),
                    FileStatus::Deleted => ("✖ ", theme.status_d),
                    FileStatus::Untracked => ("? ", theme.status_u),
                    FileStatus::Renamed => ("➜ ", theme.key_fg),
                    FileStatus::Copied => ("✚ ", theme.status_a),
                };
                let (stage_badge, stage_color) =
                    match item.stage_status.unwrap_or(StageStatus::Unstaged) {
                        StageStatus::Staged => ("󰄬 ", theme.status_a),
                        StageStatus::PartiallyStaged => ("󰄬* ", theme.key_fg),
                        StageStatus::Untracked => ("? ", theme.status_u),
                        StageStatus::Unstaged => ("  ", theme.line_num_fg),
                    };
                let (file_ico, ico_color) = file_icon(&item.name);
                let path_text = item.name.clone();
                let stats = format!(" +{} -{}", item.additions, item.deletions);

                s.push(Span::styled(
                    status_badge,
                    Style::default().fg(status_color).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    stage_badge,
                    Style::default()
                        .fg(stage_color)
                        .bg(theme.selected_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                s.push(Span::styled(
                    file_ico,
                    Style::default().fg(ico_color).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    path_text,
                    Style::default()
                        .fg(theme.selected_fg)
                        .bg(theme.selected_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                s.push(Span::styled(
                    stats,
                    Style::default().fg(theme.line_num_fg).bg(theme.selected_bg),
                ));
            }
            s.push(Span::styled(
                " ▏",
                Style::default().fg(theme.key_fg).bg(theme.selected_bg),
            ));

            let commit_header_h = if file_tree_area.height.saturating_sub(2) < 16 {
                3
            } else {
                4
            };
            let y = if active_commit_info.is_some() || active_stash_info.is_some() {
                file_tree_area.y
                    + 2
                    + commit_header_h
                    + (selected_file_idx.saturating_sub(file_scroll)) as u16
            } else {
                // Border, then the (possibly wrapped) Changes header, then the list.
                file_tree_area.y
                    + 1
                    + changes_header_rows.max(1)
                    + (selected_file_idx.saturating_sub(file_scroll)) as u16
            };
            (s, y)
        }
        DrawerTab::Commits => {
            if active_commit_info.is_some() {
                if items.is_empty() || selected_file_idx >= items.len() {
                    return;
                }
                let item = &items[selected_file_idx];
                let indent = "  ".repeat(item.depth);
                let mut s = vec![
                    Span::styled(
                        "▎",
                        Style::default()
                            .fg(theme.key_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(indent, Style::default().bg(theme.selected_bg)),
                ];
                let (file_ico, ico_color) = file_icon(&item.name);
                s.push(Span::styled(
                    file_ico,
                    Style::default().fg(ico_color).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    item.name.clone(),
                    Style::default()
                        .fg(theme.selected_fg)
                        .bg(theme.selected_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                s.push(Span::styled(
                    format!(" +{} -{}", item.additions, item.deletions),
                    Style::default().fg(theme.line_num_fg).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    " ▏",
                    Style::default().fg(theme.key_fg).bg(theme.selected_bg),
                ));
                let commit_header_h = if file_tree_area.height.saturating_sub(2) < 16 {
                    3
                } else {
                    4
                };
                let y = file_tree_area.y
                    + 2
                    + commit_header_h
                    + (selected_file_idx.saturating_sub(file_scroll)) as u16;
                (s, y)
            } else {
                if commits.is_empty() || selected_commit_idx >= commits.len() {
                    return;
                }
                let commit = &commits[selected_commit_idx];
                let short_h = &commit.hash[..7.min(commit.hash.len())];
                let subject = commit.message.lines().next().unwrap_or("").trim();
                let s = vec![
                    Span::styled(
                        "▎",
                        Style::default()
                            .fg(theme.key_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {} ", short_h),
                        Style::default()
                            .fg(theme.header_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        subject,
                        Style::default()
                            .fg(theme.selected_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({}) · 👤 {}", commit.date, commit.author),
                        Style::default().fg(theme.line_num_fg).bg(theme.selected_bg),
                    ),
                    Span::styled(
                        " ▏",
                        Style::default().fg(theme.key_fg).bg(theme.selected_bg),
                    ),
                ];
                let y = file_tree_area.y
                    + 2
                    + (selected_commit_idx.saturating_sub(commit_scroll)) as u16;
                (s, y)
            }
        }
        DrawerTab::Stashes => {
            if active_stash_info.is_some() {
                if items.is_empty() || selected_file_idx >= items.len() {
                    return;
                }
                let item = &items[selected_file_idx];
                let indent = "  ".repeat(item.depth);
                let mut s = vec![
                    Span::styled(
                        "▎",
                        Style::default()
                            .fg(theme.key_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(indent, Style::default().bg(theme.selected_bg)),
                ];
                let (file_ico, ico_color) = file_icon(&item.name);
                s.push(Span::styled(
                    file_ico,
                    Style::default().fg(ico_color).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    item.name.clone(),
                    Style::default()
                        .fg(theme.selected_fg)
                        .bg(theme.selected_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                s.push(Span::styled(
                    format!(" +{} -{}", item.additions, item.deletions),
                    Style::default().fg(theme.line_num_fg).bg(theme.selected_bg),
                ));
                s.push(Span::styled(
                    " ▏",
                    Style::default().fg(theme.key_fg).bg(theme.selected_bg),
                ));
                let commit_header_h = if file_tree_area.height.saturating_sub(2) < 16 {
                    3
                } else {
                    4
                };
                let y = file_tree_area.y
                    + 2
                    + commit_header_h
                    + (selected_file_idx.saturating_sub(file_scroll)) as u16;
                (s, y)
            } else {
                if stashes.is_empty() || selected_stash_idx >= stashes.len() {
                    return;
                }
                let stash = &stashes[selected_stash_idx];
                let s = vec![
                    Span::styled(
                        "▎",
                        Style::default()
                            .fg(theme.key_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" 󰮎 {:<9} ", stash.selector),
                        Style::default()
                            .fg(theme.key_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        &stash.message,
                        Style::default()
                            .fg(theme.selected_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({})", stash.date),
                        Style::default().fg(theme.line_num_fg).bg(theme.selected_bg),
                    ),
                    Span::styled(
                        " ▏",
                        Style::default().fg(theme.key_fg).bg(theme.selected_bg),
                    ),
                ];
                let y =
                    file_tree_area.y + 2 + (selected_stash_idx.saturating_sub(stash_scroll)) as u16;
                (s, y)
            }
        }
    };

    let total_len = spans
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>();

    // ONLY show overlay if the line content overflows the drawer's inner width!
    if total_len <= avail_w {
        return;
    }

    let commit_header_h = if file_tree_area.height.saturating_sub(2) < 16 {
        3
    } else {
        4
    };
    let min_y = if active_commit_info.is_some() || active_stash_info.is_some() {
        file_tree_area.y + 2 + commit_header_h
    } else {
        file_tree_area.y + 2
    };

    if row_y < min_y || row_y >= file_tree_area.y + file_tree_area.height.saturating_sub(1) {
        return;
    }

    let screen_w = frame.area().width;
    let overlay_w = (total_len as u16 + 1).min(screen_w.saturating_sub(file_tree_area.x + 1));

    let overlay_rect = Rect {
        x: file_tree_area.x,
        y: row_y,
        width: overlay_w,
        height: 1,
    };

    use ratatui::widgets::Clear;
    frame.render_widget(Clear, overlay_rect);
    frame.render_widget(Paragraph::new(Line::from(spans)), overlay_rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::ChangeStats;
    use std::collections::HashSet;
    use std::path::Path;

    fn file(path: &str, section: DiffSection) -> FileDiff {
        FileDiff {
            old_path: None,
            new_path: PathBuf::from(path),
            status: FileStatus::Modified,
            stage_status: StageStatus::Unstaged,
            section,
            stats: ChangeStats::default(),
            hunks: Vec::new(),
            aligned_rows: Vec::new(),
            is_binary: false,
        }
    }

    const TREE_ICON: &str = "\u{f07b}";
    const FLAT_ICON: &str = "\u{f021a}";
    const SEARCH_ICON: &str = "\u{f0349}";

    /// Every (mode, language) with its full, unreduced mode text and count word.
    fn header_cases() -> Vec<(FileViewMode, Language, String, &'static str)> {
        vec![
            (
                FileViewMode::Tree,
                Language::En,
                format!(" Mode: {TREE_ICON} Folders [t]"),
                "files",
            ),
            (
                FileViewMode::Tree,
                Language::Pt,
                format!(" Modo: {TREE_ICON} Pastas [t]"),
                "arquivos",
            ),
            (
                FileViewMode::Flat,
                Language::En,
                format!(" Mode: {FLAT_ICON} Flat [t]"),
                "files",
            ),
            (
                FileViewMode::Flat,
                Language::Pt,
                format!(" Modo: {FLAT_ICON} Lista [t]"),
                "arquivos",
            ),
        ]
    }

    fn filter_lead(language: Language, query: &str) -> String {
        let filter = match language {
            Language::En => "Filter",
            Language::Pt => "Filtro",
        };
        format!(" {SEARCH_ICON} {filter}: {query}_")
    }

    fn text_width(text: &str) -> usize {
        Line::from(text).width()
    }

    /// Narrowest width at which `rows` shows everything unreduced (two rows
    /// when `wrapped`, else one joined row).
    fn full_width(lead: &str, count: &str, wrapped: bool) -> usize {
        if wrapped {
            text_width(lead).max(text_width(&format!(" {count}")))
        } else {
            text_width(&format!("{lead} · {count}"))
        }
    }

    #[test]
    fn changes_header_uses_one_row_when_everything_fits() {
        for count in [1, 12, 1234] {
            for (mode, language, lead, files) in header_cases() {
                let count_text = format!("({count} {files})");
                for width in [full_width(&lead, &count_text, false) as u16, 80, 200] {
                    for max_rows in [1, 2] {
                        let plain = changes_header(width, max_rows, count, mode, language, None);
                        assert_eq!(plain, vec![format!("{lead} · {count_text}")]);

                        let query = "abc";
                        let filter_lead = filter_lead(language, query);
                        let filtered =
                            changes_header(200, max_rows, count, mode, language, Some(query));
                        assert_eq!(filtered, vec![format!("{filter_lead} · {count_text}")]);
                    }
                }
            }
        }
        assert_eq!(
            changes_header(80, 2, 1234, FileViewMode::Tree, Language::En, None),
            vec![format!(" Mode: {TREE_ICON} Folders [t] · (1234 files)")]
        );
        assert_eq!(
            changes_header(80, 2, 1234, FileViewMode::Flat, Language::Pt, None),
            vec![format!(" Modo: {FLAT_ICON} Lista [t] · (1234 arquivos)")]
        );
    }

    #[test]
    fn changes_header_wraps_to_a_second_row_keeping_every_piece_of_information() {
        for count in [1, 12, 1234] {
            for (mode, language, lead, files) in header_cases() {
                let count_text = format!("({count} {files})");
                let one_row = full_width(&lead, &count_text, false);
                let two_rows = full_width(&lead, &count_text, true);
                assert!(two_rows < one_row);
                for width in two_rows..one_row {
                    let rows = changes_header(width as u16, 2, count, mode, language, None);
                    assert_eq!(
                        rows,
                        vec![lead.clone(), format!(" {count_text}")],
                        "width={width}"
                    );
                }

                // Filter mode wraps the same way: whole query on row 1, count on row 2.
                let query = "abc";
                let filter_lead = filter_lead(language, query);
                let one_row = full_width(&filter_lead, &count_text, false);
                let two_rows = full_width(&filter_lead, &count_text, true);
                for width in two_rows..one_row {
                    let rows = changes_header(width as u16, 2, count, mode, language, Some(query));
                    assert_eq!(rows, vec![filter_lead.clone(), format!(" {count_text}")]);
                }
            }
        }
    }

    #[test]
    fn changes_header_reduces_only_when_two_rows_do_not_fit() {
        for count in [1, 12, 1234] {
            for (mode, language, lead, files) in header_cases() {
                let count_text = format!("({count} {files})");
                let two_rows = full_width(&lead, &count_text, true);
                let mode_name = lead
                    .split_whitespace()
                    .nth(2)
                    .expect("mode name")
                    .to_string();
                for width in 0..=120u16 {
                    let rows = changes_header(width, 2, count, mode, language, None);
                    let text = rows.join("\n");
                    let intact = text.contains(&lead) && text.contains(&count_text);
                    assert_eq!(
                        intact,
                        usize::from(width) >= two_rows,
                        "width={width} count={count} {mode:?}/{language:?}: {rows:?}"
                    );
                    // Reduction is progressive: a piece is only gone once every
                    // cheaper-to-lose piece before it is gone.
                    if text.contains("Mode:") || text.contains("Modo:") {
                        assert!(text.contains("[t]") && text.contains(&count_text));
                    }
                    if text.contains("[t]") {
                        assert!(text.contains(&mode_name), "{rows:?}");
                    }
                    if text.contains(&mode_name) {
                        assert!(text.contains(&format!("({count}")), "{rows:?}");
                    }
                }

                // Without room for a second row the single row is reduced instead.
                let one_row = full_width(&lead, &count_text, false);
                for width in two_rows..one_row {
                    let rows = changes_header(width as u16, 1, count, mode, language, None);
                    assert_eq!(rows.len(), 1);
                    assert!(text_width(&rows[0]) <= width);
                    assert!(rows[0].contains(&count.to_string()));
                    assert_ne!(rows[0], format!("{lead} · {count_text}"));
                }
            }
        }

        // Exact single-row ladder for EN / Folders / 1234 files: each rung is
        // the previous one minus one thing, down to the bare number.
        let ladder = |width| changes_header(width, 1, 1234, FileViewMode::Tree, Language::En, None);
        let rungs = [
            (35, format!(" Mode: {TREE_ICON} Folders [t] · (1234 files)")),
            (29, format!(" {TREE_ICON} Folders [t] · (1234 files)")),
            (23, format!(" {TREE_ICON} Folders [t] · (1234)")),
            (21, " Folders [t] · (1234)".to_string()),
            (17, " Folders · (1234)".to_string()),
            (13, " (1234 files)".to_string()),
            (6, "(1234)".to_string()),
            (4, "1234".to_string()),
            (0, String::new()),
        ];
        for (i, (min_width, text)) in rungs.iter().enumerate() {
            assert_eq!(ladder(*min_width), vec![text.clone()], "width={min_width}");
            if let Some((next_min, _)) = rungs.get(i + 1) {
                // One column narrower than a rung's minimum falls to the next rung
                // (or lower), never straight to a partial number.
                assert_eq!(
                    ladder(min_width - 1).concat(),
                    rungs[i + 1..]
                        .iter()
                        .find(|(m, _)| *m < *min_width)
                        .map(|(_, t)| t.clone())
                        .unwrap(),
                    "width={}",
                    min_width - 1
                );
                assert!(next_min < min_width);
            }
        }
    }

    #[test]
    fn changes_header_count_is_never_partial() {
        for language in [Language::En, Language::Pt] {
            for mode in [FileViewMode::Tree, FileViewMode::Flat] {
                for count in [1, 12, 1234, usize::MAX] {
                    let digits = count.to_string();
                    for filter in [None, Some("very long filter query 界")] {
                        for max_rows in [1, 2] {
                            for width in 0..=80u16 {
                                let rows =
                                    changes_header(width, max_rows, count, mode, language, filter);
                                assert!(!rows.is_empty() && rows.len() <= usize::from(max_rows));
                                assert!(rows.iter().all(|r| text_width(r) <= usize::from(width)));
                                let text = rows.join(" ");
                                // Every digit run in the header is the whole number.
                                for run in text
                                    .split(|c: char| !c.is_ascii_digit())
                                    .filter(|run| !run.is_empty())
                                {
                                    assert_eq!(run, digits, "partial count: {rows:?}");
                                }
                                if usize::from(width) >= digits.len() {
                                    assert!(text.contains(&digits), "width={width}: {rows:?}");
                                } else {
                                    assert_eq!(rows, vec![String::new()]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn changes_filter_header_cuts_the_query_last_and_keeps_the_count() {
        let query = "very long filter query 界";
        // The query gets a whole row of its own before it is abbreviated.
        let rows = changes_header(40, 2, 1234, FileViewMode::Tree, Language::En, Some(query));
        assert_eq!(rows[0], filter_lead(Language::En, query));
        assert_eq!(rows[1], " (1234 files)");

        // Too narrow even for that: the query is abbreviated, the count stays whole.
        let rows = changes_header(20, 2, 1234, FileViewMode::Tree, Language::En, Some(query));
        assert_eq!(rows.len(), 2);
        assert!(rows[0].starts_with(&format!(" {SEARCH_ICON} Filter: very")));
        assert_eq!(rows[1], " (1234 files)");

        // One row: the count is kept in front of the abbreviated query.
        let rows = changes_header(30, 1, 1234, FileViewMode::Tree, Language::En, Some(query));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].starts_with(" (1234) · "), "{rows:?}");
        assert!(text_width(&rows[0]) <= 30);

        // No room for any of the query: only the count survives.
        let rows = changes_header(10, 2, 1234, FileViewMode::Tree, Language::En, Some(query));
        assert_eq!(rows, vec!["(1234)".to_string()]);
    }

    #[test]
    fn changes_header_wraps_only_in_panes_tall_enough_to_keep_a_list() {
        assert_eq!(CHANGES_WRAP_MIN_HEIGHT, 7);
        for height in 0..CHANGES_WRAP_MIN_HEIGHT {
            assert_eq!(changes_header_budget(height, true), 1, "height={height}");
        }
        for height in CHANGES_WRAP_MIN_HEIGHT..40 {
            assert_eq!(changes_header_budget(height, true), 2, "height={height}");
            // A two-row header always leaves at least five list rows.
            assert!(height - 2 >= 5);
        }
        assert_eq!(changes_header_budget(40, false), 1);
    }

    /// Renders the whole drawer and returns its interior rows (border columns cut).
    fn drawer_rows(
        items: &[TreeItem],
        width: u16,
        height: u16,
        filter_mode: bool,
        mode: FileViewMode,
        language: Language,
    ) -> Vec<String> {
        use ratatui::{backend::TestBackend, Terminal};

        let theme = Theme::vscode_dark();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                render_drawer(
                    frame,
                    frame.area(),
                    DrawerTab::Changes,
                    items,
                    0,
                    0,
                    &[],
                    0,
                    0,
                    &[],
                    0,
                    0,
                    None,
                    None,
                    true,
                    filter_mode,
                    "very long filter query 界",
                    mode,
                    language,
                    &theme,
                    None,
                );
            })
            .unwrap();
        (1..height - 1)
            .map(|y| {
                (1..width - 1)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn changes_count_survives_drawer_width_modes_and_languages() {
        for count in [1, 12, 1234] {
            let files: Vec<_> = (0..count)
                .map(|i| file(&format!("src/file{i}.rs"), DiffSection::Changes))
                .collect();
            let indices: Vec<_> = (0..count).collect();
            for mode in [FileViewMode::Tree, FileViewMode::Flat] {
                for language in [Language::En, Language::Pt] {
                    let items = build_tree_items(&files, &indices, &HashSet::new(), mode, language);
                    for width in [20, 30, 40, 60, 80] {
                        for filter_mode in [false, true] {
                            // 6 rows: a short pane (reduced single-row header);
                            // 14 rows: tall enough for the wrapped header.
                            for height in [6u16, 14] {
                                let rows =
                                    drawer_rows(&items, width, height, filter_mode, mode, language);
                                let header_rows = changes_header_rows(
                                    Rect::new(0, 0, width, height),
                                    &items,
                                    filter_mode,
                                    "very long filter query 界",
                                    mode,
                                    language,
                                ) as usize;
                                let header = rows[..header_rows].join("\n");
                                assert!(
                                    header.contains(&format!("({count})"))
                                        || header.contains(&format!("({count} files)"))
                                        || header.contains(&format!("({count} arquivos)")),
                                    "width={width}, height={height}, count={count}, mode={mode:?}, language={language:?}, filter={filter_mode}: {header}"
                                );
                                assert!(
                                    rows[header_rows].contains("src"),
                                    "list must start right below the header: {rows:?}"
                                );
                                assert!(
                                    rows[header_rows - 1].trim() != ""
                                        && !rows[header_rows - 1].contains("src/"),
                                    "last header row is not a list row"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn changes_drawer_wraps_without_losing_info_and_keeps_the_list() {
        let files: Vec<_> = (0..40)
            .map(|i| file(&format!("src/file{i}.rs"), DiffSection::Changes))
            .collect();
        let indices: Vec<_> = (0..40).collect();
        let items = build_tree_items(
            &files,
            &indices,
            &HashSet::new(),
            FileViewMode::Flat,
            Language::En,
        );

        // 28 columns: the single-row header does not fit, two rows do.
        let rows = drawer_rows(&items, 28, 14, false, FileViewMode::Flat, Language::En);
        assert_eq!(rows[0].trim_end(), format!(" Mode: {FLAT_ICON} Flat [t]"));
        assert_eq!(rows[1].trim_end(), " (40 files)");
        assert!(rows[2].contains("file0.rs"));

        // At 80 columns it all stays on one row, so the list gains a row.
        let rows = drawer_rows(&items, 80, 14, false, FileViewMode::Flat, Language::En);
        assert_eq!(
            rows[0].trim_end(),
            format!(" Mode: {FLAT_ICON} Flat [t] · (40 files)")
        );
        assert!(rows[1].contains("file0.rs"));

        // Short panes keep one (reduced) header row and every row below it is list.
        for height in 4..=8u16 {
            let rows = drawer_rows(&items, 22, height, false, FileViewMode::Flat, Language::En);
            let list = rows.iter().filter(|r| r.contains(".rs")).count();
            assert_eq!(list, usize::from(height - 3), "height={height}: {rows:?}");
        }

        // Tall enough to wrap: at least five list rows remain.
        let rows = drawer_rows(&items, 22, 9, false, FileViewMode::Flat, Language::En);
        assert_eq!(rows.iter().filter(|r| r.contains(".rs")).count(), 5);
    }

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    fn tree(files: &[FileDiff], collapsed: &[&str]) -> Vec<TreeItem> {
        let collapsed: HashSet<PathBuf> = collapsed.iter().map(PathBuf::from).collect();
        let all: Vec<usize> = (0..files.len()).collect();
        build_tree_items(files, &all, &collapsed, FileViewMode::Tree, Language::En)
    }

    fn target_at(files: &[FileDiff], items: &[TreeItem], path: &str) -> StageTarget {
        let item = items
            .iter()
            .find(|i| i.path == Path::new(path))
            .unwrap_or_else(|| panic!("no tree item {}", path));
        let all: Vec<usize> = (0..files.len()).collect();
        resolve_stage_target(item, files, &all).unwrap()
    }

    #[test]
    fn stage_target_of_a_file_is_that_file() {
        let files = vec![
            file("src/a.rs", DiffSection::Changes),
            file("src/b.rs", DiffSection::Changes),
        ];
        let items = tree(&files, &[]);
        let t = target_at(&files, &items, "src/b.rs");
        assert_eq!(t.scope, StageScope::File(PathBuf::from("src/b.rs")));
        assert_eq!(t.paths, paths(&["src/b.rs"]));
    }

    #[test]
    fn stage_target_of_a_directory_is_recursive_even_when_collapsed() {
        let files = vec![
            file("src/a.rs", DiffSection::Changes),
            file("src/ui/app.rs", DiffSection::Changes),
            file("src/ui/widgets/w.rs", DiffSection::Changes),
            file("srcfoo/x.rs", DiffSection::Changes),
            file("top.txt", DiffSection::Changes),
        ];
        let items = tree(&files, &[]);
        let t = target_at(&files, &items, "src");
        assert_eq!(t.scope, StageScope::Dir(PathBuf::from("src")));
        assert_eq!(
            t.paths,
            paths(&["src/a.rs", "src/ui/app.rs", "src/ui/widgets/w.rs"]),
            "subdirectories included, sibling `srcfoo` excluded"
        );
        let t = target_at(&files, &items, "src/ui");
        assert_eq!(t.paths, paths(&["src/ui/app.rs", "src/ui/widgets/w.rs"]));

        let items = tree(&files, &["src"]);
        assert!(items
            .iter()
            .all(|i| i.path == Path::new("src") || !i.path.starts_with("src")));
        let t = target_at(&files, &items, "src");
        assert_eq!(t.paths.len(), 3, "collapsed folder still covers its files");
    }

    #[test]
    fn stage_target_inside_a_section_stays_in_that_section() {
        let files = vec![
            file("src/a.rs", DiffSection::Staged),
            file("src/a.rs", DiffSection::Changes),
            file("src/b.rs", DiffSection::Changes),
            file("docs/c.md", DiffSection::Staged),
        ];
        let items = tree(&files, &[]);

        let t = target_at(&files, &items, ":changes/src");
        assert_eq!(t.scope, StageScope::Dir(PathBuf::from("src")));
        assert_eq!(t.paths, paths(&["src/a.rs", "src/b.rs"]));
        let t = target_at(&files, &items, ":staged/src");
        assert_eq!(t.scope, StageScope::Dir(PathBuf::from("src")));
        assert_eq!(t.paths, paths(&["src/a.rs"]));

        let t = target_at(&files, &items, ":staged");
        assert_eq!(t.scope, StageScope::Section(DiffSection::Staged));
        assert_eq!(t.paths, paths(&["docs/c.md", "src/a.rs"]));
        let t = target_at(&files, &items, ":changes");
        assert_eq!(t.scope, StageScope::Section(DiffSection::Changes));
        assert_eq!(t.paths, paths(&["src/a.rs", "src/b.rs"]));
    }

    #[test]
    fn stage_target_only_covers_files_visible_through_the_filter() {
        let files = vec![
            file("src/a.rs", DiffSection::Changes),
            file("src/b.rs", DiffSection::Changes),
        ];
        let visible = vec![1];
        let items = build_tree_items(
            &files,
            &visible,
            &HashSet::new(),
            FileViewMode::Tree,
            Language::En,
        );
        let dir = items.iter().find(|i| i.is_dir).unwrap();
        let t = resolve_stage_target(dir, &files, &visible).unwrap();
        assert_eq!(t.paths, paths(&["src/b.rs"]));
    }

    #[test]
    fn files_pending_in_keeps_only_entries_of_the_wanted_section() {
        let files = vec![
            file("src/a.rs", DiffSection::Staged),
            file("src/a.rs", DiffSection::Changes),
            file("src/b.rs", DiffSection::Changes),
            file("docs/c.md", DiffSection::Staged),
        ];
        let target = paths(&["src/a.rs", "src/b.rs"]);
        assert_eq!(
            files_pending_in(&files, &target, DiffSection::Changes),
            vec![1, 2]
        );
        assert_eq!(
            files_pending_in(&files, &target, DiffSection::Staged),
            vec![0]
        );
        assert!(files_pending_in(&files, &paths(&["src/b.rs"]), DiffSection::Staged).is_empty());
    }

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
