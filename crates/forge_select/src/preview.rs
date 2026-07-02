mod utils;
mod types;
mod terminal_guard;
mod run_select_ui;

use std::io::Write;
use std::cmp;
use std::path::{self, PathBuf};
use std::fs;
use colored::Colorize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::queue;
use nucleo::Nucleo;

use crate::preview::utils::*;
pub use crate::preview::types::*;

/// Selector UI entry points.
pub use crate::preview::run_select_ui::run_select_ui;
pub(crate) use crate::preview::run_select_ui::run_select_ui_values;

#[derive(Debug, PartialEq, Eq)]
enum PickerAction {
    Continue,
    Accept,
    Toggle,
    Exit,
    PreviewScrollUp,
    PreviewScrollDown,
    PreviewPageUp,
    PreviewPageDown,
}

fn handle_key_event(
    key: KeyEvent,
    query: &mut String,
    matched_len: usize,
    selected_index: &mut usize,
    has_preview: bool,
) -> PickerAction {
    match key {
        KeyEvent {
            code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, ..
        }
        | KeyEvent { code: KeyCode::Esc, .. } => PickerAction::Exit,
        KeyEvent { code: KeyCode::Char('U'), .. } if has_preview => PickerAction::PreviewPageUp,
        KeyEvent { code: KeyCode::Char('u'), modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewPageUp
        }
        KeyEvent { code: KeyCode::PageUp, modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewPageUp
        }
        KeyEvent { code: KeyCode::Char('D'), .. } if has_preview => PickerAction::PreviewPageDown,
        KeyEvent { code: KeyCode::Char('d'), modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewPageDown
        }
        KeyEvent { code: KeyCode::PageDown, modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewPageDown
        }
        KeyEvent { code: KeyCode::Char('K'), .. } if has_preview => PickerAction::PreviewScrollUp,
        KeyEvent { code: KeyCode::Char('k'), modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewScrollUp
        }
        KeyEvent { code: KeyCode::Up, modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewScrollUp
        }
        KeyEvent { code: KeyCode::Char('J'), .. } if has_preview => PickerAction::PreviewScrollDown,
        KeyEvent { code: KeyCode::Char('j'), modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewScrollDown
        }
        KeyEvent { code: KeyCode::Down, modifiers, .. }
            if has_preview && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            PickerAction::PreviewScrollDown
        }
        KeyEvent { code: KeyCode::Enter, .. } => PickerAction::Accept,
        KeyEvent { code: KeyCode::BackTab, .. } | KeyEvent { code: KeyCode::Tab, .. } => {
            PickerAction::Toggle
        }
        KeyEvent { code: KeyCode::Up, .. } => {
            if matched_len > 0 {
                *selected_index = selected_index.saturating_sub(1);
            }
            PickerAction::Continue
        }
        KeyEvent { code: KeyCode::Down, .. } => {
            if matched_len > 0 {
                *selected_index = cmp::min(*selected_index + 1, matched_len.saturating_sub(1));
            }
            PickerAction::Continue
        }
        KeyEvent { code: KeyCode::PageUp, .. } => {
            if matched_len > 0 {
                *selected_index = selected_index.saturating_sub(10);
            }
            PickerAction::Continue
        }
        KeyEvent { code: KeyCode::PageDown, .. } => {
            if matched_len > 0 {
                *selected_index = cmp::min(*selected_index + 10, matched_len.saturating_sub(1));
            }
            PickerAction::Continue
        }
        KeyEvent { code: KeyCode::Backspace, .. } => {
            query.pop();
            PickerAction::Continue
        }
        KeyEvent { code: KeyCode::Char(ch), modifiers, .. }
            if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT =>
        {
            query.push(ch);
            PickerAction::Continue
        }
        _ => PickerAction::Continue,
    }
}

fn max_preview_scroll_offset(
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

fn bottom_preview_height(height: u16, body_height: u16, percent: u16) -> u16 {
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

fn preview_content_height(
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
    let header_height = 2u16.saturating_add(header_rows as u16);
    let body_height = height.saturating_sub(header_height).max(1);

    (match layout.placement {
        PreviewPlacement::Right => body_height,
        PreviewPlacement::Bottom => {
            bottom_preview_height(height, body_height, layout.percent).saturating_sub(2)
        }
    }) as usize
}

fn mouse_over_preview(
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
            let preview_width = ((width as u32 * layout.percent as u32) / 100) as u16;
            let preview_width = preview_width.clamp(10, width.saturating_sub(10));
            let list_width = width.saturating_sub(preview_width + 3).max(10);
            let preview_x = list_width + 3;
            column >= preview_x && column < width && row >= header_height && row < height
        }
        PreviewPlacement::Bottom => {
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

fn matched_rows(matcher: &Nucleo<SelectRow>) -> Vec<&SelectRow> {
    matcher
        .snapshot()
        .matched_items(..)
        .map(|item| item.data)
        .collect()
}

/// idk why you guys lock this perfect agent on Unix by hardcoding the path
fn render_preview(command: &str, row: &SelectRow, working_dir: Option<&path::Path>) -> String {
    if command.trim().is_empty() {
        return String::new();
    }

    // Preview = read the selected file's contents directly.
    // No shell, no /bin/sh, cross-platform.
    let path = if row.raw.is_empty() {
        PathBuf::new()
    } else if path::Path::new(&row.raw).is_absolute() {
        PathBuf::from(&row.raw)
    } else if let Some(base_dir) = working_dir {
        base_dir.join(&row.raw)
    } else {
        PathBuf::from(&row.raw)
    };

    // fix the bug that the path is not escaped huh?
    if path.is_dir() {
        return format!("{}: {}", row.display, "Is a directory.How can you preview it???".bright_red().italic());
    }

    match fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .take(500)
            .collect::<Vec<_>>()
            .join("\n"),
        Err(error) => format!("Cannot preview {}: {error}", row.display),
    }
}

struct PreviewUi<'a> {
    prompt: &'a str,
    query: &'a str,
    total_rows: usize,
    matched_rows: &'a [&'a SelectRow],
    header_rows: &'a [&'a SelectRow],
    selected_index: usize,
    scroll_offset: &'a mut usize,
    preview: &'a str,
    preview_scroll_offset: usize,
    layout: PreviewLayout,
    reserved_height: u16,
    viewport_top_row: u16,
}

fn draw_preview_ui(stderr: &mut impl Write, ui: PreviewUi<'_>) -> anyhow::Result<()> {
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
    let header_height = 2u16.saturating_add(header_rows.len() as u16);
    let body_height = height.saturating_sub(header_height).max(1);

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

    let visible_rows = list_height as usize;
    if visible_rows > 0 {
        if selected_index < *scroll_offset {
            *scroll_offset = selected_index;
        } else if selected_index >= scroll_offset.saturating_add(visible_rows) {
            *scroll_offset = selected_index.saturating_sub(visible_rows.saturating_sub(1));
        }
    }

    for row_index in 0..reserved_height {
        queue!(
            stderr,
            viewport_move_to(0, row_index, top_offset),
            Clear(ClearType::CurrentLine)
        )?;
    }
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

        let preview_content_height = match layout.placement {
            PreviewPlacement::Bottom => preview_height.saturating_sub(2),
            PreviewPlacement::Right => preview_height,
        } as usize;
        let preview_width_for_content = match layout.placement {
            PreviewPlacement::Bottom => preview_width.saturating_sub(4),
            PreviewPlacement::Right => preview_width,
        } as usize;
        let preview_lines = wrap_preview_lines(preview, preview_width_for_content.max(1));
        let preview_scroll_offset = preview_scroll_offset.min(
            preview_lines
                .len()
                .saturating_sub(preview_content_height.max(1)),
        );
        for row_index in 0..preview_height {
            let y = preview_y + row_index;
            if layout.placement == PreviewPlacement::Bottom && row_index == 0 {
                continue;
            }
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

fn preview_scroll_indicator(scroll_offset: usize, line_count: usize) -> String {
    format!("{}/{line_count}", scroll_offset.saturating_add(1))
}

fn wrap_preview_lines(preview: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }

    preview
        .lines()
        .flat_map(|line| wrap_ansi_line(line, max_width))
        .collect()
}

fn wrap_ansi_line(line: &str, max_width: usize) -> Vec<String> {
    const WRAP_ICON: &str = "↪ ";
    const WRAP_ICON_WIDTH: usize = 2;

    if line.is_empty() {
        return vec![String::new()];
    }

    let mut wrapped_lines = Vec::new();
    let mut current_line = String::new();
    let mut visible_width = 0usize;
    let mut chars = line.chars().peekable();
    let mut is_continuation = false;

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            current_line.push(ch);
            for ansi_ch in chars.by_ref() {
                current_line.push(ansi_ch);
                if ansi_ch.is_ascii_alphabetic() || ansi_ch == '~' {
                    break;
                }
            }
            continue;
        }

        let current_limit = if is_continuation {
            max_width.saturating_sub(WRAP_ICON_WIDTH).max(1)
        } else {
            max_width
        };

        if visible_width >= current_limit {
            let pushed = if is_continuation {
                format!("{WRAP_ICON}{current_line}")
            } else {
                current_line.clone()
            };
            wrapped_lines.push(pushed);
            current_line = String::new();
            visible_width = 0;
            is_continuation = true;
        }

        current_line.push(ch);
        visible_width = visible_width.saturating_add(1);
    }

    if !current_line.is_empty() {
        let pushed = if is_continuation {
            format!("{WRAP_ICON}{current_line}")
        } else {
            current_line
        };
        wrapped_lines.push(pushed);
    }

    if wrapped_lines.is_empty() {
        vec![String::new()]
    } else {
        wrapped_lines
    }
}

fn format_prompt_query(prompt: &str, query: &str) -> String {
    if query.is_empty() || prompt.ends_with(char::is_whitespace) {
        format!("{prompt}{query}")
    } else {
        format!("{prompt} {query}")
    }
}

fn match_count_width(matched: usize, total: usize) -> u16 {
    format!("{matched}/{total}").chars().count() as u16
}

fn truncate_line_with_ellipsis(value: &str, max_width: usize) -> String {
    const ELLIPSIS: &str = "…";
    let full_width = value.chars().count();
    if full_width <= max_width {
        return value.to_string();
    }

    if max_width <= ELLIPSIS.len() {
        return ELLIPSIS.chars().take(max_width).collect();
    }

    let keep_width = max_width.saturating_sub(ELLIPSIS.len());
    let prefix: String = value.chars().take(keep_width).collect();
    format!("{prefix}{ELLIPSIS}")
}

fn truncate_line(value: &str, max_width: usize) -> String {
    let mut rendered = String::new();
    let mut visible_width = 0usize;
    let mut chars = value.chars().peekable();
    let mut truncated = false;
    let mut has_ansi = false;

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            has_ansi = true;
            rendered.push(ch);
            for ansi_ch in chars.by_ref() {
                rendered.push(ansi_ch);
                if ansi_ch.is_ascii_alphabetic() || ansi_ch == '~' {
                    break;
                }
            }
            continue;
        }

        if visible_width >= max_width {
            truncated = true;
            break;
        }

        rendered.push(ch);
        visible_width = visible_width.saturating_add(1);
    }

    if truncated && has_ansi {
        rendered.push_str("\u{1b}[0m");
    }

    rendered
}

#[cfg(test)]
#[path = "../tests/preview_test.rs"]
mod tests;
