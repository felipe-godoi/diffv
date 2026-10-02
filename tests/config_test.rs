use diffv::config::Config;
use diffv::ui::theme::Theme;

#[test]
fn test_default_config_parsing() {
    let toml_str = r#"
[ui]
theme = "tokyonight"
default_view = "unified"
show_line_numbers = true
syntax_highlighting = true
overview_ruler = false
tab_width = 2

[diff]
algorithm = "myers"
ignore_whitespace = true
context_lines = 5

[watcher]
enabled = false
debounce_ms = 200
watch_untracked = false

[editor]
command = "code"
args = ["--goto", "{{file}}:{{line}}"]
use_nvr = false
"#;

    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.ui.theme, "tokyonight");
    assert_eq!(cfg.ui.default_view, "unified");
    assert_eq!(cfg.ui.tab_width, 2);
    assert!(!cfg.ui.overview_ruler);

    assert_eq!(cfg.diff.algorithm, "myers");
    assert!(cfg.diff.ignore_whitespace);
    assert_eq!(cfg.diff.context_lines, 5);

    assert!(!cfg.watcher.enabled);
    assert_eq!(cfg.watcher.debounce_ms, 200);

    assert_eq!(cfg.editor.command, "code");
    assert_eq!(cfg.editor.args, vec!["--goto", "{{file}}:{{line}}"]);
    assert!(!cfg.editor.use_nvr);
}

#[test]
fn test_all_themes_loading() {
    let themes = [
        "auto",
        "terminal",
        "vscode-dark",
        "tokyonight",
        "catppuccin",
        "gruvbox",
    ];
    for theme_name in themes {
        let theme = Theme::from_name(theme_name);
        assert!(!theme.name.is_empty());
    }
}

#[test]
fn test_update_config_opt_out_and_channel() {
    use diffv::config::UpdateChannel;

    // 1. Default configuration
    let default_cfg = Config::default();
    assert!(default_cfg.update.auto_update);
    assert_eq!(default_cfg.update.channel, UpdateChannel::Stable);

    // 2. Opt-out of auto-update
    let opt_out_toml = r#"
[update]
auto_update = false
"#;
    let cfg: Config = toml::from_str(opt_out_toml).unwrap();
    assert!(!cfg.update.auto_update);
    assert_eq!(cfg.update.channel, UpdateChannel::Stable);

    // 3. Opt-in to beta channel
    let beta_toml = r#"
[update]
channel = "beta"
"#;
    let cfg_beta: Config = toml::from_str(beta_toml).unwrap();
    assert!(cfg_beta.update.auto_update);
    assert_eq!(cfg_beta.update.channel, UpdateChannel::Beta);

    // 4. Using enabled alias
    let alias_toml = r#"
[update]
enabled = false
channel = "beta"
"#;
    let cfg_alias: Config = toml::from_str(alias_toml).unwrap();
    assert!(!cfg_alias.update.auto_update);
    assert_eq!(cfg_alias.update.channel, UpdateChannel::Beta);

    // 5. Nightly channel and aliases
    let nightly_toml = r#"
[update]
channel = "nightly"
"#;
    let cfg_nightly: Config = toml::from_str(nightly_toml).unwrap();
    assert_eq!(cfg_nightly.update.channel, UpdateChannel::Nightly);

    assert_eq!("rc".parse::<UpdateChannel>().unwrap(), UpdateChannel::Beta);
    assert_eq!(
        "preview".parse::<UpdateChannel>().unwrap(),
        UpdateChannel::Beta
    );
    assert_eq!(
        "dev".parse::<UpdateChannel>().unwrap(),
        UpdateChannel::Nightly
    );
    assert_eq!(
        "canary".parse::<UpdateChannel>().unwrap(),
        UpdateChannel::Nightly
    );
    assert_eq!(
        "main".parse::<UpdateChannel>().unwrap(),
        UpdateChannel::Nightly
    );
}

#[test]
fn test_config_save_persistence() {
    use diffv::config::UpdateChannel;
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    let mut cfg = Config::default();
    cfg.update.auto_update = false;
    cfg.update.channel = UpdateChannel::Beta;
    cfg.ui.theme = "tokyonight".to_string();
    cfg.ui.default_view = "unified".to_string();

    cfg.save_to_path(&config_path).unwrap();
    assert!(config_path.exists());

    let loaded = Config::load_from_path(&config_path).unwrap();
    assert!(!loaded.update.auto_update);
    assert_eq!(loaded.update.channel, UpdateChannel::Beta);
    assert_eq!(loaded.ui.theme, "tokyonight");
    assert_eq!(loaded.ui.default_view, "unified");
}
