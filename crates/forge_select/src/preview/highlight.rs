//! Syntect-based syntax highlighting for the preview pane.
//!
//! The escape sequences emitted here survive wrapping and truncation
//! because the downstream renderer (`text::wrap_preview_lines`,
//! `text::truncate_line`) is ANSI-aware.

use std::path;
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

/// Syntect theme used for preview highlighting.
const PREVIEW_THEME: &str = "base16-ocean.dark";

/// Returns the process-wide syntect assets, loading the bundled syntax and
/// theme sets exactly once (loading them is expensive).
fn highlight_assets() -> &'static (SyntaxSet, ThemeSet) {
    static ASSETS: OnceLock<(SyntaxSet, ThemeSet)> = OnceLock::new();
    ASSETS.get_or_init(|| (SyntaxSet::load_defaults_newlines(), ThemeSet::load_defaults()))
}

/// Syntax-highlights preview `content` based on the file `path`, returning
/// ANSI-escaped text ready for the terminal.
///
/// The syntax is chosen by file extension first, then by first-line
/// detection (e.g. shebangs). Content whose syntax cannot be determined is
/// returned unchanged, as are lines that fail to highlight, so the preview
/// always degrades gracefully to plain text.
pub(crate) fn highlight_preview(content: &str, path: &path::Path) -> String {
    let (syntax_set, theme_set) = highlight_assets();
    let Some(theme) = theme_set.themes.get(PREVIEW_THEME) else {
        return content.to_string();
    };

    // Detect the syntax from the extension, falling back to first-line
    // detection for extensionless files such as scripts with shebangs.
    let syntax = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| syntax_set.find_syntax_by_extension(extension))
        .or_else(|| {
            content
                .lines()
                .next()
                .and_then(|line| syntax_set.find_syntax_by_first_line(line))
        });
    let Some(syntax) = syntax else {
        // Unknown file type: skip highlighting entirely instead of paying
        // for a plain-text highlight pass.
        return content.to_string();
    };

    // Highlight line by line, closing styles at each line end so wrapped or
    // truncated lines never leak colors into neighboring cells.
    let mut highlighter = HighlightLines::new(syntax, theme);
    content
        .lines()
        .map(|line| match highlighter.highlight_line(line, syntax_set) {
            Ok(ranges) => format!("{}\x1b[0m", as_24_bit_terminal_escaped(&ranges, false)),
            Err(_) => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "../../tests/highlight_test.rs"]
mod tests;
