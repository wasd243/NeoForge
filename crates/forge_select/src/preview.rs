//! Shared fuzzy-selector ("picker") UI engine backing every selector in this
//! crate (`select`, `multi`, `widget`).
//!
//! This file is only the module root: it wires the submodules together and
//! re-exports their items so siblings can address everything through
//! `crate::preview::*` without depending on the internal file layout.
//!
//! Submodule overview:
//! - [`types`] - public data types (`SelectRow`, `SelectUiOptions`, ...).
//! - [`run_select_ui`] - the event loop driving the picker.
//! - [`keymap`] - key event to [`PickerAction`] mapping.
//! - [`layout`] - viewport geometry (pane sizes, mouse hit-testing).
//! - [`render`] - frame painting into the reserved inline viewport.
//! - [`source`] - fuzzy-match extraction and preview file reading.
//! - [`text`] - ANSI-aware wrapping and truncation helpers.
//! - [`utils`] - inline viewport reservation and cursor math.
//! - [`terminal_guard`] - RAII raw-mode/mouse-capture guard.

mod keymap;
mod layout;
mod render;
mod run_select_ui;
mod source;
mod terminal_guard;
mod text;
mod types;
mod utils;

/// Public selector types (rows, layout and options).
pub use crate::preview::types::*;

/// Selector UI entry points.
pub use crate::preview::run_select_ui::run_select_ui;
pub(crate) use crate::preview::run_select_ui::run_select_ui_values;

// Internal wiring: re-export implementation items so sibling modules (and
// the test module below) can reach them via `super::` / `crate::preview::`.
pub(crate) use crate::preview::keymap::{PickerAction, handle_key_event};
pub(crate) use crate::preview::layout::{
    max_preview_scroll_offset, mouse_over_preview, preview_content_height,
};
// Only the test module consumes this through `crate::preview::`; gate the
// re-export to keep non-test builds warning-free.
#[cfg(test)]
pub(crate) use crate::preview::layout::bottom_preview_height;
pub(crate) use crate::preview::render::{PreviewUi, draw_preview_ui};
pub(crate) use crate::preview::source::{matched_rows, render_preview};

#[cfg(test)]
#[path = "../tests/preview_test.rs"]
mod tests;
