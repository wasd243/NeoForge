use std::cmp;
use std::collections::BTreeSet;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, MouseEventKind};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config as NucleoConfig, Nucleo, Utf32String};

use super::terminal_guard::TerminalGuard;
use super::utils::{
    desired_select_viewport_height, reserve_inline_viewport_space, restore_select_viewport,
};
use super::{
    PickerAction, PreviewUi, SelectMode, SelectUiOptions, draw_preview_ui, handle_key_event,
    matched_rows, max_preview_scroll_offset, mouse_over_preview, preview_content_height,
    render_preview,
};

/// Runs the shared nucleo-backed selector UI and returns the selected raw
/// value.
///
/// # Errors
///
/// Returns an error if terminal setup, event handling, rendering, or preview
/// command execution setup fails.
pub fn run_select_ui(options: SelectUiOptions) -> anyhow::Result<Option<String>> {
    Ok(run_select_ui_values(options)?.and_then(|values| values.into_iter().next()))
}

/// Runs the shared nucleo-backed selector UI and returns all selected raw
/// values.
///
/// # Errors
///
/// Returns an error if terminal setup, event handling, rendering, or preview
/// command execution setup fails.
pub(crate) fn run_select_ui_values(
    options: SelectUiOptions,
) -> anyhow::Result<Option<Vec<String>>> {
    let SelectUiOptions {
        prompt,
        query,
        rows,
        header_lines,
        mode,
        preview,
        preview_layout,
        initial_raw,
        working_dir,
    } = options;
    let header_count = header_lines.min(rows.len());
    let header_rows = rows.iter().take(header_count).collect::<Vec<_>>();
    let data_rows = rows.iter().skip(header_count).cloned().collect::<Vec<_>>();
    if data_rows.is_empty() {
        return Ok(None);
    }

    let mut matcher = Nucleo::new(NucleoConfig::DEFAULT, Arc::new(|| {}), None, 1);
    let injector = matcher.injector();
    for row in data_rows.iter().cloned() {
        injector.push(row, |item, columns| {
            if let Some(column) = columns.get_mut(0) {
                *column = Utf32String::from(item.search.as_str());
            }
        });
    }
    drop(injector);

    let mut query = query.unwrap_or_default();
    matcher
        .pattern
        .reparse(0, &query, CaseMatching::Smart, Normalization::Smart, false);
    let _ = matcher.tick(50);

    let guard = TerminalGuard::enter()?;
    let mut stderr = io::BufWriter::new(io::stderr());
    let prompt = prompt.unwrap_or_else(|| "❯ ".to_string());
    let preview_command = preview.unwrap_or_default();
    let initial_matched_rows = matched_rows(&matcher);
    // When a preview command is present, reserve the maximum available viewport
    // height upfront. Without this, the initial reservation (calculated with
    // zero preview lines) is too small: once a preview renders it consumes the
    // configured percentage of the reserved space and leaves only 1–2 rows for
    // the list, even when many items match.
    let initial_desired_height = if !preview_command.is_empty() {
        u16::MAX
    } else {
        desired_select_viewport_height(
            header_rows.len(),
            initial_matched_rows.len(),
            0,
            preview_layout,
        )
    };
    let (reserved_height, viewport_top_row) =
        reserve_inline_viewport_space(&mut stderr, initial_desired_height)?;
    let mut selected_index = 0usize;
    let mut initial_raw = initial_raw;
    let mut initial_selection_applied = false;
    let mut scroll_offset = 0usize;
    let mut preview_scroll_offset = 0usize;
    let mut queued_indices = BTreeSet::new();
    let mut preview_cache = String::new();
    let mut last_preview_key = String::new();
    let mut last_query = query.clone();

    let mut needs_render = true;
    loop {
        if query != last_query {
            matcher.pattern.reparse(
                0,
                &query,
                CaseMatching::Smart,
                Normalization::Smart,
                query.starts_with(&last_query),
            );
            last_query = query.clone();
            let _ = matcher.tick(50);
            selected_index = 0;
            scroll_offset = 0;
            preview_scroll_offset = 0;
            needs_render = true;
        }

        let matched_rows = matched_rows(&matcher);
        if !initial_selection_applied {
            if let Some(initial_raw) = initial_raw.take()
                && let Some(index) = matched_rows.iter().position(|row| row.raw == initial_raw)
            {
                selected_index = index;
                needs_render = true;
            }
            initial_selection_applied = true;
        }

        if matched_rows.is_empty() {
            if selected_index != 0 || scroll_offset != 0 {
                needs_render = true;
            }
            selected_index = 0;
            scroll_offset = 0;
        } else if selected_index >= matched_rows.len() {
            selected_index = matched_rows.len().saturating_sub(1);
            needs_render = true;
        }

        let selected_row = matched_rows.get(selected_index).copied();
        let preview_key = selected_row
            .map(|row| format!("{}\0{}", row.raw, query))
            .unwrap_or_default();
        if preview_key != last_preview_key {
            preview_cache = selected_row
                .map(|row| render_preview(&preview_command, row, working_dir.as_deref()))
                .unwrap_or_else(|| "No matches".to_string());
            preview_scroll_offset = 0;
            last_preview_key = preview_key;
            needs_render = true;
        }

        let rendered_preview = if preview_command.is_empty() {
            ""
        } else {
            &preview_cache
        };

        if needs_render {
            draw_preview_ui(
                &mut stderr,
                PreviewUi {
                    prompt: &prompt,
                    query: &query,
                    total_rows: data_rows.len(),
                    matched_rows: &matched_rows,
                    header_rows: &header_rows,
                    selected_index,
                    scroll_offset: &mut scroll_offset,
                    preview: rendered_preview,
                    preview_scroll_offset,
                    layout: preview_layout,
                    reserved_height,
                    viewport_top_row,
                },
            )?;
            needs_render = false;
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                // On Windows, crossterm reports key Release events in addition
                // to Press/Repeat (Unix reports only Press). Ignore Release so a
                // stray Release — notably the Release of the Enter key that
                // opened this picker — isn't read as a fresh keystroke that
                // instantly accepts the default selection and closes the picker.
                Event::Key(key) if key.kind == KeyEventKind::Release => {}
                Event::Key(key) => {
                    match handle_key_event(
                        key,
                        &mut query,
                        matched_rows.len(),
                        &mut selected_index,
                        !preview_command.is_empty(),
                    ) {
                        PickerAction::Continue => {
                            needs_render = true;
                        }
                        PickerAction::PreviewScrollUp => {
                            preview_scroll_offset = preview_scroll_offset.saturating_sub(1);
                            needs_render = true;
                        }
                        PickerAction::PreviewScrollDown => {
                            preview_scroll_offset = preview_scroll_offset.saturating_add(1);
                            needs_render = true;
                        }
                        PickerAction::PreviewPageUp => {
                            let page_size = preview_content_height(
                                header_rows.len(),
                                matched_rows.len(),
                                &preview_cache,
                                preview_layout,
                                reserved_height,
                            )
                            .saturating_sub(1)
                            .max(1);
                            preview_scroll_offset = preview_scroll_offset.saturating_sub(page_size);
                            needs_render = true;
                        }
                        PickerAction::PreviewPageDown => {
                            let page_size = preview_content_height(
                                header_rows.len(),
                                matched_rows.len(),
                                &preview_cache,
                                preview_layout,
                                reserved_height,
                            )
                            .saturating_sub(1)
                            .max(1);
                            preview_scroll_offset = preview_scroll_offset.saturating_add(page_size);
                            needs_render = true;
                        }
                        PickerAction::Toggle => {
                            if mode == SelectMode::Multi && selected_row.is_some() {
                                if !queued_indices.remove(&selected_index) {
                                    queued_indices.insert(selected_index);
                                }
                                selected_index = cmp::min(
                                    selected_index + 1,
                                    matched_rows.len().saturating_sub(1),
                                );
                                needs_render = true;
                            }
                        }
                        PickerAction::Accept => {
                            if mode == SelectMode::Multi && !queued_indices.is_empty() {
                                restore_select_viewport(
                                    &mut stderr,
                                    reserved_height,
                                    viewport_top_row,
                                )?;
                                drop(guard);
                                let selected = queued_indices
                                    .iter()
                                    .filter_map(|index| matched_rows.get(*index))
                                    .map(|row| row.raw.clone())
                                    .collect::<Vec<_>>();
                                return Ok(Some(selected));
                            }

                            if let Some(row) = selected_row {
                                restore_select_viewport(
                                    &mut stderr,
                                    reserved_height,
                                    viewport_top_row,
                                )?;
                                drop(guard);
                                return Ok(Some(vec![row.raw.clone()]));
                            }
                        }
                        PickerAction::Exit => {
                            restore_select_viewport(
                                &mut stderr,
                                reserved_height,
                                viewport_top_row,
                            )?;
                            drop(guard);
                            return Ok(None);
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if !preview_command.is_empty()
                        && mouse_over_preview(
                            mouse.column,
                            mouse.row,
                            header_rows.len(),
                            matched_rows.len(),
                            &preview_cache,
                            preview_layout,
                            reserved_height,
                        )
                    {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                preview_scroll_offset = preview_scroll_offset.saturating_sub(3);
                                needs_render = true;
                            }
                            MouseEventKind::ScrollDown => {
                                preview_scroll_offset = preview_scroll_offset.saturating_add(3);
                                needs_render = true;
                            }
                            _ => {}
                        }
                    } else {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                selected_index = selected_index.saturating_sub(1);
                                needs_render = true;
                            }
                            MouseEventKind::ScrollDown => {
                                selected_index = cmp::min(
                                    selected_index.saturating_add(1),
                                    matched_rows.len().saturating_sub(1),
                                );
                                needs_render = true;
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(_, _) => {
                    needs_render = true;
                }
                _ => {}
            }
        }

        if !preview_command.is_empty() {
            let clamped_offset = preview_scroll_offset.min(max_preview_scroll_offset(
                &preview_cache,
                header_rows.len(),
                matched_rows.len(),
                preview_layout,
                reserved_height,
            ));
            if clamped_offset != preview_scroll_offset {
                preview_scroll_offset = clamped_offset;
                needs_render = true;
            }
        }
    }
}
