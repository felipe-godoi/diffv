use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::config::Config;
use crate::core::engine::DiffEngine;
use crate::core::models::{
    CommitEntry, DrawerTab, FileDiff, Language, RepoStats, StashEntry, WorktreeEntry,
};
use crate::git::actions::{
    discard_file, discard_hunk, stage_file, stage_hunk, stage_partial_hunk, unstage_file,
    unstage_hunk,
};
use crate::git::provider::GitProvider;
use crate::integration::clipboard::copy_hunk_as_markdown;
use crate::ui::components::file_tree::{
    build_tree_items, render_drawer, FileViewMode, TreeItem,
};
use crate::ui::components::header::render_header;
use crate::ui::components::help_popup::{render_confirm_popup, render_help_popup};
use crate::ui::components::history_popup::render_history_popup;
use crate::ui::components::ruler::render_ruler;
use crate::ui::components::side_by_side::{render_side_by_side, ColumnSide};
use crate::ui::components::status_bar::render_status_bar;
use crate::ui::components::unified::render_unified;
use crate::ui::components::worktree_popup::render_worktree_popup;
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    FileTree,
    DiffView,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DiscardHunk(usize),
    DiscardFile,
}

pub enum AppMode {
    Git {
        target_ref: Option<String>,
        git_provider: GitProvider,
    },
    FilePair(PathBuf, PathBuf),
    DirPair(PathBuf, PathBuf),
    Stdin,
}

pub struct App {
    pub mode: AppMode,
    pub files: Vec<FileDiff>,
    pub filtered_indices: Vec<usize>,
    pub selected_filtered_idx: usize,
    pub file_tree_scroll: usize,
    pub repo_stats: RepoStats,
    pub focus: Focus,
    pub is_unified: bool,
    pub watch_mode: bool,
    pub show_help: bool,
    pub filter_mode: bool,
    pub filter_query: String,
    pub scroll_y: usize,
    pub selected_row: usize,
    pub notification: Option<(String, Instant)>,
    pub confirm_action: Option<(ConfirmAction, String)>,
    pub config: Config,
    pub theme: Theme,
    pub should_quit: bool,
    pub editor_request: Option<(PathBuf, usize)>,
    pub staged_only: bool,
    pub ignore_whitespace: bool,

    // Visual mode & column side
    pub visual_mode: bool,
    pub visual_anchor: usize,
    pub column_side: ColumnSide,

    // File tree mode & directory collapsing
    pub file_view_mode: FileViewMode,
    pub collapsed_dirs: HashSet<PathBuf>,
    pub tree_items: Vec<TreeItem>,
    pub selected_tree_idx: usize,

    // File commit history (popup for single file history 'H')
    pub show_history: bool,
    pub history_commits: Vec<CommitEntry>,
    pub selected_history_idx: usize,
    pub history_scroll: usize,
    pub active_commit_view: Option<(String, FileDiff)>,

    // Layout resizing & viewport geometry
    pub file_tree_width: u16,
    pub show_drawer: bool,
    pub is_dragging_divider: bool,
    pub viewport_height: usize,
    pub file_tree_height: usize,

    // Internationalization & Drawer Tabs
    pub language: Language,
    pub drawer_tab: DrawerTab,
    pub repo_commits: Vec<CommitEntry>,
    pub selected_repo_commit_idx: usize,
    pub repo_commit_scroll: usize,
    pub stashes: Vec<StashEntry>,
    pub selected_stash_idx: usize,
    pub stash_scroll: usize,

    // Worktrees modal switcher
    pub show_worktrees: bool,
    pub worktrees: Vec<WorktreeEntry>,
    pub selected_worktree_idx: usize,
    pub worktree_scroll: usize,

    // Snapshot of live diffs when viewing a commit or stash diff
    pub live_snapshot: Option<(Vec<FileDiff>, RepoStats)>,
}

impl App {
    pub fn new(
        mode: AppMode,
        config: Config,
        watch_mode: bool,
        staged_only: bool,
        unified: bool,
        theme_override: Option<String>,
        ignore_whitespace: bool,
        history_mode: bool,
    ) -> anyhow::Result<Self> {
        let theme_name = theme_override.unwrap_or_else(|| config.ui.theme.clone());
        let theme = Theme::from_name(&theme_name);

        let (worktrees, active_wt_idx) = match &mode {
            AppMode::Git { git_provider, .. } => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| git_provider.repo_root.clone());
                let list = git_provider.get_worktrees(&cwd).unwrap_or_default();
                let idx = list.iter().position(|w| w.is_current).unwrap_or(0);
                (list, idx)
            }
            _ => (Vec::new(), 0),
        };

        let (repo_commits, stashes) = match &mode {
            AppMode::Git { git_provider, .. } => {
                let c = git_provider.get_repo_commits(50).unwrap_or_default();
                let s = git_provider.get_stashes().unwrap_or_default();
                (c, s)
            }
            _ => (Vec::new(), Vec::new()),
        };

        let mut app = Self {
            mode,
            files: Vec::new(),
            filtered_indices: Vec::new(),
            selected_filtered_idx: 0,
            file_tree_scroll: 0,
            repo_stats: RepoStats::default(),
            focus: Focus::FileTree,
            is_unified: unified || config.ui.default_view == "unified",
            watch_mode,
            show_help: false,
            filter_mode: false,
            filter_query: String::new(),
            scroll_y: 0,
            selected_row: 0,
            notification: None,
            confirm_action: None,
            config,
            theme,
            should_quit: false,
            editor_request: None,
            staged_only,
            ignore_whitespace,
            visual_mode: false,
            visual_anchor: 0,
            column_side: ColumnSide::Right,
            file_view_mode: FileViewMode::Tree, // Folders ("Pastas") is default!
            collapsed_dirs: HashSet::new(),
            tree_items: Vec::new(),
            selected_tree_idx: 0,
            show_history: false,
            history_commits: Vec::new(),
            selected_history_idx: 0,
            history_scroll: 0,
            active_commit_view: None,
            file_tree_width: 32,
            show_drawer: true,
            is_dragging_divider: false,
            viewport_height: 25,
            file_tree_height: 25,
            language: Language::En, // English is default!
            drawer_tab: DrawerTab::Changes,
            repo_commits,
            selected_repo_commit_idx: 0,
            repo_commit_scroll: 0,
            stashes,
            selected_stash_idx: 0,
            stash_scroll: 0,
            show_worktrees: false,
            worktrees,
            selected_worktree_idx: active_wt_idx,
            worktree_scroll: 0,
            live_snapshot: None,
        };

        app.reload_diffs_internal(false)?;
        if app.files.is_empty() {
            let msg = match app.language {
                Language::En => "No differences found.",
                Language::Pt => "Nenhuma alteração encontrada.",
            };
            app.set_notification(msg);
        }

        if history_mode {
            app.open_file_history();
        }

        Ok(app)
    }

    pub fn current_file(&self) -> Option<&FileDiff> {
        if let Some((_, commit_diff)) = &self.active_commit_view {
            return Some(commit_diff);
        }
        self.get_underlying_file()
    }

    pub fn get_underlying_file(&self) -> Option<&FileDiff> {
        if self.file_view_mode == FileViewMode::Tree {
            if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                if let Some(idx) = item.file_index {
                    return self.files.get(idx);
                }
            }
            return None;
        }

        if self.filtered_indices.is_empty() {
            return None;
        }
        let real_idx = self.filtered_indices[self.selected_filtered_idx];
        self.files.get(real_idx)
    }

    pub fn set_notification(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), Instant::now()));
    }

    pub fn reload_diffs(&mut self) {
        if let Err(e) = self.reload_diffs_internal(true) {
            self.set_notification(format!("Reload error: {}", e));
        }
    }

    fn reload_diffs_internal(&mut self, is_live_reload: bool) -> anyhow::Result<()> {
        let previous_path = self.current_file().map(|f| f.new_path.clone());
        let prev_scroll = self.scroll_y;
        let prev_row = self.selected_row;

        let diff_engine = DiffEngine::new(
            &self.config.diff.algorithm,
            self.config.diff.context_lines,
            self.ignore_whitespace,
        );

        match &self.mode {
            AppMode::Git { target_ref, git_provider } => {
                let (files, stats) = git_provider.load_diffs(
                    target_ref.as_deref(),
                    self.staged_only,
                    self.config.watcher.watch_untracked,
                    self.ignore_whitespace,
                )?;
                self.files = files;
                self.repo_stats = stats;
            }
            AppMode::FilePair(a, b) => {
                let diff = diff_engine.compare_files(a, b)?;
                self.repo_stats = RepoStats {
                    repo_name: "files".to_string(),
                    branch: "local".to_string(),
                    root_dir: PathBuf::from("."),
                    total_additions: diff.stats.additions,
                    total_deletions: diff.stats.deletions,
                    file_count: 1,
                };
                self.files = vec![diff];
            }
            AppMode::DirPair(a, b) => {
                let files = diff_engine.compare_directories(a, b)?;
                let mut total_add = 0;
                let mut total_del = 0;
                for f in &files {
                    total_add += f.stats.additions;
                    total_del += f.stats.deletions;
                }
                self.repo_stats = RepoStats {
                    repo_name: "directories".to_string(),
                    branch: "local".to_string(),
                    root_dir: PathBuf::from("."),
                    total_additions: total_add,
                    total_deletions: total_del,
                    file_count: files.len(),
                };
                self.files = files;
            }
            AppMode::Stdin => {
                let (files, stats) = diff_engine.compare_stdin()?;
                self.files = files;
                self.repo_stats = stats;
            }
        }

        self.update_filter();

        // Restore file selection if previous file still exists
        if let Some(prev_p) = previous_path {
            if self.file_view_mode == FileViewMode::Tree {
                if let Some(pos) = self.tree_items.iter().position(|item| item.path == prev_p) {
                    self.selected_tree_idx = pos;
                }
            } else if let Some(pos) = self.filtered_indices.iter().position(|&idx| self.files[idx].new_path == prev_p) {
                self.selected_filtered_idx = pos;
            }
        }

        // Restore scroll positions safely
        if let Some(cur_file) = self.current_file() {
            let max_rows = cur_file.aligned_rows.len();
            if max_rows > 0 {
                self.selected_row = prev_row.min(max_rows - 1);
                self.scroll_y = prev_scroll.min(max_rows - 1);
            } else {
                self.selected_row = 0;
                self.scroll_y = 0;
            }
        }

        if is_live_reload {
            self.set_notification("⚡ [disk updated]");
        }

        Ok(())
    }

    pub fn update_filter(&mut self) {
        if self.filter_query.is_empty() {
            self.filtered_indices = (0..self.files.len()).collect();
        } else {
            let q = self.filter_query.to_lowercase();
            self.filtered_indices = self
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.display_path().to_lowercase().contains(&q))
                .map(|(idx, _)| idx)
                .collect();
        }

        if self.selected_filtered_idx >= self.filtered_indices.len() {
            self.selected_filtered_idx = self.filtered_indices.len().saturating_sub(1);
        }

        self.tree_items = build_tree_items(
            &self.files,
            &self.filtered_indices,
            &self.collapsed_dirs,
            self.file_view_mode,
        );

        if self.selected_tree_idx >= self.tree_items.len() {
            self.selected_tree_idx = self.tree_items.len().saturating_sub(1);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Clear old notification if older than 4 seconds
        if let Some((_, time)) = self.notification {
            if time.elapsed() > Duration::from_secs(4) {
                self.notification = None;
            }
        }

        // 1. Worktrees Modal Active
        if self.show_worktrees {
            match key.code {
                KeyCode::Esc | KeyCode::Char('W') | KeyCode::Char('q') => {
                    self.show_worktrees = false;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.selected_worktree_idx + 1 < self.worktrees.len() {
                        self.selected_worktree_idx += 1;
                        if self.selected_worktree_idx >= self.worktree_scroll + 10 {
                            self.worktree_scroll = self.selected_worktree_idx.saturating_sub(9);
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if self.selected_worktree_idx > 0 {
                        self.selected_worktree_idx -= 1;
                        if self.selected_worktree_idx < self.worktree_scroll {
                            self.worktree_scroll = self.selected_worktree_idx;
                        }
                    }
                }
                KeyCode::Enter => {
                    self.switch_to_selected_worktree();
                }
                _ => {}
            }
            return;
        }

        // 2. Help Modal Active
        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') || key.code == KeyCode::Char('q') {
                self.show_help = false;
            }
            return;
        }

        // 2. Confirm Dialog Active
        if let Some((action, _)) = self.confirm_action.take() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_confirm_action(action);
                }
                _ => {
                    self.set_notification("Action cancelled");
                }
            }
            return;
        }

        // 3. History Modal Active
        if self.show_history {
            match key.code {
                KeyCode::Esc | KeyCode::Char('H') | KeyCode::Char('q') => {
                    self.show_history = false;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.selected_history_idx + 1 < self.history_commits.len() {
                        self.selected_history_idx += 1;
                        if self.selected_history_idx >= self.history_scroll + 12 {
                            self.history_scroll = self.selected_history_idx.saturating_sub(11);
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if self.selected_history_idx > 0 {
                        self.selected_history_idx -= 1;
                        if self.selected_history_idx < self.history_scroll {
                            self.history_scroll = self.selected_history_idx;
                        }
                    }
                }
                KeyCode::Enter => {
                    self.load_selected_commit_diff();
                }
                _ => {}
            }
            return;
        }

        // 4. Filter Search Active
        if self.filter_mode {
            match key.code {
                KeyCode::Enter => {
                    self.filter_mode = false;
                }
                KeyCode::Esc => {
                    self.filter_mode = false;
                    self.filter_query.clear();
                    self.update_filter();
                }
                KeyCode::Backspace => {
                    self.filter_query.pop();
                    self.update_filter();
                }
                KeyCode::Char(c) => {
                    self.filter_query.push(c);
                    self.update_filter();
                }
                _ => {}
            }
            return;
        }

        // 5. Normal / Visual Navigation
        match key.code {
            KeyCode::Esc => {
                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                    self.files = saved_files;
                    self.repo_stats = saved_stats;
                    self.active_commit_view = None;
                    self.update_filter();
                    let msg = match self.language {
                        Language::En => "Returned to live working tree diff",
                        Language::Pt => "Retornou para o diff da árvore de trabalho",
                    };
                    self.set_notification(msg);
                } else if self.active_commit_view.is_some() {
                    self.active_commit_view = None;
                    let msg = match self.language {
                        Language::En => "Returned to live working tree diff",
                        Language::Pt => "Retornou para o diff da árvore de trabalho",
                    };
                    self.set_notification(msg);
                } else if self.visual_mode {
                    self.visual_mode = false;
                    let msg = match self.language {
                        Language::En => "Exited visual mode",
                        Language::Pt => "Saiu do modo visual",
                    };
                    self.set_notification(msg);
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('H') => {
                if self.active_commit_view.is_some() {
                    self.active_commit_view = None;
                    let msg = match self.language {
                        Language::En => "Returned to live working tree diff",
                        Language::Pt => "Retornou para o diff da árvore de trabalho",
                    };
                    self.set_notification(msg);
                } else {
                    self.open_file_history();
                }
            }
            KeyCode::Char('W') => {
                self.show_worktrees = true;
            }
            KeyCode::Char('L') => {
                self.language = match self.language {
                    Language::En => Language::Pt,
                    Language::Pt => Language::En,
                };
                let msg = match self.language {
                    Language::En => "Language: English",
                    Language::Pt => "Idioma: Português",
                };
                self.set_notification(msg);
            }
            KeyCode::Char('b') => {
                self.show_drawer = !self.show_drawer;
                let msg = if self.show_drawer {
                    match self.language {
                        Language::En => "Sidebar visible",
                        Language::Pt => "Painel lateral visível",
                    }
                } else {
                    match self.language {
                        Language::En => "Sidebar hidden (press 'b' to show)",
                        Language::Pt => "Painel oculto (pressione 'b' para exibir)",
                    }
                };
                self.set_notification(msg);
                if !self.show_drawer && self.focus == Focus::FileTree {
                    self.focus = Focus::DiffView;
                }
            }
            KeyCode::Char('1') => {
                self.show_drawer = true;
                self.drawer_tab = DrawerTab::Changes;
                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                    self.files = saved_files;
                    self.repo_stats = saved_stats;
                    self.active_commit_view = None;
                    self.update_filter();
                }
                self.focus = Focus::FileTree;
                let msg = match self.language {
                    Language::En => "Tab: Changes",
                    Language::Pt => "Aba: Mudanças",
                };
                self.set_notification(msg);
            }
            KeyCode::Char('2') => {
                self.show_drawer = true;
                self.drawer_tab = DrawerTab::Commits;
                self.focus = Focus::FileTree;
                let msg = match self.language {
                    Language::En => "Tab: Commits (j/k select, Enter view)",
                    Language::Pt => "Aba: Commits (j/k seleciona, Enter visualiza)",
                };
                self.set_notification(msg);
            }
            KeyCode::Char('3') => {
                self.show_drawer = true;
                self.drawer_tab = DrawerTab::Stashes;
                self.focus = Focus::FileTree;
                let msg = match self.language {
                    Language::En => "Tab: Stashes (j/k select, Enter view)",
                    Language::Pt => "Aba: Stashes (j/k seleciona, Enter visualiza)",
                };
                self.set_notification(msg);
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Tab => {
                if !self.show_drawer {
                    self.show_drawer = true;
                    self.focus = Focus::FileTree;
                } else {
                    self.focus = match self.focus {
                        Focus::FileTree => Focus::DiffView,
                        Focus::DiffView => Focus::FileTree,
                    };
                }
            }
            KeyCode::Char('m') => {
                self.is_unified = !self.is_unified;
                let mode = if self.is_unified { "Unified" } else { "Side-by-Side" };
                self.set_notification(format!("Switched to {} view", mode));
            }
            KeyCode::Char('w') => {
                self.watch_mode = !self.watch_mode;
                let status = if self.watch_mode { "ON" } else { "OFF" };
                self.set_notification(format!("Live Watch Mode: {}", status));
            }
            KeyCode::Char('t') => {
                self.show_drawer = true;
                self.file_view_mode = match self.file_view_mode {
                    FileViewMode::Flat => FileViewMode::Tree,
                    FileViewMode::Tree => FileViewMode::Flat,
                };
                self.update_filter();
                let mode_str = match (self.file_view_mode, self.language) {
                    (FileViewMode::Flat, Language::En) => "Flat List",
                    (FileViewMode::Flat, Language::Pt) => "Lista Plana",
                    (FileViewMode::Tree, Language::En) => "Folders (Tree)",
                    (FileViewMode::Tree, Language::Pt) => "Pastas (Árvore)",
                };
                self.set_notification(format!("File Drawer: {}", mode_str));
            }
            KeyCode::Char('v') => {
                if self.focus == Focus::DiffView {
                    self.visual_mode = !self.visual_mode;
                    if self.visual_mode {
                        self.visual_anchor = self.selected_row;
                        let msg = match self.language {
                            Language::En => "-- VISUAL MODE (Select lines, press 's' to stage) --",
                            Language::Pt => "-- MODO VISUAL (Selecione linhas, pressione 's' para stage) --",
                        };
                        self.set_notification(msg);
                    } else {
                        let msg = match self.language {
                            Language::En => "Exited visual mode",
                            Language::Pt => "Saiu do modo visual",
                        };
                        self.set_notification(msg);
                    }
                }
            }
            KeyCode::Char('/') => {
                self.show_drawer = true;
                self.filter_mode = true;
                self.focus = Focus::FileTree;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.focus == Focus::FileTree && self.drawer_tab == DrawerTab::Changes && self.file_view_mode == FileViewMode::Tree {
                    // Collapse directory
                    if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                        if item.is_dir && !item.is_collapsed {
                            self.collapsed_dirs.insert(item.path.clone());
                            self.update_filter();
                        }
                    }
                } else if self.focus == Focus::DiffView {
                    self.column_side = ColumnSide::Left;
                    let msg = match self.language {
                        Language::En => "Focus: OLD version (left)",
                        Language::Pt => "Foco: versão ANTIGA (esquerda)",
                    };
                    self.set_notification(msg);
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.focus == Focus::FileTree && self.drawer_tab == DrawerTab::Changes && self.file_view_mode == FileViewMode::Tree {
                    // Expand directory
                    if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                        if item.is_dir && item.is_collapsed {
                            self.collapsed_dirs.remove(&item.path);
                            self.update_filter();
                        }
                    }
                } else if self.focus == Focus::DiffView {
                    self.column_side = ColumnSide::Right;
                    let msg = match self.language {
                        Language::En => "Focus: NEW version (right)",
                        Language::Pt => "Foco: versão NOVA (direita)",
                    };
                    self.set_notification(msg);
                }
            }
            KeyCode::Char('<') | KeyCode::Char(',') => {
                self.file_tree_width = self.file_tree_width.saturating_sub(3).max(16);
                let msg = match self.language {
                    Language::En => format!("Drawer width: {} cols", self.file_tree_width),
                    Language::Pt => format!("Largura do painel: {} colunas", self.file_tree_width),
                };
                self.set_notification(msg);
            }
            KeyCode::Char('>') | KeyCode::Char('.') => {
                self.file_tree_width = (self.file_tree_width + 3).min(80);
                let msg = match self.language {
                    Language::En => format!("Drawer width: {} cols", self.file_tree_width),
                    Language::Pt => format!("Largura do painel: {} colunas", self.file_tree_width),
                };
                self.set_notification(msg);
            }
            KeyCode::Char(' ') => {
                if self.focus == Focus::FileTree && self.drawer_tab == DrawerTab::Changes && self.file_view_mode == FileViewMode::Tree {
                    if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                        if item.is_dir {
                            if item.is_collapsed {
                                self.collapsed_dirs.remove(&item.path);
                            } else {
                                self.collapsed_dirs.insert(item.path.clone());
                            }
                            self.update_filter();
                        }
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.focus == Focus::FileTree {
                    self.file_tree_down(1);
                } else {
                    self.scroll_down(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.focus == Focus::FileTree {
                    self.file_tree_up(1);
                } else {
                    self.scroll_up(1);
                }
            }
            KeyCode::Char('J') | KeyCode::PageDown => {
                if self.focus == Focus::FileTree {
                    self.file_tree_down(10);
                } else {
                    self.scroll_down(10);
                }
            }
            KeyCode::Char('K') | KeyCode::PageUp => {
                if self.focus == Focus::FileTree {
                    self.file_tree_up(10);
                } else {
                    self.scroll_up(10);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_down(10);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_up(10);
            }
            KeyCode::Char(']') | KeyCode::Char('n') => {
                self.jump_next_hunk();
            }
            KeyCode::Char('[') | KeyCode::Char('p') => {
                self.jump_prev_hunk();
            }
            KeyCode::Enter => {
                if self.focus == Focus::FileTree {
                    match self.drawer_tab {
                        DrawerTab::Changes => {
                            if self.file_view_mode == FileViewMode::Tree {
                                if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                                    if item.is_dir {
                                        if item.is_collapsed {
                                            self.collapsed_dirs.remove(&item.path);
                                        } else {
                                            self.collapsed_dirs.insert(item.path.clone());
                                        }
                                        self.update_filter();
                                    } else {
                                        self.focus = Focus::DiffView;
                                    }
                                }
                            } else {
                                self.focus = Focus::DiffView;
                            }
                        }
                        DrawerTab::Commits => {
                            self.load_selected_repo_commit();
                        }
                        DrawerTab::Stashes => {
                            self.load_selected_stash();
                        }
                    }
                } else {
                    self.trigger_editor_open();
                }
            }
            KeyCode::Char('e') => {
                self.trigger_editor_open();
            }
            KeyCode::Char('c') => {
                self.copy_current_hunk();
            }
            KeyCode::Char('s') => {
                if self.visual_mode {
                    self.stage_visual_selection();
                } else {
                    self.stage_current_hunk();
                }
            }
            KeyCode::Char('u') => {
                self.unstage_current_hunk();
            }
            KeyCode::Char('d') => {
                self.request_discard_hunk();
            }
            KeyCode::Char('S') => {
                self.stage_current_file();
            }
            KeyCode::Char('U') => {
                self.unstage_current_file();
            }
            KeyCode::Char('D') => {
                self.request_discard_file();
            }
            _ => {}
        }
    }

    pub fn file_tree_down(&mut self, amount: usize) {
        let tree_vp = self.file_tree_height.saturating_sub(4).max(1);
        match self.drawer_tab {
            DrawerTab::Changes => {
                if self.file_view_mode == FileViewMode::Tree {
                    if !self.tree_items.is_empty() {
                        self.selected_tree_idx = (self.selected_tree_idx + amount).min(self.tree_items.len() - 1);
                        if self.selected_tree_idx >= self.file_tree_scroll + tree_vp {
                            self.file_tree_scroll = self.selected_tree_idx.saturating_sub(tree_vp - 1);
                        }
                        self.scroll_y = 0;
                        self.selected_row = 0;
                    }
                } else if !self.filtered_indices.is_empty() {
                    self.selected_filtered_idx = (self.selected_filtered_idx + amount).min(self.filtered_indices.len() - 1);
                    if self.selected_filtered_idx >= self.file_tree_scroll + tree_vp {
                        self.file_tree_scroll = self.selected_filtered_idx.saturating_sub(tree_vp - 1);
                    }
                    self.scroll_y = 0;
                    self.selected_row = 0;
                }
            }
            DrawerTab::Commits => {
                if !self.repo_commits.is_empty() {
                    self.selected_repo_commit_idx = (self.selected_repo_commit_idx + amount).min(self.repo_commits.len() - 1);
                    if self.selected_repo_commit_idx >= self.repo_commit_scroll + tree_vp {
                        self.repo_commit_scroll = self.selected_repo_commit_idx.saturating_sub(tree_vp - 1);
                    }
                }
            }
            DrawerTab::Stashes => {
                if !self.stashes.is_empty() {
                    self.selected_stash_idx = (self.selected_stash_idx + amount).min(self.stashes.len() - 1);
                    if self.selected_stash_idx >= self.stash_scroll + tree_vp {
                        self.stash_scroll = self.selected_stash_idx.saturating_sub(tree_vp - 1);
                    }
                }
            }
        }
    }

    pub fn file_tree_up(&mut self, amount: usize) {
        match self.drawer_tab {
            DrawerTab::Changes => {
                if self.file_view_mode == FileViewMode::Tree {
                    self.selected_tree_idx = self.selected_tree_idx.saturating_sub(amount);
                    if self.selected_tree_idx < self.file_tree_scroll {
                        self.file_tree_scroll = self.selected_tree_idx;
                    }
                    self.scroll_y = 0;
                    self.selected_row = 0;
                } else {
                    self.selected_filtered_idx = self.selected_filtered_idx.saturating_sub(amount);
                    if self.selected_filtered_idx < self.file_tree_scroll {
                        self.file_tree_scroll = self.selected_filtered_idx;
                    }
                    self.scroll_y = 0;
                    self.selected_row = 0;
                }
            }
            DrawerTab::Commits => {
                self.selected_repo_commit_idx = self.selected_repo_commit_idx.saturating_sub(amount);
                if self.selected_repo_commit_idx < self.repo_commit_scroll {
                    self.repo_commit_scroll = self.selected_repo_commit_idx;
                }
            }
            DrawerTab::Stashes => {
                self.selected_stash_idx = self.selected_stash_idx.saturating_sub(amount);
                if self.selected_stash_idx < self.stash_scroll {
                    self.stash_scroll = self.selected_stash_idx;
                }
            }
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        if let Some(file) = self.current_file() {
            let total = if self.is_unified {
                file.hunks.iter().map(|h| h.lines.len() + 1).sum()
            } else {
                file.aligned_rows.len()
            };
            if total > 0 {
                self.selected_row = (self.selected_row + amount).min(total - 1);
                let vp = self.viewport_height.saturating_sub(3).max(1);
                if self.selected_row >= self.scroll_y + vp {
                    self.scroll_y = self.selected_row.saturating_sub(vp - 1);
                }
            }
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.selected_row = self.selected_row.saturating_sub(amount);
        if self.selected_row < self.scroll_y {
            self.scroll_y = self.selected_row;
        }
    }

    pub fn switch_to_selected_worktree(&mut self) {
        if let Some(wt) = self.worktrees.get(self.selected_worktree_idx).cloned() {
            let new_root = wt.path.clone();
            for w in &mut self.worktrees {
                w.is_current = w.path == new_root;
            }
            if let AppMode::Git { git_provider, .. } = &mut self.mode {
                git_provider.repo_root = new_root.clone();
                let _ = std::env::set_current_dir(&new_root);
                self.repo_commits = git_provider.get_repo_commits(50).unwrap_or_default();
                self.stashes = git_provider.get_stashes().unwrap_or_default();
                self.selected_repo_commit_idx = 0;
                self.repo_commit_scroll = 0;
                self.selected_stash_idx = 0;
                self.stash_scroll = 0;
            }
            self.live_snapshot = None;
            self.active_commit_view = None;
            self.reload_diffs();
            self.show_worktrees = false;
            let notif = match self.language {
                Language::En => format!("Switched to worktree: {}", new_root.display()),
                Language::Pt => format!("Alternado para worktree: {}", new_root.display()),
            };
            self.set_notification(notif);
        }
    }

    pub fn load_selected_repo_commit(&mut self) {
        if let Some(commit) = self.repo_commits.get(self.selected_repo_commit_idx).cloned() {
            if let AppMode::Git { git_provider, .. } = &self.mode {
                match git_provider.load_commit_full_diff(&commit.hash) {
                    Ok(files) => {
                        if files.is_empty() {
                            let msg = match self.language {
                                Language::En => format!("No file changes in commit {}", &commit.hash[..7.min(commit.hash.len())]),
                                Language::Pt => format!("Nenhuma alteração no commit {}", &commit.hash[..7.min(commit.hash.len())]),
                            };
                            self.set_notification(msg);
                        } else {
                            if self.live_snapshot.is_none() {
                                self.live_snapshot = Some((self.files.clone(), self.repo_stats.clone()));
                            }
                            let mut stats = self.repo_stats.clone();
                            stats.branch = format!("commit: {}", &commit.hash[..7.min(commit.hash.len())]);
                            self.repo_stats = stats;
                            self.files = files;
                            self.update_filter();
                            self.selected_filtered_idx = 0;
                            self.selected_tree_idx = 0;
                            self.scroll_y = 0;
                            self.selected_row = 0;
                            self.focus = Focus::DiffView;
                            let msg = match self.language {
                                Language::En => format!("Viewing commit {} · Esc/1 to return", &commit.hash[..7.min(commit.hash.len())]),
                                Language::Pt => format!("Visualizando commit {} · Esc/1 para voltar", &commit.hash[..7.min(commit.hash.len())]),
                            };
                            self.set_notification(msg);
                        }
                    }
                    Err(e) => {
                        self.set_notification(format!("Error loading commit: {}", e));
                    }
                }
            }
        }
    }

    pub fn load_selected_stash(&mut self) {
        if let Some(stash) = self.stashes.get(self.selected_stash_idx).cloned() {
            if let AppMode::Git { git_provider, .. } = &self.mode {
                match git_provider.load_stash_diff(&stash.selector) {
                    Ok(files) => {
                        if files.is_empty() {
                            let msg = match self.language {
                                Language::En => format!("No file changes in {}", stash.selector),
                                Language::Pt => format!("Nenhuma alteração em {}", stash.selector),
                            };
                            self.set_notification(msg);
                        } else {
                            if self.live_snapshot.is_none() {
                                self.live_snapshot = Some((self.files.clone(), self.repo_stats.clone()));
                            }
                            let mut stats = self.repo_stats.clone();
                            stats.branch = format!("stash: {}", stash.selector);
                            self.repo_stats = stats;
                            self.files = files;
                            self.update_filter();
                            self.selected_filtered_idx = 0;
                            self.selected_tree_idx = 0;
                            self.scroll_y = 0;
                            self.selected_row = 0;
                            self.focus = Focus::DiffView;
                            let msg = match self.language {
                                Language::En => format!("Viewing stash {} · Esc/1 to return", stash.selector),
                                Language::Pt => format!("Visualizando stash {} · Esc/1 para voltar", stash.selector),
                            };
                            self.set_notification(msg);
                        }
                    }
                    Err(e) => {
                        self.set_notification(format!("Error loading stash: {}", e));
                    }
                }
            }
        }
    }

    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};

        let effective_tree_width = if self.show_drawer { self.file_tree_width } else { 0 };

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if effective_tree_width > 0 && mouse.column < effective_tree_width {
                    self.file_tree_down(3);
                } else {
                    self.scroll_down(3);
                }
            }
            MouseEventKind::ScrollUp => {
                if effective_tree_width > 0 && mouse.column < effective_tree_width {
                    self.file_tree_up(3);
                } else {
                    self.scroll_up(3);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.show_drawer {
                    let divider_col = effective_tree_width;
                    if mouse.column >= divider_col.saturating_sub(1) && mouse.column <= divider_col + 1 {
                        self.is_dragging_divider = true;
                        return;
                    }

                    if mouse.column < effective_tree_width {
                        self.focus = Focus::FileTree;
                        if mouse.row <= 2 {
                            let col = mouse.column;
                            let tab_w = (effective_tree_width / 3).max(1);
                            if col < tab_w {
                                self.drawer_tab = DrawerTab::Changes;
                                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                                    self.files = saved_files;
                                    self.repo_stats = saved_stats;
                                    self.active_commit_view = None;
                                    self.update_filter();
                                }
                            } else if col < tab_w * 2 {
                                self.drawer_tab = DrawerTab::Commits;
                            } else {
                                self.drawer_tab = DrawerTab::Stashes;
                            }
                            return;
                        }

                        let item_row = (mouse.row.saturating_sub(3)) as usize;
                        match self.drawer_tab {
                            DrawerTab::Changes => {
                                let target_idx = self.file_tree_scroll + item_row;
                                if self.file_view_mode == FileViewMode::Tree {
                                    if target_idx < self.tree_items.len() {
                                        self.selected_tree_idx = target_idx;
                                        self.scroll_y = 0;
                                        self.selected_row = 0;
                                        let item = &self.tree_items[target_idx];
                                        if item.is_dir {
                                            if item.is_collapsed {
                                                self.collapsed_dirs.remove(&item.path);
                                            } else {
                                                self.collapsed_dirs.insert(item.path.clone());
                                            }
                                            self.update_filter();
                                        }
                                    }
                                } else if target_idx < self.filtered_indices.len() {
                                    self.selected_filtered_idx = target_idx;
                                    self.scroll_y = 0;
                                    self.selected_row = 0;
                                }
                            }
                            DrawerTab::Commits => {
                                let target_idx = self.repo_commit_scroll + item_row;
                                if target_idx < self.repo_commits.len() {
                                    self.selected_repo_commit_idx = target_idx;
                                }
                            }
                            DrawerTab::Stashes => {
                                let target_idx = self.stash_scroll + item_row;
                                if target_idx < self.stashes.len() {
                                    self.selected_stash_idx = target_idx;
                                }
                            }
                        }
                        return;
                    }
                }

                if mouse.column > effective_tree_width && mouse.row >= 1 {
                    self.focus = Focus::DiffView;
                    let diff_inner_x = mouse.column.saturating_sub(effective_tree_width + 1);
                    if diff_inner_x < (self.viewport_height.max(30) as u16) {
                        self.column_side = ColumnSide::Left;
                    } else {
                        self.column_side = ColumnSide::Right;
                    }

                    let line_row = (mouse.row.saturating_sub(2)) as usize;
                    let target_row = self.scroll_y + line_row;
                    if let Some(file) = self.current_file() {
                        let total = if self.is_unified {
                            file.hunks.iter().map(|h| h.lines.len() + 1).sum()
                        } else {
                            file.aligned_rows.len()
                        };
                        if target_row < total {
                            self.selected_row = target_row;
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.show_drawer && (self.is_dragging_divider || (mouse.column >= self.file_tree_width.saturating_sub(2) && mouse.column <= self.file_tree_width + 2)) {
                    self.file_tree_width = mouse.column.clamp(16, 80);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.is_dragging_divider = false;
            }
            _ => {}
        }
    }

    fn jump_next_hunk(&mut self) {
        if let Some(file) = self.current_file() {
            let rows = &file.aligned_rows;
            if rows.is_empty() {
                return;
            }
            let current_hunk_idx = rows.get(self.selected_row).and_then(|r| r.hunk_index);

            for (idx, row) in rows.iter().enumerate().skip(self.selected_row + 1) {
                if let Some(h_idx) = row.hunk_index {
                    if current_hunk_idx != Some(h_idx) {
                        self.selected_row = idx;
                        self.scroll_y = idx.saturating_sub(2);
                        self.set_notification(format!("Jumped to Hunk #{}", h_idx + 1));
                        return;
                    }
                }
            }
            self.set_notification("Reached last hunk");
        }
    }

    fn jump_prev_hunk(&mut self) {
        if let Some(file) = self.current_file() {
            let rows = &file.aligned_rows;
            if rows.is_empty() {
                return;
            }
            let current_hunk_idx = rows.get(self.selected_row).and_then(|r| r.hunk_index);

            for idx in (0..self.selected_row).rev() {
                if let Some(h_idx) = rows[idx].hunk_index {
                    if current_hunk_idx != Some(h_idx) {
                        // Find the start of this hunk
                        let start_of_hunk = rows
                            .iter()
                            .take(idx + 1)
                            .rposition(|r| r.hunk_index == Some(h_idx))
                            .unwrap_or(idx);
                        self.selected_row = start_of_hunk;
                        self.scroll_y = start_of_hunk.saturating_sub(2);
                        self.set_notification(format!("Jumped to Hunk #{}", h_idx + 1));
                        return;
                    }
                }
            }
            self.set_notification("Reached first hunk");
        }
    }

    fn trigger_editor_open(&mut self) {
        if let Some(file) = self.current_file() {
            let line_no = file
                .aligned_rows
                .get(self.selected_row)
                .and_then(|r| match self.column_side {
                    ColumnSide::Right => r.right.as_ref().and_then(|l| l.new_line_no).or_else(|| r.left.as_ref().and_then(|l| l.old_line_no)),
                    ColumnSide::Left => r.left.as_ref().and_then(|l| l.old_line_no).or_else(|| r.right.as_ref().and_then(|l| l.new_line_no)),
                })
                .unwrap_or(1);

            let repo_dir = match &self.mode {
                AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
                _ => PathBuf::from("."),
            };

            let target_path = if self.column_side == ColumnSide::Left && file.old_path.is_some() {
                file.old_path.as_ref().unwrap()
            } else {
                &file.new_path
            };

            let full_path = repo_dir.join(target_path);
            self.editor_request = Some((full_path, line_no));
        }
    }

    fn copy_current_hunk(&mut self) {
        if let Some(file) = self.current_file() {
            let hunk_opt = file
                .aligned_rows
                .get(self.selected_row)
                .and_then(|r| r.hunk_index)
                .and_then(|idx| file.hunks.get(idx));

            if let Some(hunk) = hunk_opt {
                match copy_hunk_as_markdown(&file.new_path, hunk) {
                    Ok(_) => self.set_notification("✓ Copied hunk to clipboard as Markdown!"),
                    Err(e) => self.set_notification(format!("Clipboard error: {}", e)),
                }
            } else {
                match crate::integration::clipboard::copy_file_diff_as_markdown(&file.new_path, &file.hunks) {
                    Ok(_) => self.set_notification("✓ Copied full file diff to clipboard as Markdown!"),
                    Err(e) => self.set_notification(format!("Clipboard error: {}", e)),
                }
            }
        }
    }

    fn stage_current_hunk(&mut self) {
        let (repo_root, file_path, hunk) = match self.get_current_hunk_and_context() {
            Some(v) => v,
            None => return,
        };

        match stage_hunk(&repo_root, &file_path, &hunk) {
            Ok(_) => {
                self.set_notification("✓ Hunk staged");
                self.reload_diffs();
            }
            Err(e) => self.set_notification(format!("Stage error: {}", e)),
        }
    }

    fn stage_visual_selection(&mut self) {
        let file = match self.current_file() {
            Some(f) => f,
            None => return,
        };

        let repo_root = match &self.mode {
            AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
            _ => return,
        };

        let hunk_idx = match file.aligned_rows.get(self.selected_row).and_then(|r| r.hunk_index) {
            Some(idx) => idx,
            None => return,
        };

        let hunk = match file.hunks.get(hunk_idx) {
            Some(h) => h,
            None => return,
        };

        let min_r = self.visual_anchor.min(self.selected_row);
        let max_r = self.visual_anchor.max(self.selected_row);

        // Find which lines in hunk.lines correspond to the visual range
        let mut selected_indices = Vec::new();
        for r_idx in min_r..=max_r {
            if let Some(row) = file.aligned_rows.get(r_idx) {
                if row.hunk_index != Some(hunk_idx) {
                    continue;
                }

                if let Some(l_idx) = row.left_line_idx {
                    if !selected_indices.contains(&l_idx) {
                        selected_indices.push(l_idx);
                    }
                }
                if let Some(r_idx) = row.right_line_idx {
                    if !selected_indices.contains(&r_idx) {
                        selected_indices.push(r_idx);
                    }
                }
            }
        }
        selected_indices.sort_unstable();

        if selected_indices.is_empty() {
            self.set_notification("No changed lines in visual selection to stage");
            return;
        }

        match stage_partial_hunk(&repo_root, &file.new_path, hunk, &selected_indices) {
            Ok(_) => {
                self.visual_mode = false;
                self.set_notification(format!("✓ Staged {} selected lines", selected_indices.len()));
                self.reload_diffs();
            }
            Err(e) => {
                self.set_notification(format!("Visual stage error: {}", e));
            }
        }
    }

    fn unstage_current_hunk(&mut self) {
        let (repo_root, file_path, hunk) = match self.get_current_hunk_and_context() {
            Some(v) => v,
            None => return,
        };

        match unstage_hunk(&repo_root, &file_path, &hunk) {
            Ok(_) => {
                self.set_notification("✓ Hunk unstaged");
                self.reload_diffs();
            }
            Err(e) => self.set_notification(format!("Unstage error: {}", e)),
        }
    }

    fn request_discard_hunk(&mut self) {
        if let Some(file) = self.current_file() {
            if let Some(hunk_idx) = file.aligned_rows.get(self.selected_row).and_then(|r| r.hunk_index) {
                let msg = format!("Discard Hunk #{} in {}? Changes cannot be undone.", hunk_idx + 1, file.display_path());
                self.confirm_action = Some((ConfirmAction::DiscardHunk(hunk_idx), msg));
            } else {
                self.set_notification("No hunk under cursor to discard");
            }
        }
    }

    fn stage_current_file(&mut self) {
        if let Some(file) = self.current_file() {
            let repo_root = match &self.mode {
                AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
                _ => return,
            };
            let path = file.new_path.clone();
            match stage_file(&repo_root, &path) {
                Ok(_) => {
                    self.set_notification(format!("✓ Staged entire file {}", path.display()));
                    self.reload_diffs();
                }
                Err(e) => self.set_notification(format!("Stage error: {}", e)),
            }
        }
    }

    fn unstage_current_file(&mut self) {
        if let Some(file) = self.current_file() {
            let repo_root = match &self.mode {
                AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
                _ => return,
            };
            let path = file.new_path.clone();
            match unstage_file(&repo_root, &path) {
                Ok(_) => {
                    self.set_notification(format!("✓ Unstaged file {}", path.display()));
                    self.reload_diffs();
                }
                Err(e) => self.set_notification(format!("Unstage error: {}", e)),
            }
        }
    }

    fn request_discard_file(&mut self) {
        if let Some(file) = self.current_file() {
            let msg = format!("DISCARD ALL changes in {}? All modifications will be lost.", file.display_path());
            self.confirm_action = Some((ConfirmAction::DiscardFile, msg));
        }
    }

    fn execute_confirm_action(&mut self, action: ConfirmAction) {
        let repo_root = match &self.mode {
            AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
            _ => return,
        };

        match action {
            ConfirmAction::DiscardHunk(hunk_idx) => {
                if let Some(file) = self.current_file() {
                    let path = file.new_path.clone();
                    if let Some(hunk) = file.hunks.get(hunk_idx) {
                        match discard_hunk(&repo_root, &path, hunk) {
                            Ok(_) => {
                                self.set_notification("✓ Hunk reverted");
                                self.reload_diffs();
                            }
                            Err(e) => self.set_notification(format!("Discard error: {}", e)),
                        }
                    }
                }
            }
            ConfirmAction::DiscardFile => {
                if let Some(file) = self.current_file() {
                    let path = file.new_path.clone();
                    let is_untracked = file.status == crate::core::models::FileStatus::Untracked;
                    match discard_file(&repo_root, &path, is_untracked) {
                        Ok(_) => {
                            self.set_notification(format!("✓ Discarded all changes in {}", path.display()));
                            self.reload_diffs();
                        }
                        Err(e) => self.set_notification(format!("Discard error: {}", e)),
                    }
                }
            }
        }
    }

    fn get_current_hunk_and_context(&self) -> Option<(PathBuf, PathBuf, crate::core::models::Hunk)> {
        let repo_root = match &self.mode {
            AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
            _ => return None,
        };
        let file = self.current_file()?;
        let hunk_idx = file.aligned_rows.get(self.selected_row).and_then(|r| r.hunk_index)?;
        let hunk = file.hunks.get(hunk_idx)?.clone();
        Some((repo_root, file.new_path.clone(), hunk))
    }

    pub fn open_file_history(&mut self) {
        if let AppMode::Git { git_provider, .. } = &self.mode {
            if let Some(file) = self.get_underlying_file() {
                match git_provider.get_file_history(&file.new_path, 50) {
                    Ok(commits) if !commits.is_empty() => {
                        self.history_commits = commits;
                        self.selected_history_idx = 0;
                        self.history_scroll = 0;
                        self.show_history = true;
                    }
                    _ => {
                        self.set_notification(format!("No git history found for {}", file.display_path()));
                    }
                }
            } else {
                self.set_notification("No file selected for history");
            }
        } else {
            self.set_notification("History view is only available inside Git repositories");
        }
    }

    pub fn load_selected_commit_diff(&mut self) {
        if let Some(commit) = self.history_commits.get(self.selected_history_idx) {
            let hash = commit.hash.clone();
            let msg = commit.message.clone();

            if let AppMode::Git { git_provider, .. } = &self.mode {
                if let Some(file) = self.get_underlying_file() {
                    match git_provider.load_commit_diff_for_file(&hash, &file.new_path) {
                        Ok(Some(commit_diff)) => {
                            self.active_commit_view = Some((hash.clone(), commit_diff));
                            self.show_history = false;
                            self.scroll_y = 0;
                            self.selected_row = 0;
                            self.set_notification(format!("Viewing commit {} - {} (Press Esc/H to return)", hash, msg));
                        }
                        _ => {
                            self.set_notification(format!("No diff changes in commit {} for this file", hash));
                        }
                    }
                }
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(5),    // Main
                Constraint::Length(1), // Status Bar
            ])
            .split(size);

        // 1. Render Header
        let mut display_stats = self.repo_stats.clone();
        if let Some((hash, _)) = &self.active_commit_view {
            display_stats.branch = format!("commit: {}", &hash[..7.min(hash.len())]);
        }

        let active_worktree_name = self.worktrees.iter().find(|w| w.is_current).map(|w| {
            if let Some(b) = &w.branch {
                b.as_str()
            } else {
                w.path.file_name().and_then(|n| n.to_str()).unwrap_or("main")
            }
        });

        render_header(
            frame,
            chunks[0],
            &display_stats,
            self.is_unified,
            self.watch_mode,
            active_worktree_name,
            self.language,
            &self.theme,
        );

        // Responsive drawer width on narrow terminals (e.g. half-screen < 85 columns)
        let effective_tree_width = if !self.show_drawer {
            0
        } else if size.width < 85 {
            // Allocate at most 32% of screen to drawer, but not less than 18 cols
            self.file_tree_width.min((size.width * 32 / 100).max(18)).min(size.width.saturating_sub(25))
        } else {
            self.file_tree_width.min(size.width.saturating_sub(25))
        };

        // 2. Render Main Body (File Tree + Diff View + Overview Ruler)
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(effective_tree_width), // File Tree
                Constraint::Min(20),   // Diff View
                Constraint::Length(if self.config.ui.overview_ruler { 1 } else { 0 }), // Ruler
            ])
            .split(chunks[1]);

        let file_tree_area = main_chunks[0];
        let diff_area = main_chunks[1];
        let ruler_area = main_chunks[2];

        self.viewport_height = diff_area.height as usize;
        self.file_tree_height = file_tree_area.height as usize;

        if effective_tree_width > 0 {
            let selected_file_idx = if self.file_view_mode == FileViewMode::Tree {
                self.selected_tree_idx
            } else {
                self.selected_filtered_idx
            };

            render_drawer(
                frame,
                file_tree_area,
                self.drawer_tab,
                &self.tree_items,
                selected_file_idx,
                self.file_tree_scroll,
                &self.repo_commits,
                self.selected_repo_commit_idx,
                self.repo_commit_scroll,
                &self.stashes,
                self.selected_stash_idx,
                self.stash_scroll,
                self.focus == Focus::FileTree,
                self.filter_mode,
                &self.filter_query,
                self.file_view_mode,
                self.language,
                &self.theme,
            );
        }

        let cur_file = self.current_file();
        let syntax_enabled = self.config.ui.syntax_highlighting;

        let visual_range = if self.visual_mode {
            Some((self.visual_anchor.min(self.selected_row), self.visual_anchor.max(self.selected_row)))
        } else {
            None
        };

        if self.is_unified {
            render_unified(
                frame,
                diff_area,
                cur_file,
                self.scroll_y,
                self.selected_row,
                self.focus == Focus::DiffView,
                syntax_enabled,
                &self.theme,
            );
        } else {
            render_side_by_side(
                frame,
                diff_area,
                cur_file,
                self.scroll_y,
                self.selected_row,
                visual_range,
                self.column_side,
                self.focus == Focus::DiffView,
                syntax_enabled,
                &self.theme,
            );
        }

        if self.config.ui.overview_ruler {
            let vp_height = diff_area.height as usize;
            render_ruler(frame, ruler_area, cur_file, self.scroll_y, vp_height, &self.theme);
        }

        // 3. Render Status Bar
        let notif_text = if self.visual_mode {
            match self.language {
                Language::En => Some("VISUAL MODE: Select lines with j/k, press 's' to stage selected lines, Esc to exit"),
                Language::Pt => Some("MODO VISUAL: Selecione linhas com j/k, pressione 's' para stage, Esc para sair"),
            }
        } else if self.active_commit_view.is_some() {
            match self.language {
                Language::En => Some("COMMIT VIEW: Viewing commit diff · Press [Esc] or [1] to return to Live Diff"),
                Language::Pt => Some("VISUALIZAÇÃO DE COMMIT: Pressione [Esc] ou [1] para voltar ao Live Diff"),
            }
        } else {
            self.notification.as_ref().map(|(msg, _)| msg.as_str())
        };

        let mode_label = if self.visual_mode {
            "VISUAL"
        } else if self.focus == Focus::FileTree && self.show_drawer {
            "TREE"
        } else {
            "DIFF"
        };

        render_status_bar(
            frame,
            chunks[2],
            notif_text,
            mode_label,
            self.language,
            &self.theme,
        );

        // 4. Overlays
        if self.show_help {
            render_help_popup(frame, size, self.language, &self.theme);
        } else if self.show_worktrees {
            render_worktree_popup(
                frame,
                size,
                &self.worktrees,
                self.selected_worktree_idx,
                self.worktree_scroll,
                self.language,
                &self.theme,
            );
        } else if self.show_history {
            let path_str = self
                .get_underlying_file()
                .map(|f| f.display_path())
                .unwrap_or_default();
            render_history_popup(
                frame,
                size,
                &path_str,
                &self.history_commits,
                self.selected_history_idx,
                self.history_scroll,
                &self.theme,
            );
        } else if let Some((_, msg)) = &self.confirm_action {
            render_confirm_popup(frame, size, msg, self.language, &self.theme);
        }
    }
}
