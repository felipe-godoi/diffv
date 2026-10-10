use std::borrow::Cow;

use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

/// `&content[start..end]`, borrowing when the span borrows its text.
fn slice_cow<'a>(content: &Cow<'a, str>, start: usize, end: usize) -> Cow<'a, str> {
    match content {
        Cow::Borrowed(s) => Cow::Borrowed(&s[start..end]),
        Cow::Owned(s) => Cow::Owned(s[start..end].to_string()),
    }
}

/// Scroll styled text by terminal columns, keeping the gutter and diff markers fixed.
/// Only `max_width` columns of text are kept, so the cost is bounded by the view and
/// not by the length of the line (minified files can have megabyte-long lines).
pub fn scroll_line(
    mut line: Line<'_>,
    gutter_spans: usize,
    mut offset: usize,
    max_width: usize,
) -> Line<'_> {
    let mut spans = Vec::with_capacity(line.spans.len());
    let mut used = 0usize;
    for (idx, span) in line.spans.into_iter().enumerate() {
        if idx < gutter_spans {
            spans.push(span);
            continue;
        }
        if used >= max_width {
            break;
        }
        let mut start = None;
        let mut end = span.content.len();
        for (byte, ch) in span.content.char_indices() {
            let width = ch.width().unwrap_or(0);
            if start.is_none() {
                if offset >= width && offset > 0 {
                    offset -= width;
                    continue;
                }
                if offset > 0 {
                    // Scrolled into the middle of a wide char: show its remaining half.
                    let pad = width - offset;
                    offset = 0;
                    spans.push(Span::styled(" ".repeat(pad), span.style));
                    used += pad;
                    start = Some(byte + ch.len_utf8());
                    continue;
                }
                start = Some(byte);
            }
            if used >= max_width {
                end = byte;
                break;
            }
            used += width;
        }
        if let Some(start) = start.filter(|&s| s < end) {
            spans.push(Span::styled(
                slice_cow(&span.content, start, end),
                span.style,
            ));
        }
    }
    line.spans = spans;
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scrolling_keeps_gutter_and_handles_wide_characters() {
        let line = Line::from(vec![Span::raw("12 │"), Span::raw("a界"), Span::raw("éxyz")]);
        let shifted = scroll_line(line, 1, 2, usize::MAX);
        assert_eq!(shifted.to_string(), "12 │ éxyz");
    }

    #[test]
    fn scrolling_only_keeps_the_visible_width() {
        let line = Line::from(vec![Span::raw("1 │"), Span::raw("abc"), Span::raw("defgh")]);
        assert_eq!(scroll_line(line.clone(), 1, 0, 4).to_string(), "1 │abcd");
        assert_eq!(scroll_line(line, 1, 2, 4).to_string(), "1 │cdef");
        let huge = "x".repeat(1_000_000);
        let line = Line::from(vec![Span::raw("1 │"), Span::raw(huge.as_str())]);
        assert_eq!(scroll_line(line, 1, 10, 5).width(), 3 + 5);
    }
}

/// Wraps styled text at `width` columns, repeating the gutter as blank space.
/// At most `max_rows` rows are produced: rows below the view are never shown.
pub fn wrap_line(
    line: Line<'_>,
    gutter_spans: usize,
    width: usize,
    max_rows: usize,
) -> Vec<Line<'_>> {
    let max_rows = max_rows.max(1);
    let gutter: Vec<_> = line.spans.iter().take(gutter_spans).cloned().collect();
    let gutter_width: usize = gutter.iter().map(Span::width).sum();
    let available = width.saturating_sub(gutter_width).max(1);
    let blank_gutter = || -> Vec<Span<'_>> {
        gutter
            .iter()
            .map(|s| Span::styled(" ".repeat(s.width()), s.style))
            .collect()
    };
    let mut result = Vec::new();
    let mut spans = gutter.clone();
    let mut used = 0;
    'spans: for span in line.spans.into_iter().skip(gutter_spans) {
        // Text of the current row taken from this span, as a byte range.
        let mut seg_start = 0;
        for (byte, ch) in span.content.char_indices() {
            let (cells, count) = if ch == '\t' {
                (1, 4)
            } else {
                (ch.width().unwrap_or(0), 1)
            };
            if ch == '\t' && seg_start < byte {
                spans.push(Span::styled(
                    slice_cow(&span.content, seg_start, byte),
                    span.style,
                ));
            }
            for _ in 0..count {
                if used + cells > available && used > 0 {
                    if ch != '\t' && seg_start < byte {
                        spans.push(Span::styled(
                            slice_cow(&span.content, seg_start, byte),
                            span.style,
                        ));
                    }
                    result.push(Line::from(std::mem::replace(&mut spans, blank_gutter())));
                    used = 0;
                    seg_start = byte;
                    if result.len() >= max_rows {
                        break 'spans;
                    }
                }
                if ch == '\t' {
                    spans.push(Span::styled(" ", span.style));
                }
                used += cells;
            }
            if ch == '\t' {
                seg_start = byte + 1;
            }
        }
        if seg_start < span.content.len() {
            spans.push(Span::styled(
                slice_cow(&span.content, seg_start, span.content.len()),
                span.style,
            ));
        }
    }
    if result.len() < max_rows {
        result.push(Line::from(spans));
    }
    result
}

#[cfg(test)]
mod wrap_tests {
    use super::*;
    #[test]
    fn wrap_preserves_all_text_and_hides_repeated_gutter() {
        let line = Line::from(vec![Span::raw("1 │ "), Span::raw("ab界éxyz")]);
        let rows = wrap_line(line, 1, 8, usize::MAX);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].to_string(), "1 │ ab界");
        assert_eq!(rows[1].to_string(), "    éxyz");
    }

    #[test]
    fn wrap_expands_tabs_and_splits_across_spans() {
        let line = Line::from(vec![Span::raw("1|"), Span::raw("a\tb"), Span::raw("cdefg")]);
        let rows = wrap_line(line, 1, 6, usize::MAX);
        let text: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
        assert_eq!(text, ["1|a   ", "   bcd", "  efg"]);
    }

    #[test]
    fn wrap_stops_at_max_rows() {
        let huge = "x".repeat(1_000_000);
        let line = Line::from(vec![Span::raw("1|"), Span::raw(huge.as_str())]);
        let rows = wrap_line(line, 1, 12, 3);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.width() == 12));
    }
}
