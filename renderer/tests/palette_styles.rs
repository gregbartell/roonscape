use roonscape_renderer::{
    FullFieldLayout, GallerySplitLayout, PresentationPalette, Rgb, Viewport,
    presentation_palette_styles,
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
    let layout = GallerySplitLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let palettes = [
        (
            "presentation-current",
            "fixed fallback",
            fallback,
            [
                "color: #F3EAD7",
                "background-color: #0A1429",
                "border-color: #FF7051",
            ],
        ),
        (
            "presentation-outgoing",
            "dark artwork",
            PresentationPalette {
                diagnostics_field: rgb(0x10, 0x10, 0x10),
                diagnostics_text: rgb(0xfa, 0xfa, 0xfa),
                diagnostics_border: rgb(0xff, 0xaa, 0x00),
                ..fallback
            },
            [
                "color: #FAFAFA",
                "background-color: #101010",
                "border-color: #FFAA00",
            ],
        ),
        (
            "presentation-current",
            "light artwork",
            PresentationPalette {
                diagnostics_field: rgb(0xf6, 0xf0, 0xdc),
                diagnostics_text: rgb(0x18, 0x14, 0x0f),
                diagnostics_border: rgb(0x7c, 0x31, 0x0e),
                ..fallback
            },
            [
                "color: #18140F",
                "background-color: #F6F0DC",
                "border-color: #7C310E",
            ],
        ),
    ];

    for (layer, source, palette, expected_roles) in palettes {
        let styles = presentation_palette_styles(layer, palette, &layout, &full_field_layout);
        let diagnostics = styles
            .lines()
            .find(|line| line.starts_with(&format!(".{layer} .diagnostics")))
            .unwrap_or_else(|| panic!("{source} should style diagnostics on its own layer"));

        for role in expected_roles {
            assert!(
                diagnostics.contains(role),
                "{source} diagnostics should consume semantic role {role:?}"
            );
        }
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}
