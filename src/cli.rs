use clap::Parser;
use std::path::PathBuf;

/// Release builds set `DIFFV_VERSION` (e.g. "0.2.2 (16742ac 2026-10-02)") so binaries are identifiable.
const VERSION: &str = match option_env!("DIFFV_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "diffv",
    version = VERSION,
    about = "High-performance VS Code style CLI diff viewer to use alongside your AI coding agent",
    long_about = "diffv is a high-performance terminal diff viewer featuring dual-column side-by-side view, \
    intra-line word/character highlighting, live file watching for AI coding agents, \
    seamless Tmux and editor integration ($EDITOR, vim by default), interactive hunk/file staging and discarding, \
    and support for arbitrary file/dir comparison and stdin pipes.",
    disable_version_flag = true
)]
pub struct Cli {
    /// Print version information
    #[arg(short = 'v', short_alias = 'V', long = "version", action = clap::ArgAction::Version)]
    pub version: Option<bool>,

    /// Skip the automatic update check on startup
    #[arg(short = 'n', long = "no-update", visible_aliases = ["no-auto-update", "no-up"])]
    pub no_update: bool,

    /// Opt-in to beta pre-releases
    #[arg(short = 'b', long = "beta")]
    pub beta: bool,

    /// Opt-in to nightly builds (latest build from main branch)
    #[arg(long = "nightly")]
    pub nightly: bool,

    /// Select update channel ('stable', 'beta', or 'nightly')
    #[arg(long = "channel", value_name = "CHANNEL")]
    pub channel: Option<String>,

    /// Explicitly enable or disable automatic updates (true or false)
    #[arg(long = "auto-update", value_name = "BOOL")]
    pub auto_update: Option<bool>,

    /// Check and install updates immediately
    #[arg(long = "update", visible_alias = "upgrade")]
    pub update: bool,

    /// Uninstall diffv and remove installed binaries and configurations
    #[arg(short = 'U', long = "uninstall")]
    pub uninstall: bool,

    /// Positional targets: can be a git ref (HEAD~1, branch), two files, two directories, or '-' for stdin
    #[arg(value_name = "TARGET")]
    pub targets: Vec<String>,

    /// Branch or git ref to compare the current worktree against (e.g. main, develop, origin/main)
    #[arg(
        short = 'B',
        long = "compare",
        visible_alias = "branch",
        value_name = "BRANCH"
    )]
    pub compare: Option<String>,

    /// Enable live file watching mode (auto-reload on disk changes)
    #[arg(short = 'w', long = "watch")]
    pub watch: bool,

    /// View only staged/cached changes
    #[arg(short = 's', long = "staged", visible_alias = "cached")]
    pub staged: bool,

    /// Start in unified/inline diff mode instead of side-by-side
    #[arg(short = 'u', long = "unified")]
    pub unified: bool,

    /// Override color theme (vscode-dark, tokyonight, catppuccin, gruvbox)
    #[arg(short = 't', long = "theme")]
    pub theme: Option<String>,

    /// Ignore whitespace differences
    #[arg(short = 'i', short_alias = 'W', long = "ignore-whitespace", visible_aliases = ["ignore-all-space", "ws"])]
    pub ignore_whitespace: bool,

    /// View commit history for the specified target file
    #[arg(short = 'H', long = "history", visible_alias = "log")]
    pub history: bool,

    /// Run as if started in <PATH> instead of the current working directory
    #[arg(short = 'C', long = "cwd")]
    pub cwd: Option<PathBuf>,

    /// On error, print it and wait for a key before exiting (keeps tmux popups readable)
    #[arg(short = 'e', long = "wait-on-error", visible_alias = "wait")]
    pub wait_on_error: bool,

    /// Show/hide diffv in a tmux popup, keeping its state between toggles.
    /// Meant for a tmux key binding (see README)
    #[arg(long, num_args = 3, value_names = ["CLIENT", "SESSION", "PATH"])]
    pub tmux_toggle: Option<Vec<String>>,
}
