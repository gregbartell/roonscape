use roonscape_renderer::{
    DiagnosticsStyle, FullFieldLayout, GallerySplitLayout, PresentationPalette,
    PresentationStyleLayer, PresentationTransitionStyles, Rgb, Viewport,
};

#[test]
fn semantic_palette_roles_drive_every_presentation_surface() {
    let palette = PresentationPalette::fallback();
    let layout = GallerySplitLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

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

#[test]
fn semantic_palette_roles_adapt_the_diagnostics_overlay() {
    let fallback = PresentationPalette::fallback();
    let dark = PresentationPalette {
        diagnostics_field: rgb(0x10, 0x10, 0x10),
        diagnostics_text: rgb(0xfa, 0xfa, 0xfa),
        diagnostics_border: rgb(0xff, 0xaa, 0x00),
        ..fallback
    };
    let light = PresentationPalette {
        diagnostics_field: rgb(0xf6, 0xf0, 0xdc),
        diagnostics_text: rgb(0x18, 0x14, 0x0f),
        diagnostics_border: rgb(0x7c, 0x31, 0x0e),
        ..fallback
    };

    assert_eq!(
        PresentationTransitionStyles::new(fallback, None).diagnostics(),
        vec![DiagnosticsStyle {
            layer: PresentationStyleLayer::Current,
            field: fallback.diagnostics_field,
            text: fallback.diagnostics_text,
            border: fallback.diagnostics_border,
        }]
    );
    assert_eq!(
        PresentationTransitionStyles::new(light, Some(dark)).diagnostics(),
        vec![
            DiagnosticsStyle {
                layer: PresentationStyleLayer::Current,
                field: light.diagnostics_field,
                text: light.diagnostics_text,
                border: light.diagnostics_border,
            },
            DiagnosticsStyle {
                layer: PresentationStyleLayer::Outgoing,
                field: dark.diagnostics_field,
                text: dark.diagnostics_text,
                border: dark.diagnostics_border,
            },
        ]
    );
}

fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}
