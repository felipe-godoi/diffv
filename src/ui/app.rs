use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::config::Config;
use crate::core::engine::DiffEngine;
use crate::core::models::{
    CommitEntry, DiffKind, DiffSection, DrawerTab, FileDiff, Language, RepoStats, StageStatus, StashEntry,
    WatcherScanState, WorktreeEntry,
};


use crate::git::actions::{
    discard_file, discard_hunk, stage_file, stage_hunk, stage_partial_hunk, unstage_file,
    unstage_hunk, unstage_partial_hunk,
};
use crate::git::provider::GitProvider;
use crate::integration::github::{github_repo_url, open_commit_pr};
use crate::integration::clipboard::copy_hunk_as_markdown;
use crate::ui::components::file_tree::{
    build_tree_items, render_drawer, render_drawer_line_overlay, FileViewMode, TreeItem,
};
use crate::ui::components::details_popup::{render_details_popup, DetailsContent};
use crate::ui::components::header::render_header;
use crate::ui::components::help_popup::{render_confirm_popup, render_help_popup};
use crate::ui::components::ruler::render_ruler;
use crate::ui::components::side_by_side::{render_side_by_side, ColumnSide};
use crate::ui::components::toast::{render_toast, TOAST_DURATION};
use crate::ui::components::unified::render_unified;
use crate::ui::components::worktree_popup::{render_worktree_popup, WorktreeCreationState};
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    FileTree,
    DiffView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FzfRequest {
    Files,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSource {
    Loaded,
    CurrentFile,
    RepoCommit(usize),
    Stash(usize),
}

pub struct FzfQuery {
    pub items: Vec<String>,
    pub header: String,
}

const FULL_CONTEXT_LINES: usize = 1_000_000;

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
    pub full_context: bool,
    pub search_source: SearchSource,

    // Visual mode & column side
    pub visual_mode: bool,
    pub visual_anchor: usize,
    pub column_side: ColumnSide,
    pub scroll_x: [usize; 2],
    pub diff_width: u16,
    pub wrap_lines: bool,
    pub wrap_skip: usize,
    pub diff_row_map: Vec<usize>,

    // File tree mode & directory collapsing
    pub file_view_mode: FileViewMode,
    pub collapsed_dirs: HashSet<PathBuf>,
    pub tree_items: Vec<TreeItem>,
    pub selected_tree_idx: usize,

    // File commit history (popup for single file history 'H')
    pub show_history: bool,
    pub history_commits: Vec<CommitEntry>,
    pub history_file_path: Option<PathBuf>,
    history_return_focus: Focus,
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
    pub active_commit_info: Option<CommitEntry>,
    pub active_stash_info: Option<StashEntry>,

    // Pending multi-key sequences (e.g. 'g' then 'g', 'z' then 'z', ']' then 'c')
    pub pending_key: Option<char>,
    pub pending_key_time: Option<Instant>,

    // Details popup (commit details, stash details, or verbose file details)
    pub show_details_popup: bool,
    pub details_popup_scroll: usize,

    // Worktree creation state
    pub worktree_creation: Option<WorktreeCreationState>,

    // External fzf requests
    pub fzf_request: Option<FzfRequest>,

    // Terminal geometry for responsive drag resizing
    pub term_width: u16,
    pub term_height: u16,

    // Watcher scanning state and animation
    pub watcher_state: WatcherScanState,
    pub spinner_idx: usize,
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
            full_context: false,
            search_source: SearchSource::Loaded,
            visual_mode: false,
            visual_anchor: 0,
            column_side: ColumnSide::Right,
            scroll_x: [0, 0],
            diff_width: 0,
            wrap_lines: true,
            wrap_skip: 0,
            diff_row_map: Vec::new(),
            file_view_mode: FileViewMode::Tree, // Folders ("Pastas") is default!
            collapsed_dirs: HashSet::new(),
            tree_items: Vec::new(),
            selected_tree_idx: 0,
            show_history: false,
            history_commits: Vec::new(),
            history_file_path: None,
            history_return_focus: Focus::FileTree,
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
            active_commit_info: None,
            active_stash_info: None,
            pending_key: None,
            pending_key_time: None,
            show_details_popup: false,
            details_popup_scroll: 0,
            worktree_creation: None,
            fzf_request: None,
            term_width: 80,
            term_height: 25,
            watcher_state: if watch_mode {
                WatcherScanState::Scanning { scanned_dirs: 0 }
            } else {
                WatcherScanState::Idle
            },
            spinner_idx: 0,
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

    pub fn is_watcher_scanning(&self) -> bool {
        matches!(self.watcher_state, WatcherScanState::Scanning { .. })
    }

    pub fn update_watcher_progress(&mut self, scanned: usize, total: Option<usize>) {
        if let Some(total_dirs) = total {
            self.watcher_state = WatcherScanState::Ready { total_dirs };
        } else {
            self.watcher_state = WatcherScanState::Scanning { scanned_dirs: scanned };
        }
    }

    pub fn current_file(&self) -> Option<&FileDiff> {
        if let Some((_, commit_diff)) = &self.active_commit_view {
            return Some(commit_diff);
        }
        if self.show_history { return None; }
        self.get_underlying_file()
    }

    pub fn total_diff_rows(&self) -> usize {
        if let Some(file) = self.current_file() {
            if self.is_unified {
                file.hunks.iter().map(|h| h.lines.len() + 1).sum()
            } else {
                file.aligned_rows.len()
            }
        } else {
            0
        }
    }

    pub fn toggle_language(&mut self) {
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

    pub fn toggle_file_view_mode(&mut self) {
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

    pub fn switch_drawer_tab(&mut self, new_tab: DrawerTab) {
        if self.show_history || self.active_commit_info.is_some() || self.active_stash_info.is_some() { return; }
        self.show_drawer = true;
        if (self.active_commit_info.is_some() || self.active_stash_info.is_some() || self.active_commit_view.is_some())
            && new_tab == DrawerTab::Changes
        {
            self.active_commit_info = None;
            self.active_stash_info = None;
            self.active_commit_view = None;
            if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                self.files = saved_files;
                self.repo_stats = saved_stats;
                self.update_filter();
            }
        } else if new_tab == DrawerTab::Commits {
            if self.active_commit_info.is_some() {
                self.active_commit_info = None;
                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                    self.files = saved_files;
                    self.repo_stats = saved_stats;
                    self.update_filter();
                }
            }
            self.active_stash_info = None;
        } else if new_tab == DrawerTab::Stashes {
            if self.active_stash_info.is_some() {
                self.active_stash_info = None;
                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                    self.files = saved_files;
                    self.repo_stats = saved_stats;
                    self.update_filter();
                }
            }
            self.active_commit_info = None;
        }

        self.drawer_tab = new_tab;
        self.focus = Focus::FileTree;

        let msg = match (new_tab, self.language) {
            (DrawerTab::Changes, Language::En) => "Tab: Changes (Live Diff)",
            (DrawerTab::Changes, Language::Pt) => "Aba: Mudanças (Diff ao vivo)",
            (DrawerTab::Commits, Language::En) => "Tab: Commits (j/k select, Enter view)",
            (DrawerTab::Commits, Language::Pt) => "Aba: Commits (j/k seleciona, Enter visualiza)",
            (DrawerTab::Stashes, Language::En) => "Tab: Stashes (j/k select, Enter view)",
            (DrawerTab::Stashes, Language::Pt) => "Aba: Stashes (j/k seleciona, Enter visualiza)",
        };
        self.set_notification(msg);
    }

    pub fn scroll_viewport_down(&mut self, amount: usize) {
        if self.wrap_lines {
            let remaining = self.diff_row_map.iter().filter(|&&idx| idx == self.scroll_y).count();
            if remaining > amount {
                self.wrap_skip += amount;
                return;
            }
            self.wrap_skip = 0;
        }
        let total = self.total_diff_rows();
        if total > 0 {
            let max_scroll = total.saturating_sub(1);
            self.scroll_y = (self.scroll_y + amount).min(max_scroll);
            if self.selected_row < self.scroll_y {
                self.selected_row = self.scroll_y;
            }
        }
    }


    pub fn scroll_viewport_up(&mut self, amount: usize) {
        if self.wrap_lines && self.wrap_skip > 0 {
            self.wrap_skip = self.wrap_skip.saturating_sub(amount);
            return;
        }
        let vp = self.viewport_height.saturating_sub(3).max(1);
        self.scroll_y = self.scroll_y.saturating_sub(amount);
        if self.selected_row >= self.scroll_y + vp {
            self.selected_row = (self.scroll_y + vp).saturating_sub(1);
        }
    }

    pub fn jump_to_file(&mut self, path: &str) -> bool {
        self.jump_to_file_in(path, DiffSection::Changes)
    }

    /// Selects `path`, preferring its copy in `section` when it appears in both.
    pub fn jump_to_file_in(&mut self, path: &str, section: DiffSection) -> bool {
        let clean_path = path.trim();
        let matches = |f: &FileDiff| {
            f.new_path.to_string_lossy() == clean_path
                || f.display_path() == clean_path
                || (f.old_path.as_ref().map(|p| p.to_string_lossy() == clean_path).unwrap_or(false))
        };
        let found_idx = self.files.iter().position(|f| matches(f) && f.section == section)
            .or_else(|| self.files.iter().position(matches));

        if let Some(idx) = found_idx {
            if self.file_view_mode == FileViewMode::Tree {
                if let Some(tree_idx) = self.tree_items.iter().position(|t| t.file_index == Some(idx)) {
                    self.selected_tree_idx = tree_idx;
                }
            } else if let Some(filt_idx) = self.filtered_indices.iter().position(|&i| i == idx) {
                self.selected_filtered_idx = filt_idx;
            }
            self.selected_row = 0;
            self.wrap_skip = 0; self.scroll_y = 0;
            self.focus = Focus::DiffView;
            return true;
        }
        false
    }

    pub fn jump_to_line(&mut self, target_line_no: usize) {
        if let Some(file) = self.current_file() {
            if self.is_unified {
                let mut current_row = 0;
                let mut found = None;
                for hunk in &file.hunks {
                    current_row += 1;
                    for line in &hunk.lines {
                        if line.new_line_no == Some(target_line_no) || line.old_line_no == Some(target_line_no) {
                            found = Some(current_row);
                            break;
                        }
                        current_row += 1;
                    }
                    if found.is_some() {
                        break;
                    }
                }
                if let Some(r) = found {
                    self.selected_row = r;
                    let vp = self.viewport_height.saturating_sub(3).max(1);
                    self.scroll_y = r.saturating_sub(vp / 2);
                }
            } else {
                let row_idx = file.aligned_rows.iter().position(|r| {
                    r.right.as_ref().and_then(|l| l.new_line_no) == Some(target_line_no)
                        || r.left.as_ref().and_then(|l| l.old_line_no) == Some(target_line_no)
                });

                if let Some(row) = row_idx {
                    self.selected_row = row;
                    let vp = self.viewport_height.saturating_sub(3).max(1);
                    self.scroll_y = row.saturating_sub(vp / 2);
                }
            }
        }
    }

    fn search_label(&self, source: SearchSource) -> String {
        let (en, pt) = match source {
            SearchSource::CurrentFile => {
                let path = self.current_file().map(|f| f.display_path()).unwrap_or_default();
                (format!("file {}", path), format!("arquivo {}", path))
            }
            SearchSource::RepoCommit(idx) => {
                let hash: String = self.repo_commits[idx].hash.chars().take(7).collect();
                (format!("commit {}", hash), format!("commit {}", hash))
            }
            SearchSource::Stash(idx) => {
                let sel = self.stashes[idx].selector.clone();
                (format!("stash {}", sel), format!("stash {}", sel))
            }
            SearchSource::Loaded => match (&self.active_commit_info, &self.active_stash_info) {
                (Some(commit), _) => {
                    let hash: String = commit.hash.chars().take(7).collect();
                    (format!("commit {}", hash), format!("commit {}", hash))
                }
                (None, Some(stash)) => (format!("stash {}", stash.selector), format!("stash {}", stash.selector)),
                (None, None) => ("working tree changes".to_string(), "mudanças da working tree".to_string()),
            },
        };
        match self.language {
            Language::En => en,
            Language::Pt => pt,
        }
    }

    fn resolve_search_source(&self, request: FzfRequest) -> SearchSource {
        let in_diff = self.focus == Focus::DiffView || (self.show_history && self.active_commit_view.is_some());
        if request == FzfRequest::Text && in_diff && self.current_file().is_some() {
            return SearchSource::CurrentFile;
        }
        if self.active_commit_info.is_some() || self.active_stash_info.is_some() || self.show_history {
            return SearchSource::Loaded;
        }
        match self.drawer_tab {
            DrawerTab::Commits if self.selected_repo_commit_idx < self.repo_commits.len() => SearchSource::RepoCommit(self.selected_repo_commit_idx),
            DrawerTab::Stashes if self.selected_stash_idx < self.stashes.len() => SearchSource::Stash(self.selected_stash_idx),
            _ => SearchSource::Loaded,
        }
    }

    /// Builds the fzf candidates for the current context: the open diff file,
    /// the selected/open commit or stash, or the working tree changes.
    pub fn prepare_fzf(&mut self, request: FzfRequest) -> FzfQuery {
        self.sync_git_context();
        let source = self.resolve_search_source(request);
        self.search_source = source;
        let external = match (source, &self.mode) {
            (SearchSource::RepoCommit(idx), AppMode::Git { git_provider, .. }) => git_provider.load_commit_full_diff(&self.repo_commits[idx].hash),
            (SearchSource::Stash(idx), AppMode::Git { git_provider, .. }) => git_provider.load_stash_diff(&self.stashes[idx].selector),
            _ => Ok(Vec::new()),
        };
        let external = match external {
            Ok(files) => files,
            Err(e) => {
                self.set_notification(format!("Search error: {}", e));
                Vec::new()
            }
        };
        let files: Vec<&FileDiff> = match source {
            SearchSource::CurrentFile => self.current_file().into_iter().collect(),
            SearchSource::Loaded => self.files.iter().collect(),
            _ => external.iter().collect(),
        };
        let items = match request {
            FzfRequest::Files => {
                let mut seen = std::collections::HashSet::new();
                files.iter().map(|f| f.display_path()).filter(|p| seen.insert(p.clone())).collect()
            }
            FzfRequest::Text => diff_text_lines(&files),
        };
        let label = self.search_label(source);
        let header = match (request, self.language) {
            (FzfRequest::Files, Language::En) => format!("Files in {} (Esc to cancel)", label),
            (FzfRequest::Files, Language::Pt) => format!("Arquivos em {} (Esc para cancelar)", label),
            (FzfRequest::Text, Language::En) => format!("Search text in {} (Esc to cancel)", label),
            (FzfRequest::Text, Language::Pt) => format!("Buscar texto em {} (Esc para cancelar)", label),
        };
        FzfQuery { items, header }
    }

    fn enter_search_source(&mut self) {
        match self.search_source {
            SearchSource::RepoCommit(idx) => {
                self.selected_repo_commit_idx = idx;
                self.load_selected_repo_commit();
            }
            SearchSource::Stash(idx) => {
                self.selected_stash_idx = idx;
                self.load_selected_stash();
            }
            SearchSource::Loaded | SearchSource::CurrentFile => {}
        }
    }

    pub fn handle_fzf_file_result(&mut self, selected_file: String) {
        self.enter_search_source();
        if self.jump_to_file(&selected_file) {
            let msg = match self.language {
                Language::En => format!("Jumped to file: {}", selected_file),
                Language::Pt => format!("Saltou para o arquivo: {}", selected_file),
            };
            self.set_notification(msg);
        }
    }

    pub fn handle_fzf_text_result(&mut self, selected_line: String) {
        let mut fields = selected_line.split('\t');
        let location = fields.next().unwrap_or_default();
        let section = if selected_line.ends_with("\tstaged") { DiffSection::Staged } else { DiffSection::Changes };
        let Some((path, line)) = location.rsplit_once(':') else { return; };
        let line_no: usize = line.parse().unwrap_or(0);
        if self.search_source != SearchSource::CurrentFile {
            self.enter_search_source();
            if !self.jump_to_file_in(path, section) {
                return;
            }
        }
        self.focus = Focus::DiffView;
        if line_no > 0 {
            self.jump_to_line(line_no);
        }
        let msg = match self.language {
            Language::En => format!("Jumped to {}:{}", path, line_no),
            Language::Pt => format!("Saltou para {}:{}", path, line_no),
        };
        self.set_notification(msg);
    }

    fn effective_context_lines(&self) -> usize {
        if self.full_context { FULL_CONTEXT_LINES } else { self.config.diff.context_lines }
    }

    fn sync_git_context(&mut self) {
        let context = self.effective_context_lines();
        if let AppMode::Git { git_provider, .. } = &mut self.mode {
            git_provider.context_lines = context;
        }
    }

    fn cursor_line_no(&self) -> Option<usize> {
        let file = self.current_file()?;
        if self.is_unified {
            let mut row = 0;
            for hunk in &file.hunks {
                if row == self.selected_row {
                    return hunk.lines.first().and_then(|l| l.new_line_no.or(l.old_line_no));
                }
                row += 1;
                for line in &hunk.lines {
                    if row == self.selected_row {
                        return line.new_line_no.or(line.old_line_no);
                    }
                    row += 1;
                }
            }
            None
        } else {
            let r = file.aligned_rows.get(self.selected_row)?;
            r.right.as_ref().and_then(|l| l.new_line_no).or_else(|| r.left.as_ref().and_then(|l| l.old_line_no))
        }
    }

    /// Hunk under a view row, plus the hunk-line index when the row is a line
    /// (unified rows include one header row per hunk; side-by-side rows are aligned pairs).
    fn row_hunk(&self, row: usize) -> Option<(usize, Vec<usize>)> {
        let file = self.current_file()?;
        if self.is_unified {
            let mut start = 0;
            for (h_idx, hunk) in file.hunks.iter().enumerate() {
                let end = start + hunk.lines.len();
                if row == start {
                    return Some((h_idx, Vec::new()));
                }
                if row <= end {
                    return Some((h_idx, vec![row - start - 1]));
                }
                start = end + 1;
            }
            None
        } else {
            let r = file.aligned_rows.get(row)?;
            Some((r.hunk_index?, r.left_line_idx.into_iter().chain(r.right_line_idx).collect()))
        }
    }

    fn cursor_hunk(&self) -> Option<usize> {
        self.row_hunk(self.selected_row).map(|(h, _)| h)
    }

    fn hunk_start_row(&self, hunk_idx: usize) -> Option<usize> {
        let file = self.current_file()?;
        if self.is_unified {
            Some(file.hunks.iter().take(hunk_idx).map(|h| h.lines.len() + 1).sum())
        } else {
            file.aligned_rows.iter().position(|r| r.hunk_index == Some(hunk_idx))
        }
    }

    fn column_labels(&self) -> [&'static str; 2] {
        if self.active_commit_view.is_some() || self.active_commit_info.is_some() {
            return ["PARENT", "COMMIT"];
        }
        if self.active_stash_info.is_some() {
            return ["BASE", "STASH"];
        }
        let working_tree = matches!(&self.mode, AppMode::Git { target_ref: None, .. }) && !self.staged_only;
        match self.current_file() {
            Some(file) if working_tree && file.section == DiffSection::Staged => ["HEAD", "STAGED"],
            Some(file) if working_tree => {
                let partly_staged = self.files.iter().any(|f| f.section == DiffSection::Staged && f.new_path == file.new_path);
                [if partly_staged { "STAGED" } else { "HEAD" }, "WORKING TREE"]
            }
            _ => ["ORIGINAL", "MODIFIED"],
        }
    }

    fn selected_commit_hash(&self) -> Option<String> {
        if self.show_history {
            return self.history_commits.get(self.selected_history_idx).map(|c| c.hash.clone());
        }
        if let Some(commit) = &self.active_commit_info {
            return Some(commit.hash.clone());
        }
        if self.drawer_tab == DrawerTab::Commits {
            return self.repo_commits.get(self.selected_repo_commit_idx).map(|c| c.hash.clone());
        }
        None
    }

    /// Opens the GitHub pull request for the commit under focus (commits tab,
    /// open commit or file history), falling back to the commit page.
    pub fn open_selected_commit_pr(&mut self) {
        let Some(hash) = self.selected_commit_hash() else {
            self.set_notification(match self.language {
                Language::En => "Select a commit (Commits tab or file history) to open its PR",
                Language::Pt => "Selecione um commit (aba Commits ou histórico) para abrir o PR",
            });
            return;
        };
        let AppMode::Git { git_provider, .. } = &self.mode else { return; };
        let repo_root = git_provider.repo_root.clone();
        let Some(repo_url) = github_repo_url(&repo_root) else {
            self.set_notification(match self.language {
                Language::En => "Remote 'origin' is not a GitHub repository",
                Language::Pt => "O remote 'origin' não é um repositório do GitHub",
            });
            return;
        };
        let short: String = hash.chars().take(7).collect();
        open_commit_pr(repo_root, repo_url, hash);
        self.set_notification(match self.language {
            Language::En => format!("Opening PR for {} in the browser…", short),
            Language::Pt => format!("Abrindo PR de {} no navegador…", short),
        });
    }

    fn current_section(&self) -> Option<DiffSection> {
        self.current_file().map(|f| f.section)
    }

    fn require_section(&mut self, wanted: DiffSection) -> bool {
        if self.current_section() == Some(wanted) {
            return true;
        }
        self.set_notification(match (wanted, self.language) {
            (DiffSection::Changes, Language::En) => "Already staged · u to unstage",
            (DiffSection::Changes, Language::Pt) => "Já está staged · u para unstage",
            (DiffSection::Staged, Language::En) => "Not staged · select it under Staged to unstage",
            (DiffSection::Staged, Language::Pt) => "Não está staged · selecione em Staged para unstage",
        });
        false
    }

    fn can_act_on_hunk(&mut self) -> bool {
        if !self.can_modify_index() {
            return false;
        }
        if self.full_context {
            self.set_notification(match self.language {
                Language::En => "Full file is one hunk · press x to collapse, or use v to select lines",
                Language::Pt => "Arquivo inteiro é um hunk só · aperte x para recolher, ou use v para selecionar linhas",
            });
            return false;
        }
        true
    }

    fn can_modify_index(&mut self) -> bool {
        if self.active_commit_info.is_some() || self.active_stash_info.is_some() || self.show_history {
            self.set_notification(match self.language {
                Language::En => "Staging only works on working tree changes",
                Language::Pt => "Stage só funciona nas mudanças da working tree",
            });
            return false;
        }
        true
    }

    /// Toggles between hunk-only diffs and the whole file as context,
    /// keeping the current file and cursor line.
    pub fn toggle_full_context(&mut self) {
        let path = self.current_file().map(|f| f.display_path());
        let line = self.cursor_line_no();
        let focus = self.focus;
        let scroll_x = self.scroll_x;
        self.full_context = !self.full_context;

        if self.active_commit_view.is_some() {
            self.active_commit_view = None;
            self.load_selected_commit_diff();
        } else if self.active_commit_info.is_some() {
            self.load_selected_repo_commit();
        } else if self.active_stash_info.is_some() {
            self.load_selected_stash();
        }
        if let Err(e) = self.reload_diffs_internal(false) {
            self.set_notification(format!("Reload error: {}", e));
            return;
        }

        if self.active_commit_view.is_none() {
            if let Some(path) = &path {
                self.jump_to_file(path);
            }
        }
        if let Some(line) = line {
            self.jump_to_line(line);
        }
        self.focus = focus;
        self.scroll_x = scroll_x;
        let msg = match (self.language, self.full_context) {
            (Language::En, true) => "Showing full file · x to collapse",
            (Language::En, false) => "Showing changes only · x to expand",
            (Language::Pt, true) => "Mostrando arquivo inteiro · x para recolher",
            (Language::Pt, false) => "Mostrando apenas mudanças · x para expandir",
        };
        self.set_notification(msg);
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

    pub fn expire_notification(&mut self) {
        if self.notification.as_ref().is_some_and(|(_, time)| time.elapsed() > TOAST_DURATION) {
            self.notification = None;
        }
    }

    pub fn set_notification(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), Instant::now()));
    }

    pub fn reload_diffs(&mut self) {
        if let Err(e) = self.reload_diffs_internal(true) {
            self.set_notification(format!("Reload error: {}", e));
        }
    }

    /// After the selected file disappears (e.g. fully staged), land on the
    /// closest file instead of a section header or folder.
    fn select_nearest_tree_file(&mut self) {
        let start = self.selected_tree_idx.min(self.tree_items.len().saturating_sub(1));
        let forward = (start..self.tree_items.len()).find(|&i| self.tree_items[i].file_index.is_some());
        let backward = (0..start).rev().find(|&i| self.tree_items[i].file_index.is_some());
        if let Some(pos) = forward.or(backward) {
            self.selected_tree_idx = pos;
        }
    }

    fn reload_diffs_internal(&mut self, is_live_reload: bool) -> anyhow::Result<()> {
        let previous_file = self.current_file().map(|f| (f.new_path.clone(), f.section));
        let prev_scroll = self.scroll_y;
        let prev_row = self.selected_row;

        self.sync_git_context();
        let diff_engine = DiffEngine::new(
            &self.config.diff.algorithm,
            self.effective_context_lines(),
            self.ignore_whitespace,
        );

        let (files, stats) = match &self.mode {
            AppMode::Git { target_ref, git_provider } => git_provider.load_diffs(
                target_ref.as_deref(),
                self.staged_only,
                self.config.watcher.watch_untracked,
                self.ignore_whitespace,
            )?,
            AppMode::FilePair(a, b) => {
                let diff = diff_engine.compare_files(a, b)?;
                let stats = RepoStats {
                    repo_name: "files".to_string(),
                    branch: "local".to_string(),
                    root_dir: PathBuf::from("."),
                    total_additions: diff.stats.additions,
                    total_deletions: diff.stats.deletions,
                    file_count: 1,
                };
                (vec![diff], stats)
            }
            AppMode::DirPair(a, b) => {
                let files = diff_engine.compare_directories(a, b)?;
                let stats = RepoStats {
                    repo_name: "directories".to_string(),
                    branch: "local".to_string(),
                    root_dir: PathBuf::from("."),
                    total_additions: files.iter().map(|f| f.stats.additions).sum(),
                    total_deletions: files.iter().map(|f| f.stats.deletions).sum(),
                    file_count: files.len(),
                };
                (files, stats)
            }
            AppMode::Stdin => diff_engine.compare_stdin()?,
        };

        // While a commit or stash is open, the working tree only refreshes the
        // snapshot restored on exit; the inspected files stay untouched.
        if self.active_commit_info.is_some() || self.active_stash_info.is_some() {
            self.live_snapshot = Some((files, stats));
            return Ok(());
        }
        self.files = files;
        self.repo_stats = stats;

        self.update_filter();

        // Restore file selection if previous file still exists
        if let Some(prev) = previous_file {
            let same = |f: &FileDiff| f.new_path == prev.0 && f.section == prev.1;
            if self.file_view_mode == FileViewMode::Tree {
                match self.tree_items.iter().position(|item| item.file_index.is_some_and(|i| same(&self.files[i]))) {
                    Some(pos) => self.selected_tree_idx = pos,
                    None => self.select_nearest_tree_file(),
                }
            } else if let Some(pos) = self.filtered_indices.iter().position(|&idx| same(&self.files[idx])) {
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
                self.wrap_skip = 0; self.scroll_y = 0;
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
            self.language,
        );

        if self.selected_tree_idx >= self.tree_items.len() {
            self.selected_tree_idx = self.tree_items.len().saturating_sub(1);
        }
    }

    fn return_to_changes(&mut self) {
        self.show_history = false;
        self.active_commit_info = None;
        self.active_stash_info = None;
        self.active_commit_view = None;
        self.visual_mode = false;
        self.filter_mode = false;
        self.filter_query.clear();
        if let Some((files, stats)) = self.live_snapshot.take() {
            self.files = files;
            self.repo_stats = stats;
        }
        self.drawer_tab = DrawerTab::Changes;
        self.focus = Focus::FileTree;
        self.show_drawer = true;
        self.scroll_x = [0, 0];
        self.wrap_skip = 0;
        self.update_filter();
    }

    fn horizontal_index(&self) -> usize {
        if self.is_unified || self.column_side == ColumnSide::Right { 1 } else { 0 }
    }

    fn horizontal_limit(&self) -> usize {
        use unicode_width::UnicodeWidthStr;
        let idx = self.horizontal_index();
        let max_width = self.current_file().map(|f| {
            if self.is_unified {
                f.hunks.iter().flat_map(|h| &h.lines).map(|l| l.content.width()).max().unwrap_or(0)
            } else {
                f.aligned_rows.iter().filter_map(|r| if idx == 0 { r.left.as_ref() } else { r.right.as_ref() })
                    .map(|l| l.content.width()).max().unwrap_or(0)
            }
        }).unwrap_or(0);
        let width = self.diff_width.saturating_sub(2) as usize;
        let content_width = if self.is_unified { width.saturating_sub(16) } else { (width.saturating_sub(1) / 2).saturating_sub(10) };
        max_width.saturating_sub(content_width.max(1))
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.expire_notification();

        // 0. Details Modal Active
        if self.show_details_popup {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'Q') | KeyCode::Char('i') => {
                    self.show_details_popup = false;
                    self.details_popup_scroll = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.details_popup_scroll += 1;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.details_popup_scroll = self.details_popup_scroll.saturating_sub(1);
                }
                KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.details_popup_scroll += 10;
                }
                KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.details_popup_scroll = self.details_popup_scroll.saturating_sub(10);
                }
                KeyCode::Enter => {
                    if self.drawer_tab == DrawerTab::Commits && self.active_commit_info.is_none() && !self.show_history {
                        self.show_details_popup = false;
                        self.details_popup_scroll = 0;
                        self.load_selected_repo_commit();
                    } else if self.drawer_tab == DrawerTab::Stashes && self.active_stash_info.is_none() && !self.show_history {
                        self.show_details_popup = false;
                        self.details_popup_scroll = 0;
                        self.load_selected_stash();
                    }
                }
                _ => {}
            }
            return;
        }

        // Search shortcuts also work from temporary drawers and while filtering.
        if !self.show_worktrees && !self.show_help && self.confirm_action.is_none()
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            let request = match key.code {
                KeyCode::Char('p' | 'P') => Some(FzfRequest::Files),
                KeyCode::Char('f' | 'F') => Some(FzfRequest::Text),
                _ => None,
            };
            if let Some(request) = request {
                self.pending_key = None;
                self.pending_key_time = None;
                self.filter_mode = false;
                self.fzf_request = Some(request);
                return;
            }
        }

        // Check pending key sequence timeout (1 second)
        if let Some(time) = self.pending_key_time {
            if time.elapsed() > Duration::from_millis(1000) {
                self.pending_key = None;
                self.pending_key_time = None;
            }
        }

        if let Some(pending) = self.pending_key.take() {
            self.pending_key_time = None;
            match pending {
                'g' => match key.code {
                    KeyCode::Char('g') => {
                        // gg: jump to top of view
                        if self.focus == Focus::DiffView {
                            self.selected_row = 0;
                            self.wrap_skip = 0; self.scroll_y = 0;
                        } else if self.drawer_tab == DrawerTab::Changes || self.active_commit_info.is_some() || self.active_stash_info.is_some() {
                            if self.file_view_mode == FileViewMode::Tree {
                                self.selected_tree_idx = 0;
                                self.file_tree_scroll = 0;
                            } else {
                                self.selected_filtered_idx = 0;
                                self.file_tree_scroll = 0;
                            }
                        } else if self.drawer_tab == DrawerTab::Commits {
                            self.selected_repo_commit_idx = 0;
                            self.repo_commit_scroll = 0;
                        } else if self.drawer_tab == DrawerTab::Stashes {
                            self.selected_stash_idx = 0;
                            self.stash_scroll = 0;
                        }
                        return;
                    }
                    KeyCode::Char('h') => {
                        // gh: open git history
                        self.open_file_history();
                        return;
                    }
                    KeyCode::Char('l') => {
                        // gl: toggle language
                        self.toggle_language();
                        return;
                    }
                    _ => {}
                },
                'z' => match key.code {
                    KeyCode::Char('z') => {
                        // zz: center cursor in viewport
                        let vp = self.viewport_height.saturating_sub(3).max(1);
                        self.scroll_y = self.selected_row.saturating_sub(vp / 2);
                        return;
                    }
                    KeyCode::Char('t') => {
                        // zt: scroll cursor to top of viewport
                        self.scroll_y = self.selected_row;
                        return;
                    }
                    KeyCode::Char('b') => {
                        // zb: scroll cursor to bottom of viewport
                        let vp = self.viewport_height.saturating_sub(3).max(1);
                        self.scroll_y = self.selected_row.saturating_sub(vp.saturating_sub(1));
                        return;
                    }
                    _ => {}
                },
                ']' => {
                    if key.code == KeyCode::Char('c') {
                        // ]c was pressed: already jumped hunk on ']'
                        return;
                    }
                }
                '['
                    if key.code == KeyCode::Char('c') => {
                        // [c was pressed: already jumped hunk on '['
                        return;
                    }
                _ => {}
            }
        }

        // 1. Worktrees Modal Active
        if self.show_worktrees {
            if let Some(creation) = &mut self.worktree_creation {
                match key.code {
                    KeyCode::Esc => {
                        self.worktree_creation = None;
                    }
                    KeyCode::Tab => {
                        if creation.active_field == 1 { creation.complete_branch(); }
                        creation.active_field = 1 - creation.active_field;
                    }
                    KeyCode::BackTab => { creation.active_field = 1 - creation.active_field; }
                    KeyCode::Down | KeyCode::Up => {
                        let count = creation.candidates().len();
                        if creation.active_field == 1 && count > 0 {
                            creation.selected_candidate = if key.code == KeyCode::Down {
                                (creation.selected_candidate + 1) % count
                            } else { (creation.selected_candidate + count - 1) % count };
                        } else { creation.active_field = 1 - creation.active_field; }
                    }
                    KeyCode::Backspace => {
                        if creation.active_field == 0 {
                            creation.path_input.pop();
                            creation.path_manual = true;
                        } else {
                            creation.branch_input.pop();
                            creation.update_branch();
                        }
                    }
                    KeyCode::Char(c) => {
                        if creation.active_field == 0 {
                            creation.path_input.push(c);
                            creation.path_manual = true;
                        } else {
                            creation.branch_input.push(c);
                            creation.update_branch();
                        }
                    }
                    KeyCode::Enter => {
                        let path = creation.path_input.clone();
                        let branch = creation.branch_input.clone();
                        if let AppMode::Git { git_provider, .. } = &mut self.mode {
                            match git_provider.add_worktree(&path, &branch) {
                                Ok(new_path) => {
                                    let cwd = std::env::current_dir().unwrap_or_else(|_| git_provider.repo_root.clone());
                                    if let Ok(wts) = git_provider.get_worktrees(&cwd) {
                                        self.worktrees = wts;
                                    }
                                    if let Some(pos) = self.worktrees.iter().position(|w| w.path == new_path) {
                                        self.selected_worktree_idx = pos;
                                        self.switch_to_selected_worktree();
                                    }
                                    self.worktree_creation = None;
                                    self.show_worktrees = false;
                                    let msg = match self.language {
                                        Language::En => format!("Created & switched to worktree: {}", new_path.display()),
                                        Language::Pt => format!("Criado e alternado para worktree: {}", new_path.display()),
                                    };
                                    self.set_notification(msg);
                                }
                                Err(e) => {
                                    if let Some(c) = &mut self.worktree_creation {
                                        c.error_msg = Some(e.to_string());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Esc | KeyCode::Char('W') | KeyCode::Char('q' | 'Q') => {
                    self.show_worktrees = false;
                }
                KeyCode::Char('a') | KeyCode::Char('n') | KeyCode::Char('c') => {
                    if let AppMode::Git { git_provider, .. } = &self.mode {
                        let base = if git_provider.repo_root.join(".worktree").is_dir() {
                            ".worktree/".to_string()
                        } else { "../".to_string() };
                        self.worktree_creation = Some(WorktreeCreationState::new(base, git_provider.get_branches()));
                    }
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
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') || matches!(key.code, KeyCode::Char('q' | 'Q')) {
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

        if matches!(key.code, KeyCode::Char('q' | 'Q')) {
            if self.show_history || self.active_commit_info.is_some() || self.active_stash_info.is_some()
                || self.active_commit_view.is_some() || self.live_snapshot.is_some()
                || self.drawer_tab != DrawerTab::Changes || self.focus != Focus::FileTree
                || self.visual_mode || self.filter_mode || !self.filter_query.is_empty()
            { self.return_to_changes(); } else { self.should_quit = true; }
            return;
        }

        // Temporary file history drawer
        if self.show_history && self.focus == Focus::FileTree {
            match key.code {
                KeyCode::Tab | KeyCode::BackTab if self.active_commit_view.is_some() => self.focus = Focus::DiffView,
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => self.file_tree_down(1),
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => self.file_tree_up(1),
                KeyCode::PageDown => self.file_tree_down(self.file_tree_height.saturating_sub(4).max(1)),
                KeyCode::PageUp => self.file_tree_up(self.file_tree_height.saturating_sub(4).max(1)),
                KeyCode::Char('G') => self.file_tree_down(self.history_commits.len()),
                KeyCode::Home => self.file_tree_up(self.history_commits.len()),
                KeyCode::Esc | KeyCode::Char('H') | KeyCode::Char('q') => {
                    self.show_history = false;
                    self.active_commit_view = None;
                    self.history_file_path = None;
                    self.focus = self.history_return_focus;
                }
                KeyCode::Char('j') | KeyCode::Down => self.file_tree_down(1),
                KeyCode::Char('k') | KeyCode::Up => self.file_tree_up(1),
                KeyCode::Char('i') => {
                    self.show_details_popup = true;
                    self.details_popup_scroll = 0;
                }
                KeyCode::Char('o') => self.open_selected_commit_pr(),
                KeyCode::Char('r') => {
                    self.wrap_lines = !self.wrap_lines;
                    self.wrap_skip = 0;
                    self.scroll_x = [0, 0];
                }
                KeyCode::Char('?') => self.show_help = true,
                KeyCode::Enter => {
                    self.load_selected_commit_diff();
                    if self.active_commit_view.is_some() { self.focus = Focus::DiffView; }
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
                if self.visual_mode {
                    self.visual_mode = false;
                    let msg = match self.language {
                        Language::En => "Exited visual mode",
                        Language::Pt => "Saiu do modo visual",
                    };
                    self.set_notification(msg);
                } else if self.focus == Focus::DiffView {
                    self.focus = Focus::FileTree;
                    self.show_drawer = true;
                    let msg = match self.language {
                        Language::En => "Returned to File Drawer",
                        Language::Pt => "Retornou ao painel lateral",
                    };
                    self.set_notification(msg);
                } else if self.active_commit_info.is_some() {
                    self.active_commit_info = None;
                    if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                        self.files = saved_files;
                        self.repo_stats = saved_stats;
                        self.update_filter();
                    }
                    self.focus = Focus::FileTree;
                    self.drawer_tab = DrawerTab::Commits;
                    let msg = match self.language {
                        Language::En => "Returned to Commits list",
                        Language::Pt => "Retornou à lista de commits",
                    };
                    self.set_notification(msg);
                } else if self.active_stash_info.is_some() {
                    self.active_stash_info = None;
                    if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                        self.files = saved_files;
                        self.repo_stats = saved_stats;
                        self.update_filter();
                    }
                    self.focus = Focus::FileTree;
                    self.drawer_tab = DrawerTab::Stashes;
                    let msg = match self.language {
                        Language::En => "Returned to Stashes list",
                        Language::Pt => "Retornou à lista de stashes",
                    };
                    self.set_notification(msg);
                } else if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
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
                } else {
                    let msg = match self.language {
                        Language::En => "Press 'q' to quit diffv",
                        Language::Pt => "Pressione 'q' para sair do diffv",
                    };
                    self.set_notification(msg);
                }
            }

            KeyCode::Char('H') => {
                if self.focus == Focus::DiffView {
                    self.selected_row = self.scroll_y;
                } else if self.active_commit_view.is_some() {
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
            KeyCode::Char('M') => {
                if self.focus == Focus::DiffView {
                    let vp = self.viewport_height.saturating_sub(3).max(1);
                    let total = self.total_diff_rows();
                    self.selected_row = (self.scroll_y + vp / 2).min(total.saturating_sub(1));
                }
            }
            KeyCode::Char('L') => {
                if self.focus == Focus::DiffView {
                    let vp = self.viewport_height.saturating_sub(3).max(1);
                    let total = self.total_diff_rows();
                    self.selected_row = (self.scroll_y + vp.saturating_sub(1)).min(total.saturating_sub(1));
                } else {
                    self.toggle_language();
                }
            }
            KeyCode::F(2) => {
                self.toggle_language();
            }
            KeyCode::Char('i') => {
                self.show_details_popup = !self.show_details_popup;
                self.details_popup_scroll = 0;
            }
            KeyCode::Char('g') => {
                self.pending_key = Some('g');
                self.pending_key_time = Some(Instant::now());
            }
            KeyCode::Char('G') => {
                if self.focus == Focus::DiffView {
                    let total = self.total_diff_rows();
                    if total > 0 {
                        self.selected_row = total - 1;
                        let vp = self.viewport_height.saturating_sub(3).max(1);
                        self.scroll_y = self.selected_row.saturating_sub(vp.saturating_sub(1));
                    }
                } else if self.drawer_tab == DrawerTab::Changes || self.active_commit_info.is_some() || self.active_stash_info.is_some() {
                    if self.file_view_mode == FileViewMode::Tree {
                        if !self.tree_items.is_empty() {
                            self.selected_tree_idx = self.tree_items.len() - 1;
                            let vp = self.file_tree_height.saturating_sub(4).max(1);
                            self.file_tree_scroll = self.selected_tree_idx.saturating_sub(vp.saturating_sub(1));
                        }
                    } else if !self.filtered_indices.is_empty() {
                        self.selected_filtered_idx = self.filtered_indices.len() - 1;
                        let vp = self.file_tree_height.saturating_sub(4).max(1);
                        self.file_tree_scroll = self.selected_filtered_idx.saturating_sub(vp.saturating_sub(1));
                    }
                } else if self.drawer_tab == DrawerTab::Commits {
                    if !self.repo_commits.is_empty() {
                        self.selected_repo_commit_idx = self.repo_commits.len() - 1;
                        let vp = self.file_tree_height.saturating_sub(4).max(1);
                        self.repo_commit_scroll = self.selected_repo_commit_idx.saturating_sub(vp.saturating_sub(1));
                    }
                } else if self.drawer_tab == DrawerTab::Stashes
                    && !self.stashes.is_empty() {
                        self.selected_stash_idx = self.stashes.len() - 1;
                        let vp = self.file_tree_height.saturating_sub(4).max(1);
                        self.stash_scroll = self.selected_stash_idx.saturating_sub(vp.saturating_sub(1));
                    }
            }
            KeyCode::Char('z') => {
                if self.focus == Focus::DiffView {
                    self.pending_key = Some('z');
                    self.pending_key_time = Some(Instant::now());
                }
            }
            KeyCode::Char('W') => {
                self.show_worktrees = true;
            }
            KeyCode::Char('b') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
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
                self.switch_drawer_tab(DrawerTab::Changes);
            }
            KeyCode::Char('2') => {
                self.switch_drawer_tab(DrawerTab::Commits);
            }
            KeyCode::Char('3') => {
                self.switch_drawer_tab(DrawerTab::Stashes);
            }
            KeyCode::Tab | KeyCode::BackTab if self.focus == Focus::DiffView => {
                self.column_side = if self.column_side == ColumnSide::Left { ColumnSide::Right } else { ColumnSide::Left };
            }
            KeyCode::Tab | KeyCode::BackTab if self.show_history || self.active_commit_info.is_some() || self.active_stash_info.is_some() => {
                self.focus = if self.focus == Focus::FileTree { Focus::DiffView } else { Focus::FileTree };
            }
            KeyCode::Tab => {
                let next = match self.drawer_tab {
                    DrawerTab::Changes => DrawerTab::Commits,
                    DrawerTab::Commits => DrawerTab::Stashes,
                    DrawerTab::Stashes => DrawerTab::Changes,
                };
                self.switch_drawer_tab(next);
            }
            KeyCode::BackTab => {
                let prev = match self.drawer_tab {
                    DrawerTab::Changes => DrawerTab::Stashes,
                    DrawerTab::Commits => DrawerTab::Changes,
                    DrawerTab::Stashes => DrawerTab::Commits,
                };
                self.switch_drawer_tab(prev);
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.fzf_request = Some(FzfRequest::Files);
            }
            KeyCode::Char('\\') => {
                self.fzf_request = Some(FzfRequest::Text);
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focus == Focus::FileTree {
                    self.file_tree_down(1);
                } else {
                    self.scroll_viewport_down(1);
                }
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focus == Focus::FileTree {
                    self.file_tree_up(1);
                } else {
                    self.scroll_viewport_up(1);
                }
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }

            KeyCode::Char('r') => {
                self.wrap_lines = !self.wrap_lines;
                self.wrap_skip = 0;
                self.scroll_x = [0, 0];
            }
            KeyCode::Char('m') => {
                self.is_unified = !self.is_unified;
                let mode = if self.is_unified { "Unified" } else { "Side-by-Side" };
                self.set_notification(format!("Switched to {} view", mode));
            }
            KeyCode::Char('x') => self.toggle_full_context(),
            KeyCode::Char('o') => self.open_selected_commit_pr(),
            KeyCode::Char('w') => {
                self.watch_mode = !self.watch_mode;
                let status = if self.watch_mode { "ON" } else { "OFF" };
                self.set_notification(format!("Live Watch Mode: {}", status));
            }
            KeyCode::Char('t') => {
                self.toggle_file_view_mode();
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
            KeyCode::Char('0') | KeyCode::Home if self.focus == Focus::DiffView => {
                let idx = self.horizontal_index(); self.scroll_x[idx] = 0;
            }
            KeyCode::Char('$') | KeyCode::End if self.focus == Focus::DiffView => {
                let idx = self.horizontal_index();
                self.scroll_x[idx] = self.horizontal_limit();
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
                    let idx = self.horizontal_index();
                    self.scroll_x[idx] = self.scroll_x[idx].saturating_sub(4);
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
                    let idx = self.horizontal_index();
                    self.scroll_x[idx] = self.scroll_x[idx].saturating_add(4).min(self.horizontal_limit());
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
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let page = self.viewport_height.saturating_sub(4).max(1);
                if self.focus == Focus::FileTree {
                    self.file_tree_up(page);
                } else {
                    self.scroll_up(page);
                }
            }
            KeyCode::Char('J') | KeyCode::PageDown => {
                let page = self.viewport_height.saturating_sub(4).max(1);
                if self.focus == Focus::FileTree {
                    self.file_tree_down(page);
                } else {
                    self.scroll_down(page);
                }
            }
            KeyCode::Char('K') | KeyCode::PageUp => {
                let page = self.viewport_height.saturating_sub(4).max(1);
                if self.focus == Focus::FileTree {
                    self.file_tree_up(page);
                } else {
                    self.scroll_up(page);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let half = (self.viewport_height.saturating_sub(3) / 2).max(1);
                if self.focus == Focus::FileTree {
                    self.file_tree_down(half);
                } else {
                    self.scroll_down(half);
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let half = (self.viewport_height.saturating_sub(3) / 2).max(1);
                if self.focus == Focus::FileTree {
                    self.file_tree_up(half);
                } else {
                    self.scroll_up(half);
                }
            }
            KeyCode::Char(']') => {
                self.jump_next_hunk();
                self.pending_key = Some(']');
                self.pending_key_time = Some(Instant::now());
            }
            KeyCode::Char('[') => {
                self.jump_prev_hunk();
                self.pending_key = Some('[');
                self.pending_key_time = Some(Instant::now());
            }
            KeyCode::Char('n') => {
                self.jump_next_hunk();
            }
            KeyCode::Char('N') | KeyCode::Char('p') => {
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
                            if self.active_commit_info.is_some() {
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
                            } else {
                                self.load_selected_repo_commit();
                            }
                        }
                        DrawerTab::Stashes => {
                            if self.active_stash_info.is_some() {
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
                            } else {
                                self.load_selected_stash();
                            }
                        }
                    }
                } else {
                    self.scroll_down(1);
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
                if self.visual_mode {
                    self.unstage_visual_selection();
                } else {
                    self.unstage_current_hunk();
                }
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
        if self.show_history {
            let previous = self.selected_history_idx;
            self.selected_history_idx = (self.selected_history_idx + amount).min(self.history_commits.len().saturating_sub(1));
            let rows = self.file_tree_height.saturating_sub(4).max(1);
            if self.selected_history_idx < self.history_scroll { self.history_scroll = self.selected_history_idx; }
            if self.selected_history_idx >= self.history_scroll + rows { self.history_scroll = self.selected_history_idx.saturating_sub(rows - 1); }
            if self.selected_history_idx != previous { self.load_selected_commit_diff(); }
            return;
        }

        let tree_vp = self.file_tree_height.saturating_sub(4).max(1);
        if self.drawer_tab == DrawerTab::Changes || self.active_commit_info.is_some() || self.active_stash_info.is_some() {
            if self.file_view_mode == FileViewMode::Tree {
                if !self.tree_items.is_empty() {
                    self.selected_tree_idx = (self.selected_tree_idx + amount).min(self.tree_items.len() - 1);
                    if self.selected_tree_idx >= self.file_tree_scroll + tree_vp {
                        self.file_tree_scroll = self.selected_tree_idx.saturating_sub(tree_vp - 1);
                    }
                    self.wrap_skip = 0; self.scroll_y = 0;
                    self.selected_row = 0;
                }
            } else if !self.filtered_indices.is_empty() {
                self.selected_filtered_idx = (self.selected_filtered_idx + amount).min(self.filtered_indices.len() - 1);
                if self.selected_filtered_idx >= self.file_tree_scroll + tree_vp {
                    self.file_tree_scroll = self.selected_filtered_idx.saturating_sub(tree_vp - 1);
                }
                self.wrap_skip = 0; self.scroll_y = 0;
                self.selected_row = 0;
            }
            return;
        }

        match self.drawer_tab {
            DrawerTab::Commits => {
                if !self.repo_commits.is_empty() {
                    self.selected_repo_commit_idx = (self.selected_repo_commit_idx + amount).min(self.repo_commits.len() - 1);
                    if self.selected_repo_commit_idx >= self.repo_commit_scroll + tree_vp {
                        self.repo_commit_scroll = self.selected_repo_commit_idx.saturating_sub(tree_vp - 1);
                    }
                }
            }
            DrawerTab::Stashes
                if !self.stashes.is_empty() => {
                    self.selected_stash_idx = (self.selected_stash_idx + amount).min(self.stashes.len() - 1);
                    if self.selected_stash_idx >= self.stash_scroll + tree_vp {
                        self.stash_scroll = self.selected_stash_idx.saturating_sub(tree_vp - 1);
                    }
                }
            _ => {}
        }
    }

    pub fn file_tree_up(&mut self, amount: usize) {
        if self.show_history {
            let previous = self.selected_history_idx;
            self.selected_history_idx = self.selected_history_idx.saturating_sub(amount);
            let rows = self.file_tree_height.saturating_sub(4).max(1);
            if self.selected_history_idx < self.history_scroll { self.history_scroll = self.selected_history_idx; }
            if self.selected_history_idx >= self.history_scroll + rows { self.history_scroll = self.selected_history_idx.saturating_sub(rows - 1); }
            if self.selected_history_idx != previous { self.load_selected_commit_diff(); }
            return;
        }

        if self.drawer_tab == DrawerTab::Changes || self.active_commit_info.is_some() || self.active_stash_info.is_some() {
            if self.file_view_mode == FileViewMode::Tree {
                self.selected_tree_idx = self.selected_tree_idx.saturating_sub(amount);
                if self.selected_tree_idx < self.file_tree_scroll {
                    self.file_tree_scroll = self.selected_tree_idx;
                }
                self.wrap_skip = 0; self.scroll_y = 0;
                self.selected_row = 0;
            } else {
                self.selected_filtered_idx = self.selected_filtered_idx.saturating_sub(amount);
                if self.selected_filtered_idx < self.file_tree_scroll {
                    self.file_tree_scroll = self.selected_filtered_idx;
                }
                self.wrap_skip = 0; self.scroll_y = 0;
                self.selected_row = 0;
            }
            return;
        }

        match self.drawer_tab {
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
            _ => {}
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.wrap_skip = 0;
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
        self.wrap_skip = 0;
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
        self.sync_git_context();
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
                            stats.file_count = files.len();
                            stats.total_additions = files.iter().map(|f| f.stats.additions).sum();
                            stats.total_deletions = files.iter().map(|f| f.stats.deletions).sum();
                            self.repo_stats = stats;
                            self.files = files;
                            self.active_commit_info = Some(commit.clone());
                            self.active_stash_info = None;
                            self.update_filter();
                            self.selected_filtered_idx = 0;
                            self.selected_tree_idx = 0;
                            self.wrap_skip = 0; self.scroll_y = 0;
                            self.selected_row = 0;
                            self.focus = Focus::FileTree;
                            let msg = match self.language {
                                Language::En => format!("Inspecting commit {} ({} files) · Esc/2 to list", &commit.hash[..7.min(commit.hash.len())], self.files.len()),
                                Language::Pt => format!("Inspecionando commit {} ({} arquivos) · Esc/2 para lista", &commit.hash[..7.min(commit.hash.len())], self.files.len()),
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
        self.sync_git_context();
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
                            stats.file_count = files.len();
                            stats.total_additions = files.iter().map(|f| f.stats.additions).sum();
                            stats.total_deletions = files.iter().map(|f| f.stats.deletions).sum();
                            self.repo_stats = stats;
                            self.files = files;
                            self.active_stash_info = Some(stash.clone());
                            self.active_commit_info = None;
                            self.update_filter();
                            self.selected_filtered_idx = 0;
                            self.selected_tree_idx = 0;
                            self.wrap_skip = 0; self.scroll_y = 0;
                            self.selected_row = 0;
                            self.focus = Focus::FileTree;
                            let msg = match self.language {
                                Language::En => format!("Inspecting stash {} ({} files) · Esc/3 to list", stash.selector, self.files.len()),
                                Language::Pt => format!("Inspecionando stash {} ({} arquivos) · Esc/3 para lista", stash.selector, self.files.len()),
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

        if self.show_details_popup {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.details_popup_scroll += 3;
                }
                MouseEventKind::ScrollUp => {
                    self.details_popup_scroll = self.details_popup_scroll.saturating_sub(3);
                }
                _ => {}
            }
            return;
        }

        if self.show_help || self.show_worktrees || self.confirm_action.is_some() { return; }

        let effective_tree_width = if !self.show_drawer {
            0
        } else if self.term_width > 0 && self.term_width < 85 {
            self.file_tree_width.min((self.term_width * 32 / 100).max(18)).min(self.term_width.saturating_sub(25))
        } else if self.term_width > 0 {
            self.file_tree_width.min(self.term_width.saturating_sub(25))
        } else {
            self.file_tree_width
        };

        if self.show_history && mouse.column < effective_tree_width {
            match mouse.kind {
                MouseEventKind::ScrollDown => self.file_tree_down(3),
                MouseEventKind::ScrollUp => self.file_tree_up(3),
                MouseEventKind::Down(MouseButton::Left) if mouse.row >= 4 => {
                    let idx = self.history_scroll + mouse.row.saturating_sub(4) as usize;
                    if idx < self.history_commits.len() {
                        self.selected_history_idx = idx;
                        self.load_selected_commit_diff();
                    }
                }
                _ => {}
            }
            self.focus = Focus::FileTree;
            return;
        }

        if mouse.column >= effective_tree_width &&
            (matches!(mouse.kind, MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight) ||
             (mouse.modifiers.contains(KeyModifiers::SHIFT) && matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown))) {
            let idx = self.horizontal_index();
            self.scroll_x[idx] = if matches!(mouse.kind, MouseEventKind::ScrollLeft | MouseEventKind::ScrollUp) {
                self.scroll_x[idx].saturating_sub(4)
            } else { self.scroll_x[idx].saturating_add(4).min(self.horizontal_limit()) };
            return;
        }

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
                    if mouse.column >= divider_col.saturating_sub(2) && mouse.column <= divider_col + 2 {
                        self.is_dragging_divider = true;
                        return;
                    }


                    if mouse.column < effective_tree_width {
                        self.focus = Focus::FileTree;
                        if mouse.row <= 2 {
                            if self.active_commit_info.is_some() || self.active_stash_info.is_some() { return; }
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

                        if self.active_commit_info.is_some() {
                            let commit_header_h: u16 = if self.viewport_height < 18 { 3 } else { 4 };
                            if mouse.row <= 2 + commit_header_h {
                                self.active_commit_info = None;
                                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                                    self.files = saved_files;
                                    self.repo_stats = saved_stats;
                                    self.update_filter();
                                }
                                self.focus = Focus::FileTree;
                                return;
                            }
                            if mouse.row == 3 + commit_header_h {
                                return;
                            }
                            let item_row = (mouse.row.saturating_sub(4 + commit_header_h)) as usize;
                            let target_idx = self.file_tree_scroll + item_row;
                            if self.file_view_mode == FileViewMode::Tree {
                                if target_idx < self.tree_items.len() {
                                    self.selected_tree_idx = target_idx;
                                    self.wrap_skip = 0; self.scroll_y = 0;
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
                                self.wrap_skip = 0; self.scroll_y = 0;
                                self.selected_row = 0;
                            }
                            return;
                        }

                        if self.active_stash_info.is_some() {
                            let stash_header_h: u16 = if self.viewport_height < 18 { 3 } else { 4 };
                            if mouse.row <= 2 + stash_header_h {
                                self.active_stash_info = None;
                                if let Some((saved_files, saved_stats)) = self.live_snapshot.take() {
                                    self.files = saved_files;
                                    self.repo_stats = saved_stats;
                                    self.update_filter();
                                }
                                self.focus = Focus::FileTree;
                                return;
                            }
                            if mouse.row == 3 + stash_header_h {
                                return;
                            }
                            let item_row = (mouse.row.saturating_sub(4 + stash_header_h)) as usize;
                            let target_idx = self.file_tree_scroll + item_row;
                            if self.file_view_mode == FileViewMode::Tree {
                                if target_idx < self.tree_items.len() {
                                    self.selected_tree_idx = target_idx;
                                    self.wrap_skip = 0; self.scroll_y = 0;
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
                                self.wrap_skip = 0; self.scroll_y = 0;
                                self.selected_row = 0;
                            }
                            return;
                        }

                        if mouse.row == 3 {
                            if self.drawer_tab == DrawerTab::Changes {
                                self.toggle_file_view_mode();
                            }
                            return;
                        }

                        let item_row = (mouse.row.saturating_sub(4)) as usize;
                        match self.drawer_tab {
                            DrawerTab::Changes => {
                                let target_idx = self.file_tree_scroll + item_row;
                                if self.file_view_mode == FileViewMode::Tree {
                                    if target_idx < self.tree_items.len() {
                                        self.selected_tree_idx = target_idx;
                                        self.wrap_skip = 0; self.scroll_y = 0;
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
                                    self.wrap_skip = 0; self.scroll_y = 0;
                                    self.selected_row = 0;
                                }
                            }
                            DrawerTab::Commits => {
                                let target_idx = self.repo_commit_scroll + item_row;
                                if target_idx < self.repo_commits.len() {
                                    if self.selected_repo_commit_idx == target_idx {
                                        self.load_selected_repo_commit();
                                    } else {
                                        self.selected_repo_commit_idx = target_idx;
                                    }
                                }
                            }
                            DrawerTab::Stashes => {
                                let target_idx = self.stash_scroll + item_row;
                                if target_idx < self.stashes.len() {
                                    if self.selected_stash_idx == target_idx {
                                        self.load_selected_stash();
                                    } else {
                                        self.selected_stash_idx = target_idx;
                                    }
                                }
                            }
                        }
                        return;
                    }
                }

                if mouse.column > effective_tree_width && mouse.row >= 1 {
                    if self.drawer_tab == DrawerTab::Commits && self.active_commit_info.is_none() && !self.show_history {
                        self.load_selected_repo_commit();
                        return;
                    }
                    if self.drawer_tab == DrawerTab::Stashes && self.active_stash_info.is_none() && !self.show_history {
                        self.load_selected_stash();
                        return;
                    }

                    self.focus = Focus::DiffView;
                    let diff_inner_x = mouse.column.saturating_sub(effective_tree_width + 1);
                    if diff_inner_x < self.diff_width.saturating_sub(3) / 2 {
                        self.column_side = ColumnSide::Left;
                    } else {
                        self.column_side = ColumnSide::Right;
                    }

                    if mouse.row <= 2 {
                        return;
                    }
                    let line_row = (mouse.row.saturating_sub(3)) as usize;
                    let target_row = self.diff_row_map.get(line_row).copied().unwrap_or(usize::MAX);
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
                if self.show_drawer && (self.is_dragging_divider || (mouse.column >= effective_tree_width.saturating_sub(2) && mouse.column <= effective_tree_width + 2)) {
                    self.is_dragging_divider = true;
                    let max_w = if self.term_width > 0 { self.term_width.saturating_sub(25).max(18) } else { 80 };
                    let min_w = 16.min(max_w);
                    self.file_tree_width = mouse.column.clamp(min_w, max_w);
                }
            }
            MouseEventKind::Up(_) => {
                self.is_dragging_divider = false;
            }
            _ => {}

        }
    }

    fn jump_next_hunk(&mut self) {
        let Some(total) = self.current_file().map(|f| f.hunks.len()) else { return; };
        let next = match self.cursor_hunk() {
            Some(h) => h + 1,
            None => (0..total).find(|&h| self.hunk_start_row(h).is_some_and(|r| r > self.selected_row)).unwrap_or(total),
        };
        match self.hunk_start_row(next).filter(|_| next < total) {
            Some(row) => {
                self.selected_row = row;
                self.scroll_y = row.saturating_sub(2);
                self.set_notification(format!("Jumped to Hunk #{}", next + 1));
            }
            None => self.set_notification("Reached last hunk"),
        }
    }

    fn jump_prev_hunk(&mut self) {
        let Some(total) = self.current_file().map(|f| f.hunks.len()) else { return; };
        let prev = match self.cursor_hunk() {
            Some(h) => h.checked_sub(1),
            None => (0..total).rev().find(|&h| self.hunk_start_row(h).is_some_and(|r| r < self.selected_row)),
        };
        match prev.and_then(|h| self.hunk_start_row(h).map(|row| (h, row))) {
            Some((h, row)) => {
                self.selected_row = row;
                self.scroll_y = row.saturating_sub(2);
                self.set_notification(format!("Jumped to Hunk #{}", h + 1));
            }
            None => self.set_notification("Reached first hunk"),
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

            let target_path = match &file.old_path {
                Some(old) if self.column_side == ColumnSide::Left => old,
                _ => &file.new_path,
            };

            let full_path = repo_dir.join(target_path);
            self.editor_request = Some((full_path, line_no));
        }
    }

    fn copy_current_hunk(&mut self) {
        if let Some(file) = self.current_file() {
            let hunk_opt = self.cursor_hunk().and_then(|idx| file.hunks.get(idx));

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
        if !self.can_act_on_hunk() || !self.require_section(DiffSection::Changes) {
            return;
        }
        if self.current_file().is_some_and(|f| f.stage_status == StageStatus::Untracked) {
            self.stage_current_file();
            return;
        }
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
        self.apply_visual_selection(false);
    }

    fn unstage_visual_selection(&mut self) {
        self.apply_visual_selection(true);
    }

    fn apply_visual_selection(&mut self, unstage: bool) {
        let wanted = if unstage { DiffSection::Staged } else { DiffSection::Changes };
        if !self.can_modify_index() || !self.require_section(wanted) {
            return;
        }
        let repo_root = match &self.mode {
            AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
            _ => return,
        };
        let Some(hunk_idx) = self.cursor_hunk() else { return; };
        let min_r = self.visual_anchor.min(self.selected_row);
        let max_r = self.visual_anchor.max(self.selected_row);
        let mut selected_indices: Vec<usize> = (min_r..=max_r)
            .filter_map(|row| self.row_hunk(row))
            .filter(|(h, _)| *h == hunk_idx)
            .flat_map(|(_, lines)| lines)
            .collect();
        selected_indices.sort_unstable();
        selected_indices.dedup();

        if selected_indices.is_empty() {
            self.set_notification("No changed lines in visual selection");
            return;
        }
        let Some(file) = self.current_file() else { return; };
        let Some(hunk) = file.hunks.get(hunk_idx) else { return; };

        let result = if unstage {
            unstage_partial_hunk(&repo_root, &file.new_path, hunk, &selected_indices)
        } else {
            stage_partial_hunk(&repo_root, &file.new_path, hunk, &selected_indices)
        };
        let verb = if unstage { "Unstaged" } else { "Staged" };
        match result {
            Ok(_) => {
                self.visual_mode = false;
                self.set_notification(format!("✓ {} {} selected lines", verb, selected_indices.len()));
                self.reload_diffs();
            }
            Err(e) => {
                self.set_notification(format!("{} error: {}", verb, e));
            }
        }
    }

    fn unstage_current_hunk(&mut self) {
        if !self.can_act_on_hunk() || !self.require_section(DiffSection::Staged) {
            return;
        }
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
        if !self.can_act_on_hunk() {
            return;
        }
        if self.current_section() == Some(DiffSection::Staged) {
            self.set_notification(match self.language {
                Language::En => "Unstage it first (u), then discard from Changes",
                Language::Pt => "Faça unstage antes (u) e descarte em Mudanças",
            });
            return;
        }
        if let Some(file) = self.current_file() {
            if let Some(hunk_idx) = self.cursor_hunk() {
                let msg = format!("Discard Hunk #{} in {}? Changes cannot be undone.", hunk_idx + 1, file.display_path());
                self.confirm_action = Some((ConfirmAction::DiscardHunk(hunk_idx), msg));
            } else {
                self.set_notification("No hunk under cursor to discard");
            }
        }
    }

    fn stage_current_file(&mut self) {
        if !self.can_modify_index() {
            return;
        }
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
        if !self.can_modify_index() {
            return;
        }
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
        if !self.can_modify_index() {
            return;
        }
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
        let hunk_idx = self.cursor_hunk()?;
        let file = self.current_file()?;
        let hunk = file.hunks.get(hunk_idx)?.clone();
        Some((repo_root, file.new_path.clone(), hunk))
    }

    pub fn open_file_history(&mut self) {
        if let AppMode::Git { git_provider, .. } = &self.mode {
            if let Some(file) = self.get_underlying_file() {
                let path = file.new_path.clone();
                match git_provider.get_file_history(&path, 50) {
                    Ok(commits) if !commits.is_empty() => {
                        self.history_return_focus = self.focus;
                        self.history_file_path = Some(path);
                        self.history_commits = commits;
                        self.selected_history_idx = 0;
                        self.history_scroll = 0;
                        self.show_history = true;
                        self.show_drawer = true;
                        self.focus = Focus::FileTree;
                        self.load_selected_commit_diff();
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
        let Some(commit) = self.history_commits.get(self.selected_history_idx) else { return; };
        let hash = commit.hash.clone();
        if self.active_commit_view.as_ref().map(|(active, _)| active == &hash).unwrap_or(false) { return; }
        let path = self.history_file_path.clone().or_else(|| self.get_underlying_file().map(|f| f.new_path.clone()));
        self.active_commit_view = None;
        self.sync_git_context();
        self.scroll_x = [0, 0];
        self.wrap_skip = 0;
        self.scroll_y = 0;
        self.selected_row = 0;
        if let (AppMode::Git { git_provider, .. }, Some(path)) = (&self.mode, path) {
            match git_provider.load_commit_diff_for_file(&hash, &path) {
                Ok(Some(diff)) => self.active_commit_view = Some((hash, diff)),
                Ok(None) => self.set_notification(match self.language {
                    Language::En => "No text diff for this file in the selected commit",
                    Language::Pt => "Sem diff de texto para este arquivo no commit selecionado",
                }),
                Err(err) => self.set_notification(format!("Error loading history diff: {}", err)),
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();
        self.term_width = size.width;
        self.term_height = size.height;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(5),    // Main
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
            self.watcher_state,
            self.spinner_idx,
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
        self.diff_width = diff_area.width;
        let ruler_area = main_chunks[2];

        self.viewport_height = diff_area.height as usize;
        self.file_tree_height = file_tree_area.height as usize;

        let selected_file_idx = if self.file_view_mode == FileViewMode::Tree {
            self.selected_tree_idx
        } else {
            self.selected_filtered_idx
        };

        if effective_tree_width > 0 {
            render_drawer(

                frame,
                file_tree_area,
                if self.show_history { DrawerTab::Commits } else { self.drawer_tab },
                &self.tree_items,
                selected_file_idx,
                self.file_tree_scroll,
                if self.show_history { &self.history_commits } else { &self.repo_commits },
                if self.show_history { self.selected_history_idx } else { self.selected_repo_commit_idx },
                if self.show_history { self.history_scroll } else { self.repo_commit_scroll },
                &self.stashes,
                self.selected_stash_idx,
                self.stash_scroll,
                if self.show_history { None } else { self.active_commit_info.as_ref() },
                if self.show_history { None } else { self.active_stash_info.as_ref() },
                self.focus == Focus::FileTree,
                self.filter_mode,
                &self.filter_query,
                self.file_view_mode,
                self.language,
                &self.theme,
                if self.show_history { Some(match self.language { Language::En => "File history · o PR", Language::Pt => "Histórico do arquivo · o PR" }) } else { None },
            );
        }

        let is_commits_overview = self.drawer_tab == DrawerTab::Commits && self.active_commit_info.is_none();
        let is_stashes_overview = self.drawer_tab == DrawerTab::Stashes && self.active_stash_info.is_none();

        if is_commits_overview && !self.show_history {
            if let Some(commit) = self.repo_commits.get(self.selected_repo_commit_idx) {
                crate::ui::components::commit_view::render_commit_overview(
                    frame,
                    diff_area,
                    commit,
                    self.language,
                    &self.theme,
                );
            }
        } else if is_stashes_overview && !self.show_history {
            if let Some(stash) = self.stashes.get(self.selected_stash_idx) {
                crate::ui::components::commit_view::render_stash_overview(
                    frame,
                    diff_area,
                    stash,
                    self.language,
                    &self.theme,
                );
            }
        } else {
            if self.wrap_lines && self.selected_row >= self.scroll_y {
                use unicode_width::UnicodeWidthStr;
                let width = if self.is_unified { diff_area.width.saturating_sub(18) } else { (diff_area.width.saturating_sub(3) / 2).saturating_sub(10) }.max(1) as usize;
                let height = self.current_file().map(|f| {
                    if self.is_unified {
                        f.hunks.iter().flat_map(|h| std::iter::once(1usize).chain(h.lines.iter().map(|l| l.content.width().max(1).div_ceil(width))))
                            .skip(self.scroll_y).take(self.selected_row - self.scroll_y + 1).sum::<usize>()
                    } else {
                        f.aligned_rows.iter().skip(self.scroll_y).take(self.selected_row - self.scroll_y + 1).map(|r| {
                            let len = r.left.as_ref().map(|l| l.content.width()).unwrap_or(0).max(r.right.as_ref().map(|l| l.content.width()).unwrap_or(0));
                            len.max(1).div_ceil(width)
                        }).sum::<usize>()
                    }
                }).unwrap_or(0);
                if height > diff_area.height.saturating_sub(3) as usize { self.scroll_y = self.selected_row; }
            }
            let mut row_map = Vec::new();
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
                    self.scroll_x[1],
                    self.wrap_lines,
                    self.wrap_skip,
                    &mut row_map,
                    self.selected_row,
                    visual_range,
                    self.focus == Focus::DiffView,
                    syntax_enabled,
                    self.full_context,
                    &self.theme,
                );
            } else {
                render_side_by_side(
                    frame,
                    diff_area,
                    cur_file,
                    self.scroll_y,
                    self.scroll_x,
                    self.wrap_lines,
                    self.wrap_skip,
                    &mut row_map,
                    self.selected_row,
                    visual_range,
                    self.column_side,
                    self.column_labels(),
                    self.focus == Focus::DiffView,
                    syntax_enabled,
                    self.full_context,
                    &self.theme,
                );
            }

            if self.config.ui.overview_ruler {
                let vp_height = diff_area.height as usize;
                render_ruler(frame, ruler_area, cur_file, self.scroll_y, vp_height, &self.theme);
            }
            self.diff_row_map = row_map;
        }

        // 4. Overlays
        if self.show_help {
            render_help_popup(frame, size, self.language, &self.theme);
        } else if self.show_details_popup {
            let cur_file = self.current_file();
            let content = if self.show_history && !self.history_commits.is_empty() {
                DetailsContent::Commit(&self.history_commits[self.selected_history_idx], cur_file.map(std::slice::from_ref).unwrap_or(&[]))
            } else if let Some(commit) = &self.active_commit_info {
                DetailsContent::Commit(commit, &self.files)
            } else if self.drawer_tab == DrawerTab::Commits && !self.repo_commits.is_empty() {
                DetailsContent::Commit(&self.repo_commits[self.selected_repo_commit_idx], &[])
            } else if let Some(stash) = &self.active_stash_info {
                DetailsContent::Stash(stash, &self.files)
            } else if self.drawer_tab == DrawerTab::Stashes && !self.stashes.is_empty() {
                DetailsContent::Stash(&self.stashes[self.selected_stash_idx], &[])
            } else if let Some(f) = cur_file {
                DetailsContent::File(f)
            } else {
                DetailsContent::Commit(&CommitEntry {
                    hash: "N/A".into(),
                    author: "N/A".into(),
                    date: "N/A".into(),
                    message: "No commit or file details available".into(),
                }, &[])
            };
            render_details_popup(frame, size, content, self.details_popup_scroll, self.language, &self.theme);
        } else if self.show_worktrees {
            render_worktree_popup(
                frame,
                size,
                &self.worktrees,
                self.selected_worktree_idx,
                self.worktree_scroll,
                self.worktree_creation.as_ref(),
                self.language,
                &self.theme,
            );
        } else if let Some((_, msg)) = &self.confirm_action {
            render_confirm_popup(frame, size, msg, self.language, &self.theme);
        }

        // 5. Drawer Line Overlay (extends row over right border if name exceeds drawer width)
        if self.show_drawer && !self.show_help && !self.show_worktrees && !self.show_history && !self.show_details_popup && self.confirm_action.is_none() {
            render_drawer_line_overlay(
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
                self.active_commit_info.as_ref(),
                self.active_stash_info.as_ref(),
                &self.theme,
            );
        }

        // Floating notification styled with the active theme
        if let Some((msg, _)) = &self.notification {
            render_toast(frame, size, msg, &self.theme);
        }
    }
}


fn diff_text_lines(files: &[&FileDiff]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        let path = f.display_path();
        for line in f.hunks.iter().flat_map(|h| &h.lines) {
            let line_no = line.new_line_no.or(line.old_line_no).unwrap_or(0);
            let prefix = match line.kind {
                DiffKind::Addition => "+",
                DiffKind::Deletion => "-",
                _ => " ",
            };
            let section = if f.section == DiffSection::Staged { "\tstaged" } else { "" };
            out.push(format!("{}:{}\t{} {}{}", path, line_no, prefix, line.content.trim_end_matches(['\r', '\n']), section));
        }
    }
    out
}
