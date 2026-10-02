pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

pub fn get_tmux_popup_snippet() -> &'static str {
    r##"# Add to your ~/.tmux.conf:
# Press Prefix + d to open diffv in a floating 92% x 90% popup with live watch mode:
bind-key d display-popup -d "#{pane_current_path}" -w 92% -h 90% -E "diffv --watch"
"##
}
