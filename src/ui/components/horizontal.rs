use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

// Scroll styled text by terminal columns, keeping the gutter and diff markers fixed.
pub fn scroll_line(mut line: Line<'_>, gutter_spans: usize, mut offset: usize) -> Line<'_> {
    if offset == 0 {
        return line;
    }
    let mut spans = Vec::new();
    for (idx, span) in line.spans.into_iter().enumerate() {
        if idx < gutter_spans {
            spans.push(span);
            continue;
        }
        let mut text = String::new();
        for ch in span.content.chars() {
            let width = ch.width().unwrap_or(0);
            if offset >= width && offset > 0 {
                offset -= width;
                continue;
            }
            if offset > 0 {
                text.push_str(&" ".repeat(width - offset));
                offset = 0;
            } else {
                text.push(ch);
            }
        }
        spans.push(Span::styled(text, span.style));
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
        let shifted = scroll_line(line, 1, 2);
        assert_eq!(shifted.to_string(), "12 │ éxyz");
    }
}

pub fn wrap_line(line: Line<'_>, gutter_spans: usize, width: usize) -> Vec<Line<'_>> {
    let gutter: Vec<_> = line.spans.iter().take(gutter_spans).cloned().collect();
    let gutter_width: usize = gutter.iter().map(Span::width).sum();
    let available = width.saturating_sub(gutter_width).max(1);
    let mut result = Vec::new();
    let mut spans = gutter.clone();
    let mut used = 0;
    for span in line.spans.into_iter().skip(gutter_spans) {
        for ch in span.content.chars() {
            let text = if ch == '\t' {
                "    ".to_string()
            } else {
                ch.to_string()
            };
            for ch in text.chars() {
                let cells = ch.width().unwrap_or(0);
                if used + cells > available && used > 0 {
                    result.push(Line::from(spans));
                    spans = gutter
                        .iter()
                        .map(|s| Span::styled(" ".repeat(s.width()), s.style))
                        .collect();
                    used = 0;
                }
                if let Some(last) = spans
                    .last_mut()
                    .filter(|s| s.style == span.style && used > 0)
                {
                    last.content.to_mut().push(ch);
                } else {
                    spans.push(Span::styled(ch.to_string(), span.style));
                }
                used += cells;
            }
        }
    }
    result.push(Line::from(spans));
    result
}

#[cfg(test)]
mod wrap_tests {
    use super::*;
    #[test]
    fn wrap_preserves_all_text_and_hides_repeated_gutter() {
        let line = Line::from(vec![Span::raw("1 │ "), Span::raw("ab界éxyz")]);
        let rows = wrap_line(line, 1, 8);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].to_string(), "1 │ ab界");
        assert_eq!(rows[1].to_string(), "    éxyz");
    }
}
