#[path = "support/representative_viewports.rs"]
mod representative_viewports;
mod support;

use gtk::pango::prelude::FontFamilyExt;
use gtk::pango::{self, FontDescription, Layout};
use gtk::prelude::FontMapExt;
use roonscape_renderer::{
    FullFieldFontSize, FullFieldLayout, FullFieldLineLayout, IdentityLineLayout, IdentityPlacement,
    NowPlayingLayout, Presentation, PresentationStatusDecoration, TextOverflow, Viewport,
    parse_snapshot, presentation_from_snapshot, register_packaged_fallback_fonts,
};

const FULL_FIELD_FIXTURES: [(&str, bool, bool); 7] = [
    ("stopped.json", false, true),
    ("loading-empty.json", false, true),
    ("pairing-required.json", true, false),
    ("disconnected.json", true, false),
    ("output-unavailable.json", true, false),
    ("playing-empty.json", false, true),
    ("paused-empty.json", false, true),
];

#[test]
fn centers_a_sixty_percent_full_field_composition_inside_every_safe_viewport() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        assert_centered_composition(viewport);
        assert_centered_composition(Viewport::new(
            viewport.width_px - 96,
            viewport.height_px - 72,
        ));
    }

    fn assert_centered_composition(viewport: Viewport) {
        let layout = FullFieldLayout::for_viewport(viewport);
        let left_space = layout.composition_left_viewport_x_px;
        let right_space = viewport.width_px - left_space - layout.composition_width_px;

        assert!(
            (layout.composition_width_px as i64 * 5 - viewport.width_px as i64 * 3).abs() <= 2,
            "full-field composition should occupy 60% at {viewport:?}",
        );
        assert!(
            left_space.abs_diff(right_space) <= 1,
            "full-field composition should be horizontally centered at {viewport:?}",
        );
        assert_eq!(
            layout.text_left_viewport_x_px,
            left_space + layout.accent_width_px + layout.accent_padding_px,
            "all Full-field copy should share the inset text edge at {viewport:?}",
        );
        assert!(layout.composition_width_px <= viewport.width_px);
    }
}

#[test]
fn fixture_scenarios_resolve_full_field_explanations_only_when_required() {
    for (fixture, has_explanation, _) in FULL_FIELD_FIXTURES {
        let snapshot = parse_snapshot(&support::fixture(fixture))
            .expect("Full-field Fixture Scenario should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("Full-field Fixture Scenario should produce a presentation");
        let Presentation::FullField(presentation) = presentation else {
            panic!("{fixture} should use a Full-field Presentation");
        };

        assert_eq!(
            presentation.explanation.is_some(),
            has_explanation,
            "{fixture}"
        );
    }
}

#[test]
fn full_field_presentation_accent_uses_approved_extents() {
    let expected_accent_bottoms_px = [
        (400, 440),
        (500, 552),
        (650, 702),
        (660, 721),
        (620, 703),
        (1_180, 1_293),
        (1_300, 1_413),
    ];

    for (viewport, (heading_bottom_px, explanation_bottom_px)) in
        representative_viewports::REPRESENTATIVE_VIEWPORTS
            .into_iter()
            .zip(expected_accent_bottoms_px)
    {
        let layout = FullFieldLayout::for_viewport(viewport);

        assert_eq!(
            layout.accent_bottom_viewport_y_px(false),
            heading_bottom_px,
            "heading-only copy should use the approved accent extent at {viewport:?}",
        );
        assert_eq!(
            layout.accent_bottom_viewport_y_px(true),
            explanation_bottom_px,
            "explanation-bearing copy should use the approved accent extent at {viewport:?}",
        );
    }
}

#[test]
fn centers_the_fixed_heading_slot_and_adds_explanation_space_only_below_it() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        let heading_slot_bottom = layout.heading_slot.bottom_viewport_y_px();

        assert!(
            (layout.heading_slot.top_viewport_y_px * 2 + layout.heading_slot.height_px)
                .abs_diff(viewport.height_px)
                <= 1,
            "the heading slot should own the vertical center at {viewport:?}",
        );
        assert_eq!(
            layout.explanation_slot.top_viewport_y_px,
            heading_slot_bottom + layout.explanation_spacing_px,
            "the explanation slot should begin below the fixed heading slot at {viewport:?}",
        );
        assert_eq!(
            layout.presentation_status_slot.top_viewport_y_px
                + layout.presentation_status_slot.height_px
                + layout.status_spacing_px,
            layout.heading_slot.top_viewport_y_px,
            "Presentation Status should occupy one stable slot above the heading at {viewport:?}",
        );
        assert!(
            layout.accent_bottom_viewport_y_px(true) <= viewport.height_px,
            "explanation-bearing copy should remain vertically bounded at {viewport:?}",
        );
    }
}

#[test]
fn preserves_the_established_full_field_identity_anchor_but_not_the_now_playing_rail() {
    let expected_bottom_margins_px = [74, 88, 174, 118, 106, 212, 235];

    for (viewport, expected_bottom_margin_px) in representative_viewports::REPRESENTATIVE_VIEWPORTS
        .into_iter()
        .zip(expected_bottom_margins_px)
    {
        let full_field = FullFieldLayout::for_viewport(viewport);
        let now_playing = NowPlayingLayout::for_viewport(viewport);

        assert_eq!(
            full_field.identity_anchor.margin_bottom_px(0),
            expected_bottom_margin_px,
            "Full-field identity geometry should remain unchanged at {viewport:?}",
        );
        assert_ne!(
            full_field.identity_anchor, now_playing.footer_anchor,
            "the raised Now Playing footer must not move Full-field identities at {viewport:?}",
        );
        assert_ne!(
            full_field.presentation_status_slot.top_viewport_y_px,
            now_playing
                .artwork_field_anchors
                .presentation_status_top_viewport_y_px,
            "Full-field status should be independent of the Now Playing imaginary square at {viewport:?}",
        );
        assert_eq!(
            full_field.identity_placement,
            IdentityPlacement::BottomRight
        );
        assert_eq!(
            full_field.identity_line,
            IdentityLineLayout {
                maximum_lines: 1,
                overflow: TextOverflow::EllipsizeEnd,
            }
        );
    }
}

#[test]
fn retains_full_field_horizontal_identity_geometry() {
    let expected_geometry = [
        (428, 26),
        (536, 32),
        (536, 32),
        (643, 38),
        (858, 51),
        (1287, 77),
        (1287, 77),
    ];

    for (viewport, (identity_width_px, identity_right_inset_px)) in
        representative_viewports::REPRESENTATIVE_VIEWPORTS
            .into_iter()
            .zip(expected_geometry)
    {
        let layout = FullFieldLayout::for_viewport(viewport);

        assert_eq!(
            (layout.identity_width_px, layout.identity_right_inset_px),
            (identity_width_px, identity_right_inset_px),
            "Now Playing geometry must not reshape Full-field identities at {viewport:?}",
        );
    }
}

#[test]
fn uses_distance_legible_full_field_status_and_identity_sizes() {
    let expected_symbol_sizes = [56, 62, 62, 74, 96, 96, 96];
    let expected_status_sizes = [29, 35, 35, 42, 52, 52, 52];
    let expected_identity_sizes = [20, 20, 20, 24, 32, 48, 48];

    for (index, viewport) in representative_viewports::REPRESENTATIVE_VIEWPORTS
        .into_iter()
        .enumerate()
    {
        let layout = FullFieldLayout::for_viewport(viewport);
        assert_eq!(
            layout.presentation_status.decoration,
            PresentationStatusDecoration::Circle,
            "Full-field Presentation Status should retain its circular treatment at {viewport:?}",
        );
        assert_eq!(
            layout.presentation_status.symbol_size_px, expected_symbol_sizes[index],
            "Full-field status symbol size should remain unchanged at {viewport:?}",
        );
        assert_eq!(
            layout.presentation_status.font_px, expected_status_sizes[index],
            "Full-field status text should remain readable at {viewport:?}",
        );
        assert_eq!(
            layout.identity_px, expected_identity_sizes[index],
            "Full-field identities should remain readable at {viewport:?}",
        );
    }
}

#[test]
fn raises_the_full_field_explanation_cap_for_television_distance() {
    let layout = FullFieldLayout::for_viewport(Viewport::new(3_840, 2_160));

    assert_eq!(layout.explanation_font.preferred_px, 48);
}

#[test]
fn fixture_scenarios_resolve_full_field_identity_only_when_available() {
    for (fixture, _, has_identity) in FULL_FIELD_FIXTURES {
        let snapshot = parse_snapshot(&support::fixture(fixture))
            .expect("Full-field identity Fixture Scenario should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("Full-field identity Fixture Scenario should produce a presentation");
        let Presentation::FullField(presentation) = presentation else {
            panic!("{fixture} should use a Full-field Presentation");
        };

        assert_eq!(presentation.identity.is_some(), has_identity, "{fixture}");
    }
}

#[test]
fn keeps_approved_full_field_copy_complete_at_the_largest_fitting_size() {
    let font_map = full_field_font_map();
    let context = font_map.create_context();
    let headings = [
        "Nothing is playing",
        "Preparing playback",
        "Enable RoonScape",
        "Waiting for Roon",
        "Check the selected output",
        "Now Playing details unavailable",
    ];
    let explanations = [
        "In a Roon client, open Settings → Extensions and enable RoonScape.",
        "Check Roon Server and the network.",
        "Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.",
    ];

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        assert_eq!(
            (layout.heading_line, layout.explanation_line),
            (
                FullFieldLineLayout {
                    maximum_lines: 1,
                    wrap: false,
                    overflow: TextOverflow::EllipsizeEnd,
                },
                FullFieldLineLayout {
                    maximum_lines: 1,
                    wrap: false,
                    overflow: TextOverflow::EllipsizeEnd,
                },
            ),
        );
        let available_width_px = layout.text_width_px();

        for heading in headings {
            assert_largest_fitting_line(
                &context,
                heading,
                "Libre Baskerville",
                layout.heading_font,
                available_width_px,
                viewport,
            );
        }
        for explanation in explanations {
            assert_largest_fitting_line(
                &context,
                explanation,
                "IBM Plex Sans",
                layout.explanation_font,
                available_width_px,
                viewport,
            );
        }
    }
}

#[test]
fn retains_preferred_size_for_short_copy_and_shrinks_only_over_capacity_copy() {
    let font_map = full_field_font_map();
    let context = font_map.create_context();

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        let available_width_px = layout.text_width_px();
        let short_heading = fitting_size(
            &context,
            "Nothing is playing",
            "Libre Baskerville",
            layout.heading_font,
            available_width_px,
        );
        let long_heading = fitting_size(
            &context,
            "Now Playing details unavailable",
            "Libre Baskerville",
            layout.heading_font,
            available_width_px,
        );
        let long_explanation = fitting_size(
            &context,
            "Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.",
            "IBM Plex Sans",
            layout.explanation_font,
            available_width_px,
        );

        assert_eq!(
            short_heading, layout.heading_font.preferred_px,
            "{viewport:?}"
        );
        assert!(
            long_heading < layout.heading_font.preferred_px,
            "{viewport:?}"
        );
        assert!(
            long_explanation < layout.explanation_font.preferred_px,
            "{viewport:?}",
        );
    }
}

fn full_field_font_map() -> pango::FontMap {
    let renderer_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    register_packaged_fallback_fonts(renderer_root)
        .expect("packaged Full-field fonts should register");
    let font_map = pangocairo::FontMap::new();
    font_map.changed();
    let available_families = font_map
        .list_families()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert!(available_families.contains("Libre Baskerville"));
    assert!(available_families.contains("IBM Plex Sans"));
    font_map
}

fn assert_largest_fitting_line(
    context: &pango::Context,
    text: &str,
    family: &str,
    sizes: FullFieldFontSize,
    available_width_px: u32,
    viewport: Viewport,
) {
    let chosen = fitting_size(context, text, family, sizes, available_width_px);
    assert!(
        line_fits(context, text, family, chosen, available_width_px),
        "{text:?} should be complete at {viewport:?}",
    );
    if chosen < sizes.preferred_px {
        assert!(
            !line_fits(context, text, family, chosen + 1, available_width_px),
            "{text:?} should use the largest fitting size at {viewport:?}",
        );
    }
}

fn fitting_size(
    context: &pango::Context,
    text: &str,
    family: &str,
    sizes: FullFieldFontSize,
    available_width_px: u32,
) -> u32 {
    sizes.fitting_font_size(|font_size_px| {
        line_fits(context, text, family, font_size_px, available_width_px)
    })
}

fn line_fits(
    context: &pango::Context,
    text: &str,
    family: &str,
    font_size_px: u32,
    available_width_px: u32,
) -> bool {
    let line = Layout::new(context);
    let mut font = FontDescription::from_string(family);
    font.set_absolute_size(f64::from(font_size_px * pango::SCALE as u32));
    line.set_font_description(Some(&font));
    line.set_text(text);
    line.set_width(available_width_px as i32 * pango::SCALE);
    line.set_ellipsize(pango::EllipsizeMode::End);
    line.line_count() == 1 && !line.is_ellipsized()
}
