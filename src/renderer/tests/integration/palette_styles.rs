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

    for (selector, property, expected) in [
        (".presentation-current", "background-color", "#071522"),
        (".presentation-current", "color", "#F3EAD7"),
        (".presentation-current .artist", "color", "#C9C5BD"),
        (".presentation-current .album", "color", "#C9C5BD"),
        (".presentation-current .lyric-next", "color", "#C9C5BD"),
        (
            ".presentation-current .lyric-masthead-artist",
            "color",
            "#C9C5BD",
        ),
        (".presentation-current .time", "color", "#C9C5BD"),
        (".presentation-current .identity-name", "color", "#C9C5BD"),
        (".presentation-current .title", "color", "#F3EAD7"),
        (".presentation-current .lyric-current", "color", "#F3EAD7"),
        (
            ".presentation-current .lyric-masthead-title",
            "color",
            "#F3EAD7",
        ),
        (
            ".presentation-current .full-field-heading",
            "color",
            "#F3EAD7",
        ),
        (".presentation-current .activity-detail", "color", "#9299A8"),
        (".presentation-current .lyric-previous", "color", "#9299A8"),
        (
            ".presentation-current .full-field-explanation",
            "color",
            "#9299A8",
        ),
        (".presentation-current .identity-label", "color", "#9299A8"),
        (
            ".presentation-current .identity-separator",
            "background-color",
            "#9299A8",
        ),
        (
            ".presentation-current.full-field .full-copy",
            "border-left",
            "6px solid #FF7051",
        ),
        (
            ".presentation-current .activity-waveform",
            "color",
            "#FF7051",
        ),
        (
            ".presentation-current .activity-heading",
            "color",
            "#F3EAD7",
        ),
        (
            ".presentation-current .progress-track",
            "background-color",
            "#2F3645",
        ),
        (
            ".presentation-current .progress-fill trough",
            "min-height",
            "5px",
        ),
        (
            ".presentation-current .progress-fill progress",
            "min-height",
            "5px",
        ),
        (
            ".presentation-current .progress-fill progress",
            "background-color",
            "#FF7051",
        ),
    ] {
        assert_eq!(
            css_property(&styles, selector, property).as_deref(),
            Some(expected),
            "{selector}: {property}"
        );
    }
    assert_eq!(
        css_property(
            &styles,
            ".presentation-current.now-playing",
            "background-image"
        ),
        None,
        "Now Playing gradient ownership should stay with the renderer texture"
    );
}

#[test]
fn presentation_status_emphasis_uses_full_and_muted_accent_without_glow() {
    let palette = PresentationPalette::fallback();
    let layout = NowPlayingLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

    for (selector, color) in [
        (".presentation-current .status-full", "#FF7051"),
        (".presentation-current .status-muted", "#C38781"),
    ] {
        assert_eq!(
            css_property(&styles, selector, "color").as_deref(),
            Some(color)
        );
        for property in ["box-shadow", "text-shadow"] {
            assert_eq!(
                css_property(&styles, selector, property),
                None,
                "{selector} must have no {property}"
            );
        }
    }
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

    for (selector, color) in [
        (".presentation-current .progress-track", "#2F3645"),
        (".presentation-current .progress-fill progress", "#FF7051"),
        (".presentation-outgoing .progress-track", "#404550"),
        (".presentation-outgoing .progress-fill progress", "#D8B032"),
    ] {
        assert_eq!(
            css_property(&styles, selector, "background-color").as_deref(),
            Some(color),
            "{selector}"
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

    for (selector, property, expected) in [
        (
            ".presentation-current .artwork-print-plate",
            "background-color",
            "#FF7051",
        ),
        (
            ".presentation-current .artwork",
            "border",
            "1px solid alpha(#F3EAD7, 0.16)",
        ),
        (
            ".presentation-current .artwork",
            "background-color",
            "#142856",
        ),
        (
            ".presentation-current .artwork",
            "box-shadow",
            "0 3px 12px alpha(#071522, 0.38)",
        ),
    ] {
        assert_eq!(
            css_property(&styles, selector, property).as_deref(),
            Some(expected),
            "{selector}: {property}"
        );
    }
    assert_eq!(
        css_property(
            &styles,
            ".presentation-current .artwork-print-plate",
            "box-shadow"
        ),
        None,
        "the shadow must not move behind the combined artwork-and-plate stack"
    );
}

#[test]
fn artwork_keyline_and_shadow_reach_the_approved_television_geometry() {
    let palette = PresentationPalette::fallback();
    let layout = NowPlayingLayout::for_viewport(Viewport::new(3_840, 2_160));
    let full_field_layout = FullFieldLayout::for_viewport(Viewport::new(3_840, 2_160));
    let styles =
        PresentationTransitionStyles::new(palette, None).to_css(&layout, &full_field_layout);

    for (property, expected) in [
        ("border", "2px solid alpha(#F3EAD7, 0.16)"),
        ("background-color", "#142856"),
        ("box-shadow", "0 6px 28px alpha(#071522, 0.38)"),
    ] {
        assert_eq!(
            css_property(&styles, ".presentation-current .artwork", property).as_deref(),
            Some(expected),
            "{property}"
        );
    }
}

// Inspect the flat rules emitted by to_css, independent of selector grouping,
// declaration order, and whitespace. Native renderer tests check GTK parsing.
fn css_property(css: &str, selector: &str, property: &str) -> Option<String> {
    css.split('}')
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(selectors, _)| {
            selectors
                .split(',')
                .any(|candidate| candidate.split_whitespace().eq(selector.split_whitespace()))
        })
        .flat_map(|(_, declarations)| declarations.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .next_back()
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
