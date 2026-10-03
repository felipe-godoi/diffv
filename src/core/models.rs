use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Copied,
}

impl FileStatus {
    pub fn code(&self) -> &'static str {
        match self {
            FileStatus::Modified => "M",
            FileStatus::Added => "A",
            FileStatus::Deleted => "D",
            FileStatus::Renamed => "R",
            FileStatus::Untracked => "?",
            FileStatus::Copied => "C",
        }
    }
}

/// Which side of the index a working-tree diff belongs to:
/// `Staged` is HEAD → index, `Changes` is index → working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffSection {
    #[default]
    Changes,
    Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Unstaged,
    Staged,
    PartiallyStaged,
    Untracked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChangeStats {
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Context,
    Addition,
    Deletion,
    Virtual, // Padding / filler line in side-by-side
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub is_intraline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub content: String,
    pub old_line_no: Option<usize>,
    pub new_line_no: Option<usize>,
    pub spans: Vec<HighlightSpan>,
}

impl DiffLine {
    pub fn virtual_line() -> Self {
        Self {
            kind: DiffKind::Virtual,
            content: String::new(),
            old_line_no: None,
            new_line_no: None,
            spans: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignedRow {
    pub left: Option<DiffLine>,
    pub right: Option<DiffLine>,
    pub hunk_index: Option<usize>,
    pub left_line_idx: Option<usize>,
    pub right_line_idx: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub old_path: Option<PathBuf>,
    pub new_path: PathBuf,
    pub status: FileStatus,
    pub stage_status: StageStatus,
    pub section: DiffSection,
    pub stats: ChangeStats,
    pub hunks: Vec<Hunk>,
    pub aligned_rows: Vec<AlignedRow>,
    pub is_binary: bool,
}

impl FileDiff {
    pub fn display_path(&self) -> String {
        self.new_path.to_string_lossy().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoStats {
    pub repo_name: String,
    pub branch: String,
    pub root_dir: PathBuf,
    pub total_additions: usize,
    pub total_deletions: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub index: usize,
    pub selector: String, // e.g. "stash@{0}"
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerTab {
    #[default]
    Changes,
    Commits,
    Stashes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    En,
    Pt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatcherScanState {
    #[default]
    Idle,
    Scanning {
        scanned_dirs: usize,
    },
    Ready {
        total_dirs: usize,
    },
}
