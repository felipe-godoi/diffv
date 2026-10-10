use once_cell::sync::Lazy;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color as SynColor, Theme as SynTheme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

pub static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
pub static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

/// Lines longer than this (minified bundles, lockfiles, generated data) are shown
/// without syntax colors: syntect's regexes get super-linear on them and a single
/// such line would stall every frame that shows it.
pub const MAX_HIGHLIGHT_LEN: usize = 1_000;

static THEME: Lazy<&'static SynTheme> = Lazy::new(|| {
    let theme_key = if crate::ui::theme::detect_dark_mode() {
        "base16-ocean.dark"
    } else {
        "base16-ocean.light"
    };
    THEME_SET
        .themes
        .get(theme_key)
        .unwrap_or(&THEME_SET.themes["base16-ocean.dark"])
});

/// Cache keyed by (syntax, line hash): the same line in two files of the same
/// language shares its tokens.
type TokenCache = HashMap<(usize, u64), Arc<[SyntaxToken]>>;
static HIGHLIGHT_CACHE: Lazy<Mutex<TokenCache>> =
    Lazy::new(|| Mutex::new(HashMap::with_capacity(8192)));
const HIGHLIGHT_CACHE_CAP: usize = 16_384;

/// `find_syntax_by_extension` scans every syntax definition, so the lookup is
/// resolved once per extension / file name.
static SYNTAX_BY_NAME: Lazy<Mutex<HashMap<OsString, usize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxToken {
    pub start: usize,
    pub end: usize,
    pub fg_color: (u8, u8, u8),
}

pub struct SyntaxHighlighter {
    _private: (),
}

fn syntax_index(path: &Path) -> usize {
    let key = path
        .extension()
        .or_else(|| path.file_name())
        .map(OsString::from)
        .unwrap_or_default();
    if let Some(&idx) = SYNTAX_BY_NAME.lock().ok().as_ref().and_then(|m| m.get(&key)) {
        return idx;
    }
    let syntax: &SyntaxReference = path
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| SYNTAX_SET.find_syntax_by_extension(ext))
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| SYNTAX_SET.find_syntax_by_token(name))
        })
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let idx = SYNTAX_SET
        .syntaxes()
        .iter()
        .position(|s| std::ptr::eq(s, syntax))
        .unwrap_or(0);
    if let Ok(mut map) = SYNTAX_BY_NAME.lock() {
        map.insert(key, idx);
    }
    idx
}

impl SyntaxHighlighter {
    /// Tokens of one line; empty for empty or overly long lines (render them plain).
    pub fn highlight_line(path: &Path, line: &str) -> Arc<[SyntaxToken]> {
        if line.is_empty() || line.len() > MAX_HIGHLIGHT_LEN {
            return Arc::from([]);
        }

        let syntax_idx = syntax_index(path);
        let mut hasher = DefaultHasher::new();
        line.hash(&mut hasher);
        let key = (syntax_idx, hasher.finish());

        if let Ok(cache) = HIGHLIGHT_CACHE.lock() {
            if let Some(tokens) = cache.get(&key) {
                return tokens.clone();
            }
        }

        let syntax = &SYNTAX_SET.syntaxes()[syntax_idx];
        let mut highlighter = HighlightLines::new(syntax, &THEME);

        let mut tokens = Vec::new();
        let mut offset = 0;

        // syntect's newline syntaxes expect the line terminator.
        let mut line_with_nl = String::with_capacity(line.len() + 1);
        line_with_nl.push_str(line);
        if !line.ends_with('\n') {
            line_with_nl.push('\n');
        }

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

        let tokens: Arc<[SyntaxToken]> = tokens.into();
        if let Ok(mut cache) = HIGHLIGHT_CACHE.lock() {
            if cache.len() > HIGHLIGHT_CACHE_CAP {
                cache.clear();
            }
            cache.insert(key, tokens.clone());
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_cover_the_line_and_are_cached_per_language() {
        let line = "let x = 42;";
        let a = SyntaxHighlighter::highlight_line(Path::new("a.rs"), line);
        assert!(!a.is_empty());
        assert_eq!(a.last().unwrap().end, line.len());
        // Another Rust file reuses the cached tokens.
        let b = SyntaxHighlighter::highlight_line(Path::new("dir/b.rs"), line);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn huge_lines_are_left_plain() {
        let line = "x".repeat(MAX_HIGHLIGHT_LEN + 1);
        assert!(SyntaxHighlighter::highlight_line(Path::new("a.js"), &line).is_empty());
    }
}
