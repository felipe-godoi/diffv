use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "diffv",
    version,
    about = "High-performance VS Code style CLI diff viewer and companion for AI coding agents",
    long_about = "diffv is a high-performance terminal diff viewer featuring dual-column side-by-side view, \
    intra-line word/character highlighting, live file watching for AI coding agents, \
    seamless Neovim and Tmux integration, interactive hunk/file staging and discarding, \
    and support for arbitrary file/dir comparison and stdin pipes."
)]
pub struct Cli {
    /// Skip the automatic update check on startup
    #[arg(long)]
    pub no_update: bool,

    /// Positional targets: can be a git ref (HEAD~1, branch), two files, two directories, or '-' for stdin
    #[arg(value_name = "TARGET")]
    pub targets: Vec<String>,

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
    #[arg(long = "theme")]
    pub theme: Option<String>,

    /// Ignore whitespace differences
    #[arg(long = "ignore-whitespace")]
    pub ignore_whitespace: bool,

    /// View commit history for the specified target file
    #[arg(short = 'H', long = "history")]
    pub history: bool,

    /// Run as if started in <PATH> instead of the current working directory
    #[arg(short = 'C', long = "cwd")]
    pub cwd: Option<PathBuf>,
}
