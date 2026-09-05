use roonscape_renderer::{
    DiagnosticsStyle, FullFieldLayout, NowPlayingLayout, PresentationPalette,
    PresentationStyleLayer, PresentationTransitionStyles, Rgb, Viewport,
};

#[test]
fn semantic_palette_roles_drive_every_presentation_surface() {
    let palette = PresentationPalette::fallback();
    let layout = NowPlayingLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

    for declaration in [
        "background-color: #071522; color: #F3EAD7",
        ".artist, .presentation-current .album, .presentation-current .lyric-next, .presentation-current .lyric-masthead-artist, .presentation-current .time, .presentation-current .identity-name { color: #C9C5BD; }",
        ".title, .presentation-current .lyric-current, .presentation-current .lyric-masthead-title, .presentation-current .full-field-heading { color: #F3EAD7; }",
        ".activity-detail, .presentation-current .lyric-previous, .presentation-current .full-field-explanation { color: #9299A8; }",
        ".identity-label { color: #9299A8; }",
        ".identity-separator { background-color: #9299A8; }",
        ".full-field .full-copy { border-left: 6px solid #FF7051; }",
        ".presentation-current .status-full { color: #FF7051; }",
        ".full-field-heading { color: #F3EAD7; }",
        ".full-field-explanation { color: #9299A8; }",
        ".activity-waveform { color: #FF7051; }",
        ".activity-heading { color: #F3EAD7; }",
        ".progress-track { background-color: #2F3645; }",
        ".progress-fill trough, .presentation-current .progress-fill progress { min-height: 5px; }",
        ".progress-fill progress { background-color: #FF7051; }",
    ] {
        assert!(
            styles.contains(declaration),
            "presentation styles should contain {declaration:?}"
        );
    }
    assert!(
        !styles.contains(".presentation-current.now-playing { background-image"),
        "Now Playing gradient ownership should stay with the renderer texture",
    );
}

#[test]
fn presentation_status_emphasis_uses_full_and_muted_accent_without_glow() {
    let palette = PresentationPalette::fallback();
    let layout = NowPlayingLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

    for declaration in [
        ".presentation-current .status-full { color: #FF7051; }",
        ".presentation-current .status-muted { color: #C38781; }",
    ] {
        assert!(
            styles.contains(declaration),
            "Presentation Status styles should contain {declaration:?}",
        );
    }
    let status_uses_shadow = styles
        .lines()
        .any(|rule| rule.contains("status") && rule.contains("shadow"));
    assert!(!styles.contains("glow"));
    assert!(
        !status_uses_shadow,
        "generated Presentation Status styles must contain no shadow or halo CSS",
    );
}

#[test]
fn transition_layers_keep_their_own_progress_palette_roles() {
    let current = PresentationPalette::fallback();
    let outgoing = PresentationPalette {
        progress_track: rgb(0x40, 0x45, 0x50),
        progress_fill: rgb(0xd8, 0xb0, 0x32),
        ..current
    };
    let layout = NowPlayingLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles = PresentationTransitionStyles::new(current, Some(outgoing))
        .to_css(&layout, &full_field_layout);

    for declaration in [
        ".presentation-current .progress-track { background-color: #2F3645; }",
        ".presentation-current .progress-fill progress { background-color: #FF7051; }",
        ".presentation-outgoing .progress-track { background-color: #404550; }",
        ".presentation-outgoing .progress-fill progress { background-color: #D8B032; }",
    ] {
        assert!(
            styles.contains(declaration),
            "each transition layer should retain {declaration:?}",
        );
    }
}

#[test]
fn print_plate_uses_the_accent_while_depth_stays_on_the_artwork_surface() {
    let palette = PresentationPalette::fallback();
    let layout = NowPlayingLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);

    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

    assert!(
        styles
            .contains(".presentation-current .artwork-print-plate { background-color: #FF7051; }")
    );
    assert!(styles.contains(
        ".presentation-current .artwork { border: 1px solid alpha(#F3EAD7, 0.16); background-color: #142856; box-shadow: 0 3px 12px alpha(#071522, 0.38); }"
    ));
    assert!(
        !styles.contains(".artwork-print-plate { box-shadow:"),
        "the shadow must not move behind the combined artwork-and-plate stack",
    );
}

#[test]
fn artwork_keyline_and_shadow_reach_the_approved_television_geometry() {
    let palette = PresentationPalette::fallback();
    let layout = NowPlayingLayout::for_viewport(Viewport::new(3_840, 2_160));
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::new(3_840, 2_160));

    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

    assert!(styles.contains(
        ".presentation-current .artwork { border: 2px solid alpha(#F3EAD7, 0.16); background-color: #142856; box-shadow: 0 6px 28px alpha(#071522, 0.38); }"
    ));
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
