use roonscape_renderer::{
    FullFieldLayout, GallerySplitLayout, PresentationPalette, Rgb, Viewport,
    diagnostics_palette_styles, presentation_palette_styles,
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

#[test]
fn semantic_palette_roles_adapt_the_diagnostics_overlay() {
    let fallback = PresentationPalette::fallback();
    let palettes = [
        (
            "fixed fallback",
            fallback,
            ".diagnostics { color: #F3EAD7; background-color: #0A1429; border-color: #FF7051; }",
        ),
        (
            "dark artwork",
            PresentationPalette {
                diagnostics_field: Rgb {
                    red: 0x10,
                    green: 0x10,
                    blue: 0x10,
                },
                diagnostics_text: Rgb {
                    red: 0xfa,
                    green: 0xfa,
                    blue: 0xfa,
                },
                diagnostics_border: Rgb {
                    red: 0xff,
                    green: 0xaa,
                    blue: 0x00,
                },
                ..fallback
            },
            ".diagnostics { color: #FAFAFA; background-color: #101010; border-color: #FFAA00; }",
        ),
        (
            "light artwork",
            PresentationPalette {
                diagnostics_field: Rgb {
                    red: 0xf6,
                    green: 0xf0,
                    blue: 0xdc,
                },
                diagnostics_text: Rgb {
                    red: 0x18,
                    green: 0x14,
                    blue: 0x0f,
                },
                diagnostics_border: Rgb {
                    red: 0x7c,
                    green: 0x31,
                    blue: 0x0e,
                },
                ..fallback
            },
            ".diagnostics { color: #18140F; background-color: #F6F0DC; border-color: #7C310E; }",
        ),
    ];

    for (source, palette, expected) in palettes {
        assert_eq!(
            diagnostics_palette_styles(palette).trim(),
            expected,
            "diagnostics should consume the {source} semantic roles"
        );
    }
}
