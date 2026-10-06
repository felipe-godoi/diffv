use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    Nightly,
}

impl UpdateChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Beta => "beta",
            UpdateChannel::Nightly => "nightly",
        }
    }
}

impl std::str::FromStr for UpdateChannel {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "stable" | "release" => Ok(UpdateChannel::Stable),
            "beta" | "rc" | "preview" => Ok(UpdateChannel::Beta),
            "nightly" | "dev" | "canary" | "main" | "rolling" => Ok(UpdateChannel::Nightly),
            other => anyhow::bail!(
                "Unknown update channel '{}'. Expected 'stable', 'beta', or 'nightly'.",
                other
            ),
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
    let env = EditorEnv {
        git_editor: std::env::var("GIT_EDITOR").ok(),
        visual: std::env::var("VISUAL").ok(),
        editor: std::env::var("EDITOR").ok(),
    };
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    editor_command_from(
        &env,
        |name| dirs.iter().any(|dir| is_executable(&dir.join(name))),
        is_executable,
    )
}

/// Editor-related environment variables, read once so the resolution stays testable.
#[derive(Debug, Default, Clone)]
struct EditorEnv {
    git_editor: Option<String>,
    visual: Option<String>,
    editor: Option<String>,
}

/// Default `[editor] command` when the config file sets none (nvr is handled
/// separately, only with `use_nvr` inside `$NVIM`):
/// 1. the first non-empty of `$GIT_EDITOR`, `$VISUAL`, `$EDITOR` (git's order);
/// 2. the system default editor: `editor` on the `PATH` (the Debian/Ubuntu
///    `update-alternatives` entry), else `/usr/bin/editor` if executable;
/// 3. `vi`, then `nano`, whichever is on the `PATH`; `vi` as the last resort.
///
/// `on_path` says whether a command name resolves in the `PATH`;
/// `is_executable` checks an absolute path. No subprocess is spawned.
fn editor_command_from(
    env: &EditorEnv,
    on_path: impl Fn(&str) -> bool,
    is_executable: impl Fn(&Path) -> bool,
) -> String {
    let from_env = [&env.git_editor, &env.visual, &env.editor]
        .into_iter()
        .flatten()
        .map(|value| value.trim())
        .find(|value| !value.is_empty());
    if let Some(editor) = from_env {
        return editor.to_string();
    }
    if on_path("editor") {
        return "editor".to_string();
    }
    let system_editor = Path::new("/usr/bin/editor");
    if is_executable(system_editor) {
        return system_editor.display().to_string();
    }
    ["vi", "nano"]
        .into_iter()
        .find(|name| on_path(name))
        .unwrap_or("vi")
        .to_string()
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
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
            eprintln!(
                "diffv: ignoring invalid config {}: {:#}",
                path.display(),
                err
            );
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
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn env(git_editor: Option<&str>, visual: Option<&str>, editor: Option<&str>) -> EditorEnv {
        EditorEnv {
            git_editor: git_editor.map(String::from),
            visual: visual.map(String::from),
            editor: editor.map(String::from),
        }
    }

    fn resolve(env: &EditorEnv, on_path: &[&str], executables: &[&str]) -> String {
        editor_command_from(
            env,
            |name| on_path.contains(&name),
            |path| executables.iter().any(|e| Path::new(e) == path),
        )
    }

    #[test]
    fn editor_env_vars_follow_gits_order() {
        let all = ["editor", "vi", "nano"];
        assert_eq!(
            resolve(&env(Some("hx"), Some("code -w"), Some("nano")), &all, &[]),
            "hx"
        );
        assert_eq!(
            resolve(&env(None, Some("code -w"), Some("nano")), &all, &[]),
            "code -w"
        );
        assert_eq!(
            resolve(&env(None, None, Some(" micro ")), &all, &[]),
            "micro"
        );
        // Whitespace-only values are ignored.
        assert_eq!(
            resolve(&env(Some("  "), Some(""), Some("emacs")), &all, &[]),
            "emacs"
        );
    }

    #[test]
    fn without_env_vars_the_system_default_editor_is_used() {
        let none = env(None, None, None);
        assert_eq!(resolve(&none, &["editor", "vi", "nano"], &[]), "editor");
        // Not on the PATH, but the alternatives link exists.
        assert_eq!(
            resolve(&none, &["vi"], &["/usr/bin/editor"]),
            "/usr/bin/editor"
        );
    }

    #[test]
    fn falls_back_to_vi_then_nano_then_the_vi_name() {
        let none = env(Some(" "), None, None);
        assert_eq!(resolve(&none, &["vi", "nano"], &[]), "vi");
        assert_eq!(resolve(&none, &["nano"], &[]), "nano");
        assert_eq!(resolve(&none, &[], &[]), "vi");
    }

    #[test]
    fn executable_check_needs_an_executable_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ed");
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(!is_executable(&file));
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(is_executable(&file));
        assert!(!is_executable(dir.path()));
        assert!(!is_executable(&dir.path().join("missing")));
    }
}
