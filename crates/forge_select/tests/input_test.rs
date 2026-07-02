use crate::ForgeWidget;
use pretty_assertions::assert_eq;
use crate::input::strip_bracketed_paste;

#[test]
fn test_input_builder_creates() {
    let builder = ForgeWidget::input("Enter name");
    assert_eq!(builder.message, "Enter name");
    assert_eq!(builder.allow_empty, false);
}

#[test]
fn test_input_builder_with_default() {
    let builder = ForgeWidget::input("Enter key").with_default("mykey");
    assert_eq!(builder.default, Some("mykey".to_string()));
}

#[test]
fn test_input_builder_allow_empty() {
    let builder = ForgeWidget::input("Enter").allow_empty(true);
    assert_eq!(builder.allow_empty, true);
}

#[test]
fn test_strip_bracketed_paste() {
    let fixture = "\x1b[200~myapikey\x1b[201~";
    let actual = strip_bracketed_paste(fixture);
    let expected = "myapikey";
    assert_eq!(actual, expected);

    let fixture = "myapikey";
    let actual = strip_bracketed_paste(fixture);
    let expected = "myapikey";
    assert_eq!(actual, expected);

    let fixture = "\x1b[200~myapikey";
    let actual = strip_bracketed_paste(fixture);
    let expected = "myapikey";
    assert_eq!(actual, expected);
}
