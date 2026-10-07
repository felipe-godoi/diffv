//! View scrolling that is independent of the cursor (mouse wheel, Ctrl+e / Ctrl+y):
//! the view offset moves, and the cursor is only dragged along when it would
//! leave the visible window, keeping a small `scrolloff`-style margin.

/// Lines kept between the cursor and the window edge when the view drags it.
pub const SCROLLOFF: usize = 2;

/// Lines moved per mouse wheel notch.
pub const WHEEL_STEP: usize = 3;

/// Moves a view offset by `delta`, clamped to `0..=max_offset`.
pub fn scroll_offset(offset: usize, delta: isize, max_offset: usize) -> usize {
    offset.saturating_add_signed(delta).min(max_offset)
}

/// Largest offset for a list of `total` rows shown `visible` at a time.
pub fn max_list_offset(total: usize, visible: usize) -> usize {
    total.saturating_sub(visible.max(1))
}

/// Pulls `cursor` into the window `[offset, offset + visible)` with `margin` rows of
/// breathing room. The margin shrinks on small windows and is dropped at the
/// content edges, so the first and last rows stay reachable.
pub fn clamp_cursor(
    cursor: usize,
    offset: usize,
    visible: usize,
    total: usize,
    margin: usize,
) -> usize {
    if total == 0 {
        return 0;
    }
    let visible = visible.max(1);
    let margin = margin.min((visible - 1) / 2);
    let last = total - 1;
    let low = if offset == 0 { 0 } else { offset + margin };
    let high = if offset + visible > last {
        last
    } else {
        offset + visible - 1 - margin
    };
    cursor.clamp(low.min(last), high.max(low).min(last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_moves_by_delta_and_clamps_to_limits() {
        assert_eq!(scroll_offset(0, 3, 10), 3);
        assert_eq!(scroll_offset(9, 3, 10), 10);
        assert_eq!(scroll_offset(2, -3, 10), 0);
        assert_eq!(max_list_offset(50, 20), 30);
        assert_eq!(max_list_offset(5, 20), 0);
        assert_eq!(max_list_offset(5, 0), 4);
    }

    #[test]
    fn cursor_inside_the_window_does_not_move() {
        assert_eq!(clamp_cursor(15, 10, 20, 100, SCROLLOFF), 15);
    }

    #[test]
    fn cursor_is_dragged_with_a_margin_when_the_view_passes_it() {
        // Scrolling down past the cursor pushes it to the top edge + margin.
        assert_eq!(clamp_cursor(5, 10, 20, 100, SCROLLOFF), 12);
        // Scrolling up past the cursor pulls it to the bottom edge - margin.
        assert_eq!(clamp_cursor(50, 10, 20, 100, SCROLLOFF), 27);
    }

    #[test]
    fn margin_is_dropped_at_content_edges_and_on_tiny_windows() {
        // At the top of the content the first row is reachable.
        assert_eq!(clamp_cursor(0, 0, 20, 100, SCROLLOFF), 0);
        // At the bottom the last row is reachable.
        assert_eq!(clamp_cursor(99, 80, 20, 100, SCROLLOFF), 99);
        assert_eq!(clamp_cursor(70, 80, 20, 100, SCROLLOFF), 82);
        // A 1-row window has no room for a margin.
        assert_eq!(clamp_cursor(3, 7, 1, 100, SCROLLOFF), 7);
        // Short content: everything is visible, cursor stays put but in range.
        assert_eq!(clamp_cursor(9, 0, 20, 5, SCROLLOFF), 4);
        assert_eq!(clamp_cursor(0, 0, 20, 0, SCROLLOFF), 0);
    }
}
