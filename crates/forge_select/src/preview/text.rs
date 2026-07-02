//! ANSI-escape-aware text helpers for the selector renderer.
//!
//! All width computations here count visible characters only: ANSI escape
//! sequences (`ESC` up to the terminating alphabetic byte or `~`) are copied
//! through verbatim without contributing to the visible width.

/// Formats the `current/total` scroll position indicator shown in the
/// bottom preview pane border.
pub(crate) fn preview_scroll_indicator(scroll_offset: usize, line_count: usize) -> String {
    format!("{}/{line_count}", scroll_offset.saturating_add(1))
}

/// Wraps every line of the preview text to `max_width` visible columns,
/// returning the flattened list of display lines.
///
/// Returns an empty list when `max_width` is zero (no room to render).
pub(crate) fn wrap_preview_lines(preview: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }

    preview
        .lines()
        .flat_map(|line| wrap_ansi_line(line, max_width))
        .collect()
}

/// Wraps a single line to `max_width` visible columns, preserving ANSI
/// escape sequences and prefixing continuation lines with a `↪ ` marker.
///
/// Empty input produces a single empty line so callers still render a row.
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
        // Copy ANSI escape sequences through without counting their width.
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

        // Continuation lines lose the columns taken by the wrap marker.
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

    // Flush the trailing partial line, if any.
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

/// Joins the prompt and query with a single separating space, unless the
/// query is empty or the prompt already ends with whitespace.
pub(crate) fn format_prompt_query(prompt: &str, query: &str) -> String {
    if query.is_empty() || prompt.ends_with(char::is_whitespace) {
        format!("{prompt}{query}")
    } else {
        format!("{prompt} {query}")
    }
}

/// Returns the rendered width of the `matched/total` counter shown next to
/// the separator line.
pub(crate) fn match_count_width(matched: usize, total: usize) -> u16 {
    format!("{matched}/{total}").chars().count() as u16
}

/// Truncates `value` to `max_width` characters, appending an ellipsis when
/// content was cut off. Intended for plain (non-ANSI) list rows.
pub(crate) fn truncate_line_with_ellipsis(value: &str, max_width: usize) -> String {
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

/// Truncates `value` to `max_width` visible columns while preserving ANSI
/// escape sequences, appending a style reset when styled content was cut so
/// stray attributes never bleed into subsequent cells.
pub(crate) fn truncate_line(value: &str, max_width: usize) -> String {
    let mut rendered = String::new();
    let mut visible_width = 0usize;
    let mut chars = value.chars().peekable();
    let mut truncated = false;
    let mut has_ansi = false;

    while let Some(ch) = chars.next() {
        // Copy ANSI escape sequences through without counting their width.
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

    // Close any open styling so the cut-off does not leak colors.
    if truncated && has_ansi {
        rendered.push_str("\u{1b}[0m");
    }

    rendered
}
