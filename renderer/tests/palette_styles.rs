use roonscape_renderer::{
    FullFieldLayout, GallerySplitLayout, PresentationPalette, Viewport, presentation_palette_styles,
};

#[test]
fn semantic_palette_roles_drive_every_presentation_surface() {
    let palette = PresentationPalette::fallback();
    let layout = GallerySplitLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles =
        presentation_palette_styles("presentation-current", palette, &layout, &full_field_layout);

    for declaration in [
        "background-color: #071522; color: #F3EAD7",
        "linear-gradient(118deg, #142856 0%, #071522 62%, #0A1429 100%)",
        ".artist, .presentation-current .album, .presentation-current .time, .presentation-current .identity-name { color: #C9C5BD; }",
        ".identity-label { color: #9299A8; }",
        ".full-field .full-copy { border-left: 6px solid #FF7051; }",
        ".playback-state { color: #FF7051; }",
        ".full-field-heading { color: #F3EAD7; }",
        ".full-field-explanation { color: #9299A8; }",
        "progressbar trough { background-color: #9299A8; }",
        "progressbar progress { background-color: #FF7051; }",
    ] {
        assert!(
            styles.contains(declaration),
            "presentation styles should contain {declaration:?}"
        );
    }
}
