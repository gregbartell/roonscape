use roonscape_renderer::{FullFieldLayout, IdentityPlacement, Viewport};

#[test]
fn scales_the_shared_full_field_accent_composition_from_reference_to_fixture() {
    let reference = FullFieldLayout::for_viewport(Viewport::REFERENCE);
    let tall = FullFieldLayout::for_viewport(Viewport::new(3840, 2400));
    let windowed = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    assert_eq!(reference.outer_gutter_px, 160);
    assert_eq!(reference.copy_width_px, 1088);
    assert_eq!(reference.accent_width_px, 15);
    assert_eq!(reference.accent_padding_px, 144);
    assert_eq!(reference.heading_px, 208);
    assert_eq!(reference.explanation_px, 46);
    assert_eq!(reference.identity_width_px, 864);
    assert_eq!(reference.identity_right_inset_px, 77);
    assert_eq!(reference.identity_placement, IdentityPlacement::BottomRight);

    assert_eq!(tall.outer_gutter_px, reference.outer_gutter_px);
    assert_eq!(tall.copy_width_px, reference.copy_width_px);
    assert_eq!(tall.heading_px, reference.heading_px);
    assert_eq!(tall.status_spacing_px, 80);

    assert_eq!(windowed.outer_gutter_px, 67);
    assert_eq!(windowed.copy_width_px, 1088);
    assert_eq!(windowed.accent_width_px, 6);
    assert_eq!(windowed.accent_padding_px, 64);
    assert_eq!(windowed.heading_px, 99);
    assert_eq!(windowed.explanation_px, 22);
    assert_eq!(windowed.identity_width_px, 608);
    assert_eq!(windowed.identity_right_inset_px, 32);
    assert_eq!(windowed.identity_placement, IdentityPlacement::BottomRight);
}
