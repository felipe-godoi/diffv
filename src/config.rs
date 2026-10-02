use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub diff: DiffConfig,
    #[serde(default)]
    pub watcher: WatcherConfig,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_view")]
    pub default_view: String,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default = "default_true")]
    pub syntax_highlighting: bool,
    #[serde(default = "default_true")]
    pub overview_ruler: bool,
    #[serde(default = "default_tab_width")]
    pub tab_width: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffConfig {
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    #[serde(default)]
    pub ignore_whitespace: bool,
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default = "default_true")]
    pub watch_untracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "default_editor_command")]
    pub command: String,
    #[serde(default = "default_editor_args")]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub use_nvr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Beta => "beta",
        }
    }
}

impl std::str::FromStr for UpdateChannel {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "stable" => Ok(UpdateChannel::Stable),
            "beta" | "main" | "nightly" => Ok(UpdateChannel::Beta),
            other => anyhow::bail!("Unknown update channel '{}'. Expected 'stable' or 'beta'.", other),
        }
    }
}

impl std::fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_true", alias = "enabled")]
    pub auto_update: bool,
    #[serde(default)]
    pub channel: UpdateChannel,
}

fn default_theme() -> String {
    "auto".to_string()
}
fn default_view() -> String {
    "side-by-side".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_tab_width() -> usize {
    4
}
fn default_algorithm() -> String {
    "patience".to_string()
}
fn default_context_lines() -> usize {
    3
}
fn default_debounce_ms() -> u64 {
    150
}
fn default_editor_command() -> String {
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            return editor;
        }
    }
    "nvim".to_string()
}
fn default_editor_args() -> Vec<String> {
    vec!["+{{line}}".to_string(), "{{file}}".to_string()]
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            default_view: default_view(),
            show_line_numbers: default_true(),
            syntax_highlighting: default_true(),
            overview_ruler: default_true(),
            tab_width: default_tab_width(),
        }
    }
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            algorithm: default_algorithm(),
            ignore_whitespace: false,
            context_lines: default_context_lines(),
        }
    }
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            debounce_ms: default_debounce_ms(),
            watch_untracked: default_true(),
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: default_editor_command(),
            args: default_editor_args(),
            use_nvr: default_true(),
        }
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_update: true,
            channel: UpdateChannel::Stable,
        }
    }
}


impl Config {
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("diffv").join("config.toml"))
    }

    /// Loads the user config; an invalid file is reported (before the TUI
    /// starts) instead of being silently replaced by defaults.
    pub fn load() -> Self {
        let Some(path) = Self::config_path().filter(|p| p.exists()) else {
            return Config::default();
        };
        Self::load_from_path(&path).unwrap_or_else(|err| {
            eprintln!("diffv: ignoring invalid config {}: {:#}", path.display(), err);
            Config::default()
        })
    }

    pub fn load_from_path(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&content)?;
        Ok(cfg)
    }

    /// Persists the configuration into the user's config file (~/.config/diffv/config.toml).
    pub fn save(&self) -> anyhow::Result<PathBuf> {
        let path = Self::config_path().ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.save_to_path(&path)?;
        Ok(path)
    }

    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }
}
