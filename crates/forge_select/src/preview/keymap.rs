//! Keyboard input handling for the selector UI.
//!
//! Translates raw `crossterm` key events into high-level [`PickerAction`]s
//! that the event loop in `run_select_ui` executes. Query editing and list
//! navigation mutate state in place and report [`PickerAction::Continue`];
//! everything else is surfaced as a dedicated action.

use std::cmp;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// High-level outcome of a single key press inside the picker.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PickerAction {
    /// No terminal action; the query or selection may have been edited in
    /// place and the UI should re-render.
    Continue,
    /// Accept the current selection (`Enter`).
    Accept,
    /// Toggle the highlighted row in multi-select mode (`Tab`/`BackTab`).
    Toggle,
    /// Abort the picker without a selection (`Esc` / `Ctrl-C`).
    Exit,
    /// Scroll the preview pane up by one line (`Shift+K` / `Shift+Up`).
    PreviewScrollUp,
    /// Scroll the preview pane down by one line (`Shift+J` / `Shift+Down`).
    PreviewScrollDown,
    /// Scroll the preview pane up by one page (`Shift+U` / `Shift+PageUp`).
    PreviewPageUp,
    /// Scroll the preview pane down by one page (`Shift+D` /
    /// `Shift+PageDown`).
    PreviewPageDown,
}

/// Maps a key event to a [`PickerAction`], mutating the fuzzy `query` and the
/// highlighted `selected_index` in place for editing and navigation keys.
///
/// Preview scrolling bindings (shifted `J`/`K`/`U`/`D`, shifted arrows and
/// shifted Page keys) are only active when `has_preview` is true so plain
/// typing is never hijacked when no preview pane is shown. Uppercase
/// character arms (e.g. `Char('K')`) cover terminals that report shifted
/// letters as uppercase without the SHIFT modifier flag.
///
/// # Arguments
///
/// * `key` - Key event reported by crossterm.
/// * `query` - Current fuzzy search query, edited in place.
/// * `matched_len` - Number of currently matched rows, used to bound
///   navigation.
/// * `selected_index` - Currently highlighted row index, moved in place.
/// * `has_preview` - Whether a preview pane is visible.
pub(crate) fn handle_key_event(
    key: KeyEvent,
    query: &mut String,
    matched_len: usize,
    selected_index: &mut usize,
    has_preview: bool,
) -> PickerAction {
    match key {
        // Abort: Ctrl-C or Esc.
        KeyEvent {
            code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, ..
        }
        | KeyEvent { code: KeyCode::Esc, .. } => PickerAction::Exit,
        // Preview page-up: Shift+U (as uppercase char or SHIFT modifier) or
        // Shift+PageUp.
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
        // Preview page-down: Shift+D or Shift+PageDown.
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
        // Preview line-up: Shift+K or Shift+Up.
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
        // Preview line-down: Shift+J or Shift+Down.
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
        // Accept / toggle selection.
        KeyEvent { code: KeyCode::Enter, .. } => PickerAction::Accept,
        KeyEvent { code: KeyCode::BackTab, .. } | KeyEvent { code: KeyCode::Tab, .. } => {
            PickerAction::Toggle
        }
        // List navigation: single step with arrows, ten rows per Page key.
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
        // Query editing: backspace deletes, printable characters append.
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
