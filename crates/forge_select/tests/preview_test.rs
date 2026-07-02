use crate::preview::{
    PreviewLayout, PreviewPlacement, bottom_preview_height, desired_select_viewport_height,
    preview_select_viewport_height,
};

#[test]
fn test_desired_select_viewport_height_right_ignores_preview_line_count() {
    let fixture = PreviewLayout { placement: PreviewPlacement::Right, percent: 50 };
    let actual = desired_select_viewport_height(1, 2, 285, fixture);
    let expected = 5;
    assert_eq!(actual, expected);
}

#[test]
fn test_desired_select_viewport_height_bottom_includes_preview_line_count() {
    let fixture = PreviewLayout { placement: PreviewPlacement::Bottom, percent: 50 };
    let actual = desired_select_viewport_height(1, 2, 4, fixture);
    let expected = 11;
    assert_eq!(actual, expected);
}

#[test]
fn test_preview_select_viewport_height_keeps_prompt_safety_row() {
    let fixture = 20;
    let actual = preview_select_viewport_height(fixture);
    let expected = 19;
    assert_eq!(actual, expected);
}

#[test]
fn test_bottom_preview_height_keeps_preview_in_small_windows() {
    let fixture = (10, 50);
    let actual = bottom_preview_height(fixture.0, fixture.0, fixture.1);
    let expected = 7;
    assert_eq!(actual, expected);
}

#[test]
fn test_bottom_preview_height_caps_preview_to_keep_visible_list() {
    let fixture = (20, 50);
    let actual = bottom_preview_height(fixture.0, fixture.0, fixture.1);
    let expected = 14;
    assert_eq!(actual, expected);
}

#[test]
fn test_bottom_preview_height_uses_extra_body_space() {
    let fixture = (28, 28, 50);
    let actual = bottom_preview_height(fixture.0, fixture.1, fixture.2);
    let expected = 20;
    assert_eq!(actual, expected);
}
