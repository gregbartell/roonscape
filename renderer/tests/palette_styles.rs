use roonscape_renderer::{
    GallerySplitLayout, PresentationPalette, Viewport, presentation_palette_styles,
};

#[test]
fn semantic_palette_roles_drive_every_presentation_surface() {
    let palette = PresentationPalette::fallback();
    let layout = GallerySplitLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles = presentation_palette_styles("presentation-current", palette, &layout);

    for declaration in [
        "background-color: #071522; color: #F3EAD7",
        "linear-gradient(118deg, #142856 0%, #071522 62%, #0A1429 100%)",
        ".artist, .presentation-current .album, .presentation-current .time, .presentation-current .identity-name, .presentation-current .unavailable-explanation { color: #C9C5BD; }",
        ".identity-label { color: #9299A8; }",
        ".playback-state, .presentation-current .unavailable-state { color: #FF7051; }",
        "progressbar trough { background-color: #9299A8; }",
        "progressbar progress { background-color: #FF7051; }",
    ] {
        assert!(
            styles.contains(declaration),
            "presentation styles should contain {declaration:?}"
        );
    }
}
