//! Frame rendering for the selector UI.
//!
//! [`draw_preview_ui`] paints one complete frame into the reserved inline
//! viewport: prompt + query line, match counter with separator, header rows,
//! the scrollable results list and (optionally) the preview pane with its
//! divider or box border.

use std::io::Write;

use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType};

use super::layout::bottom_preview_height;
use super::text::{
    format_prompt_query, match_count_width, preview_scroll_indicator, truncate_line,
    truncate_line_with_ellipsis, wrap_preview_lines,
};
use super::types::{PreviewLayout, PreviewPlacement, SelectRow};
use super::utils::viewport_move_to;

/// All state required to render a single picker frame.
pub(crate) struct PreviewUi<'a> {
    /// Prompt text rendered before the query.
    pub(crate) prompt: &'a str,
    /// Current fuzzy search query.
    pub(crate) query: &'a str,
    /// Total number of selectable rows (before filtering).
    pub(crate) total_rows: usize,
    /// Rows currently matched by the fuzzy query, in score order.
    pub(crate) matched_rows: &'a [&'a SelectRow],
    /// Non-selectable header rows rendered above the list.
    pub(crate) header_rows: &'a [&'a SelectRow],
    /// Index of the highlighted row within `matched_rows`.
    pub(crate) selected_index: usize,
    /// First visible list row; adjusted in place to keep the selection
    /// visible.
    pub(crate) scroll_offset: &'a mut usize,
    /// Rendered preview text; empty disables the preview pane.
    pub(crate) preview: &'a str,
    /// First visible preview line.
    pub(crate) preview_scroll_offset: usize,
    /// Preview pane placement and size configuration.
    pub(crate) layout: PreviewLayout,
    /// Terminal rows reserved for the whole picker.
    pub(crate) reserved_height: u16,
    /// Terminal row where the reserved viewport starts.
    pub(crate) viewport_top_row: u16,
}

/// Renders one complete picker frame into the reserved viewport.
///
/// # Errors
///
/// Returns an error if the terminal size cannot be queried or writing the
/// queued commands to `stderr` fails.
pub(crate) fn draw_preview_ui(stderr: &mut impl Write, ui: PreviewUi<'_>) -> anyhow::Result<()> {
    let PreviewUi {
        prompt,
        query,
        total_rows,
        matched_rows,
        header_rows,
        selected_index,
        scroll_offset,
        preview,
        preview_scroll_offset,
        layout,
        reserved_height,
        viewport_top_row,
    } = ui;
    let (width, full_height) = terminal::size()?;
    let width = width.max(20);

    let has_preview = !preview.is_empty();
    // Always render into the full reserved region. Computing a smaller
    // desired_height from current content and capping height to it would leave
    // the already-reserved terminal rows blank, wasting visible space.
    // Keep one safety row at the bottom of the reserved region to avoid
    // terminal-specific implicit wrap/scroll behavior when writing at the
    // final visible row. The reservation already accounts for that safety row
    // when preview is enabled, so use all reserved rows here.
    let height = reserved_height.max(1);
    let max_top_row = full_height.saturating_sub(height.max(1));
    let top_offset = viewport_top_row.min(max_top_row);
    // Header area = prompt row + separator row + header rows.
    let header_height = 2u16.saturating_add(header_rows.len() as u16);
    let body_height = height.saturating_sub(header_height).max(1);

    // Split the body into list and preview rectangles. With a right-side
    // preview the panes share the width (with a 3-column gutter for the
    // divider); with a bottom preview they share the height. Without a
    // preview the list gets the whole body.
    let (
        list_x,
        list_y,
        list_width,
        list_height,
        preview_x,
        preview_y,
        preview_width,
        preview_height,
    ) = if has_preview {
        match layout.placement {
            PreviewPlacement::Right => {
                let preview_width = ((width as u32 * layout.percent as u32) / 100) as u16;
                let preview_width = preview_width.clamp(10, width.saturating_sub(10));
                let list_width = width.saturating_sub(preview_width + 3).max(10);
                (
                    0,
                    header_height,
                    list_width,
                    body_height,
                    list_width + 3,
                    header_height,
                    preview_width,
                    body_height,
                )
            }
            PreviewPlacement::Bottom => {
                let preview_height = bottom_preview_height(height, body_height, layout.percent);
                let list_height = body_height.saturating_sub(preview_height).max(1);
                (
                    0,
                    header_height,
                    width,
                    list_height,
                    0,
                    header_height + list_height,
                    width,
                    preview_height,
                )
            }
        }
    } else {
        (0, header_height, width, body_height, 0, height, 0, 0)
    };

    // Adjust the list scroll offset so the selected row stays in view.
    let visible_rows = list_height as usize;
    if visible_rows > 0 {
        if selected_index < *scroll_offset {
            *scroll_offset = selected_index;
        } else if selected_index >= scroll_offset.saturating_add(visible_rows) {
            *scroll_offset = selected_index.saturating_sub(visible_rows.saturating_sub(1));
        }
    }

    // Clear the whole reserved region before painting the new frame.
    for row_index in 0..reserved_height {
        queue!(
            stderr,
            viewport_move_to(0, row_index, top_offset),
            Clear(ClearType::CurrentLine)
        )?;
    }
    // Row 0: prompt + query line.
    queue!(
        stderr,
        viewport_move_to(0, 0, top_offset),
        SetAttribute(Attribute::Bold),
        SetForegroundColor(Color::AnsiValue(110)),
        Print(truncate_line(
            &format_prompt_query(prompt, query),
            width as usize
        )),
        ResetColor,
        SetAttribute(Attribute::Reset)
    )?;
    // Row 1: `matched/total` counter followed by a horizontal separator.
    queue!(
        stderr,
        viewport_move_to(2, 1, top_offset),
        SetForegroundColor(Color::AnsiValue(144)),
        Print(format!("{}/{}", matched_rows.len(), total_rows)),
        SetForegroundColor(Color::AnsiValue(59)),
        Print(" "),
        Print(truncate_line(
            &"─".repeat(width as usize),
            width.saturating_sub(3 + match_count_width(matched_rows.len(), total_rows)) as usize,
        )),
        ResetColor
    )?;
    // Rows 2..header_height: non-selectable header rows.
    for (index, row) in header_rows.iter().enumerate() {
        let row_y = 2u16.saturating_add(index as u16);
        if row_y < header_height {
            queue!(
                stderr,
                viewport_move_to(2, row_y, top_offset),
                SetAttribute(Attribute::Bold),
                SetForegroundColor(Color::AnsiValue(109))
            )?;
            queue!(
                stderr,
                Print(truncate_line(
                    &row.display,
                    width.saturating_sub(2) as usize
                ))
            )?;
            queue!(stderr, ResetColor, SetAttribute(Attribute::Reset))?;
        }
    }

    // Results list: one row per visible matched item, with a `▌` marker and
    // highlighted colors for the selected row.
    for row_index in 0..list_height {
        queue!(
            stderr,
            viewport_move_to(list_x, list_y + row_index, top_offset),
            Clear(ClearType::CurrentLine)
        )?;
        let item_index = *scroll_offset + row_index as usize;
        if let Some(row) = matched_rows.get(item_index) {
            let is_selected = item_index == selected_index;
            let marker = "▌";
            let content_width = list_width.saturating_sub(2) as usize;
            if is_selected {
                queue!(
                    stderr,
                    viewport_move_to(list_x, list_y + row_index, top_offset),
                    SetAttribute(Attribute::Bold),
                    SetForegroundColor(Color::AnsiValue(161)),
                    SetBackgroundColor(Color::AnsiValue(236)),
                    Print(marker),
                    SetForegroundColor(Color::AnsiValue(254)),
                    Print(" "),
                    Print(truncate_line_with_ellipsis(&row.display, content_width)),
                    ResetColor,
                    SetAttribute(Attribute::Reset)
                )?;
            } else {
                queue!(
                    stderr,
                    viewport_move_to(list_x, list_y + row_index, top_offset),
                    SetForegroundColor(Color::AnsiValue(236)),
                    Print(marker),
                    ResetColor,
                    Print(" "),
                    Print(truncate_line_with_ellipsis(&row.display, content_width))
                )?;
            }
        }
    }

    if has_preview {
        // Pane chrome: a vertical divider for the right placement, a box
        // top border for the bottom placement.
        match layout.placement {
            PreviewPlacement::Right => {
                let divider_x = list_width + 1;
                for row_index in 0..body_height {
                    queue!(
                        stderr,
                        viewport_move_to(divider_x, header_height + row_index, top_offset),
                        Print("│")
                    )?;
                }
            }
            PreviewPlacement::Bottom => {
                queue!(
                    stderr,
                    viewport_move_to(0, preview_y, top_offset),
                    SetForegroundColor(Color::AnsiValue(59)),
                    Print("┌"),
                    Print("─".repeat(width.saturating_sub(2) as usize)),
                    Print("┐"),
                    ResetColor
                )?;
            }
        }

        // The bottom placement loses two rows to the box border and four
        // columns to the side borders plus padding.
        let preview_content_height = match layout.placement {
            PreviewPlacement::Bottom => preview_height.saturating_sub(2),
            PreviewPlacement::Right => preview_height,
        } as usize;
        let preview_width_for_content = match layout.placement {
            PreviewPlacement::Bottom => preview_width.saturating_sub(4),
            PreviewPlacement::Right => preview_width,
        } as usize;
        let preview_lines = wrap_preview_lines(preview, preview_width_for_content.max(1));
        // Clamp the scroll offset so the pane never scrolls past the content.
        let preview_scroll_offset = preview_scroll_offset.min(
            preview_lines
                .len()
                .saturating_sub(preview_content_height.max(1)),
        );
        for row_index in 0..preview_height {
            let y = preview_y + row_index;
            // Bottom placement: row 0 is the already-drawn top border.
            if layout.placement == PreviewPlacement::Bottom && row_index == 0 {
                continue;
            }
            // Bottom placement: the last row is the bottom border.
            if layout.placement == PreviewPlacement::Bottom
                && row_index == preview_height.saturating_sub(1)
            {
                queue!(
                    stderr,
                    viewport_move_to(preview_x, y, top_offset),
                    SetForegroundColor(Color::AnsiValue(59)),
                    Print("└"),
                    Print("─".repeat(preview_width.saturating_sub(2) as usize)),
                    Print("┘"),
                    ResetColor
                )?;
                continue;
            }

            // Bottom placement draws side borders and indents the content;
            // right placement uses the full pane width.
            let (content_x, content_width) = if layout.placement == PreviewPlacement::Bottom {
                queue!(
                    stderr,
                    viewport_move_to(preview_x, y, top_offset),
                    SetForegroundColor(Color::AnsiValue(59)),
                    Print("│"),
                    viewport_move_to(preview_x + preview_width.saturating_sub(1), y, top_offset),
                    Print("│"),
                    ResetColor
                )?;
                (preview_x + 2, preview_width.saturating_sub(4))
            } else {
                (preview_x, preview_width)
            };

            // Blank the content cell, then print the preview line (if any).
            queue!(
                stderr,
                viewport_move_to(content_x, y, top_offset),
                Print(" ".repeat(content_width as usize))
            )?;
            let line_index = if layout.placement == PreviewPlacement::Bottom {
                preview_scroll_offset + row_index.saturating_sub(1) as usize
            } else {
                preview_scroll_offset + row_index as usize
            };
            if let Some(line) = preview_lines.get(line_index) {
                queue!(
                    stderr,
                    viewport_move_to(content_x, y, top_offset),
                    Print(truncate_line(line, content_width as usize))
                )?;
            }

            // Bottom placement: overlay a `current/total` scroll indicator
            // at the right edge of the first content row.
            if layout.placement == PreviewPlacement::Bottom
                && row_index == 1
                && !preview_lines.is_empty()
            {
                let indicator =
                    preview_scroll_indicator(preview_scroll_offset, preview_lines.len());
                let indicator_width = indicator.chars().count() as u16;
                if indicator_width.saturating_add(1) < preview_width {
                    queue!(
                        stderr,
                        viewport_move_to(
                            preview_x + preview_width.saturating_sub(indicator_width + 2),
                            y,
                            top_offset,
                        ),
                        SetAttribute(Attribute::Reverse),
                        SetForegroundColor(Color::AnsiValue(144)),
                        Print(indicator),
                        ResetColor,
                        SetAttribute(Attribute::Reset),
                        SetForegroundColor(Color::AnsiValue(59)),
                        Print(" "),
                        Print("│"),
                        ResetColor
                    )?;
                }
            }
        }
    }

    stderr.flush()?;
    Ok(())
}
