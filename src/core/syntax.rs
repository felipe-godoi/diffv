use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color as SynColor, ThemeSet};
use syntect::parsing::SyntaxSet;

pub static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
pub static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

static HIGHLIGHT_CACHE: Lazy<Mutex<HashMap<(PathBuf, String), Vec<SyntaxToken>>>> =
    Lazy::new(|| Mutex::new(HashMap::with_capacity(4096)));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxToken {
    pub start: usize,
    pub end: usize,
    pub fg_color: (u8, u8, u8),
}

pub struct SyntaxHighlighter {
    _private: (),
}

impl SyntaxHighlighter {
    pub fn highlight_line(
        path: &Path,
        line: &str,
    ) -> Vec<SyntaxToken> {
        if line.is_empty() {
            return Vec::new();
        }

        let cache_key = (path.to_path_buf(), line.to_string());
        if let Ok(cache) = HIGHLIGHT_CACHE.lock() {
            if let Some(tokens) = cache.get(&cache_key) {
                return tokens.clone();
            }
        }

        let syntax = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| SYNTAX_SET.find_syntax_by_extension(ext))
            .or_else(|| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| SYNTAX_SET.find_syntax_by_token(name))
            })
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let is_dark = crate::ui::theme::detect_dark_mode();
        let theme_key = if is_dark { "base16-ocean.dark" } else { "base16-ocean.light" };
        let theme = THEME_SET.themes.get(theme_key).unwrap_or(&THEME_SET.themes["base16-ocean.dark"]);
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut tokens = Vec::new();
        let mut offset = 0;

        // Ensure string ends with newline for syntect if needed
        let line_with_nl = if line.ends_with('\n') {
            line.to_string()
        } else {
            format!("{}\n", line)
        };

        if let Ok(ranges) = highlighter.highlight_line(&line_with_nl, &SYNTAX_SET) {
            for (style, text) in ranges {
                let text_len = text.trim_end_matches('\n').len();
                if text_len == 0 {
                    continue;
                }
                let SynColor { r, g, b, .. } = style.foreground;
                tokens.push(SyntaxToken {
                    start: offset,
                    end: offset + text_len,
                    fg_color: (r, g, b),
                });
                offset += text_len;
                if offset >= line.len() {
                    break;
                }
            }
        }

        if let Ok(mut cache) = HIGHLIGHT_CACHE.lock() {
            if cache.len() > 8192 {
                cache.clear();
            }
            cache.insert(cache_key, tokens.clone());
        }

        tokens
    }
}
