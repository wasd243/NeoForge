//! Geometry calculations for the selector viewport.
//!
//! Answers "where do the list and the preview pane live, and how big are
//! they" for both preview placements ([`PreviewPlacement::Right`] and
//! [`PreviewPlacement::Bottom`]). These helpers are shared by the renderer
//! (`render`), the event loop (preview scroll clamping) and mouse hit
//! testing so the three never disagree about the layout.

use crossterm::terminal;

use super::types::{PreviewLayout, PreviewPlacement};
use super::utils::{desired_select_viewport_height, select_viewport_height};

/// Returns the maximum preview scroll offset for the current preview
/// content, i.e. the largest offset that still keeps the last preview line
/// visible.
///
/// # Arguments
///
/// * `preview` - Rendered preview text.
/// * `header_rows` - Number of non-selectable header rows.
/// * `matched_rows` - Number of currently matched rows.
/// * `layout` - Preview pane layout configuration.
/// * `reserved_height` - Terminal rows reserved for the whole picker.
pub(crate) fn max_preview_scroll_offset(
    preview: &str,
    header_rows: usize,
    matched_rows: usize,
    layout: PreviewLayout,
    reserved_height: u16,
) -> usize {
    preview.lines().count().saturating_sub(
        preview_content_height(header_rows, matched_rows, preview, layout, reserved_height).max(1),
    )
}

/// Computes the height of the bottom preview pane (including its top and
/// bottom border rows) for a bottom placement.
///
/// The pane takes `percent` of the total height, but is grown to consume any
/// body space beyond a small guaranteed list area, and clamped so at least a
/// minimal list (3-8 rows depending on terminal size) and a minimal preview
/// (3 rows) both stay visible.
///
/// # Arguments
///
/// * `height` - Total picker viewport height.
/// * `body_height` - Viewport height minus the header area.
/// * `percent` - Requested preview height as a percentage of `height`.
pub(crate) fn bottom_preview_height(height: u16, body_height: u16, percent: u16) -> u16 {
    let requested = ((height as u32 * percent as u32) / 100) as u16;
    let minimum_preview_height = 3;
    let minimum_list_height = (body_height / 3).clamp(3, 8);
    let maximum_preview_height = body_height.saturating_sub(minimum_list_height);
    let preview_height = requested.max(maximum_preview_height);

    preview_height.clamp(
        minimum_preview_height.min(body_height),
        maximum_preview_height
            .max(minimum_preview_height)
            .min(body_height),
    )
}

/// Returns the number of preview text lines that fit inside the preview pane
/// (excluding border rows for the bottom placement).
///
/// Falls back to `1` when the terminal size cannot be queried.
///
/// # Arguments
///
/// * `header_rows` - Number of non-selectable header rows.
/// * `matched_rows` - Number of currently matched rows.
/// * `preview` - Rendered preview text (its line count feeds the desired
///   viewport height).
/// * `layout` - Preview pane layout configuration.
/// * `reserved_height` - Terminal rows reserved for the whole picker.
pub(crate) fn preview_content_height(
    header_rows: usize,
    matched_rows: usize,
    preview: &str,
    layout: PreviewLayout,
    reserved_height: u16,
) -> usize {
    let Ok((_, height)) = terminal::size() else {
        return 1;
    };
    let desired_height =
        desired_select_viewport_height(header_rows, matched_rows, preview.lines().count(), layout);
    let height = select_viewport_height(height, desired_height).min(reserved_height);
    // Header area = prompt row + separator row + header rows.
    let header_height = 2u16.saturating_add(header_rows as u16);
    let body_height = height.saturating_sub(header_height).max(1);

    (match layout.placement {
        // Right placement: the preview spans the full body height.
        PreviewPlacement::Right => body_height,
        // Bottom placement: subtract the two box-border rows.
        PreviewPlacement::Bottom => {
            bottom_preview_height(height, body_height, layout.percent).saturating_sub(2)
        }
    }) as usize
}

/// Returns whether the given terminal cell lies inside the preview pane.
///
/// Used to decide whether mouse wheel events scroll the preview pane or the
/// results list. Mirrors the geometry computed by the renderer; falls back
/// to `false` when the terminal size cannot be queried.
///
/// # Arguments
///
/// * `column` - Zero-based terminal column of the mouse event.
/// * `row` - Zero-based terminal row of the mouse event.
/// * `header_rows` - Number of non-selectable header rows.
/// * `matched_rows` - Number of currently matched rows.
/// * `preview` - Rendered preview text.
/// * `layout` - Preview pane layout configuration.
/// * `reserved_height` - Terminal rows reserved for the whole picker.
pub(crate) fn mouse_over_preview(
    column: u16,
    row: u16,
    header_rows: usize,
    matched_rows: usize,
    preview: &str,
    layout: PreviewLayout,
    reserved_height: u16,
) -> bool {
    let Ok((width, height)) = terminal::size() else {
        return false;
    };
    let width = width.max(20);
    let desired_height =
        desired_select_viewport_height(header_rows, matched_rows, preview.lines().count(), layout);
    let height = select_viewport_height(height, desired_height).min(reserved_height);
    let header_height = 2u16.saturating_add(header_rows as u16);
    let body_height = height.saturating_sub(header_height).max(1);

    match layout.placement {
        PreviewPlacement::Right => {
            // Same split as the renderer: preview takes `percent` of the
            // width (clamped to keep both panes usable) and sits after the
            // list plus a 3-column gutter (space + divider + space).
            let preview_width = ((width as u32 * layout.percent as u32) / 100) as u16;
            let preview_width = preview_width.clamp(10, width.saturating_sub(10));
            let list_width = width.saturating_sub(preview_width + 3).max(10);
            let preview_x = list_width + 3;
            column >= preview_x && column < width && row >= header_height && row < height
        }
        PreviewPlacement::Bottom => {
            // The bottom pane starts right below the list area and spans the
            // full width.
            let preview_height = bottom_preview_height(height, body_height, layout.percent);
            let list_height = body_height.saturating_sub(preview_height).max(1);
            let preview_y = header_height + list_height;
            preview_height > 0
                && column < width
                && row >= preview_y
                && row < preview_y.saturating_add(preview_height)
        }
    }
}
