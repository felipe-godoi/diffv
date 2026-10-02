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
    let themes = ["auto", "terminal", "vscode-dark", "tokyonight", "catppuccin", "gruvbox"];
    for theme_name in themes {
        let theme = Theme::from_name(theme_name);
        assert!(!theme.name.is_empty());
    }
}
