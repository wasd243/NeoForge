use std::path;
use pretty_assertions::assert_eq;
use crate::preview::highlight::highlight_preview;

#[test]
fn test_highlight_preview_adds_ansi_for_known_extension() {
    let fixture = "fn main() {}";
    let actual = highlight_preview(fixture, path::Path::new("main.rs"));
    assert!(actual.contains('\u{1b}'));
}

#[test]
fn test_highlight_preview_detects_syntax_from_shebang_line() {
    let fixture = "#!/bin/bash\necho hello";
    let actual = highlight_preview(fixture, path::Path::new("script"));
    assert!(actual.contains('\u{1b}'));
}

#[test]
fn test_highlight_preview_passes_through_unknown_syntax() {
    let fixture = "plain text content";
    let actual = highlight_preview(fixture, path::Path::new("notes.zzzunknown"));
    let expected = "plain text content";
    assert_eq!(actual, expected);
}

#[test]
fn test_highlight_preview_keeps_line_count() {
    let fixture = "fn main() {\n    let x = 1;\n}";
    let actual = highlight_preview(fixture, path::Path::new("main.rs"))
        .lines()
        .count();
    let expected = 3;
    assert_eq!(actual, expected);
}
