use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::{Config, UpdateChannel};
use crate::core::engine::DiffEngine;
use crate::core::models::{
    CommitEntry, DiffKind, DiffSection, DrawerTab, FileDiff, Language, RepoStats, StageStatus,
    StashEntry, WatcherScanState, WorktreeEntry,
};

use crate::git::actions::{
    discard_file, discard_hunk, stage_file, stage_hunk, stage_partial_hunk, stage_paths,
    unstage_file, unstage_hunk, unstage_partial_hunk, unstage_paths,
};
use crate::git::provider::GitProvider;
use crate::integration::clipboard::copy_hunk_as_markdown;
use crate::integration::github::{github_repo_url, open_commit_pr};
use crate::ui::components::branch_popup::{render_branch_popup, BranchSelectorState};
use crate::ui::components::details_popup::{render_details_popup, DetailsContent};
use crate::ui::components::file_tree::{
    build_tree_items, changes_header_rows, files_pending_in, render_drawer,
    render_drawer_line_overlay, resolve_stage_target, FileViewMode, StageScope, StageTarget,
    TreeItem,
};
use crate::ui::components::header::render_header;
use crate::ui::components::help_popup::{render_confirm_popup, render_help_popup};
use crate::ui::components::picker::{
    picker_key_action, render_picker_popup, PickerAction, PickerState,
};
use crate::ui::components::ruler::render_ruler;
use crate::ui::components::settings_popup::{render_settings_popup, SettingItem, SETTING_ITEMS};
use crate::ui::components::side_by_side::{render_side_by_side, ColumnSide};
use crate::ui::components::style::centered_rect;
use crate::ui::components::toast::{render_toast, TOAST_DURATION};
use crate::ui::components::unified::render_unified;
use crate::ui::components::update_popup::render_update_popup;
use crate::ui::components::worktree_popup::{render_worktree_popup, WorktreeCreationState};
use crate::ui::scroll::{clamp_cursor, max_list_offset, scroll_offset, SCROLLOFF, WHEEL_STEP};
use crate::ui::selection::{extract_text, is_selected, pane_lines, MouseSelection, TextMap};
use crate::ui::theme::Theme;
use crate::update::UpdateEvent;

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
    /// Diffs the candidates come from (the built-in picker's preview uses them).
    pub files: Vec<FileDiff>,
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
    /// Outcome of the startup update check, shown as a popup until dismissed.
    pub update_popup: Option<UpdateEvent>,
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
    pub worktree_filter: String,
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
    /// Candidates + typed query carried into fzf when Ctrl+G leaves the built-in picker.
    pub fzf_carry: Option<(FzfQuery, String)>,
    // Built-in picker used for fzf requests when fzf is not installed
    pub picker: Option<PickerState>,
    /// Preview pane on/off, kept between picker openings.
    pub picker_preview: bool,

    // Terminal geometry for responsive drag resizing
    pub term_width: u16,
    pub term_height: u16,

    // Watcher scanning state and animation
    pub watcher_state: WatcherScanState,
    pub spinner_idx: usize,

    // Settings popup state
    pub show_settings: bool,
    pub settings_selected_idx: usize,

    // Branch comparison selector popup state
    pub show_branch_selector: bool,
    pub branch_selector: Option<BranchSelectorState>,

    // Mouse text selection over the diff panes
    pub text_map: TextMap,
    pub mouse_selection: Option<MouseSelection>,
    selection_key: Option<SelectionKey>,
    pending_click: Option<crossterm::event::MouseEvent>,
}

/// Identifies the rendered diff a mouse selection was made on; line ids are only
/// meaningful while it stays the same.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectionKey {
    path: PathBuf,
    section: DiffSection,
    is_unified: bool,
    full_context: bool,
    rows: usize,
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
                let cwd =
                    std::env::current_dir().unwrap_or_else(|_| git_provider.repo_root.clone());
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

        let wrap_lines = config.ui.wrap_lines;
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
            update_popup: None,
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
            wrap_lines,
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
            worktree_filter: String::new(),
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
            fzf_carry: None,
            picker: None,
            picker_preview: true,
            term_width: 80,
            term_height: 25,
            watcher_state: if watch_mode {
                WatcherScanState::Scanning { scanned_dirs: 0 }
            } else {
                WatcherScanState::Idle
            },
            spinner_idx: 0,
            show_settings: false,
            settings_selected_idx: 0,
            show_branch_selector: false,
            branch_selector: None,
            text_map: TextMap::default(),
            mouse_selection: None,
            selection_key: None,
            pending_click: None,
        };

        app.reload_diffs_internal(false)?;
        if app.files.is_empty() {
            let msg = match app.language {
                Language::En => "No differences found.",
                Language::Pt => "Nenhuma alteração encontrada.",
            };
            app.set_notification(msg);
        } else if app.config.update.channel == UpdateChannel::Beta {
            let msg = match app.language {
                Language::En => "Beta channel active (latest main builds) · Config [C]",
                Language::Pt => "Canal Beta ativo (builds da main) · Configurações [C]",
            };
            app.set_notification(msg);
        } else if !app.config.update.auto_update {
            let msg = match app.language {
                Language::En => "Auto-update disabled · Config [C]",
                Language::Pt => "Auto-update desativado · Configurações [C]",
            };
            app.set_notification(msg);
        }

        if history_mode {
            app.open_file_history();
        }

        Ok(app)
    }

    pub fn open_settings(&mut self) {
        self.show_settings = true;
    }

    pub fn close_settings(&mut self) {
        self.show_settings = false;
        self.save_settings();
    }

    pub fn toggle_settings(&mut self) {
        if self.show_settings {
            self.close_settings();
        } else {
            self.open_settings();
        }
    }

    pub fn open_branch_selector(&mut self) {
        if let AppMode::Git {
            target_ref,
            git_provider,
        } = &self.mode
        {
            let branches = git_provider.get_branches();
            let current_target = target_ref.clone();
            let mut state = BranchSelectorState::new(branches, current_target);
            state.select_current_target(self.language);
            self.branch_selector = Some(state);
            self.show_branch_selector = true;
        } else {
            let msg = match self.language {
                Language::En => "Branch comparison is only available in git repositories",
                Language::Pt => "Comparação de branch só está disponível em repositórios git",
            };
            self.set_notification(msg);
        }
    }

    pub fn close_branch_selector(&mut self) {
        self.show_branch_selector = false;
        self.branch_selector = None;
    }

    pub fn apply_selected_branch(&mut self) {
        let chosen = if let Some(state) = &self.branch_selector {
            state.selected_branch(self.language)
        } else {
            None
        };
        self.set_comparison_branch(chosen);
        self.close_branch_selector();
    }

    pub fn set_comparison_branch(&mut self, branch: Option<String>) {
        if let AppMode::Git { target_ref, .. } = &mut self.mode {
            *target_ref = branch.clone();
            self.live_snapshot = None;
            self.active_commit_info = None;
            self.active_stash_info = None;
            self.active_commit_view = None;
            self.drawer_tab = DrawerTab::Changes;
            self.selected_filtered_idx = 0;
            self.selected_tree_idx = 0;
            self.scroll_y = 0;
            self.selected_row = 0;
            self.reload_diffs();

            let notif = match branch {
                Some(ref b) => match self.language {
                    Language::En => format!("Comparing worktree against '{}'", b),
                    Language::Pt => format!("Comparando worktree com '{}'", b),
                },
                None => match self.language {
                    Language::En => "Restored default worktree diff (HEAD)".to_string(),
                    Language::Pt => "Restaurado diff padrão do worktree (HEAD)".to_string(),
                },
            };
            self.set_notification(notif);
        }
    }

    pub fn current_comparison_branch(&self) -> Option<&str> {
        match &self.mode {
            AppMode::Git { target_ref, .. } => target_ref.as_deref(),
            _ => None,
        }
    }

    /// Save the active view, including a CLI override, on normal shutdown.
    pub fn save_view_preferences_to_path(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        self.config.ui.wrap_lines = self.wrap_lines;
        self.config.ui.default_view = if self.is_unified {
            "unified"
        } else {
            "side-by-side"
        }
        .to_string();
        self.config.save_to_path(path)
    }

    pub fn save_view_preferences(&mut self) -> anyhow::Result<()> {
        let path = Config::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.save_view_preferences_to_path(&path)
    }

    pub fn save_settings(&mut self) {
        match self.config.save() {
            Ok(path) => {
                let msg = match self.language {
                    Language::En => format!("Settings saved to {}", path.display()),
                    Language::Pt => format!("Configurações salvas em {}", path.display()),
                };
                self.set_notification(msg);
            }
            Err(err) => {
                let msg = match self.language {
                    Language::En => format!("Could not save settings: {}", err),
                    Language::Pt => format!("Erro ao salvar configurações: {}", err),
                };
                self.set_notification(msg);
            }
        }
    }

    pub fn toggle_update_channel(&mut self) {
        self.config.update.channel = match self.config.update.channel {
            UpdateChannel::Stable => UpdateChannel::Beta,
            UpdateChannel::Beta => UpdateChannel::Nightly,
            UpdateChannel::Nightly => UpdateChannel::Stable,
        };
        let _ = self.config.save();
        let msg = match self.config.update.channel {
            UpdateChannel::Stable => match self.language {
                Language::En => "Channel: Stable (official releases) · Config [C]",
                Language::Pt => "Canal: Stable (versões estáveis) · Configurações [C]",
            },
            UpdateChannel::Beta => match self.language {
                Language::En => "Channel: Beta (pre-releases & RC) · Config [C]",
                Language::Pt => "Canal: Beta (pré-lançamentos e RC) · Configurações [C]",
            },
            UpdateChannel::Nightly => match self.language {
                Language::En => "Channel: Nightly (latest build from main) · Config [C]",
                Language::Pt => "Canal: Nightly (última build da branch main) · Configurações [C]",
            },
        };
        self.set_notification(msg);
    }

    pub fn cycle_setting(&mut self, forward: bool) {
        let Some(&item) = SETTING_ITEMS.get(self.settings_selected_idx) else {
            return;
        };
        match item {
            SettingItem::AutoUpdate => {
                self.config.update.auto_update = !self.config.update.auto_update;
                let msg = if self.config.update.auto_update {
                    match self.language {
                        Language::En => "Auto-Update: Enabled · Config [C]",
                        Language::Pt => "Auto-Update: Ativado · Configurações [C]",
                    }
                } else {
                    match self.language {
                        Language::En => "Auto-Update: Disabled (opt-out) · Config [C]",
                        Language::Pt => "Auto-Update: Desativado (opt-out) · Configurações [C]",
                    }
                };
                self.set_notification(msg);
            }
            SettingItem::UpdateChannel => {
                self.config.update.channel = if forward {
                    match self.config.update.channel {
                        UpdateChannel::Stable => UpdateChannel::Beta,
                        UpdateChannel::Beta => UpdateChannel::Nightly,
                        UpdateChannel::Nightly => UpdateChannel::Stable,
                    }
                } else {
                    match self.config.update.channel {
                        UpdateChannel::Stable => UpdateChannel::Nightly,
                        UpdateChannel::Nightly => UpdateChannel::Beta,
                        UpdateChannel::Beta => UpdateChannel::Stable,
                    }
                };
                let msg = match self.config.update.channel {
                    UpdateChannel::Stable => match self.language {
                        Language::En => "Channel: Stable (official releases) · Config [C]",
                        Language::Pt => "Canal: Stable (versões estáveis) · Configurações [C]",
                    },
                    UpdateChannel::Beta => match self.language {
                        Language::En => "Channel: Beta (pre-releases & RC) · Config [C]",
                        Language::Pt => "Canal: Beta (pré-lançamentos e RC) · Configurações [C]",
                    },
                    UpdateChannel::Nightly => match self.language {
                        Language::En => "Channel: Nightly (latest build from main) · Config [C]",
                        Language::Pt => {
                            "Canal: Nightly (última build da branch main) · Configurações [C]"
                        }
                    },
                };
                self.set_notification(msg);
            }
            SettingItem::Theme => {
                let themes = [
                    "auto",
                    "terminal",
                    "vscode-dark",
                    "tokyonight",
                    "catppuccin",
                    "gruvbox",
                ];
                let cur = themes
                    .iter()
                    .position(|&t| t == self.config.ui.theme)
                    .unwrap_or(0);
                let next = if forward {
                    (cur + 1) % themes.len()
                } else {
                    (cur + themes.len() - 1) % themes.len()
                };
                self.config.ui.theme = themes[next].to_string();
                self.theme = Theme::from_name(&self.config.ui.theme);
                let msg = match self.language {
                    Language::En => format!("Theme: {}", self.config.ui.theme),
                    Language::Pt => format!("Tema: {}", self.config.ui.theme),
                };
                self.set_notification(msg);
            }
            SettingItem::DefaultView => {
                self.config.ui.default_view = if self.config.ui.default_view == "unified" {
                    "side-by-side".to_string()
                } else {
                    "unified".to_string()
                };
                let msg = match self.language {
                    Language::En => format!("Default view: {}", self.config.ui.default_view),
                    Language::Pt => format!("Visualização padrão: {}", self.config.ui.default_view),
                };
                self.set_notification(msg);
            }
            SettingItem::LineNumbers => {
                self.config.ui.show_line_numbers = !self.config.ui.show_line_numbers;
                let msg = match (self.config.ui.show_line_numbers, self.language) {
                    (true, Language::En) => "Line numbers: Enabled",
                    (false, Language::En) => "Line numbers: Disabled",
                    (true, Language::Pt) => "Números de linha: Ativado",
                    (false, Language::Pt) => "Números de linha: Desativado",
                };
                self.set_notification(msg);
            }
            SettingItem::OverviewRuler => {
                self.config.ui.overview_ruler = !self.config.ui.overview_ruler;
                let msg = match (self.config.ui.overview_ruler, self.language) {
                    (true, Language::En) => "Overview ruler: Enabled",
                    (false, Language::En) => "Overview ruler: Disabled",
                    (true, Language::Pt) => "Régua lateral: Ativada",
                    (false, Language::Pt) => "Régua lateral: Desativada",
                };
                self.set_notification(msg);
            }
            SettingItem::TabWidth => {
                self.config.ui.tab_width = match self.config.ui.tab_width {
                    2 => 4,
                    4 => 8,
                    _ => 2,
                };
                let msg = match self.language {
                    Language::En => format!("Tab width: {} spaces", self.config.ui.tab_width),
                    Language::Pt => format!("Largura do tab: {} espaços", self.config.ui.tab_width),
                };
                self.set_notification(msg);
            }
            SettingItem::DiffAlgorithm => {
                self.config.diff.algorithm = if self.config.diff.algorithm == "patience" {
                    "myers".to_string()
                } else {
                    "patience".to_string()
                };
                let _ = self.reload_diffs_internal(false);
                let msg = match self.language {
                    Language::En => format!("Diff algorithm: {}", self.config.diff.algorithm),
                    Language::Pt => format!("Algoritmo de diff: {}", self.config.diff.algorithm),
                };
                self.set_notification(msg);
            }
            SettingItem::IgnoreWhitespace => {
                self.config.diff.ignore_whitespace = !self.config.diff.ignore_whitespace;
                self.ignore_whitespace = self.config.diff.ignore_whitespace;
                let _ = self.reload_diffs_internal(false);
                let msg = match (self.config.diff.ignore_whitespace, self.language) {
                    (true, Language::En) => "Ignore whitespace: Enabled",
                    (false, Language::En) => "Ignore whitespace: Disabled",
                    (true, Language::Pt) => "Ignorar espaços: Ativado",
                    (false, Language::Pt) => "Ignorar espaços: Desativado",
                };
                self.set_notification(msg);
            }
            SettingItem::SearchEngine => {
                use crate::config::SearchEngine;
                let fzf_available = crate::integration::fzf::is_fzf_available();
                self.config.search.engine = if forward {
                    self.config.search.engine.next(fzf_available)
                } else {
                    match self.config.search.engine {
                        SearchEngine::Auto => SearchEngine::Builtin,
                        SearchEngine::Builtin if fzf_available => SearchEngine::Fzf,
                        SearchEngine::Builtin | SearchEngine::Fzf => SearchEngine::Auto,
                    }
                };
                let msg =
                    search_engine_message(self.config.search.engine, fzf_available, self.language);
                self.set_notification(msg);
            }
            SettingItem::WatcherEnabled => {
                self.config.watcher.enabled = !self.config.watcher.enabled;
                let msg = match (self.config.watcher.enabled, self.language) {
                    (true, Language::En) => "Auto watch mode: Enabled",
                    (false, Language::En) => "Auto watch mode: Disabled",
                    (true, Language::Pt) => "Modo watch automático: Ativado",
                    (false, Language::Pt) => "Modo watch automático: Desativado",
                };
                self.set_notification(msg);
            }
        }
        let _ = self.config.save();
    }

    pub fn is_watcher_scanning(&self) -> bool {
        matches!(self.watcher_state, WatcherScanState::Scanning { .. })
    }

    pub fn update_watcher_progress(&mut self, scanned: usize, total: Option<usize>) {
        if let Some(total_dirs) = total {
            self.watcher_state = WatcherScanState::Ready { total_dirs };
        } else {
            self.watcher_state = WatcherScanState::Scanning {
                scanned_dirs: scanned,
            };
        }
    }

    pub fn current_file(&self) -> Option<&FileDiff> {
        if let Some((_, commit_diff)) = &self.active_commit_view {
            return Some(commit_diff);
        }
        if self.show_history {
            return None;
        }
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
        if self.show_history
            || self.active_commit_info.is_some()
            || self.active_stash_info.is_some()
        {
            return;
        }
        self.show_drawer = true;
        if (self.active_commit_info.is_some()
            || self.active_stash_info.is_some()
            || self.active_commit_view.is_some())
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
            let remaining = self
                .diff_row_map
                .iter()
                .filter(|&&idx| idx == self.scroll_y)
                .count();
            if remaining > amount {
                self.wrap_skip += amount;
                return;
            }
            self.wrap_skip = 0;
        }
        let total = self.total_diff_rows();
        if total > 0 {
            self.scroll_y = scroll_offset(self.scroll_y, amount as isize, total - 1);
        }
        self.keep_diff_cursor_in_view();
    }

    pub fn scroll_viewport_up(&mut self, amount: usize) {
        if self.wrap_lines && self.wrap_skip > 0 {
            self.wrap_skip = self.wrap_skip.saturating_sub(amount);
            return;
        }
        self.scroll_y = self.scroll_y.saturating_sub(amount);
        self.keep_diff_cursor_in_view();
    }

    /// Logical diff rows fully visible from `scroll_y` (wrapped rows count by height).
    fn visible_diff_rows(&self) -> usize {
        let vp = self.viewport_height.saturating_sub(3).max(1);
        let Some(file) = self.current_file().filter(|_| self.wrap_lines) else {
            return vp;
        };
        let mut used = 0;
        let mut count = 0;
        for height in wrapped_row_heights(
            file,
            self.is_unified,
            wrap_width(self.is_unified, self.diff_width),
        )
        .skip(self.scroll_y)
        {
            if count > 0 && used + height > vp {
                break;
            }
            used += height;
            count += 1;
        }
        count.max(1)
    }

    /// Drags the diff cursor along when the view scrolled past it.
    fn keep_diff_cursor_in_view(&mut self) {
        let total = self.total_diff_rows();
        let visible = self.visible_diff_rows();
        self.selected_row =
            clamp_cursor(self.selected_row, self.scroll_y, visible, total, SCROLLOFF);
    }

    /// Mouse wheel over the diff: scrolls the view, not the cursor.
    pub fn wheel_diff(&mut self, down: bool) {
        if down {
            self.scroll_viewport_down(WHEEL_STEP);
        } else {
            self.scroll_viewport_up(WHEEL_STEP);
        }
    }

    /// Mouse wheel over the drawer (files / commits / stashes / file history):
    /// scrolls the list and only moves the selection when it would leave the view.
    pub fn wheel_drawer(&mut self, down: bool) {
        let delta = if down {
            WHEEL_STEP as isize
        } else {
            -(WHEEL_STEP as isize)
        };
        let base_rows = self.file_tree_height.saturating_sub(4).max(1);
        let inspecting = self.active_commit_info.is_some() || self.active_stash_info.is_some();
        let lists_files = self.drawer_tab == DrawerTab::Changes || inspecting;
        let (offset, cursor, total, visible) = if self.show_history {
            (
                &mut self.history_scroll,
                &mut self.selected_history_idx,
                self.history_commits.len(),
                base_rows,
            )
        } else if lists_files {
            let header_rows = if !inspecting {
                0
            } else if self.viewport_height < 18 {
                3
            } else {
                4
            };
            let visible = base_rows.saturating_sub(header_rows).max(1);
            if self.file_view_mode == FileViewMode::Tree {
                let total = self.tree_items.len();
                (
                    &mut self.file_tree_scroll,
                    &mut self.selected_tree_idx,
                    total,
                    visible,
                )
            } else {
                let total = self.filtered_indices.len();
                (
                    &mut self.file_tree_scroll,
                    &mut self.selected_filtered_idx,
                    total,
                    visible,
                )
            }
        } else if self.drawer_tab == DrawerTab::Commits {
            (
                &mut self.repo_commit_scroll,
                &mut self.selected_repo_commit_idx,
                self.repo_commits.len(),
                base_rows,
            )
        } else {
            (
                &mut self.stash_scroll,
                &mut self.selected_stash_idx,
                self.stashes.len(),
                base_rows,
            )
        };
        *offset = scroll_offset(*offset, delta, max_list_offset(total, visible));
        let previous = *cursor;
        *cursor = clamp_cursor(*cursor, *offset, visible, total, SCROLLOFF);
        if *cursor == previous {
            return;
        }
        if self.show_history {
            self.load_selected_commit_diff();
        } else if lists_files {
            self.wrap_skip = 0;
            self.scroll_y = 0;
            self.selected_row = 0;
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
                || (f
                    .old_path
                    .as_ref()
                    .map(|p| p.to_string_lossy() == clean_path)
                    .unwrap_or(false))
        };
        let found_idx = self
            .files
            .iter()
            .position(|f| matches(f) && f.section == section)
            .or_else(|| self.files.iter().position(matches));

        if let Some(idx) = found_idx {
            if self.file_view_mode == FileViewMode::Tree {
                if let Some(tree_idx) = self
                    .tree_items
                    .iter()
                    .position(|t| t.file_index == Some(idx))
                {
                    self.selected_tree_idx = tree_idx;
                }
            } else if let Some(filt_idx) = self.filtered_indices.iter().position(|&i| i == idx) {
                self.selected_filtered_idx = filt_idx;
            }
            self.selected_row = 0;
            self.wrap_skip = 0;
            self.scroll_y = 0;
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
                        if line.new_line_no == Some(target_line_no)
                            || line.old_line_no == Some(target_line_no)
                        {
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
                let path = self
                    .current_file()
                    .map(|f| f.display_path())
                    .unwrap_or_default();
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
                (None, Some(stash)) => (
                    format!("stash {}", stash.selector),
                    format!("stash {}", stash.selector),
                ),
                (None, None) => (
                    "working tree changes".to_string(),
                    "mudanças da working tree".to_string(),
                ),
            },
        };
        match self.language {
            Language::En => en,
            Language::Pt => pt,
        }
    }

    fn resolve_search_source(&self, request: FzfRequest) -> SearchSource {
        let in_diff = self.focus == Focus::DiffView
            || (self.show_history && self.active_commit_view.is_some());
        if request == FzfRequest::Text && in_diff && self.current_file().is_some() {
            return SearchSource::CurrentFile;
        }
        if self.active_commit_info.is_some()
            || self.active_stash_info.is_some()
            || self.show_history
        {
            return SearchSource::Loaded;
        }
        match self.drawer_tab {
            DrawerTab::Commits if self.selected_repo_commit_idx < self.repo_commits.len() => {
                SearchSource::RepoCommit(self.selected_repo_commit_idx)
            }
            DrawerTab::Stashes if self.selected_stash_idx < self.stashes.len() => {
                SearchSource::Stash(self.selected_stash_idx)
            }
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
            (SearchSource::RepoCommit(idx), AppMode::Git { git_provider, .. }) => {
                git_provider.load_commit_full_diff(&self.repo_commits[idx].hash)
            }
            (SearchSource::Stash(idx), AppMode::Git { git_provider, .. }) => {
                git_provider.load_stash_diff(&self.stashes[idx].selector)
            }
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
                files
                    .iter()
                    .map(|f| f.display_path())
                    .filter(|p| seen.insert(p.clone()))
                    .collect()
            }
            FzfRequest::Text => diff_text_lines(&files),
        };
        let label = self.search_label(source);
        let header = match (request, self.language) {
            (FzfRequest::Files, Language::En) => format!("Files in {} (Esc to cancel)", label),
            (FzfRequest::Files, Language::Pt) => {
                format!("Arquivos em {} (Esc para cancelar)", label)
            }
            (FzfRequest::Text, Language::En) => format!("Search text in {} (Esc to cancel)", label),
            (FzfRequest::Text, Language::Pt) => {
                format!("Buscar texto em {} (Esc para cancelar)", label)
            }
        };
        let files = files.into_iter().cloned().collect();
        FzfQuery {
            items,
            header,
            files,
        }
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

    /// Opens the built-in picker for `request` (used when fzf is not installed).
    /// Opens the built-in picker for `request`; `fzf_missing` says fzf was wanted
    /// (search engine `auto` / `fzf`) but is not installed.
    pub fn open_builtin_picker(&mut self, request: FzfRequest, fzf_missing: bool) {
        let query = self.prepare_fzf(request);
        if query.items.is_empty() {
            return;
        }
        self.open_picker(request, query, "");
        if fzf_missing {
            let msg = match self.language {
                Language::En => "fzf not found on PATH · using the built-in picker",
                Language::Pt => "fzf não encontrado no PATH · usando o picker interno",
            };
            self.set_notification(msg);
        }
    }

    fn open_picker(&mut self, request: FzfRequest, query: FzfQuery, text: &str) {
        let mut picker = PickerState::new(request, query.items, query.header);
        picker.files = query.files;
        picker.show_preview = self.picker_preview;
        if !text.is_empty() {
            picker.query = text.to_string();
            picker.refilter();
        }
        self.picker = Some(picker);
    }

    /// Persists `engine` (as the Ctrl+G shortcut does) and says where the search went.
    fn set_search_engine(&mut self, engine: crate::config::SearchEngine, fzf_available: bool) {
        self.config.search.engine = engine;
        let _ = self.config.save();
        let msg = search_engine_message(engine, fzf_available, self.language);
        self.set_notification(msg);
    }

    /// Ctrl+G pressed inside fzf: reopen the same search in the built-in picker,
    /// already filtered by the query typed in fzf.
    pub fn switch_search_to_builtin(&mut self, request: FzfRequest, query: FzfQuery, text: &str) {
        self.set_search_engine(crate::config::SearchEngine::Builtin, true);
        self.open_picker(request, query, text);
    }

    /// Ctrl+G pressed inside the built-in picker: hand its candidates and query to
    /// fzf (run by the main loop). Without fzf the picker stays open.
    fn switch_search_to_fzf(&mut self) {
        if !crate::integration::fzf::is_fzf_available() {
            let msg = match self.language {
                Language::En => "fzf not found on PATH · staying in the built-in picker",
                Language::Pt => "fzf não encontrado no PATH · continuando no picker interno",
            };
            self.set_notification(msg);
            return;
        }
        let Some(picker) = self.picker.take() else {
            return;
        };
        self.set_search_engine(crate::config::SearchEngine::Fzf, true);
        let query = FzfQuery {
            items: picker.items,
            header: picker.header,
            files: picker.files,
        };
        self.fzf_carry = Some((query, picker.query));
        self.fzf_request = Some(picker.request);
    }

    /// Ctrl+G: auto → fzf → builtin → auto (fzf skipped when not installed), saved
    /// to config.toml like a settings change.
    pub fn cycle_search_engine(&mut self) {
        let fzf_available = crate::integration::fzf::is_fzf_available();
        self.config.search.engine = self.config.search.engine.next(fzf_available);
        let _ = self.config.save();
        let msg = search_engine_message(self.config.search.engine, fzf_available, self.language);
        self.set_notification(msg);
    }

    fn handle_picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        match picker_key_action(key) {
            PickerAction::Cancel => self.picker = None,
            PickerAction::Accept => {
                let request = picker.request;
                let selected = picker.selected_item().cloned();
                self.picker = None;
                match (selected, request) {
                    (Some(item), FzfRequest::Files) => self.handle_fzf_file_result(item),
                    (Some(item), FzfRequest::Text) => self.handle_fzf_text_result(item),
                    (None, _) => {}
                }
            }
            PickerAction::Move(delta) => picker.move_by(delta),
            PickerAction::ClearQuery => {
                picker.query.clear();
                picker.refilter();
            }
            PickerAction::Backspace => {
                picker.query.pop();
                picker.refilter();
            }
            PickerAction::Type(c) => {
                picker.query.push(c);
                picker.refilter();
            }
            PickerAction::SwitchEngine => self.switch_search_to_fzf(),
            PickerAction::TogglePreview => {
                picker.toggle_preview();
                self.picker_preview = picker.show_preview;
            }
            PickerAction::Ignore => {}
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
        let section = if selected_line.ends_with("\tstaged") {
            DiffSection::Staged
        } else {
            DiffSection::Changes
        };
        let Some((path, line)) = location.rsplit_once(':') else {
            return;
        };
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
        if self.full_context {
            FULL_CONTEXT_LINES
        } else {
            self.config.diff.context_lines
        }
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
                    return hunk
                        .lines
                        .first()
                        .and_then(|l| l.new_line_no.or(l.old_line_no));
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
            r.right
                .as_ref()
                .and_then(|l| l.new_line_no)
                .or_else(|| r.left.as_ref().and_then(|l| l.old_line_no))
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
            Some((
                r.hunk_index?,
                r.left_line_idx
                    .into_iter()
                    .chain(r.right_line_idx)
                    .collect(),
            ))
        }
    }

    fn cursor_hunk(&self) -> Option<usize> {
        self.row_hunk(self.selected_row).map(|(h, _)| h)
    }

    fn hunk_start_row(&self, hunk_idx: usize) -> Option<usize> {
        let file = self.current_file()?;
        if self.is_unified {
            Some(
                file.hunks
                    .iter()
                    .take(hunk_idx)
                    .map(|h| h.lines.len() + 1)
                    .sum(),
            )
        } else {
            file.aligned_rows
                .iter()
                .position(|r| r.hunk_index == Some(hunk_idx))
        }
    }

    fn column_labels(&self) -> [&'static str; 2] {
        if self.active_commit_view.is_some() || self.active_commit_info.is_some() {
            return ["PARENT", "COMMIT"];
        }
        if self.active_stash_info.is_some() {
            return ["BASE", "STASH"];
        }
        let working_tree = matches!(
            &self.mode,
            AppMode::Git {
                target_ref: None,
                ..
            }
        ) && !self.staged_only;
        match self.current_file() {
            Some(file) if working_tree && file.section == DiffSection::Staged => ["HEAD", "STAGED"],
            Some(file) if working_tree => {
                let partly_staged = self
                    .files
                    .iter()
                    .any(|f| f.section == DiffSection::Staged && f.new_path == file.new_path);
                [
                    if partly_staged { "STAGED" } else { "HEAD" },
                    "WORKING TREE",
                ]
            }
            _ => ["ORIGINAL", "MODIFIED"],
        }
    }

    fn selected_commit_hash(&self) -> Option<String> {
        if self.show_history {
            return self
                .history_commits
                .get(self.selected_history_idx)
                .map(|c| c.hash.clone());
        }
        if let Some(commit) = &self.active_commit_info {
            return Some(commit.hash.clone());
        }
        if self.drawer_tab == DrawerTab::Commits {
            return self
                .repo_commits
                .get(self.selected_repo_commit_idx)
                .map(|c| c.hash.clone());
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
        let AppMode::Git { git_provider, .. } = &self.mode else {
            return;
        };
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
            (DiffSection::Staged, Language::Pt) => {
                "Não está staged · selecione em Staged para unstage"
            }
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
        if self.active_commit_info.is_some()
            || self.active_stash_info.is_some()
            || self.show_history
        {
            self.set_notification(match self.language {
                Language::En => "Staging only works on working tree changes",
                Language::Pt => "Stage só funciona nas mudanças da working tree",
            });
            return false;
        }
        if self.current_comparison_branch().is_some() {
            self.set_notification(match self.language {
                Language::En => {
                    "Staging is disabled while comparing with another branch · Reset to Default [B]"
                }
                Language::Pt => {
                    "Stage desativado ao comparar com outra branch · Volte ao Padrão [B]"
                }
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
        if self
            .notification
            .as_ref()
            .is_some_and(|(_, time)| time.elapsed() > TOAST_DURATION)
        {
            self.notification = None;
        }
    }

    pub fn set_notification(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), Instant::now()));
    }

    /// Startup update check progress: downloading, installed or failed. A result
    /// replaces the "downloading" popup, reopening it if it was dismissed.
    pub fn show_update_event(&mut self, event: UpdateEvent) {
        self.update_popup = Some(event);
    }

    pub fn reload_diffs(&mut self) {
        if let Err(e) = self.reload_diffs_internal(true) {
            self.set_notification(format!("Reload error: {}", e));
        }
    }

    /// After the selected file disappears (e.g. fully staged), land on the
    /// closest file instead of a section header or folder.
    fn select_nearest_tree_file(&mut self) {
        let start = self
            .selected_tree_idx
            .min(self.tree_items.len().saturating_sub(1));
        let forward =
            (start..self.tree_items.len()).find(|&i| self.tree_items[i].file_index.is_some());
        let backward = (0..start)
            .rev()
            .find(|&i| self.tree_items[i].file_index.is_some());
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
            AppMode::Git {
                target_ref,
                git_provider,
            } => git_provider.load_diffs(
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
                match self
                    .tree_items
                    .iter()
                    .position(|item| item.file_index.is_some_and(|i| same(&self.files[i])))
                {
                    Some(pos) => self.selected_tree_idx = pos,
                    None => self.select_nearest_tree_file(),
                }
            } else if let Some(pos) = self
                .filtered_indices
                .iter()
                .position(|&idx| same(&self.files[idx]))
            {
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
                self.wrap_skip = 0;
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
            self.language,
        );

        if self.selected_tree_idx >= self.tree_items.len() {
            self.selected_tree_idx = self.tree_items.len().saturating_sub(1);
        }
    }

    pub fn set_folder_collapse(&mut self, collapse: bool, selected_only: bool) {
        if self.focus != Focus::FileTree || self.file_view_mode != FileViewMode::Tree {
            return;
        }
        let selected = self
            .tree_items
            .get(self.selected_tree_idx)
            .map(|i| i.path.clone());
        let scope = if selected_only {
            let Some(item) = self
                .tree_items
                .get(self.selected_tree_idx)
                .filter(|i| i.is_dir)
            else {
                return;
            };
            Some(item.path.clone())
        } else {
            None
        };
        self.update_folder_collapse(collapse, scope);
        self.update_filter();
        if let Some(path) = selected {
            // Preserve the selected folder, or its nearest visible ancestor.
            if let Some(idx) = self
                .tree_items
                .iter()
                .enumerate()
                .filter(|(_, i)| i.path == path || (i.is_dir && path.starts_with(&i.path)))
                .max_by_key(|(_, i)| i.path.components().count())
                .map(|(idx, _)| idx)
            {
                self.selected_tree_idx = idx;
            }
        }
        self.file_tree_scroll = self.file_tree_scroll.min(self.selected_tree_idx);
    }

    fn collapse_folder(&mut self, path: PathBuf) {
        self.update_folder_collapse(true, Some(path));
    }

    fn update_folder_collapse(&mut self, collapse: bool, scope: Option<PathBuf>) {
        // Build an expanded tree so hidden descendants participate too. Include
        // filtered-out files to make the whole-tree action truly global.
        let indices: Vec<_> = (0..self.files.len()).collect();
        let expanded = build_tree_items(
            &self.files,
            &indices,
            &HashSet::new(),
            FileViewMode::Tree,
            self.language,
        );
        for item in expanded
            .into_iter()
            .filter(|i| i.is_dir && scope.as_ref().is_none_or(|p| i.path.starts_with(p)))
        {
            if collapse {
                self.collapsed_dirs.insert(item.path);
            } else {
                self.collapsed_dirs.remove(&item.path);
            }
        }
    }

    pub fn return_to_changes(&mut self) {
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
        if self.is_unified || self.column_side == ColumnSide::Right {
            1
        } else {
            0
        }
    }

    fn horizontal_limit(&self) -> usize {
        use unicode_width::UnicodeWidthStr;
        let idx = self.horizontal_index();
        let max_width = self
            .current_file()
            .map(|f| {
                if self.is_unified {
                    f.hunks
                        .iter()
                        .flat_map(|h| &h.lines)
                        .map(|l| l.content.width())
                        .max()
                        .unwrap_or(0)
                } else {
                    f.aligned_rows
                        .iter()
                        .filter_map(|r| {
                            if idx == 0 {
                                r.left.as_ref()
                            } else {
                                r.right.as_ref()
                            }
                        })
                        .map(|l| l.content.width())
                        .max()
                        .unwrap_or(0)
                }
            })
            .unwrap_or(0);
        let width = self.diff_width.saturating_sub(2) as usize;
        let content_width = if self.is_unified {
            width.saturating_sub(16)
        } else {
            (width.saturating_sub(1) / 2).saturating_sub(10)
        };
        max_width.saturating_sub(content_width.max(1))
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.expire_notification();

        // Update popup is drawn above everything else, so it takes keys first
        if self.update_popup.is_some() {
            match key.code {
                KeyCode::Char('Q') => self.should_quit = true,
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => self.update_popup = None,
                _ => {}
            }
            return;
        }

        // Built-in search picker: it is a text prompt (like fzf), so every key goes to it
        if self.picker.is_some() {
            self.handle_picker_key(key);
            return;
        }

        // Shift+Q unconditionally quits the program regardless of navigation/modal state
        if key.code == KeyCode::Char('Q') {
            self.should_quit = true;
            return;
        }

        // -1. Settings Modal Active
        if self.show_settings {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'Q' | 'C') => {
                    self.close_settings();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.settings_selected_idx = self.settings_selected_idx.saturating_sub(1);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.settings_selected_idx =
                        (self.settings_selected_idx + 1).min(SETTING_ITEMS.len().saturating_sub(1));
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    self.cycle_setting(false);
                }
                KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.cycle_setting(true);
                }
                KeyCode::Char('s' | 'S') => {
                    self.save_settings();
                }
                _ => {}
            }
            return;
        }

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
                KeyCode::PageDown | KeyCode::Char('d')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.details_popup_scroll += 10;
                }
                KeyCode::PageUp | KeyCode::Char('u')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.details_popup_scroll = self.details_popup_scroll.saturating_sub(10);
                }
                KeyCode::Enter => {
                    if self.drawer_tab == DrawerTab::Commits
                        && self.active_commit_info.is_none()
                        && !self.show_history
                    {
                        self.show_details_popup = false;
                        self.details_popup_scroll = 0;
                        self.load_selected_repo_commit();
                    } else if self.drawer_tab == DrawerTab::Stashes
                        && self.active_stash_info.is_none()
                        && !self.show_history
                    {
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
        if !self.show_worktrees
            && !self.show_help
            && self.confirm_action.is_none()
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
                    KeyCode::Char('g') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // gg: jump to top of view
                        if self.focus == Focus::DiffView {
                            self.selected_row = 0;
                            self.wrap_skip = 0;
                            self.scroll_y = 0;
                        } else if self.drawer_tab == DrawerTab::Changes
                            || self.active_commit_info.is_some()
                            || self.active_stash_info.is_some()
                        {
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
                    KeyCode::Char(c @ ('M' | 'R' | 'c' | 'o')) => {
                        self.set_folder_collapse(matches!(c, 'M' | 'c'), matches!(c, 'c' | 'o'));
                        return;
                    }
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
                '[' if key.code == KeyCode::Char('c') => {
                    // [c was pressed: already jumped hunk on '['
                    return;
                }
                _ => {}
            }
        }

        // 0. Branch Selector Modal Active
        if self.show_branch_selector {
            if let Some(selector) = &mut self.branch_selector {
                let items_count = selector.filtered_items(self.language).len();
                match key.code {
                    KeyCode::Esc => {
                        self.close_branch_selector();
                    }
                    KeyCode::Enter => {
                        self.apply_selected_branch();
                    }
                    KeyCode::Up => {
                        selector.move_up();
                    }
                    KeyCode::Down => {
                        selector.move_down(items_count);
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        selector.move_up();
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        selector.move_down(items_count);
                    }
                    KeyCode::PageUp => {
                        selector.move_up_by(10);
                    }
                    KeyCode::PageDown => {
                        selector.move_down_by(10, items_count);
                    }
                    KeyCode::Backspace => {
                        selector.filter.pop();
                        selector.update_filter();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        selector.filter.clear();
                        selector.update_filter();
                    }
                    KeyCode::Char(c) => {
                        selector.filter.push(c);
                        selector.update_filter();
                    }
                    _ => {}
                }
            }
            return;
        }

        // 1. Worktrees Modal Active
        if self.show_worktrees {
            if let Some(creation) = &mut self.worktree_creation {
                match key.code {
                    KeyCode::Esc => {
                        self.worktree_creation = None;
                    }
                    KeyCode::Tab => {
                        if creation.active_field == 1 {
                            creation.complete_branch();
                        }
                        creation.active_field = 1 - creation.active_field;
                    }
                    KeyCode::BackTab => {
                        creation.active_field = 1 - creation.active_field;
                    }
                    KeyCode::Down | KeyCode::Up => {
                        let count = creation.candidates().len();
                        if creation.active_field == 1 && count > 0 {
                            creation.selected_candidate = if key.code == KeyCode::Down {
                                (creation.selected_candidate + 1) % count
                            } else {
                                (creation.selected_candidate + count - 1) % count
                            };
                        } else {
                            creation.active_field = 1 - creation.active_field;
                        }
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
                                    let cwd = std::env::current_dir()
                                        .unwrap_or_else(|_| git_provider.repo_root.clone());
                                    if let Ok(wts) = git_provider.get_worktrees(&cwd) {
                                        self.worktrees = wts;
                                    }
                                    if let Some(pos) =
                                        self.worktrees.iter().position(|w| w.path == new_path)
                                    {
                                        self.selected_worktree_idx = pos;
                                        self.switch_to_selected_worktree();
                                    }
                                    self.worktree_creation = None;
                                    self.show_worktrees = false;
                                    let msg = match self.language {
                                        Language::En => format!(
                                            "Created & switched to worktree: {}",
                                            new_path.display()
                                        ),
                                        Language::Pt => format!(
                                            "Criado e alternado para worktree: {}",
                                            new_path.display()
                                        ),
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

            let filtered_len = self.filtered_worktrees().len();

            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT)
            {
                match key.code {
                    KeyCode::Char('a') | KeyCode::Char('n') | KeyCode::Char('c') => {
                        if let AppMode::Git { git_provider, .. } = &self.mode {
                            let base = if git_provider.repo_root.join(".worktree").is_dir() {
                                ".worktree/".to_string()
                            } else {
                                "../".to_string()
                            };
                            self.worktree_creation = Some(WorktreeCreationState::new(
                                base,
                                git_provider.get_branches(),
                            ));
                        }
                    }
                    KeyCode::Char('j') => {
                        if filtered_len > 0 && self.selected_worktree_idx + 1 < filtered_len {
                            self.selected_worktree_idx += 1;
                            if self.selected_worktree_idx >= self.worktree_scroll + 10 {
                                self.worktree_scroll = self.selected_worktree_idx.saturating_sub(9);
                            }
                        }
                    }
                    KeyCode::Char('k') => {
                        if self.selected_worktree_idx > 0 {
                            self.selected_worktree_idx -= 1;
                            if self.selected_worktree_idx < self.worktree_scroll {
                                self.worktree_scroll = self.selected_worktree_idx;
                            }
                        }
                    }
                    KeyCode::Char('w') | KeyCode::Char('q') => {
                        self.show_worktrees = false;
                        self.worktree_filter.clear();
                    }
                    KeyCode::Char('u') => {
                        self.worktree_filter.clear();
                        self.selected_worktree_idx = 0;
                        self.worktree_scroll = 0;
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Esc => {
                        self.show_worktrees = false;
                        self.worktree_filter.clear();
                    }
                    KeyCode::Down => {
                        if filtered_len > 0 && self.selected_worktree_idx + 1 < filtered_len {
                            self.selected_worktree_idx += 1;
                            if self.selected_worktree_idx >= self.worktree_scroll + 10 {
                                self.worktree_scroll = self.selected_worktree_idx.saturating_sub(9);
                            }
                        }
                    }
                    KeyCode::Up => {
                        if self.selected_worktree_idx > 0 {
                            self.selected_worktree_idx -= 1;
                            if self.selected_worktree_idx < self.worktree_scroll {
                                self.worktree_scroll = self.selected_worktree_idx;
                            }
                        }
                    }
                    KeyCode::PageDown => {
                        if filtered_len > 0 {
                            self.selected_worktree_idx =
                                (self.selected_worktree_idx + 10).min(filtered_len - 1);
                            if self.selected_worktree_idx >= self.worktree_scroll + 10 {
                                self.worktree_scroll = self.selected_worktree_idx.saturating_sub(9);
                            }
                        }
                    }
                    KeyCode::PageUp => {
                        self.selected_worktree_idx = self.selected_worktree_idx.saturating_sub(10);
                        if self.selected_worktree_idx < self.worktree_scroll {
                            self.worktree_scroll = self.selected_worktree_idx;
                        }
                    }
                    KeyCode::Backspace => {
                        self.worktree_filter.pop();
                        self.selected_worktree_idx = 0;
                        self.worktree_scroll = 0;
                    }
                    KeyCode::Enter => {
                        self.switch_to_selected_worktree();
                    }
                    KeyCode::Char(c) => {
                        self.worktree_filter.push(c);
                        self.selected_worktree_idx = 0;
                        self.worktree_scroll = 0;
                    }
                    _ => {}
                }
            }
            return;
        }

        // 2. Help Modal Active
        if self.show_help {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('?')
                || matches!(key.code, KeyCode::Char('q' | 'Q'))
            {
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

        if matches!(key.code, KeyCode::Char('q')) {
            if self.show_history
                || self.active_commit_info.is_some()
                || self.active_stash_info.is_some()
                || self.active_commit_view.is_some()
                || self.live_snapshot.is_some()
                || self.drawer_tab != DrawerTab::Changes
                || self.focus != Focus::FileTree
                || self.visual_mode
                || self.filter_mode
                || !self.filter_query.is_empty()
            {
                self.return_to_changes();
            } else {
                self.should_quit = true;
            }
            return;
        }

        // Temporary file history drawer
        if self.show_history && self.focus == Focus::FileTree {
            match key.code {
                KeyCode::Tab | KeyCode::BackTab if self.active_commit_view.is_some() => {
                    self.focus = Focus::DiffView
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.file_tree_down(1)
                }
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.file_tree_up(1)
                }
                KeyCode::PageDown => {
                    self.file_tree_down(self.file_tree_height.saturating_sub(4).max(1))
                }
                KeyCode::PageUp => {
                    self.file_tree_up(self.file_tree_height.saturating_sub(4).max(1))
                }
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
                    if self.active_commit_view.is_some() {
                        self.focus = Focus::DiffView;
                    }
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

        // 5. Mouse selection: Ctrl+C copies it, Esc clears it
        if self.mouse_selection.is_some() {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.copy_mouse_selection();
                return;
            }
            if key.code == KeyCode::Esc {
                self.mouse_selection = None;
                return;
            }
        }

        // 6. Normal / Visual Navigation
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
                    self.selected_row =
                        (self.scroll_y + vp.saturating_sub(1)).min(total.saturating_sub(1));
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
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_search_engine();
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
                } else if self.drawer_tab == DrawerTab::Changes
                    || self.active_commit_info.is_some()
                    || self.active_stash_info.is_some()
                {
                    if self.file_view_mode == FileViewMode::Tree {
                        if !self.tree_items.is_empty() {
                            self.selected_tree_idx = self.tree_items.len() - 1;
                            let vp = self.file_tree_height.saturating_sub(4).max(1);
                            self.file_tree_scroll =
                                self.selected_tree_idx.saturating_sub(vp.saturating_sub(1));
                        }
                    } else if !self.filtered_indices.is_empty() {
                        self.selected_filtered_idx = self.filtered_indices.len() - 1;
                        let vp = self.file_tree_height.saturating_sub(4).max(1);
                        self.file_tree_scroll = self
                            .selected_filtered_idx
                            .saturating_sub(vp.saturating_sub(1));
                    }
                } else if self.drawer_tab == DrawerTab::Commits {
                    if !self.repo_commits.is_empty() {
                        self.selected_repo_commit_idx = self.repo_commits.len() - 1;
                        let vp = self.file_tree_height.saturating_sub(4).max(1);
                        self.repo_commit_scroll = self
                            .selected_repo_commit_idx
                            .saturating_sub(vp.saturating_sub(1));
                    }
                } else if self.drawer_tab == DrawerTab::Stashes && !self.stashes.is_empty() {
                    self.selected_stash_idx = self.stashes.len() - 1;
                    let vp = self.file_tree_height.saturating_sub(4).max(1);
                    self.stash_scroll =
                        self.selected_stash_idx.saturating_sub(vp.saturating_sub(1));
                }
            }
            KeyCode::Char('z') => {
                if self.focus == Focus::DiffView || self.file_view_mode == FileViewMode::Tree {
                    self.pending_key = Some('z');
                    self.pending_key_time = Some(Instant::now());
                }
            }
            KeyCode::Char('W') => {
                self.worktree_filter.clear();
                let active_idx = self
                    .worktrees
                    .iter()
                    .position(|w| w.is_current)
                    .unwrap_or(0);
                self.selected_worktree_idx = active_idx;
                self.worktree_scroll = active_idx.saturating_sub(5);
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
                self.column_side = if self.column_side == ColumnSide::Left {
                    ColumnSide::Right
                } else {
                    ColumnSide::Left
                };
            }
            KeyCode::Tab | KeyCode::BackTab
                if self.show_history
                    || self.active_commit_info.is_some()
                    || self.active_stash_info.is_some() =>
            {
                self.focus = if self.focus == Focus::FileTree {
                    Focus::DiffView
                } else {
                    Focus::FileTree
                };
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
            KeyCode::Char('C') => {
                self.open_settings();
            }
            KeyCode::Char('B') => {
                self.open_branch_selector();
            }
            KeyCode::F(10) => {
                self.open_settings();
            }
            KeyCode::Char(',') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_settings();
            }

            KeyCode::Char('r') => {
                self.wrap_lines = !self.wrap_lines;
                self.wrap_skip = 0;
                self.scroll_x = [0, 0];
            }
            KeyCode::Char('m') => {
                self.is_unified = !self.is_unified;
                let mode = if self.is_unified {
                    "Unified"
                } else {
                    "Side-by-Side"
                };
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
                            Language::Pt => {
                                "-- MODO VISUAL (Selecione linhas, pressione 's' para stage) --"
                            }
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
                let idx = self.horizontal_index();
                self.scroll_x[idx] = 0;
            }
            KeyCode::Char('$') | KeyCode::End if self.focus == Focus::DiffView => {
                let idx = self.horizontal_index();
                self.scroll_x[idx] = self.horizontal_limit();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.focus == Focus::FileTree
                    && self.drawer_tab == DrawerTab::Changes
                    && self.file_view_mode == FileViewMode::Tree
                {
                    // Collapse directory
                    if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                        if item.is_dir && !item.is_collapsed {
                            self.collapse_folder(item.path.clone());
                            self.update_filter();
                        }
                    }
                } else if self.focus == Focus::DiffView {
                    let idx = self.horizontal_index();
                    self.scroll_x[idx] = self.scroll_x[idx].saturating_sub(4);
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.focus == Focus::FileTree
                    && self.drawer_tab == DrawerTab::Changes
                    && self.file_view_mode == FileViewMode::Tree
                {
                    // Expand directory
                    if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                        if item.is_dir && item.is_collapsed {
                            self.collapsed_dirs.remove(&item.path);
                            self.update_filter();
                        }
                    }
                } else if self.focus == Focus::DiffView {
                    let idx = self.horizontal_index();
                    self.scroll_x[idx] = self.scroll_x[idx]
                        .saturating_add(4)
                        .min(self.horizontal_limit());
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
                if self.focus == Focus::FileTree
                    && self.drawer_tab == DrawerTab::Changes
                    && self.file_view_mode == FileViewMode::Tree
                {
                    if let Some(item) = self.tree_items.get(self.selected_tree_idx) {
                        if item.is_dir {
                            if item.is_collapsed {
                                self.collapsed_dirs.remove(&item.path);
                            } else {
                                self.collapse_folder(item.path.clone());
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
                                            self.collapse_folder(item.path.clone());
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
                                    if let Some(item) = self.tree_items.get(self.selected_tree_idx)
                                    {
                                        if item.is_dir {
                                            if item.is_collapsed {
                                                self.collapsed_dirs.remove(&item.path);
                                            } else {
                                                self.collapse_folder(item.path.clone());
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
                                    if let Some(item) = self.tree_items.get(self.selected_tree_idx)
                                    {
                                        if item.is_dir {
                                            if item.is_collapsed {
                                                self.collapsed_dirs.remove(&item.path);
                                            } else {
                                                self.collapse_folder(item.path.clone());
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
            KeyCode::Char('s') if self.on_changes_file_list() => {
                self.apply_to_selected_item(false);
            }
            KeyCode::Char('u') if self.on_changes_file_list() => {
                self.apply_to_selected_item(true);
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
            self.selected_history_idx = (self.selected_history_idx + amount)
                .min(self.history_commits.len().saturating_sub(1));
            let rows = self.file_tree_height.saturating_sub(4).max(1);
            if self.selected_history_idx < self.history_scroll {
                self.history_scroll = self.selected_history_idx;
            }
            if self.selected_history_idx >= self.history_scroll + rows {
                self.history_scroll = self.selected_history_idx.saturating_sub(rows - 1);
            }
            if self.selected_history_idx != previous {
                self.load_selected_commit_diff();
            }
            return;
        }

        let tree_vp = self.file_tree_height.saturating_sub(4).max(1);
        if self.drawer_tab == DrawerTab::Changes
            || self.active_commit_info.is_some()
            || self.active_stash_info.is_some()
        {
            if self.file_view_mode == FileViewMode::Tree {
                if !self.tree_items.is_empty() {
                    self.selected_tree_idx =
                        (self.selected_tree_idx + amount).min(self.tree_items.len() - 1);
                    if self.selected_tree_idx >= self.file_tree_scroll + tree_vp {
                        self.file_tree_scroll = self.selected_tree_idx.saturating_sub(tree_vp - 1);
                    }
                    self.wrap_skip = 0;
                    self.scroll_y = 0;
                    self.selected_row = 0;
                }
            } else if !self.filtered_indices.is_empty() {
                self.selected_filtered_idx =
                    (self.selected_filtered_idx + amount).min(self.filtered_indices.len() - 1);
                if self.selected_filtered_idx >= self.file_tree_scroll + tree_vp {
                    self.file_tree_scroll = self.selected_filtered_idx.saturating_sub(tree_vp - 1);
                }
                self.wrap_skip = 0;
                self.scroll_y = 0;
                self.selected_row = 0;
            }
            return;
        }

        match self.drawer_tab {
            DrawerTab::Commits => {
                if !self.repo_commits.is_empty() {
                    self.selected_repo_commit_idx =
                        (self.selected_repo_commit_idx + amount).min(self.repo_commits.len() - 1);
                    if self.selected_repo_commit_idx >= self.repo_commit_scroll + tree_vp {
                        self.repo_commit_scroll =
                            self.selected_repo_commit_idx.saturating_sub(tree_vp - 1);
                    }
                }
            }
            DrawerTab::Stashes if !self.stashes.is_empty() => {
                self.selected_stash_idx =
                    (self.selected_stash_idx + amount).min(self.stashes.len() - 1);
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
            if self.selected_history_idx < self.history_scroll {
                self.history_scroll = self.selected_history_idx;
            }
            if self.selected_history_idx >= self.history_scroll + rows {
                self.history_scroll = self.selected_history_idx.saturating_sub(rows - 1);
            }
            if self.selected_history_idx != previous {
                self.load_selected_commit_diff();
            }
            return;
        }

        if self.drawer_tab == DrawerTab::Changes
            || self.active_commit_info.is_some()
            || self.active_stash_info.is_some()
        {
            if self.file_view_mode == FileViewMode::Tree {
                self.selected_tree_idx = self.selected_tree_idx.saturating_sub(amount);
                if self.selected_tree_idx < self.file_tree_scroll {
                    self.file_tree_scroll = self.selected_tree_idx;
                }
                self.wrap_skip = 0;
                self.scroll_y = 0;
                self.selected_row = 0;
            } else {
                self.selected_filtered_idx = self.selected_filtered_idx.saturating_sub(amount);
                if self.selected_filtered_idx < self.file_tree_scroll {
                    self.file_tree_scroll = self.selected_filtered_idx;
                }
                self.wrap_skip = 0;
                self.scroll_y = 0;
                self.selected_row = 0;
            }
            return;
        }

        match self.drawer_tab {
            DrawerTab::Commits => {
                self.selected_repo_commit_idx =
                    self.selected_repo_commit_idx.saturating_sub(amount);
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

    pub fn filtered_worktrees(&self) -> Vec<(usize, &WorktreeEntry)> {
        crate::ui::components::worktree_popup::filtered_worktrees(
            &self.worktrees,
            &self.worktree_filter,
        )
    }

    pub fn switch_to_selected_worktree(&mut self) {
        let filtered = self.filtered_worktrees();
        if let Some((_, wt)) = filtered.get(self.selected_worktree_idx).cloned() {
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
            self.worktree_filter.clear();
            let notif = match self.language {
                Language::En => format!("Switched to worktree: {}", new_root.display()),
                Language::Pt => format!("Alternado para worktree: {}", new_root.display()),
            };
            self.set_notification(notif);
        }
    }

    pub fn load_selected_repo_commit(&mut self) {
        self.sync_git_context();
        if let Some(commit) = self
            .repo_commits
            .get(self.selected_repo_commit_idx)
            .cloned()
        {
            if let AppMode::Git { git_provider, .. } = &self.mode {
                match git_provider.load_commit_full_diff(&commit.hash) {
                    Ok(files) => {
                        if files.is_empty() {
                            let msg = match self.language {
                                Language::En => format!(
                                    "No file changes in commit {}",
                                    &commit.hash[..7.min(commit.hash.len())]
                                ),
                                Language::Pt => format!(
                                    "Nenhuma alteração no commit {}",
                                    &commit.hash[..7.min(commit.hash.len())]
                                ),
                            };
                            self.set_notification(msg);
                        } else {
                            if self.live_snapshot.is_none() {
                                self.live_snapshot =
                                    Some((self.files.clone(), self.repo_stats.clone()));
                            }
                            let mut stats = self.repo_stats.clone();
                            stats.branch =
                                format!("commit: {}", &commit.hash[..7.min(commit.hash.len())]);
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
                            self.wrap_skip = 0;
                            self.scroll_y = 0;
                            self.selected_row = 0;
                            self.focus = Focus::FileTree;
                            let msg = match self.language {
                                Language::En => format!(
                                    "Inspecting commit {} ({} files) · Esc/2 to list",
                                    &commit.hash[..7.min(commit.hash.len())],
                                    self.files.len()
                                ),
                                Language::Pt => format!(
                                    "Inspecionando commit {} ({} arquivos) · Esc/2 para lista",
                                    &commit.hash[..7.min(commit.hash.len())],
                                    self.files.len()
                                ),
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
                                self.live_snapshot =
                                    Some((self.files.clone(), self.repo_stats.clone()));
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
                            self.wrap_skip = 0;
                            self.scroll_y = 0;
                            self.selected_row = 0;
                            self.focus = Focus::FileTree;
                            let msg = match self.language {
                                Language::En => format!(
                                    "Inspecting stash {} ({} files) · Esc/3 to list",
                                    stash.selector,
                                    self.files.len()
                                ),
                                Language::Pt => format!(
                                    "Inspecionando stash {} ({} arquivos) · Esc/3 para lista",
                                    stash.selector,
                                    self.files.len()
                                ),
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
        if self.update_popup.is_some() {
            return;
        }
        let modal_open = self.picker.is_some()
            || self.show_details_popup
            || self.show_settings
            || self.show_branch_selector
            || self.show_help
            || self.show_worktrees
            || self.confirm_action.is_some();
        if !modal_open && self.handle_selection_mouse(mouse) {
            return;
        }
        self.dispatch_mouse(mouse);
    }

    /// Drag-to-select over the diff text. A press on the text is held back until
    /// release: if the pointer moved it becomes a selection (and is copied),
    /// otherwise the original click is replayed so existing click behaviour is kept.
    fn handle_selection_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        use crate::ui::selection::MouseSelection;
        use crossterm::event::{MouseButton, MouseEventKind};

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.mouse_selection = None;
                self.pending_click = None;
                let divider = self.effective_tree_width();
                let on_divider = self.show_drawer
                    && mouse.column >= divider.saturating_sub(2)
                    && mouse.column <= divider + 2;
                if on_divider {
                    return false;
                }
                let Some(pane) = self.text_map.pane_at(mouse.column, mouse.row) else {
                    return false;
                };
                let Some(hit) = self.text_map.hit(pane, mouse.column, mouse.row) else {
                    return false;
                };
                self.mouse_selection = Some(MouseSelection {
                    pane,
                    anchor: hit,
                    head: hit,
                    dragging: false,
                });
                self.selection_key = self.current_selection_key();
                self.pending_click = Some(mouse);
                true
            }
            MouseEventKind::Drag(MouseButton::Left) if self.pending_click.is_some() => {
                if let Some(sel) = &mut self.mouse_selection {
                    if let Some(hit) = self.text_map.hit(sel.pane, mouse.column, mouse.row) {
                        sel.head = hit;
                        sel.dragging = true;
                    }
                }
                true
            }
            MouseEventKind::Up(_) if self.pending_click.is_some() => {
                let press = self.pending_click.take();
                if let Some(sel) = &mut self.mouse_selection {
                    if let Some(hit) = self.text_map.hit(sel.pane, mouse.column, mouse.row) {
                        if sel.dragging {
                            sel.head = hit;
                        }
                    }
                }
                if self.mouse_selection.as_ref().is_some_and(|s| s.dragging) {
                    self.copy_mouse_selection();
                } else {
                    self.mouse_selection = None;
                    if let Some(press) = press {
                        self.dispatch_mouse(press);
                    }
                    self.dispatch_mouse(mouse);
                }
                true
            }
            _ => false,
        }
    }

    fn current_selection_key(&self) -> Option<SelectionKey> {
        let file = self.current_file()?;
        Some(SelectionKey {
            path: file.new_path.clone(),
            section: file.section,
            is_unified: self.is_unified,
            full_context: self.full_context,
            rows: if self.is_unified {
                file.hunks.iter().map(|h| h.lines.len() + 1).sum()
            } else {
                file.aligned_rows.len()
            },
        })
    }

    pub fn mouse_selection_text(&self) -> Option<String> {
        let sel = self.mouse_selection.as_ref()?;
        let file = self.current_file()?;
        Some(extract_text(&pane_lines(file, sel.pane), sel.range()))
    }

    fn copy_mouse_selection(&mut self) {
        let Some(text) = self.mouse_selection_text().filter(|t| !t.is_empty()) else {
            return;
        };
        let chars = text.chars().count();
        match crate::integration::clipboard::copy_text(&text) {
            Ok(_) => self.set_notification(match self.language {
                Language::En => format!("✓ Copied {} chars to clipboard", chars),
                Language::Pt => format!("✓ {} caracteres copiados para o clipboard", chars),
            }),
            Err(e) => self.set_notification(format!("Clipboard error: {}", e)),
        }
    }

    /// Rows the Changes header takes right now (1, or 2 when it wraps), derived
    /// from the current drawer size and state so a click never uses a stale layout.
    fn current_changes_header_rows(&self) -> u16 {
        changes_header_rows(
            Rect::new(
                0,
                0,
                self.effective_tree_width(),
                self.file_tree_height as u16,
            ),
            &self.tree_items,
            self.filter_mode,
            &self.filter_query,
            self.file_view_mode,
            self.language,
        )
        .max(1)
    }

    fn effective_tree_width(&self) -> u16 {
        if !self.show_drawer {
            0
        } else if self.term_width > 0 && self.term_width < 85 {
            self.file_tree_width
                .min((self.term_width * 32 / 100).max(18))
                .min(self.term_width.saturating_sub(25))
        } else if self.term_width > 0 {
            self.file_tree_width.min(self.term_width.saturating_sub(25))
        } else {
            self.file_tree_width
        }
    }

    fn dispatch_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};

        if let Some(picker) = &mut self.picker {
            match mouse.kind {
                MouseEventKind::ScrollDown => picker.move_by(3),
                MouseEventKind::ScrollUp => picker.move_by(-3),
                _ => {}
            }
            return;
        }

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

        if self.show_settings {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.settings_selected_idx =
                        (self.settings_selected_idx + 1).min(SETTING_ITEMS.len().saturating_sub(1));
                }
                MouseEventKind::ScrollUp => {
                    self.settings_selected_idx = self.settings_selected_idx.saturating_sub(1);
                }
                _ => {}
            }
            return;
        }

        if self.show_branch_selector {
            if let Some(selector) = &mut self.branch_selector {
                let items_count = selector.filtered_items(self.language).len();
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        selector.move_down_by(3, items_count);
                    }
                    MouseEventKind::ScrollUp => {
                        selector.move_up_by(3);
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        let popup_area = centered_rect(
                            76,
                            70,
                            ratatui::layout::Rect {
                                x: 0,
                                y: 0,
                                width: self.term_width,
                                height: self.term_height,
                            },
                        );
                        if !popup_area.contains((mouse.column, mouse.row).into()) {
                            self.close_branch_selector();
                            return;
                        }
                        let inner_y = popup_area.y + 1;
                        let inner_x = popup_area.x + 2;
                        let inner_w = popup_area.width.saturating_sub(4);
                        let list_y = inner_y + 3;
                        let list_h = popup_area.height.saturating_sub(6);
                        if mouse.row >= list_y
                            && mouse.row < list_y + list_h
                            && mouse.column >= inner_x
                            && mouse.column < inner_x + inner_w
                        {
                            let clicked_idx =
                                selector.scroll_offset + (mouse.row - list_y) as usize;
                            if clicked_idx < items_count {
                                if selector.selected_idx == clicked_idx {
                                    self.apply_selected_branch();
                                    return;
                                } else {
                                    selector.selected_idx = clicked_idx;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        if self.show_help
            || self.show_worktrees
            || self.show_branch_selector
            || self.confirm_action.is_some()
        {
            return;
        }

        let effective_tree_width = self.effective_tree_width();

        if self.show_history && mouse.column < effective_tree_width {
            match mouse.kind {
                MouseEventKind::ScrollDown => self.wheel_drawer(true),
                MouseEventKind::ScrollUp => self.wheel_drawer(false),
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

        if mouse.column >= effective_tree_width
            && (matches!(
                mouse.kind,
                MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight
            ) || (mouse.modifiers.contains(KeyModifiers::SHIFT)
                && matches!(
                    mouse.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                )))
        {
            let idx = self.horizontal_index();
            self.scroll_x[idx] = if matches!(
                mouse.kind,
                MouseEventKind::ScrollLeft | MouseEventKind::ScrollUp
            ) {
                self.scroll_x[idx].saturating_sub(4)
            } else {
                self.scroll_x[idx]
                    .saturating_add(4)
                    .min(self.horizontal_limit())
            };
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let down = mouse.kind == MouseEventKind::ScrollDown;
                if effective_tree_width > 0 && mouse.column < effective_tree_width {
                    self.wheel_drawer(down);
                } else {
                    self.wheel_diff(down);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if mouse.row == 0 {
                    let branch_len = if self.current_comparison_branch().is_some() {
                        self.repo_stats.branch.len() + 18
                    } else {
                        self.repo_stats.branch.len() + 8
                    };
                    let branch_end = 10 + (branch_len as u16);
                    if mouse.column >= 10 && mouse.column <= branch_end {
                        self.open_branch_selector();
                        return;
                    } else if mouse.column > branch_end && mouse.column <= branch_end + 40 {
                        let full_dir = if self.repo_stats.root_dir.as_os_str().is_empty()
                            || self.repo_stats.root_dir == std::path::Path::new(".")
                        {
                            std::env::current_dir()
                                .unwrap_or_else(|_| self.repo_stats.root_dir.clone())
                        } else {
                            self.repo_stats.root_dir.clone()
                        };
                        self.set_notification(format!("📁 {}", full_dir.display()));
                        return;
                    }
                }

                if self.show_drawer {
                    let divider_col = effective_tree_width;
                    if mouse.column >= divider_col.saturating_sub(2)
                        && mouse.column <= divider_col + 2
                    {
                        self.is_dragging_divider = true;
                        return;
                    }

                    if mouse.column < effective_tree_width {
                        self.focus = Focus::FileTree;
                        if mouse.row <= 2 {
                            if self.active_commit_info.is_some() || self.active_stash_info.is_some()
                            {
                                return;
                            }
                            let col = mouse.column;
                            let tab_w = (effective_tree_width / 3).max(1);
                            if col < tab_w {
                                self.drawer_tab = DrawerTab::Changes;
                                if let Some((saved_files, saved_stats)) = self.live_snapshot.take()
                                {
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
                            let commit_header_h: u16 =
                                if self.viewport_height < 18 { 3 } else { 4 };
                            if mouse.row <= 2 + commit_header_h {
                                self.active_commit_info = None;
                                if let Some((saved_files, saved_stats)) = self.live_snapshot.take()
                                {
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
                                    self.wrap_skip = 0;
                                    self.scroll_y = 0;
                                    self.selected_row = 0;
                                    let item = &self.tree_items[target_idx];
                                    if item.is_dir {
                                        if item.is_collapsed {
                                            self.collapsed_dirs.remove(&item.path);
                                        } else {
                                            self.collapse_folder(item.path.clone());
                                        }
                                        self.update_filter();
                                    }
                                }
                            } else if target_idx < self.filtered_indices.len() {
                                self.selected_filtered_idx = target_idx;
                                self.wrap_skip = 0;
                                self.scroll_y = 0;
                                self.selected_row = 0;
                            }
                            return;
                        }

                        if self.active_stash_info.is_some() {
                            let stash_header_h: u16 = if self.viewport_height < 18 { 3 } else { 4 };
                            if mouse.row <= 2 + stash_header_h {
                                self.active_stash_info = None;
                                if let Some((saved_files, saved_stats)) = self.live_snapshot.take()
                                {
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
                                    self.wrap_skip = 0;
                                    self.scroll_y = 0;
                                    self.selected_row = 0;
                                    let item = &self.tree_items[target_idx];
                                    if item.is_dir {
                                        if item.is_collapsed {
                                            self.collapsed_dirs.remove(&item.path);
                                        } else {
                                            self.collapse_folder(item.path.clone());
                                        }
                                        self.update_filter();
                                    }
                                }
                            } else if target_idx < self.filtered_indices.len() {
                                self.selected_filtered_idx = target_idx;
                                self.wrap_skip = 0;
                                self.scroll_y = 0;
                                self.selected_row = 0;
                            }
                            return;
                        }

                        // The Changes header may wrap onto a second row; every header row toggles the mode.
                        let header_rows = if self.drawer_tab == DrawerTab::Changes {
                            self.current_changes_header_rows()
                        } else {
                            1
                        };
                        if mouse.row < 3 + header_rows {
                            if self.drawer_tab == DrawerTab::Changes {
                                self.toggle_file_view_mode();
                            }
                            return;
                        }

                        let item_row = (mouse.row.saturating_sub(3 + header_rows)) as usize;
                        match self.drawer_tab {
                            DrawerTab::Changes => {
                                let target_idx = self.file_tree_scroll + item_row;
                                if self.file_view_mode == FileViewMode::Tree {
                                    if target_idx < self.tree_items.len() {
                                        self.selected_tree_idx = target_idx;
                                        self.wrap_skip = 0;
                                        self.scroll_y = 0;
                                        self.selected_row = 0;
                                        let item = &self.tree_items[target_idx];
                                        if item.is_dir {
                                            if item.is_collapsed {
                                                self.collapsed_dirs.remove(&item.path);
                                            } else {
                                                self.collapse_folder(item.path.clone());
                                            }
                                            self.update_filter();
                                        }
                                    }
                                } else if target_idx < self.filtered_indices.len() {
                                    self.selected_filtered_idx = target_idx;
                                    self.wrap_skip = 0;
                                    self.scroll_y = 0;
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
                    if self.drawer_tab == DrawerTab::Commits
                        && self.active_commit_info.is_none()
                        && !self.show_history
                    {
                        self.load_selected_repo_commit();
                        return;
                    }
                    if self.drawer_tab == DrawerTab::Stashes
                        && self.active_stash_info.is_none()
                        && !self.show_history
                    {
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
                    let target_row = self
                        .diff_row_map
                        .get(line_row)
                        .copied()
                        .unwrap_or(usize::MAX);
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
                if self.show_drawer
                    && (self.is_dragging_divider
                        || (mouse.column >= effective_tree_width.saturating_sub(2)
                            && mouse.column <= effective_tree_width + 2))
                {
                    self.is_dragging_divider = true;
                    let max_w = if self.term_width > 0 {
                        self.term_width.saturating_sub(25).max(18)
                    } else {
                        80
                    };
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
        let Some(total) = self.current_file().map(|f| f.hunks.len()) else {
            return;
        };
        let next = match self.cursor_hunk() {
            Some(h) => h + 1,
            None => (0..total)
                .find(|&h| {
                    self.hunk_start_row(h)
                        .is_some_and(|r| r > self.selected_row)
                })
                .unwrap_or(total),
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
        let Some(total) = self.current_file().map(|f| f.hunks.len()) else {
            return;
        };
        let prev = match self.cursor_hunk() {
            Some(h) => h.checked_sub(1),
            None => (0..total).rev().find(|&h| {
                self.hunk_start_row(h)
                    .is_some_and(|r| r < self.selected_row)
            }),
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
                    ColumnSide::Right => r
                        .right
                        .as_ref()
                        .and_then(|l| l.new_line_no)
                        .or_else(|| r.left.as_ref().and_then(|l| l.old_line_no)),
                    ColumnSide::Left => r
                        .left
                        .as_ref()
                        .and_then(|l| l.old_line_no)
                        .or_else(|| r.right.as_ref().and_then(|l| l.new_line_no)),
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
                match crate::integration::clipboard::copy_file_diff_as_markdown(
                    &file.new_path,
                    &file.hunks,
                ) {
                    Ok(_) => {
                        self.set_notification("✓ Copied full file diff to clipboard as Markdown!")
                    }
                    Err(e) => self.set_notification(format!("Clipboard error: {}", e)),
                }
            }
        }
    }

    fn stage_current_hunk(&mut self) {
        if !self.can_act_on_hunk() || !self.require_section(DiffSection::Changes) {
            return;
        }
        if self
            .current_file()
            .is_some_and(|f| f.stage_status == StageStatus::Untracked)
        {
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
        let wanted = if unstage {
            DiffSection::Staged
        } else {
            DiffSection::Changes
        };
        if !self.can_modify_index() || !self.require_section(wanted) {
            return;
        }
        let repo_root = match &self.mode {
            AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
            _ => return,
        };
        let Some(hunk_idx) = self.cursor_hunk() else {
            return;
        };
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
        let Some(file) = self.current_file() else {
            return;
        };
        let Some(hunk) = file.hunks.get(hunk_idx) else {
            return;
        };

        let result = if unstage {
            unstage_partial_hunk(&repo_root, &file.new_path, hunk, &selected_indices)
        } else {
            stage_partial_hunk(&repo_root, &file.new_path, hunk, &selected_indices)
        };
        let verb = if unstage { "Unstaged" } else { "Staged" };
        match result {
            Ok(_) => {
                self.visual_mode = false;
                self.set_notification(format!(
                    "✓ {} {} selected lines",
                    verb,
                    selected_indices.len()
                ));
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
                let msg = format!(
                    "Discard Hunk #{} in {}? Changes cannot be undone.",
                    hunk_idx + 1,
                    file.display_path()
                );
                self.confirm_action = Some((ConfirmAction::DiscardHunk(hunk_idx), msg));
            } else {
                self.set_notification("No hunk under cursor to discard");
            }
        }
    }

    /// `s` / `u` act on the selected file-list item instead of the hunk.
    fn on_changes_file_list(&self) -> bool {
        self.focus == Focus::FileTree && self.drawer_tab == DrawerTab::Changes
    }

    /// The selected file-list item resolved to the files it covers
    /// (Tree: file, folder or section; Flat: the selected file).
    fn selected_stage_target(&self) -> Option<StageTarget> {
        if self.file_view_mode == FileViewMode::Tree {
            let item = self.tree_items.get(self.selected_tree_idx)?;
            return resolve_stage_target(item, &self.files, &self.filtered_indices);
        }
        let file = self.get_underlying_file()?;
        Some(StageTarget {
            scope: StageScope::File(file.new_path.clone()),
            paths: vec![file.new_path.clone()],
        })
    }

    /// Stages (or unstages) the whole file, folder (recursive) or section under
    /// the file-list cursor. Only files that really change are passed to git, so
    /// the toast count is exact and "nothing to do" is reported, not attempted.
    fn apply_to_selected_item(&mut self, unstage: bool) {
        if !self.can_modify_index() {
            return;
        }
        let repo_root = match &self.mode {
            AppMode::Git { git_provider, .. } => git_provider.repo_root.clone(),
            _ => {
                self.set_notification(match self.language {
                    Language::En => "Staging needs a git repository",
                    Language::Pt => "Stage precisa de um repositório git",
                });
                return;
            }
        };
        let Some(target) = self.selected_stage_target() else {
            self.set_notification(match self.language {
                Language::En => "Nothing selected to stage",
                Language::Pt => "Nada selecionado para stage",
            });
            return;
        };
        let wanted = if unstage {
            DiffSection::Staged
        } else {
            DiffSection::Changes
        };
        let pending = files_pending_in(&self.files, &target.paths, wanted);
        let label = self.stage_scope_label(&target.scope);
        if pending.is_empty() {
            self.set_notification(match (unstage, self.language) {
                (false, Language::En) => format!("Nothing to stage in {}", label),
                (false, Language::Pt) => format!("Nada para preparar em {}", label),
                (true, Language::En) => format!("Nothing staged in {}", label),
                (true, Language::Pt) => format!("Nada preparado em {}", label),
            });
            return;
        }

        let mut pathspecs: Vec<PathBuf> = Vec::new();
        for &i in &pending {
            let file = &self.files[i];
            pathspecs.push(file.new_path.clone());
            // A staged rename also stages its source's removal; undo both.
            if unstage {
                if let Some(old) = file.old_path.as_ref().filter(|p| **p != file.new_path) {
                    pathspecs.push(old.clone());
                }
            }
        }
        pathspecs.sort();
        pathspecs.dedup();

        let result = if unstage {
            unstage_paths(&repo_root, &pathspecs)
        } else {
            stage_paths(&repo_root, &pathspecs)
        };
        match result {
            Ok(()) => {
                self.reload_diffs();
                // After the reload, which would replace it with "disk updated".
                let msg = stage_done_message(
                    self.language,
                    unstage,
                    &target.scope,
                    &label,
                    pending.len(),
                );
                self.set_notification(msg);
            }
            Err(e) => self.set_notification(match (unstage, self.language) {
                (false, Language::En) => format!("Stage error: {}", e),
                (false, Language::Pt) => format!("Erro ao preparar: {}", e),
                (true, Language::En) => format!("Unstage error: {}", e),
                (true, Language::Pt) => format!("Erro ao despreparar: {}", e),
            }),
        }
    }

    fn stage_scope_label(&self, scope: &StageScope) -> String {
        match scope {
            StageScope::File(path) => path.display().to_string(),
            StageScope::Dir(path) => format!("{}/", path.display()),
            StageScope::Section(DiffSection::Staged) => "Staged".to_string(),
            StageScope::Section(DiffSection::Changes) => match self.language {
                Language::En => "Changes".to_string(),
                Language::Pt => "Mudanças".to_string(),
            },
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
            let msg = format!(
                "DISCARD ALL changes in {}? All modifications will be lost.",
                file.display_path()
            );
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
                            self.set_notification(format!(
                                "✓ Discarded all changes in {}",
                                path.display()
                            ));
                            self.reload_diffs();
                        }
                        Err(e) => self.set_notification(format!("Discard error: {}", e)),
                    }
                }
            }
        }
    }

    fn get_current_hunk_and_context(
        &self,
    ) -> Option<(PathBuf, PathBuf, crate::core::models::Hunk)> {
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
                        self.set_notification(format!(
                            "No git history found for {}",
                            file.display_path()
                        ));
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
        let Some(commit) = self.history_commits.get(self.selected_history_idx) else {
            return;
        };
        let hash = commit.hash.clone();
        if self
            .active_commit_view
            .as_ref()
            .map(|(active, _)| active == &hash)
            .unwrap_or(false)
        {
            return;
        }
        let path = self
            .history_file_path
            .clone()
            .or_else(|| self.get_underlying_file().map(|f| f.new_path.clone()));
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
                w.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("main")
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
            self.current_comparison_branch(),
            self.language,
            &self.theme,
        );

        // Responsive drawer width on narrow terminals (e.g. half-screen < 85 columns)
        let effective_tree_width = if !self.show_drawer {
            0
        } else if size.width < 85 {
            // Allocate at most 32% of screen to drawer, but not less than 18 cols
            self.file_tree_width
                .min((size.width * 32 / 100).max(18))
                .min(size.width.saturating_sub(25))
        } else {
            self.file_tree_width.min(size.width.saturating_sub(25))
        };

        // 2. Render Main Body (File Tree + Diff View + Overview Ruler)
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(effective_tree_width), // File Tree
                Constraint::Min(20),                      // Diff View
                Constraint::Length(if self.config.ui.overview_ruler { 1 } else { 0 }), // Ruler
            ])
            .split(chunks[1]);

        self.text_map.clear();
        if self.mouse_selection.is_some() && self.selection_key != self.current_selection_key() {
            self.mouse_selection = None;
            self.pending_click = None;
        }

        let file_tree_area = main_chunks[0];
        let diff_area = main_chunks[1];
        self.diff_width = diff_area.width;
        let ruler_area = main_chunks[2];

        self.viewport_height = diff_area.height as usize;
        self.file_tree_height = file_tree_area.height as usize;
        let changes_header_rows = changes_header_rows(
            file_tree_area,
            &self.tree_items,
            self.filter_mode,
            &self.filter_query,
            self.file_view_mode,
            self.language,
        );

        let selected_file_idx = if self.file_view_mode == FileViewMode::Tree {
            self.selected_tree_idx
        } else {
            self.selected_filtered_idx
        };

        if effective_tree_width > 0 {
            render_drawer(
                frame,
                file_tree_area,
                if self.show_history {
                    DrawerTab::Commits
                } else {
                    self.drawer_tab
                },
                &self.tree_items,
                selected_file_idx,
                self.file_tree_scroll,
                if self.show_history {
                    &self.history_commits
                } else {
                    &self.repo_commits
                },
                if self.show_history {
                    self.selected_history_idx
                } else {
                    self.selected_repo_commit_idx
                },
                if self.show_history {
                    self.history_scroll
                } else {
                    self.repo_commit_scroll
                },
                &self.stashes,
                self.selected_stash_idx,
                self.stash_scroll,
                if self.show_history {
                    None
                } else {
                    self.active_commit_info.as_ref()
                },
                if self.show_history {
                    None
                } else {
                    self.active_stash_info.as_ref()
                },
                self.focus == Focus::FileTree,
                self.filter_mode,
                &self.filter_query,
                self.file_view_mode,
                self.language,
                &self.theme,
                if self.show_history {
                    Some(match self.language {
                        Language::En => "File history · o PR",
                        Language::Pt => "Histórico do arquivo · o PR",
                    })
                } else {
                    None
                },
            );
        }

        let is_commits_overview =
            self.drawer_tab == DrawerTab::Commits && self.active_commit_info.is_none();
        let is_stashes_overview =
            self.drawer_tab == DrawerTab::Stashes && self.active_stash_info.is_none();

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
                let width = wrap_width(self.is_unified, diff_area.width);
                let height = self
                    .current_file()
                    .map(|f| {
                        wrapped_row_heights(f, self.is_unified, width)
                            .skip(self.scroll_y)
                            .take(self.selected_row - self.scroll_y + 1)
                            .sum::<usize>()
                    })
                    .unwrap_or(0);
                if height > diff_area.height.saturating_sub(3) as usize {
                    self.scroll_y = self.selected_row;
                }
            }
            let mut row_map = Vec::new();
            let mut text_map = std::mem::take(&mut self.text_map);
            let cur_file = self.current_file();
            let syntax_enabled = self.config.ui.syntax_highlighting;

            let visual_range = if self.visual_mode {
                Some((
                    self.visual_anchor.min(self.selected_row),
                    self.visual_anchor.max(self.selected_row),
                ))
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
                    &mut text_map,
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
                    &mut text_map,
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
                render_ruler(
                    frame,
                    ruler_area,
                    cur_file,
                    self.scroll_y,
                    vp_height,
                    &self.theme,
                );
            }
            self.diff_row_map = row_map;
            self.text_map = text_map;
            self.highlight_mouse_selection(frame);
        }

        // 4. Overlays
        if self.show_settings {
            render_settings_popup(
                frame,
                size,
                &self.config,
                self.settings_selected_idx,
                self.language,
                &self.theme,
            );
        } else if self.show_help {
            render_help_popup(frame, size, self.language, &self.theme);
        } else if self.show_details_popup {
            let cur_file = self.current_file();
            let content = if self.show_history && !self.history_commits.is_empty() {
                DetailsContent::Commit(
                    &self.history_commits[self.selected_history_idx],
                    cur_file.map(std::slice::from_ref).unwrap_or(&[]),
                )
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
                DetailsContent::Commit(
                    &CommitEntry {
                        hash: "N/A".into(),
                        author: "N/A".into(),
                        date: "N/A".into(),
                        message: "No commit or file details available".into(),
                    },
                    &[],
                )
            };
            let root_dir = if self.repo_stats.root_dir.as_os_str().is_empty()
                || self.repo_stats.root_dir == std::path::Path::new(".")
            {
                std::env::current_dir().unwrap_or_else(|_| self.repo_stats.root_dir.clone())
            } else {
                self.repo_stats.root_dir.clone()
            };
            render_details_popup(
                frame,
                size,
                content,
                &root_dir,
                self.details_popup_scroll,
                self.language,
                &self.theme,
            );
        } else if self.show_branch_selector {
            if let Some(state) = &self.branch_selector {
                render_branch_popup(frame, size, state, self.language, &self.theme);
            }
        } else if self.show_worktrees {
            render_worktree_popup(
                frame,
                size,
                &self.worktrees,
                &self.worktree_filter,
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
        if self.show_drawer
            && !self.show_settings
            && !self.show_help
            && !self.show_worktrees
            && !self.show_branch_selector
            && !self.show_history
            && !self.show_details_popup
            && self.confirm_action.is_none()
            && self.picker.is_none()
            && self.update_popup.is_none()
        {
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
                changes_header_rows,
                &self.theme,
            );
        }

        if let Some(picker) = &mut self.picker {
            render_picker_popup(frame, size, picker, self.language, &self.theme);
        }

        // Floating notification styled with the active theme
        if let Some((msg, _)) = &self.notification {
            render_toast(frame, size, msg, &self.theme);
        }

        if let Some(event) = &self.update_popup {
            render_update_popup(frame, size, event, self.language, &self.theme);
        }
    }
}

impl App {
    fn highlight_mouse_selection(&self, frame: &mut Frame) {
        let Some(sel) = &self.mouse_selection else {
            return;
        };
        let range = sel.range();
        let style = ratatui::style::Style::default()
            .fg(self.theme.selected_fg)
            .bg(self.theme.selected_bg)
            .add_modifier(ratatui::style::Modifier::REVERSED);
        let buf = frame.buffer_mut();
        for (pane, _, rows) in &self.text_map.panes {
            if *pane != sel.pane {
                continue;
            }
            for row in rows {
                for (i, &ch) in row.cells.iter().enumerate() {
                    if is_selected(range, row.line, ch) {
                        if let Some(cell) = buf.cell_mut((row.x + i as u16, row.y)) {
                            cell.set_style(style);
                        }
                    }
                }
            }
        }
    }
}

/// Text width the wrapped diff rows are measured against (gutters excluded).
fn wrap_width(is_unified: bool, diff_width: u16) -> usize {
    if is_unified {
        diff_width.saturating_sub(18)
    } else {
        (diff_width.saturating_sub(3) / 2).saturating_sub(10)
    }
    .max(1) as usize
}

/// Screen rows each logical diff row takes when wrapped at `width` columns.
fn wrapped_row_heights(
    file: &FileDiff,
    is_unified: bool,
    width: usize,
) -> Box<dyn Iterator<Item = usize> + '_> {
    use unicode_width::UnicodeWidthStr;
    if is_unified {
        Box::new(file.hunks.iter().flat_map(move |h| {
            std::iter::once(1usize).chain(
                h.lines
                    .iter()
                    .map(move |l| l.content.width().max(1).div_ceil(width)),
            )
        }))
    } else {
        Box::new(file.aligned_rows.iter().map(move |r| {
            let side = |l: &Option<crate::core::models::DiffLine>| {
                l.as_ref().map(|l| l.content.width()).unwrap_or(0)
            };
            side(&r.left).max(side(&r.right)).max(1).div_ceil(width)
        }))
    }
}

/// Toast after `s` / `u` on a file-list item, e.g. "✓ Staged 4 files under src/".
fn stage_done_message(
    language: Language,
    unstage: bool,
    scope: &StageScope,
    label: &str,
    count: usize,
) -> String {
    if let StageScope::File(_) = scope {
        return match (unstage, language) {
            (false, Language::En) => format!("✓ Staged {}", label),
            (false, Language::Pt) => format!("✓ {} preparado", label),
            (true, Language::En) => format!("✓ Unstaged {}", label),
            (true, Language::Pt) => format!("✓ {} despreparado", label),
        };
    }
    let place = match (scope, language) {
        (StageScope::Dir(_), Language::En) => "under",
        (_, Language::En) => "in",
        (_, Language::Pt) => "em",
    };
    let one = count == 1;
    match (unstage, language) {
        (false, Language::En) => format!(
            "✓ Staged {} file{} {} {}",
            count,
            if one { "" } else { "s" },
            place,
            label
        ),
        (true, Language::En) => format!(
            "✓ Unstaged {} file{} {} {}",
            count,
            if one { "" } else { "s" },
            place,
            label
        ),
        (false, Language::Pt) => format!(
            "✓ {} {} {} {}",
            count,
            if one {
                "arquivo preparado"
            } else {
                "arquivos preparados"
            },
            place,
            label
        ),
        (true, Language::Pt) => format!(
            "✓ {} {} {} {}",
            count,
            if one {
                "arquivo despreparado"
            } else {
                "arquivos despreparados"
            },
            place,
            label
        ),
    }
}

/// Toast naming the engine searches will actually use.
fn search_engine_message(
    engine: crate::config::SearchEngine,
    fzf_available: bool,
    language: Language,
) -> String {
    use crate::config::ResolvedSearch;
    let effective = match (engine.resolve(fzf_available), language) {
        (ResolvedSearch::Fzf, _) => "fzf",
        (ResolvedSearch::Builtin { .. }, Language::En) => "built-in picker",
        (ResolvedSearch::Builtin { .. }, Language::Pt) => "picker interno",
    };
    let auto = match language {
        Language::En => "auto",
        Language::Pt => "automático",
    };
    match (engine, language) {
        (crate::config::SearchEngine::Auto, Language::En) => {
            format!("Search engine: {} ({}) · Ctrl+G", auto, effective)
        }
        (crate::config::SearchEngine::Auto, Language::Pt) => {
            format!("Motor de busca: {} ({}) · Ctrl+G", auto, effective)
        }
        (_, Language::En) => format!("Search engine: {} · Ctrl+G", effective),
        (_, Language::Pt) => format!("Motor de busca: {} · Ctrl+G", effective),
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
            let section = if f.section == DiffSection::Staged {
                "\tstaged"
            } else {
                ""
            };
            out.push(format!(
                "{}:{}\t{} {}{}",
                path,
                line_no,
                prefix,
                line.content.trim_end_matches(['\r', '\n']),
                section
            ));
        }
    }
    out
}

#[cfg(test)]
mod folder_tests {
    use super::*;
    use crate::core::models::{ChangeStats, FileStatus, StageStatus};
    use std::path::Path;

    fn app() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let new = dir.path().join("new");
        std::fs::write(&old, "a\n").unwrap();
        std::fs::write(&new, "b\n").unwrap();
        let mut app = App::new(
            AppMode::FilePair(old, new),
            Config::default(),
            false,
            false,
            false,
            None,
            false,
            false,
        )
        .unwrap();
        app.files = ["src/a", "src/ui/widgets/b", "srcfoo/sub/c"]
            .into_iter()
            .map(|path| FileDiff {
                old_path: None,
                new_path: path.into(),
                status: FileStatus::Modified,
                stage_status: StageStatus::Unstaged,
                section: DiffSection::Changes,
                stats: ChangeStats::default(),
                hunks: vec![],
                aligned_rows: vec![],
                is_binary: false,
            })
            .collect();
        app.focus = Focus::FileTree;
        app.file_view_mode = FileViewMode::Tree;
        app.update_filter();
        (dir, app)
    }

    fn sequence(app: &mut App, c: char) {
        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }

    #[test]
    fn whole_tree_collapses_hidden_descendants_and_expands_all() {
        let (_dir, mut app) = app();
        app.filter_query = "src/ui".into();
        app.update_filter();
        sequence(&mut app, 'M');
        assert_eq!(app.collapsed_dirs.len(), 5);
        assert!(app
            .collapsed_dirs
            .contains(&PathBuf::from("src/ui/widgets")));
        sequence(&mut app, 'R');
        assert!(app.collapsed_dirs.is_empty());
    }

    #[test]
    fn selected_subtree_is_recursive_and_preserves_siblings_and_selection() {
        let (_dir, mut app) = app();
        app.collapsed_dirs.insert("srcfoo/sub".into());
        app.update_filter();
        app.selected_tree_idx = app
            .tree_items
            .iter()
            .position(|i| i.path == Path::new("src"))
            .unwrap();
        sequence(&mut app, 'c');
        for path in ["src", "src/ui", "src/ui/widgets", "srcfoo/sub"] {
            assert!(app.collapsed_dirs.contains(&PathBuf::from(path)));
        }
        assert_eq!(app.tree_items[app.selected_tree_idx].path, Path::new("src"));
        sequence(&mut app, 'o');
        assert_eq!(
            app.collapsed_dirs,
            HashSet::from([PathBuf::from("srcfoo/sub")])
        );
    }

    #[test]
    fn ordinary_collapse_is_recursive_and_reopening_keeps_children_collapsed() {
        for key in [
            KeyCode::Char('h'),
            KeyCode::Left,
            KeyCode::Enter,
            KeyCode::Char(' '),
        ] {
            let (_dir, mut app) = app();
            app.selected_tree_idx = app
                .tree_items
                .iter()
                .position(|i| i.path == Path::new("src"))
                .unwrap();
            app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
            for path in ["src", "src/ui", "src/ui/widgets"] {
                assert!(
                    app.collapsed_dirs.contains(Path::new(path)),
                    "{key:?}: {path}"
                );
            }
            app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            assert!(!app.collapsed_dirs.contains(Path::new("src")));
            for path in ["src/ui", "src/ui/widgets"] {
                assert!(app.collapsed_dirs.contains(Path::new(path)));
            }
            assert!(app
                .tree_items
                .iter()
                .any(|i| i.path == Path::new("src/ui") && i.is_collapsed));
            assert!(!app
                .tree_items
                .iter()
                .any(|i| i.path == Path::new("src/ui/widgets")));
            sequence(&mut app, 'o');
            assert!(app.collapsed_dirs.is_empty());
            assert!(app
                .tree_items
                .iter()
                .any(|i| i.path == Path::new("src/ui/widgets") && !i.is_collapsed));
        }
    }

    #[test]
    fn expand_selection_and_all_reach_filtered_out_descendants() {
        let (_dir, mut app) = app();
        sequence(&mut app, 'M');
        app.filter_query = "src/a".into();
        app.update_filter();
        sequence(&mut app, 'o');
        for path in ["src", "src/ui", "src/ui/widgets"] {
            assert!(!app.collapsed_dirs.contains(Path::new(path)));
        }
        for path in ["srcfoo", "srcfoo/sub"] {
            assert!(app.collapsed_dirs.contains(Path::new(path)));
        }
        sequence(&mut app, 'R');
        assert!(app.collapsed_dirs.is_empty());
        app.filter_query.clear();
        app.update_filter();
        assert!(app
            .tree_items
            .iter()
            .filter(|i| i.is_dir)
            .all(|i| !i.is_collapsed));
    }

    #[test]
    fn selected_folder_in_a_section_does_not_change_other_section() {
        let (_dir, mut app) = app();
        let mut staged = app.files[1].clone();
        staged.section = DiffSection::Staged;
        app.files.push(staged);
        app.update_filter();
        app.selected_tree_idx = app
            .tree_items
            .iter()
            .position(|i| i.path == Path::new(":changes/src"))
            .unwrap();
        sequence(&mut app, 'c');
        assert!(app
            .collapsed_dirs
            .contains(&PathBuf::from(":changes/src/ui/widgets")));
        assert!(!app.collapsed_dirs.contains(&PathBuf::from(":staged/src")));
        sequence(&mut app, 'o');
        assert!(app.collapsed_dirs.is_empty());
        sequence(&mut app, 'M');
        assert!(app.collapsed_dirs.contains(&PathBuf::from(":staged")));
        assert!(app
            .collapsed_dirs
            .contains(&PathBuf::from(":changes/src/ui/widgets")));
        sequence(&mut app, 'R');
        assert!(app.collapsed_dirs.is_empty());
    }

    #[test]
    fn selected_file_and_diff_focus_do_not_collapse_folders() {
        let (_dir, mut app) = app();
        app.selected_tree_idx = app.tree_items.iter().position(|i| !i.is_dir).unwrap();
        sequence(&mut app, 'c');
        assert!(app.collapsed_dirs.is_empty());
        app.focus = Focus::DiffView;
        sequence(&mut app, 'M');
        assert!(app.collapsed_dirs.is_empty());
    }
}
