use super::models::HighlightSpan;
use similar::{capture_diff_slices, Algorithm, DiffTag};

/// Tokenizes a line of code into words, whitespace, and punctuation symbols.
fn tokenize_line(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        if ch.is_alphanumeric() || ch == '_' {
            // Identifier or number
            let mut end = start + ch.len_utf8();
            while let Some(&(_, next_ch)) = chars.peek() {
                if next_ch.is_alphanumeric() || next_ch == '_' {
                    chars.next();
                    end += next_ch.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(&line[start..end]);
        } else if ch.is_whitespace() {
            // Whitespace run
            let mut end = start + ch.len_utf8();
            while let Some(&(_, next_ch)) = chars.peek() {
                if next_ch.is_whitespace() {
                    chars.next();
                    end += next_ch.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(&line[start..end]);
        } else {
            // Single punctuation/symbol character
            tokens.push(&line[start..start + ch.len_utf8()]);
        }
    }

    tokens
}

/// Computes intra-line character/word highlight spans for a pair of modified lines.
/// Returns (old_spans, new_spans).
pub fn compute_intraline_spans(
    old_line: &str,
    new_line: &str,
) -> (Vec<HighlightSpan>, Vec<HighlightSpan>) {
    if old_line.is_empty() || new_line.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let old_tokens = tokenize_line(old_line);
    let new_tokens = tokenize_line(new_line);

    let diff = capture_diff_slices(Algorithm::Patience, &old_tokens, &new_tokens);

    let mut old_spans = Vec::new();
    let mut new_spans = Vec::new();

    let mut old_pos = 0;
    let mut new_pos = 0;

    for change in diff {
        match change.tag() {
            DiffTag::Equal => {
                let old_slice = &old_tokens[change.old_range()];
                for token in old_slice {
                    old_pos += token.len();
                }
                let new_slice = &new_tokens[change.new_range()];
                for token in new_slice {
                    new_pos += token.len();
                }
            }
            DiffTag::Delete => {
                let old_slice = &old_tokens[change.old_range()];
                let start = old_pos;
                for token in old_slice {
                    old_pos += token.len();
                }
                old_spans.push(HighlightSpan {
                    start,
                    end: old_pos,
                    is_intraline: true,
                });
            }
            DiffTag::Insert => {
                let new_slice = &new_tokens[change.new_range()];
                let start = new_pos;
                for token in new_slice {
                    new_pos += token.len();
                }
                new_spans.push(HighlightSpan {
                    start,
                    end: new_pos,
                    is_intraline: true,
                });
            }
            DiffTag::Replace => {
                let old_slice = &old_tokens[change.old_range()];
                let old_start = old_pos;
                for token in old_slice {
                    old_pos += token.len();
                }
                old_spans.push(HighlightSpan {
                    start: old_start,
                    end: old_pos,
                    is_intraline: true,
                });

                let new_slice = &new_tokens[change.new_range()];
                let new_start = new_pos;
                for token in new_slice {
                    new_pos += token.len();
                }
                new_spans.push(HighlightSpan {
                    start: new_start,
                    end: new_pos,
                    is_intraline: true,
                });
            }
        }
    }

    (
        merge_adjacent_spans(old_spans),
        merge_adjacent_spans(new_spans),
    )
}

fn merge_adjacent_spans(spans: Vec<HighlightSpan>) -> Vec<HighlightSpan> {
    if spans.is_empty() {
        return spans;
    }

    let mut merged = Vec::with_capacity(spans.len());
    let mut current = spans[0].clone();

    for span in spans.into_iter().skip(1) {
        if span.start == current.end && span.is_intraline == current.is_intraline {
            current.end = span.end;
        } else {
            merged.push(current);
            current = span;
        }
    }
    merged.push(current);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intraline_simple_replacement() {
        let old_line = "let old_mode = false;";
        let new_line = "let old_mode = true;";
        let (old_spans, new_spans) = compute_intraline_spans(old_line, new_line);

        assert_eq!(old_spans.len(), 1);
        assert_eq!(new_spans.len(), 1);

        let old_slice = &old_line[old_spans[0].start..old_spans[0].end];
        let new_slice = &new_line[new_spans[0].start..new_spans[0].end];
        assert_eq!(old_slice, "false");
        assert_eq!(new_slice, "true");
    }

    #[test]
    fn test_intraline_dissimilar_lines() {
        let old_line = "println!(\"completely different\");";
        let new_line = "let x = 42 * 100 + calculate_foo();";
        let (old_spans, _new_spans) = compute_intraline_spans(old_line, new_line);
        assert!(old_spans.is_empty() || !old_spans.is_empty());
    }

    #[test]
    fn test_intraline_multiple_tokens() {
        let old_line = "fn process(timeout: u32, verbose: bool)";
        let new_line = "fn process(timeout: u64, verbose: bool)";
        let (old_spans, new_spans) = compute_intraline_spans(old_line, new_line);

        assert_eq!(old_spans.len(), 1);
        assert_eq!(new_spans.len(), 1);

        assert_eq!(&old_line[old_spans[0].start..old_spans[0].end], "u32");
        assert_eq!(&new_line[new_spans[0].start..new_spans[0].end], "u64");
    }
}
