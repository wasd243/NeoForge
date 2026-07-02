use std::io;
use std::io::Write;
use crossterm::{queue, terminal};
use crossterm::style::{Print};
use crossterm::cursor::{MoveTo, MoveToColumn, MoveUp};
use crossterm::terminal::{Clear, ClearType};
use crate::{PreviewLayout, PreviewPlacement};

const SELECT_VIEWPORT_PERCENT: u16 = 95;

fn max_select_viewport_height(full_height: u16) -> u16 {
    let full_height = full_height.max(1);
    ((full_height as u32 * SELECT_VIEWPORT_PERCENT as u32) / 100)
        .max(1)
        .min(full_height as u32) as u16
}

pub(super) fn select_viewport_height(full_height: u16, desired_height: u16) -> u16 {
    let full_height = full_height.max(1);
    let desired_height = desired_height.max(1);
    if desired_height <= full_height {
        desired_height
    } else {
        max_select_viewport_height(full_height)
    }
}

pub(super) fn preview_select_viewport_height(full_height: u16) -> u16 {
    let full_height = full_height.max(1);
    full_height.saturating_sub(1).max(1)
}

pub(super) fn reserve_inline_viewport_space(
    stderr: &mut impl Write,
    desired_height: u16,
) -> io::Result<(u16, u16)> {
    let (_, full_height) = terminal::size()?;
    let reserved_height = if desired_height == u16::MAX {
        preview_select_viewport_height(full_height)
    } else {
        max_select_viewport_height(full_height)
            .max(select_viewport_height(full_height, desired_height))
    };

    // Reserve space by scrolling the terminal, but leave the cursor on the
    // original prompt row. The shell completion widget expects control to
    // return on that same row so it can rewrite the current ZLE buffer.
    for _ in 0..reserved_height {
        queue!(stderr, Print("\r\n"))?;
    }
    queue!(stderr, MoveUp(reserved_height), MoveToColumn(0))?;
    stderr.flush()?;

    let cursor_top_row = full_height.saturating_sub(reserved_height.max(1));
    Ok((reserved_height, cursor_top_row))
}

pub(super) fn desired_select_viewport_height(
    header_rows: usize,
    matched_rows: usize,
    preview_lines: usize,
    layout: PreviewLayout,
) -> u16 {
    let header_height = 2u16.saturating_add(header_rows as u16);
    let list_height = (matched_rows as u16).max(1);
    let preview_lines = preview_lines as u16;

    match layout.placement {
        PreviewPlacement::Right => header_height.saturating_add(list_height),
        PreviewPlacement::Bottom if preview_lines > 0 => header_height
            .saturating_add(list_height)
            .saturating_add(preview_lines.saturating_add(2)),
        PreviewPlacement::Bottom => header_height.saturating_add(list_height),
    }
        .max(1)
}

pub(super) fn viewport_move_to(x: u16, y: u16, top_row: u16) -> MoveTo {
    MoveTo(x, top_row.saturating_add(y))
}

pub(super) fn restore_select_viewport(
    stderr: &mut impl Write,
    reserved_height: u16,
    viewport_top_row: u16,
) -> io::Result<()> {
    let (_, full_height) = terminal::size()?;
    let max_top_row = full_height.saturating_sub(reserved_height.max(1));
    let viewport_top_row = viewport_top_row.min(max_top_row);

    for row_index in 0..reserved_height {
        queue!(
            stderr,
            viewport_move_to(0, row_index, viewport_top_row),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(stderr, MoveTo(0, viewport_top_row.saturating_sub(1)))?;
    stderr.flush()
}