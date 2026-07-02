use crate::select::parse_preview_layout;
use crate::{ForgeWidget, PreviewLayout, PreviewPlacement};
use console::strip_ansi_codes;

#[test]
fn test_select_builder_creates() {
    let builder = ForgeWidget::select("Test", vec!["a", "b", "c"]);
    assert_eq!(builder.message, "Test");
    assert_eq!(builder.options, vec!["a", "b", "c"]);
}

#[test]
fn test_confirm_builder_creates() {
    let builder = ForgeWidget::confirm("Confirm?");
    assert_eq!(builder.message, "Confirm?");
}

#[test]
fn test_select_builder_with_initial_text() {
    let builder =
        ForgeWidget::select("Test", vec!["apple", "banana", "cherry"]).with_initial_text("app");
    assert_eq!(builder.initial_text, Some("app".to_string()));
}

#[test]
fn test_select_owned_builder_with_initial_text() {
    let builder =
        ForgeWidget::select("Test", vec!["apple", "banana", "cherry"]).with_initial_text("ban");
    assert_eq!(builder.initial_text, Some("ban".to_string()));
}

#[test]
fn test_ansi_stripping() {
    let fixture = ["\x1b[1mBold\x1b[0m", "\x1b[31mRed\x1b[0m"];
    let actual: Vec<String> = fixture
        .iter()
        .map(|value| strip_ansi_codes(value).to_string())
        .collect();
    let expected = vec!["Bold", "Red"];
    assert_eq!(actual, expected);
}

#[test]
fn test_display_options_are_trimmed() {
    let fixture = [
        "  openai               [empty]",
        "✓ anthropic            [api.anthropic.com]",
    ];
    let actual: Vec<String> = fixture
        .iter()
        .map(|value| strip_ansi_codes(value).trim().to_string())
        .collect();
    let expected = vec![
        "openai               [empty]".to_string(),
        "✓ anthropic            [api.anthropic.com]".to_string(),
    ];
    assert_eq!(actual, expected);
}

#[test]
fn test_with_starting_cursor() {
    let builder = ForgeWidget::select("Test", vec!["a", "b", "c"]).with_starting_cursor(2);
    assert_eq!(builder.starting_cursor, Some(2));
}

#[test]
fn test_parse_preview_layout_defaults_to_right() {
    let fixture = None;
    let actual = parse_preview_layout(fixture);
    let expected = PreviewLayout { placement: PreviewPlacement::Right, percent: 50 };
    assert_eq!(actual, expected);
}

#[test]
fn test_parse_preview_layout_supports_bottom_percent() {
    let fixture = Some("down,60%");
    let actual = parse_preview_layout(fixture);
    let expected = PreviewLayout { placement: PreviewPlacement::Bottom, percent: 60 };
    assert_eq!(actual, expected);
}
