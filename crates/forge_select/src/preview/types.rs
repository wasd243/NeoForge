use std::path::PathBuf;
use derive_setters::Setters;
use crate::preview::{run_select_ui, run_select_ui_values};
use crate::SelectMode;

/// Row rendered by the shared selector UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectRow {
    /// Machine-readable value returned when the row is selected.
    pub raw: String,
    /// User-facing text rendered in the selector list.
    pub display: String,
    /// Text indexed by the fuzzy matcher.
    pub search: String,
    /// Additional machine-readable fields used for preview placeholder
    /// expansion.
    pub fields: Vec<String>,
}

/// Placement of the selector preview pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewPlacement {
    /// Render preview to the right of the list.
    Right,
    /// Render preview below the list.
    Bottom,
}

/// Preview pane layout configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewLayout {
    /// Preview pane placement.
    pub placement: PreviewPlacement,
    /// Percentage of available space allocated to preview.
    pub percent: u16,
}

/// Options for running the shared selector UI.
#[derive(Debug, Setters)]
#[setters(into)]
pub struct SelectUiOptions {
    /// Optional prompt text displayed before the query.
    #[setters(skip)]
    pub prompt: Option<String>,
    /// Optional initial search query.
    pub query: Option<String>,
    /// Rows rendered by the selector.
    pub rows: Vec<SelectRow>,
    /// Number of leading rows treated as non-selectable headers.
    pub header_lines: usize,
    /// Selection mode.
    pub mode: SelectMode,
    /// Optional shell command used to render the selected row preview.
    pub preview: Option<String>,
    /// Preview pane layout.
    pub preview_layout: PreviewLayout,
    /// Optional raw value to focus initially.
    pub initial_raw: Option<String>,
    /// Optional working directory for resolving relative paths in preview.
    #[setters(skip)]
    pub working_dir: Option<PathBuf>,
}

impl SelectUiOptions {
    /// Creates selector options for the provided prompt and rows.
    pub fn new(prompt: impl Into<String>, rows: Vec<SelectRow>) -> Self {
        Self {
            prompt: Some(prompt.into()),
            query: None,
            rows,
            header_lines: 0,
            mode: SelectMode::Single,
            preview: None,
            preview_layout: PreviewLayout::default(),
            initial_raw: None,
            working_dir: None,
        }
    }

    /// Sets the working directory used to resolve relative paths when
    /// rendering the preview of the selected row.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Base directory against which relative row values are
    ///   resolved. `None` falls back to the process working directory.
    pub fn working_dir(mut self, working_dir: Option<PathBuf>) -> Self {
        self.working_dir = working_dir;
        self
    }

    /// Runs the selector and returns the selected row.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup, event handling, rendering, or
    /// preview command execution setup fails.
    pub fn prompt(self) -> anyhow::Result<Option<SelectRow>> {
        let rows = self.rows.clone();
        let selected_raw = run_select_ui(self)?;
        Ok(selected_raw.and_then(|raw| rows.into_iter().find(|row| row.raw == raw)))
    }

    /// Runs the selector and returns all selected rows.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup, event handling, rendering, or
    /// preview command execution setup fails.
    pub fn prompt_multi(self) -> anyhow::Result<Option<Vec<SelectRow>>> {
        let rows = self.rows.clone();
        let selected_raws = run_select_ui_values(self)?;
        Ok(selected_raws.map(|raws| {
            raws.into_iter()
                .filter_map(|raw| rows.iter().find(|row| row.raw == raw).cloned())
                .collect()
        }))
    }
}
