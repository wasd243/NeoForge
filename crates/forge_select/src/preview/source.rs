//! Data sources for the picker: fuzzy-match extraction and preview content.

use std::fs;
use std::path::{self, PathBuf};

use colored::Colorize;
use nucleo::Nucleo;

use super::types::SelectRow;

/// Maximum number of lines read from a file when rendering its preview.
const PREVIEW_MAX_LINES: usize = 500;

/// Collects the rows currently matched by the nucleo fuzzy matcher, in
/// match-score order.
pub(crate) fn matched_rows(matcher: &Nucleo<SelectRow>) -> Vec<&SelectRow> {
    matcher
        .snapshot()
        .matched_items(..)
        .map(|item| item.data)
        .collect()
}

/// Renders the preview text for the selected row by reading the referenced
/// file directly (no shell involved, cross-platform).
///
/// `command` acts only as an on/off switch: a blank command disables the
/// preview entirely and returns an empty string. Relative row values are
/// resolved against `working_dir` when provided, otherwise against the
/// process working directory. Directories and unreadable files produce a
/// human-readable message instead of file content.
///
/// # Arguments
///
/// * `command` - Preview toggle; blank disables the preview.
/// * `row` - Selected row whose `raw` value is treated as a file path.
/// * `working_dir` - Base directory for resolving relative paths.
pub(crate) fn render_preview(
    command: &str,
    row: &SelectRow,
    working_dir: Option<&path::Path>,
) -> String {
    if command.trim().is_empty() {
        return String::new();
    }

    // Resolve the row value to a concrete path: absolute values are used
    // as-is, relative ones are joined onto the working directory when one is
    // available.
    let path = if row.raw.is_empty() {
        PathBuf::new()
    } else if path::Path::new(&row.raw).is_absolute() {
        PathBuf::from(&row.raw)
    } else if let Some(base_dir) = working_dir {
        base_dir.join(&row.raw)
    } else {
        PathBuf::from(&row.raw)
    };

    if path.is_dir() {
        return format!(
            "{}: {}",
            row.display,
            "is a directory (no preview available)".bright_red().italic()
        );
    }

    // Cap the preview to keep rendering and ANSI-aware wrapping cheap for
    // very large files.
    match fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .take(PREVIEW_MAX_LINES)
            .collect::<Vec<_>>()
            .join("\n"),
        Err(error) => format!("Cannot preview {}: {error}", row.display),
    }
}
