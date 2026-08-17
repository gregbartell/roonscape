#[path = "support/representative_viewports.rs"]
mod representative_viewports;
mod support;

use roonscape_renderer::{
    FullFieldLayout, FullFieldLineLayout, IdentityLineLayout, IdentityPlacement, NowPlayingLayout,
    Presentation, TextOverflow, parse_snapshot, presentation_from_snapshot,
    register_packaged_fallback_fonts,
};

#[test]
fn bounds_full_field_states_and_identities_at_representative_landscape_viewports() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        let now_playing = NowPlayingLayout::for_viewport(viewport);
        let maximum_copy_height = layout.presentation_status.symbol_size_px
            + layout.status_spacing_px
            + layout.heading_px
            + layout.explanation_spacing_px
            + layout.explanation_px;

        assert!(
            layout.copy_width_px + layout.outer_gutter_px * 2 <= viewport.width_px,
            "full-field copy should remain horizontally bounded at {viewport:?}"
        );
        assert!(
            maximum_copy_height + layout.outer_gutter_px * 2 <= viewport.height_px,
            "full-field copy should remain vertically bounded at {viewport:?}"
        );
        assert!(layout.accent_padding_px < layout.copy_width_px);
        assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
        assert_eq!(
            layout.identity_line,
            IdentityLineLayout {
                maximum_lines: 1,
                overflow: TextOverflow::EllipsizeEnd,
            }
        );
        assert_eq!(
            layout.identity_width_px,
            now_playing.metadata_column_width_px - now_playing.metadata_right_inset_px,
            "full-field states should share the stable Output and Zone row at {viewport:?}"
        );
        assert!(
            layout.identity_width_px + layout.identity_right_inset_px + layout.outer_gutter_px
                <= viewport.width_px,
            "the identity row should remain inside the field at {viewport:?}"
        );
    }
}

#[test]
fn shares_the_imaginary_square_anchors_with_now_playing() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let full_field = FullFieldLayout::for_viewport(viewport);
        let now_playing = NowPlayingLayout::for_viewport(viewport);

        assert_eq!(
            full_field.artwork_field_anchors, now_playing.artwork_field_anchors,
            "Presentation Status and the complete Output and Zone row should share one anchor at {viewport:?}",
        );
    }
}

#[test]
fn available_presentation_forms_use_the_shared_status_and_identity_anchors() {
    let fixtures = [
        "playing.json",
        "paused.json",
        "loading.json",
        "stopped.json",
        "loading-empty.json",
        "playing-empty.json",
        "paused-empty.json",
    ];

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let shared = NowPlayingLayout::for_viewport(viewport);
        let expected = shared.artwork_field_anchors;

        for fixture in fixtures {
            let snapshot = parse_snapshot(&support::fixture(fixture))
                .expect("available layout fixture should satisfy the shared contract");
            let presentation = presentation_from_snapshot(&snapshot)
                .expect("available layout fixture should produce a presentation");
            let anchors = match presentation {
                Presentation::NowPlaying(presentation) => {
                    let layout = NowPlayingLayout::for_presentation(&presentation, viewport);
                    layout.artwork_field_anchors
                }
                Presentation::FullField(presentation) => {
                    assert!(
                        presentation.identity.is_some(),
                        "{fixture} should retain the Tracked Output and Tracked Zone",
                    );
                    let layout = FullFieldLayout::for_viewport(viewport);
                    layout.artwork_field_anchors
                }
            };

            assert_eq!(
                anchors, expected,
                "{fixture} should use the shared square anchors at {viewport:?}",
            );
        }
    }
}

#[test]
fn reserves_a_complete_long_heading_word_at_representative_landscape_viewports() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        let heading_width = layout
            .copy_width_px
            .saturating_sub(layout.accent_padding_px);

        assert!(
            heading_width * 5 >= layout.heading_px * 27,
            "full-field copy should reserve 5.4 em for a complete heading word at {viewport:?}"
        );
    }
}

#[test]
fn fits_every_approved_full_field_line_at_representative_landscape_viewports() {
    use gtk::pango::prelude::FontFamilyExt;
    use gtk::pango::{self, FontDescription, Layout};
    use gtk::prelude::FontMapExt;

    let renderer_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    register_packaged_fallback_fonts(renderer_root)
        .expect("packaged full-field fonts should register");
    let font_map = pangocairo::FontMap::new();
    font_map.changed();
    let available_families = font_map
        .list_families()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert!(available_families.contains("Libre Baskerville"));
    assert!(available_families.contains("IBM Plex Sans"));

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
    let context = font_map.create_context();

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
            "full-field copy should stay on complete lines at {viewport:?}",
        );
        let available_width_px = layout
            .copy_width_px
            .saturating_sub(layout.accent_width_px)
            .saturating_sub(layout.accent_padding_px);

        for heading in headings {
            assert_line_fits(
                &context,
                heading,
                "Libre Baskerville",
                layout.heading_px,
                available_width_px,
                viewport,
            );
        }
        for explanation in explanations {
            assert_line_fits(
                &context,
                explanation,
                "IBM Plex Sans",
                layout.explanation_px,
                available_width_px,
                viewport,
            );
        }
    }

    fn assert_line_fits(
        context: &pango::Context,
        text: &str,
        family: &str,
        font_size_px: u32,
        available_width_px: u32,
        viewport: roonscape_renderer::Viewport,
    ) {
        let line = Layout::new(context);
        let mut font = FontDescription::from_string(family);
        font.set_absolute_size(f64::from(font_size_px * pango::SCALE as u32));
        line.set_font_description(Some(&font));
        line.set_text(text);
        line.set_width(available_width_px as i32 * pango::SCALE);
        line.set_ellipsize(pango::EllipsizeMode::End);
        assert_eq!(line.line_count(), 1, "{text:?} at {viewport:?}");
        assert!(!line.is_ellipsized(), "{text:?} at {viewport:?}");
    }
}
