use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::core::models::{FileDiff, Language};
use crate::ui::app::FzfRequest;
use crate::ui::components::picker_preview::{build_preview, render_preview};
use crate::ui::components::style::{centered_rect, help_line, render_card};
use crate::ui::theme::Theme;

/// Built-in fallback for the fzf searches (Ctrl+p / Ctrl+f) when fzf is not installed.
/// It receives the same candidates as fzf and hands the chosen one back unchanged.
#[derive(Debug, Clone)]
pub struct PickerState {
    pub request: FzfRequest,
    pub items: Vec<String>,
    pub header: String,
    pub query: String,
    pub matches: Vec<usize>,
    /// Query `matches` was computed for, to narrow them when the query only grows.
    matched_query: String,
    pub selected: usize,
    pub scroll: usize,
    /// Diffs of the searched scope, used by the preview pane (`None`: the app's
    /// loaded files, passed in when rendering).
    pub files: Option<Vec<FileDiff>>,
    pub show_preview: bool,
}

/// Minimum popup inner width for showing the preview next to the list.
pub const PREVIEW_MIN_WIDTH: u16 = 70;

impl PickerState {
    pub fn new(request: FzfRequest, items: Vec<String>, header: String) -> Self {
        let mut state = Self {
            request,
            items,
            header,
            query: String::new(),
            matches: Vec::new(),
            matched_query: String::new(),
            selected: 0,
            scroll: 0,
            files: None,
            show_preview: true,
        };
        state.refilter();
        state
    }

    pub fn refilter(&mut self) {
        // Typing one more char can only drop items (no typos, AND of terms), so only
        // the current matches need another look. Negations and `$` anchors can
        // widen the result while typed, so those queries start over.
        let narrows = !self.matched_query.is_empty()
            && self.query.starts_with(&self.matched_query)
            && !self.query.contains(['!', '$']);
        self.matches = if narrows {
            let mut candidates = std::mem::take(&mut self.matches);
            candidates.sort_unstable();
            filter_subset(&self.items, &candidates, &self.query, self.request)
        } else {
            filter_items(&self.items, &self.query, self.request)
        };
        self.matched_query = self.query.clone();
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn move_by(&mut self, delta: isize) {
        let last = self.matches.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
    }

    pub fn selected_item(&self) -> Option<&String> {
        self.matches.get(self.selected).map(|&i| &self.items[i])
    }
}

/// What a key does in the picker. Follows fzf's default bindings: Esc, Ctrl+C and
/// Ctrl+Q abort; every printable key (including Shift+Q) is query text. Ctrl+G is
/// not an exit key here (unlike fzf's default): it switches the search to fzf,
/// carrying the typed query along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerAction {
    Cancel,
    Accept,
    Move(isize),
    ClearQuery,
    Backspace,
    Type(char),
    /// Ctrl+G: continue this search in the other engine (fzf) with the same query.
    SwitchEngine,
    /// Ctrl+T (or Ctrl+/ where the terminal reports it): show / hide the preview.
    TogglePreview,
    Ignore,
}

pub fn picker_key_action(key: KeyEvent) -> PickerAction {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => PickerAction::Cancel,
        KeyCode::Char('c' | 'q') if ctrl => PickerAction::Cancel,
        KeyCode::Enter => PickerAction::Accept,
        KeyCode::Up => PickerAction::Move(-1),
        KeyCode::Down => PickerAction::Move(1),
        KeyCode::Char('p' | 'k') if ctrl => PickerAction::Move(-1),
        KeyCode::Char('n' | 'j') if ctrl => PickerAction::Move(1),
        KeyCode::PageUp => PickerAction::Move(-10),
        KeyCode::PageDown => PickerAction::Move(10),
        KeyCode::Char('u') if ctrl => PickerAction::ClearQuery,
        KeyCode::Char('g') if ctrl => PickerAction::SwitchEngine,
        // Ctrl+/ is fzf's preview toggle, but terminals report it inconsistently
        // (often as Ctrl+7 or not at all), so Ctrl+T is the documented key.
        KeyCode::Char('t' | '/' | '7') if ctrl => PickerAction::TogglePreview,
        KeyCode::Backspace => PickerAction::Backspace,
        KeyCode::Char(c) if !ctrl => PickerAction::Type(c),
        _ => PickerAction::Ignore,
    }
}

/// Text candidates are `path:line<TAB>content[<TAB>staged]`; like fzf `--nth=2`
/// only the content is matched.
fn match_field(item: &str, request: FzfRequest) -> &str {
    match request {
        FzfRequest::Files => item,
        FzfRequest::Text => item.split('\t').nth(1).unwrap_or(item),
    }
}

/// Matcher settings: smart case and no typos, like fzf, with its extended syntax
/// (`'exact`, `^prefix`, `suffix$`, `!negation`; whitespace separates terms that
/// must all match).
fn matcher_config() -> frizbee::Config {
    frizbee::Config::default()
        .max_typos(Some(0))
        .casing(frizbee::CaseMatching::Smart)
}

/// Indices of the items matching every whitespace-separated term of `query`, best
/// first; ties (and an empty query) keep the original order.
///
/// Matching runs on frizbee — the SIMD Smith-Waterman engine behind fff — split
/// over all cores for big lists, so it keeps up with fzf on 100k+ candidates.
pub fn filter_items(items: &[String], query: &str, request: FzfRequest) -> Vec<usize> {
    let all: Vec<usize> = (0..items.len()).collect();
    filter_subset(items, &all, query, request)
}

/// `filter_items` over the items listed in `subset` (ascending indices).
fn filter_subset(
    items: &[String],
    subset: &[usize],
    query: &str,
    request: FzfRequest,
) -> Vec<usize> {
    if query.split_whitespace().next().is_none() {
        return subset.to_vec();
    }
    let fields: Vec<&str> = subset
        .iter()
        .map(|&idx| match_field(&items[idx], request))
        .collect();
    let mut matcher = frizbee::Matcher::from_query(query, &matcher_config());
    let mut matches: Vec<(u16, usize, usize)> = matcher
        .match_list_parallel(&fields, 0)
        .into_iter()
        .map(|m| {
            (
                m.score,
                fields[m.index as usize].len(),
                subset[m.index as usize],
            )
        })
        .collect();
    // Like fzf's default tiebreak: best score, then the shorter candidate, then
    // the original order.
    matches.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    matches.into_iter().map(|(_, _, idx)| idx).collect()
}

/// `path:line  content  [staged]` for text candidates, the path for files.
fn display_item(item: &str, request: FzfRequest) -> String {
    match request {
        FzfRequest::Files => item.to_string(),
        FzfRequest::Text => {
            let mut fields = item.split('\t');
            let location = fields.next().unwrap_or_default();
            let content = fields.next().unwrap_or_default();
            match fields.next() {
                Some(section) => format!("{}  {}  [{}]", location, content, section),
                None => format!("{}  {}", location, content),
            }
        }
    }
}

pub fn render_picker_popup(
    frame: &mut Frame,
    area: Rect,
    state: &mut PickerState,
    loaded_files: &[FileDiff],
    language: Language,
    theme: &Theme,
) {
    let popup_area = centered_rect(76, 70, area);
    let title = match (state.request, language) {
        (FzfRequest::Files, Language::En) => "Find File",
        (FzfRequest::Files, Language::Pt) => "Buscar Arquivo",
        (FzfRequest::Text, Language::En) => "Search Diff Text",
        (FzfRequest::Text, Language::Pt) => "Buscar Texto no Diff",
    };
    let inner = render_card(frame, popup_area, "󰈞", title, theme.header_fg, theme);
    if inner.height < 6 {
        return;
    }

    let [header_area, search_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", state.header),
            Style::default().fg(theme.line_num_fg),
        )),
        header_area,
    );

    let search_block = Block::default()
        .title(Span::styled(
            format!(" {}/{} ", state.matches.len(), state.items.len()),
            Style::default()
                .fg(theme.key_fg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.header_fg));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ❯ ", Style::default().fg(theme.key_fg)),
            Span::styled(
                &state.query,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(theme.header_fg)),
        ]))
        .block(search_block),
        search_area,
    );

    let (list_area, preview_area) = if state.show_preview && list_area.width >= PREVIEW_MIN_WIDTH {
        let [list, preview] =
            Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
                .areas(list_area);
        (list, Some(preview))
    } else {
        (list_area, None)
    };
    if let Some(area) = preview_area {
        let preview = build_preview(
            state.files.as_deref().unwrap_or(loaded_files),
            state.request,
            state.selected_item().map(String::as_str),
            &state.query,
            area.height.saturating_sub(2) as usize,
        );
        render_preview(frame, area, &preview, language, theme);
    }

    let max_rows = list_area.height as usize;
    if state.matches.is_empty() {
        let msg = match language {
            Language::En => "  No matches.",
            Language::Pt => "  Nenhum resultado.",
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::default().fg(theme.line_num_fg)),
            list_area,
        );
    } else {
        if state.selected < state.scroll {
            state.scroll = state.selected;
        } else if state.selected >= state.scroll + max_rows {
            state.scroll = state.selected + 1 - max_rows;
        }
        let lines: Vec<Line> = state
            .matches
            .iter()
            .enumerate()
            .skip(state.scroll)
            .take(max_rows)
            .map(|(pos, &idx)| {
                let text = display_item(&state.items[idx], state.request);
                if pos == state.selected {
                    Line::from(vec![
                        Span::styled(
                            " ❯ ",
                            Style::default()
                                .fg(theme.key_fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            text,
                            Style::default()
                                .fg(theme.fg)
                                .bg(theme.selected_bg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(text, Style::default().fg(theme.fg)),
                    ])
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), list_area);
    }

    let footer = match language {
        Language::En => [
            ("↑/↓", "move"),
            ("enter", "open"),
            ("esc", "cancel"),
            ("ctrl+t", "preview"),
            ("ctrl+g", "fzf"),
        ],
        Language::Pt => [
            ("↑/↓", "mover"),
            ("enter", "abrir"),
            ("esc", "cancelar"),
            ("ctrl+t", "preview"),
            ("ctrl+g", "fzf"),
        ],
    };
    frame.render_widget(
        Paragraph::new(help_line(&footer, footer_area.width, theme)),
        footer_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_query_keeps_every_item_in_order() {
        let files = items(&["b.rs", "a.rs", "c.rs"]);
        assert_eq!(filter_items(&files, "", FzfRequest::Files), vec![0, 1, 2]);
        assert_eq!(
            filter_items(&files, "   ", FzfRequest::Files),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn substring_matches_rank_before_fuzzy_ones() {
        let files = items(&[
            "src/ui/app_main.rs",
            "src/main.rs",
            "docs/notes.md",
            "src/m_a_i_n.rs",
        ]);
        // "src/main.rs" has "main" earlier than "app_main"; "m_a_i_n" only fuzzily.
        assert_eq!(
            filter_items(&files, "main", FzfRequest::Files),
            vec![1, 0, 3]
        );
        assert_eq!(
            filter_items(&files, "zzz", FzfRequest::Files),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn every_term_must_match_and_case_is_smart() {
        let files = items(&["src/ui/App.rs", "src/ui/app.rs", "tests/app_test.rs"]);
        // Both match; the one whose case matches the query ranks first.
        assert_eq!(
            filter_items(&files, "ui app", FzfRequest::Files),
            vec![1, 0]
        );
        // An uppercase letter makes the search case-sensitive.
        assert_eq!(filter_items(&files, "App", FzfRequest::Files), vec![0]);
        let cargo = items(&["Cargo.toml"]);
        assert_eq!(filter_items(&cargo, "ctl", FzfRequest::Files), vec![0]);
        assert!(filter_items(&cargo, "cT", FzfRequest::Files).is_empty());
    }

    #[test]
    fn text_search_only_matches_the_line_content() {
        let lines = items(&[
            "src/main.rs:10\t+ let parser = Cli::parse();",
            "src/parser.rs:3\t  fn run() {}\tstaged",
        ]);
        // "parser" is in the second path but only in the first line's content.
        assert_eq!(filter_items(&lines, "parser", FzfRequest::Text), vec![0]);
        assert_eq!(filter_items(&lines, "run", FzfRequest::Text), vec![1]);
        assert_eq!(
            display_item(&lines[1], FzfRequest::Text),
            "src/parser.rs:3    fn run() {}  [staged]"
        );
    }

    #[test]
    fn extended_query_syntax_works_like_fzf() {
        let files = items(&["src/main.rs", "src/domain.rs", "tests/main_test.rs"]);
        // "domain" contains "main" too; the negation drops the test file.
        assert_eq!(
            filter_items(&files, "'main !test", FzfRequest::Files),
            vec![0, 1]
        );
        assert_eq!(filter_items(&files, "^tests", FzfRequest::Files), vec![2]);
        assert_eq!(
            filter_items(&files, "/main.rs$", FzfRequest::Files),
            vec![0]
        );
    }

    #[test]
    fn growing_query_narrows_previous_matches_and_matches_a_fresh_filter() {
        let files: Vec<String> = (0..5_000)
            .map(|i| format!("src/module_{i}/file_{}.rs", i % 7))
            .collect();
        let mut state = PickerState::new(FzfRequest::Files, files.clone(), String::new());
        for query in [
            "m",
            "mo",
            "mod 1",
            "mod 12",
            "mod 12 file_3",
            "mod 12 !file_3",
        ] {
            state.query = query.into();
            state.refilter();
            assert_eq!(
                state.matches,
                filter_items(&files, query, FzfRequest::Files),
                "query {query:?}"
            );
        }
        state.query = "mod 1".into();
        state.refilter();
        assert_eq!(
            state.matches,
            filter_items(&files, "mod 1", FzfRequest::Files)
        );
    }

    #[test]
    fn exit_keys_match_fzf_defaults() {
        let key = |code, modifiers| picker_key_action(KeyEvent::new(code, modifiers));
        let ctrl = KeyModifiers::CONTROL;
        assert_eq!(key(KeyCode::Esc, KeyModifiers::NONE), PickerAction::Cancel);
        for c in ['c', 'q'] {
            assert_eq!(key(KeyCode::Char(c), ctrl), PickerAction::Cancel);
        }
        for c in ['t', '/'] {
            assert_eq!(key(KeyCode::Char(c), ctrl), PickerAction::TogglePreview);
        }
        // Plain t is query text.
        assert_eq!(
            key(KeyCode::Char('t'), KeyModifiers::NONE),
            PickerAction::Type('t')
        );
        // Ctrl+G switches the search engine instead of closing the picker.
        assert_eq!(key(KeyCode::Char('g'), ctrl), PickerAction::SwitchEngine);
        // Like in fzf, Shift+Q (and q) are just query text.
        assert_eq!(
            key(KeyCode::Char('Q'), KeyModifiers::SHIFT),
            PickerAction::Type('Q')
        );
        assert_eq!(
            key(KeyCode::Char('q'), KeyModifiers::NONE),
            PickerAction::Type('q')
        );
        assert_eq!(
            key(KeyCode::Enter, KeyModifiers::NONE),
            PickerAction::Accept
        );
        assert_eq!(key(KeyCode::Char('n'), ctrl), PickerAction::Move(1));
        assert_eq!(key(KeyCode::Char('x'), ctrl), PickerAction::Ignore);
    }

    #[test]
    fn preview_toggle_flips_and_starts_on() {
        let mut state = PickerState::new(FzfRequest::Files, items(&["a.rs"]), String::new());
        assert!(state.show_preview);
        state.toggle_preview();
        assert!(!state.show_preview);
        state.toggle_preview();
        assert!(state.show_preview);
    }

    #[test]
    fn selection_stays_within_matches() {
        let mut state = PickerState::new(
            FzfRequest::Files,
            items(&["a.rs", "b.rs", "ab.rs"]),
            String::new(),
        );
        state.move_by(10);
        assert_eq!(state.selected_item().map(String::as_str), Some("ab.rs"));
        state.move_by(-10);
        assert_eq!(state.selected_item().map(String::as_str), Some("a.rs"));
        state.query = "b".into();
        state.refilter();
        assert_eq!(state.selected_item().map(String::as_str), Some("b.rs"));
        state.query = "q".into();
        state.refilter();
        assert_eq!(state.selected_item(), None);
    }
}
