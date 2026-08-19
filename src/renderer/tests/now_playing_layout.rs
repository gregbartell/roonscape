#[path = "support/representative_viewports.rs"]
mod representative_viewports;
mod support;

use std::{fs, path::Path};

use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFit,
    ArtworkLayout, ArtworkReference, IdentityLineLayout, IdentityPhraseAlignment,
    IdentityPlacement, NowPlayingField, NowPlayingFooterContent, NowPlayingLayout, NowPlayingRole,
    Presentation, PresentationStatusDecoration, TextOverflow, parse_snapshot,
    presentation_from_snapshot, resolve_presentation,
};

struct UtilitySizeExpectation {
    symbol_px: u32,
    status_px: u32,
    time_px: u32,
    activity_heading_px: u32,
    activity_detail_px: u32,
    identity_px: u32,
}

fn now_playing(fixture_name: &str) -> roonscape_renderer::NowPlayingPresentation {
    let snapshot = parse_snapshot(&support::fixture(fixture_name))
        .expect("Now Playing layout fixture should be a valid shared snapshot");
    let Presentation::NowPlaying(presentation) = presentation_from_snapshot(&snapshot)
        .expect("available fixture should produce Now Playing")
    else {
        panic!("available fixture should produce Now Playing");
    };
    presentation
}

fn now_playing_from_snapshot(contents: &str) -> roonscape_renderer::NowPlayingPresentation {
    let snapshot = parse_snapshot(contents).expect("snapshot should satisfy the shared contract");
    let Presentation::NowPlaying(presentation) = presentation_from_snapshot(&snapshot)
        .expect("available snapshot should produce Now Playing")
    else {
        panic!("available snapshot should produce Now Playing");
    };
    presentation
}

fn artwork_dimensions(fixture_name: &str) -> ArtworkDimensions {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork")
        .join(fixture_name);
    let artwork = gdk_pixbuf::Pixbuf::from_file(path).expect("artwork fixture should be decodable");
    ArtworkDimensions::new(
        artwork
            .width()
            .try_into()
            .expect("artwork width should be positive"),
        artwork
            .height()
            .try_into()
            .expect("artwork height should be positive"),
    )
}

fn now_playing_with_unusable_artwork() -> roonscape_renderer::NowPlayingPresentation {
    let repository_root = tempfile::tempdir().expect("temporary repository root should be created");
    fs::write(repository_root.path().join("broken.jpg"), b"not an image")
        .expect("unusable artwork fixture should be written");
    let mut snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    snapshot.artwork = Some(ArtworkReference {
        revision: 42,
        path: "broken.jpg".to_owned(),
    });
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("snapshot with unusable artwork should produce a presentation");
    let Presentation::NowPlaying(now_playing) =
        resolve_presentation(&presentation, repository_root.path()).presentation
    else {
        panic!("usable metadata should retain Now Playing layout");
    };
    now_playing
}

#[test]
fn uses_each_representative_landscape_field_with_a_stable_metadata_hierarchy() {
    let presentation = now_playing("playing.json");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_presentation(&presentation, viewport);

        assert_eq!(
            layout.outer_gutter_px * 2
                + layout.artwork_column_width_px
                + layout.column_gap_px
                + layout.information.utility_width_px,
            viewport.width_px,
            "the composition should use the complete viewport without letterboxing"
        );
        assert_eq!(layout.field, NowPlayingField::Cohesive);
        assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
        assert_eq!(
            layout.metadata_roles,
            vec![
                NowPlayingRole::PresentationStatus,
                NowPlayingRole::Title,
                NowPlayingRole::Artist,
                NowPlayingRole::Album,
                NowPlayingRole::Progress,
            ]
        );
        assert!(layout.information.utility_width_px > 0);
        assert!(layout.typography.title.preferred_px >= layout.typography.title.reduced_px);
        assert!(layout.typography.title.reduced_px >= layout.typography.title.minimum_px);
        assert!(layout.typography.title.minimum_px >= 36);
        assert!(layout.typography.artist.minimum_px >= 18);
        assert!(layout.typography.album.minimum_px >= 15);
        assert!(layout.presentation_status.font_px >= 12);
        assert!(layout.typography.time_px >= 11);
        assert!(layout.typography.identity_px >= 11);
    }
}

#[test]
fn uses_selected_responsive_status_and_utility_sizes() {
    let expected = [
        UtilitySizeExpectation {
            symbol_px: 27,
            status_px: 19,
            time_px: 18,
            activity_heading_px: 18,
            activity_detail_px: 18,
            identity_px: 18,
        },
        UtilitySizeExpectation {
            symbol_px: 27,
            status_px: 19,
            time_px: 18,
            activity_heading_px: 18,
            activity_detail_px: 18,
            identity_px: 18,
        },
        UtilitySizeExpectation {
            symbol_px: 30,
            status_px: 21,
            time_px: 18,
            activity_heading_px: 18,
            activity_detail_px: 18,
            identity_px: 18,
        },
        UtilitySizeExpectation {
            symbol_px: 30,
            status_px: 21,
            time_px: 18,
            activity_heading_px: 18,
            activity_detail_px: 18,
            identity_px: 18,
        },
        UtilitySizeExpectation {
            symbol_px: 27,
            status_px: 19,
            time_px: 18,
            activity_heading_px: 18,
            activity_detail_px: 18,
            identity_px: 18,
        },
        UtilitySizeExpectation {
            symbol_px: 54,
            status_px: 38,
            time_px: 32,
            activity_heading_px: 32,
            activity_detail_px: 32,
            identity_px: 32,
        },
        UtilitySizeExpectation {
            symbol_px: 54,
            status_px: 38,
            time_px: 32,
            activity_heading_px: 32,
            activity_detail_px: 32,
            identity_px: 32,
        },
    ];

    for (viewport, expected) in representative_viewports::REPRESENTATIVE_VIEWPORTS
        .into_iter()
        .zip(expected)
    {
        let layout = NowPlayingLayout::for_viewport(viewport);

        assert_eq!(
            layout.presentation_status.symbol_size_px, expected.symbol_px,
            "{viewport:?}"
        );
        assert_eq!(
            layout.presentation_status.decoration,
            PresentationStatusDecoration::CircleFree,
            "Now Playing should use the compact circle-free treatment at {viewport:?}",
        );
        assert_eq!(
            layout.presentation_status.symbol_gap_px,
            ((expected.status_px as f64) * 0.42).round() as u32,
            "the glyph-to-label gap should remain approximately 0.42 em at {viewport:?}",
        );
        assert_eq!(
            layout.presentation_status.letter_spacing_px,
            ((expected.status_px as f64) * 0.105).round() as u32,
            "status tracking should remain approximately 0.105 em at {viewport:?}",
        );
        assert_eq!(
            layout.presentation_status.font_px, expected.status_px,
            "{viewport:?}"
        );
        assert_eq!(layout.typography.time_px, expected.time_px, "{viewport:?}");
        assert_eq!(
            layout.typography.activity_heading_px, expected.activity_heading_px,
            "{viewport:?}"
        );
        assert_eq!(
            layout.typography.activity_detail_px, expected.activity_detail_px,
            "{viewport:?}"
        );
        assert_eq!(
            layout.typography.identity_px, expected.identity_px,
            "{viewport:?}"
        );
    }
}

#[test]
fn groups_progress_or_activity_with_compact_bounded_identities_in_one_footer() {
    let determinate = now_playing("playing.json");
    let indeterminate = now_playing("indeterminate-progress.json");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let determinate = NowPlayingLayout::for_presentation(&determinate, viewport);
        let indeterminate = NowPlayingLayout::for_presentation(&indeterminate, viewport);

        assert_eq!(
            determinate.footer_content,
            NowPlayingFooterContent::DeterminateProgress,
        );
        assert_eq!(
            indeterminate.footer_content,
            NowPlayingFooterContent::IndeterminateActivity,
        );
        assert_eq!(
            (
                determinate.footer_gap_px,
                determinate.identity_row,
                determinate.information,
            ),
            (
                indeterminate.footer_gap_px,
                indeterminate.identity_row,
                indeterminate.information,
            ),
            "determinate and indeterminate content should use the same footer geometry at {viewport:?}",
        );
        assert_eq!(
            determinate.footer_gap_px,
            ((viewport.height_px as f64) * 0.02)
                .round()
                .clamp(17.0, 30.0) as u32,
            "footer content and identities should use the selected responsive gap at {viewport:?}",
        );
        assert_eq!(
            determinate.progress_height_px,
            ((viewport.width_px as f64) * 0.002).round().clamp(3.0, 5.0) as u32,
            "progress should use the selected flat track at {viewport:?}",
        );
        assert_eq!(
            determinate.time_spacing_px,
            ((determinate.typography.time_px as f64) * 0.58).round() as u32,
            "timing should follow progress by approximately 0.58 em at {viewport:?}",
        );

        let identity = determinate.identity_row;
        assert_eq!(
            identity.phrase_alignment,
            IdentityPhraseAlignment::Baseline,
            "each identity label and name should share a baseline at {viewport:?}",
        );
        assert!(
            identity.phrase_max_width_px * 2
                + identity.separator_size_px
                + identity.phrase_gap_px * 2
                <= determinate.information.utility_width_px,
            "identity phrases should be capped at half of the usable row at {viewport:?}",
        );
        assert_eq!(
            identity.label_gap_px,
            ((determinate.typography.identity_px as f64) * 0.42).round() as u32,
            "identity labels should sit approximately 0.42 em before their names at {viewport:?}",
        );
    }
}

#[test]
fn tightens_the_title_to_credit_gap_by_thirty_percent() {
    let expected_gaps = [22, 22, 22, 22, 22, 40, 40];

    for (viewport, expected_gap_px) in representative_viewports::REPRESENTATIVE_VIEWPORTS
        .into_iter()
        .zip(expected_gaps)
    {
        let layout = NowPlayingLayout::for_viewport(viewport);

        assert_eq!(layout.artist_spacing_px, expected_gap_px, "{viewport:?}");
        assert_eq!(
            layout.metadata_fitting.normal_title_to_credit_gap_px, expected_gap_px,
            "{viewport:?}",
        );
        assert_eq!(
            layout.metadata_fitting.compact_title_to_credit_gap_px,
            ((viewport.height_px as f64) * 0.014)
                .round()
                .clamp(14.0, 28.0) as u32,
            "{viewport:?}",
        );
        assert!(
            layout.album_spacing_px < layout.artist_spacing_px,
            "{viewport:?}"
        );
        assert_eq!(
            layout.album_spacing_px,
            ((layout.typography.album.preferred_px as f64) * 0.38).round() as u32,
            "{viewport:?}",
        );
    }
}

#[test]
fn promotes_album_to_the_first_credit_spacing_when_artist_is_missing() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_viewport(viewport);

        assert_eq!(
            layout.spacing_before_album_px(false),
            layout.artist_spacing_px,
            "the surviving Album should use the Title-to-credit gap at {viewport:?}",
        );
        assert_eq!(
            layout.spacing_before_album_px(true),
            layout.album_spacing_px,
            "Album should remain grouped with a preceding Artist at {viewport:?}",
        );
    }
}

#[test]
fn applies_one_small_upward_optical_correction_to_the_centered_metadata_group() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_viewport(viewport);

        assert_eq!(
            layout.metadata_optical_correction_px,
            ((viewport.height_px as f64) * 0.004)
                .round()
                .clamp(3.0, 10.0) as u32,
            "{viewport:?}",
        );
    }
}

#[test]
fn centers_metadata_between_status_and_the_visually_raised_footer() {
    let presentation = now_playing("playing.json");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_presentation(&presentation, viewport);
        let expected_top = layout
            .artwork_field_anchors
            .presentation_status_top_viewport_y_px
            + layout.presentation_status.symbol_size_px;
        let visual_footer_top = layout.footer_anchor.bottom_viewport_y_px - layout.footer_height_px;

        assert_eq!(layout.metadata_region_top_viewport_y_px, expected_top);
        assert_eq!(
            layout.metadata_region_bottom_viewport_y_px, visual_footer_top,
            "the centered metadata region should end at the visible footer at {viewport:?}",
        );
        assert!(
            layout.metadata_region_bottom_viewport_y_px > layout.metadata_region_top_viewport_y_px,
            "the centered metadata region should remain usable at {viewport:?}",
        );
        let example_group_height_px = layout.metadata_height_budget_px / 3;
        assert_eq!(
            layout.metadata_group_offset_px(example_group_height_px),
            (layout.metadata_region_bottom_viewport_y_px
                - layout.metadata_region_top_viewport_y_px
                - example_group_height_px)
                / 2
                - layout.metadata_optical_correction_px,
            "the complete metadata group should receive one centered offset at {viewport:?}",
        );
    }
}

#[test]
fn keeps_every_information_role_on_one_rail_with_only_musical_metadata_capped() {
    let presentation = now_playing("playing.json");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_presentation(&presentation, viewport);
        let information = layout.information;
        let expected_musical_metadata_width = information
            .utility_width_px
            .min(((viewport.height_px as f64) * 0.72).round() as u32);

        assert_eq!(
            information.left_viewport_x_px,
            layout.outer_gutter_px + layout.artwork_column_width_px + layout.column_gap_px,
            "the shared information rail should begin after artwork and gutter at {viewport:?}",
        );
        assert_eq!(
            information.left_viewport_x_px + information.utility_width_px + layout.outer_gutter_px,
            viewport.width_px,
            "the utility rail should retain all space through the trailing gutter at {viewport:?}",
        );
        assert_eq!(
            information.musical_metadata_width_px, expected_musical_metadata_width,
            "only Title, Artist, and Album should use the height-led measure cap at {viewport:?}",
        );
    }
}

#[test]
fn contains_the_artwork_and_its_depth_at_representative_landscape_viewports() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_viewport(viewport);
        let expected_artwork_size = ((viewport.height_px as f64) * 0.84)
            .round()
            .min(((viewport.width_px as f64) * 0.56).round())
            as u32;
        let vertical_clearance = (viewport
            .height_px
            .saturating_sub(layout.artwork_field_height_px))
            / 2;
        let shadow_extent =
            layout.artwork_shadow_offset_px + layout.artwork_shadow_blur_px.div_ceil(2);

        assert_eq!(
            layout.artwork_field_width_px, layout.artwork_field_height_px,
            "artwork should retain its square field at {viewport:?}"
        );
        assert_eq!(
            layout.artwork_field_width_px, expected_artwork_size,
            "artwork should use the lesser of 84% height and 56% width at {viewport:?}",
        );
        assert_eq!(
            layout.artwork_field_anchors.artwork_top_viewport_y_px, vertical_clearance,
            "the responsive artwork square should remain vertically centered at {viewport:?}",
        );
        assert!(
            layout.artwork_field_width_px <= layout.artwork_column_width_px,
            "artwork should remain inside its column at {viewport:?}"
        );
        assert!(
            shadow_extent <= vertical_clearance,
            "artwork depth should remain inside the viewport at {viewport:?}"
        );
    }
}

#[test]
fn offsets_one_same_footprint_print_plate_inside_the_artwork_gutter() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_viewport(viewport);
        let plate = layout.artwork_print_plate;
        let expected_offset_px = ((viewport.height_px as f64) * 0.0045)
            .round()
            .clamp(6.0, 12.0) as u32;

        assert_eq!(
            plate.footprint,
            ArtworkDimensions::new(
                layout.artwork_field_width_px,
                layout.artwork_field_height_px,
            ),
            "the print plate should match the square artwork reservation at {viewport:?}",
        );
        assert_eq!(
            plate.offset_px, expected_offset_px,
            "the print plate should retain the accepted responsive offset at {viewport:?}",
        );
        assert!(
            plate.offset_px < layout.column_gap_px,
            "the strict information rail should remain clear of the print plate at {viewport:?}",
        );
    }
}

#[test]
fn fits_supplied_decoration_to_the_actual_image_bounds() {
    let non_square = now_playing("non-square-artwork.json");
    let intrinsic_dimensions = artwork_dimensions("non-square.svg");

    assert_ne!(
        intrinsic_dimensions.width_px,
        intrinsic_dimensions.height_px
    );
    let layout = ArtworkLayout::for_presentation(&non_square, Some(intrinsic_dimensions));
    assert_eq!(
        layout,
        ArtworkLayout {
            content: ArtworkContent::Supplied,
            fit: ArtworkFit::Contain,
            alignment: ArtworkAlignment::Center,
            decoration: ArtworkDecoration::ContainedImage(intrinsic_dimensions),
        }
    );
    assert_eq!(
        layout.fitted_image(ArtworkDimensions::new(800, 800)),
        Some(ArtworkDimensions::new(798, 449)),
        "the image should fit inside its one-pixel border",
    );
    assert_eq!(
        layout.visible_decoration(ArtworkDimensions::new(800, 800)),
        ArtworkDimensions::new(800, 451),
        "the visible decoration should add the border around the contained image",
    );
}

#[test]
fn keeps_extreme_fitted_dimensions_visible() {
    let non_square = now_playing("non-square-artwork.json");
    let reservation = ArtworkDimensions::new(566, 566);

    for (intrinsic, expected) in [
        (
            ArtworkDimensions::new(2000, 1),
            ArtworkDimensions::new(564, 1),
        ),
        (
            ArtworkDimensions::new(1, 2000),
            ArtworkDimensions::new(1, 564),
        ),
    ] {
        assert_eq!(
            ArtworkLayout::for_presentation(&non_square, Some(intrinsic)).fitted_image(reservation),
            Some(expected),
            "valid artwork should retain at least one visible pixel on each axis",
        );
    }
}

#[test]
fn preserves_square_decoration_for_square_and_missing_artwork() {
    let square = now_playing("playing.json");
    let missing = now_playing("missing-artwork.json");
    let reservation = ArtworkDimensions::new(800, 800);
    assert_eq!(
        ArtworkLayout::for_presentation(&missing, None),
        ArtworkLayout {
            content: ArtworkContent::QuietField,
            fit: ArtworkFit::Contain,
            alignment: ArtworkAlignment::Center,
            decoration: ArtworkDecoration::QuietSquareField,
        }
    );
    assert_eq!(
        ArtworkLayout::for_presentation(&missing, None).visible_decoration(reservation),
        reservation,
        "the quiet fallback should decorate the complete square field",
    );
    let square_dimensions = artwork_dimensions("playing.svg");
    assert_eq!(
        ArtworkLayout::for_presentation(&square, Some(square_dimensions))
            .visible_decoration(reservation),
        reservation,
        "square supplied artwork should retain its framed dimensions",
    );
}

#[test]
fn keeps_the_square_reservation_invariant_across_artwork_conditions() {
    let square = now_playing("playing.json");
    let non_square = now_playing("non-square-artwork.json");
    let missing = now_playing("missing-artwork.json");
    let unusable = now_playing_with_unusable_artwork();

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let square_geometry = NowPlayingLayout::for_presentation(&square, viewport);
        let expected_reservation = (
            square_geometry.artwork_column_width_px,
            square_geometry.artwork_field_width_px,
            square_geometry.artwork_field_height_px,
            square_geometry.artwork_print_plate,
            square_geometry.information,
        );
        for (condition, presentation) in [
            ("non-square", &non_square),
            ("missing", &missing),
            ("unusable", &unusable),
        ] {
            let geometry = NowPlayingLayout::for_presentation(presentation, viewport);
            assert_eq!(
                (
                    geometry.artwork_column_width_px,
                    geometry.artwork_field_width_px,
                    geometry.artwork_field_height_px,
                    geometry.artwork_print_plate,
                    geometry.information,
                ),
                expected_reservation,
                "{condition} artwork must not change the reserved square geometry at {viewport:?}",
            );
        }
    }
}

#[test]
fn anchors_the_information_rail_and_footer_to_the_reserved_square_at_every_viewport() {
    let presentations = [
        now_playing("playing.json"),
        now_playing("non-square-artwork.json"),
        now_playing("missing-artwork.json"),
        now_playing_with_unusable_artwork(),
        now_playing("long-identities.json"),
    ];

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        for presentation in &presentations {
            let layout = NowPlayingLayout::for_presentation(presentation, viewport);
            let anchors = layout.artwork_field_anchors;
            let footer_anchor = layout.footer_anchor;
            let expected_artwork_top_viewport_y_px =
                (viewport.height_px - layout.artwork_field_height_px) / 2;
            let expected_artwork_bottom_viewport_y_px =
                expected_artwork_top_viewport_y_px + layout.artwork_field_height_px;
            let expected_rail_inset_px = ((viewport.height_px as f64) * 0.013).round() as u32;
            let expected_footer_raise_px = ((viewport.height_px as f64) * 0.048).round() as u32;

            assert_eq!(
                (
                    anchors.artwork_top_viewport_y_px,
                    anchors.presentation_status_top_viewport_y_px,
                    anchors.information_rail_bottom_viewport_y_px,
                    footer_anchor.bottom_viewport_y_px,
                    anchors.artwork_bottom_viewport_y_px,
                ),
                (
                    expected_artwork_top_viewport_y_px,
                    expected_artwork_top_viewport_y_px + expected_rail_inset_px,
                    expected_artwork_bottom_viewport_y_px - expected_rail_inset_px,
                    expected_artwork_bottom_viewport_y_px
                        - expected_rail_inset_px
                        - expected_footer_raise_px,
                    expected_artwork_bottom_viewport_y_px,
                ),
                "the rail and raised footer should use the rendered square reservation at {viewport:?}",
            );
            assert_eq!(
                anchors.information_rail_inset_px, expected_rail_inset_px,
                "the rail should sit approximately 1.3vh inside the artwork clearance at {viewport:?}",
            );
            assert_eq!(
                layout.footer_optical_raise_px, expected_footer_raise_px,
                "the footer should receive one approximately 4.8vh optical raise at {viewport:?}",
            );
        }
    }
}

#[test]
fn converts_the_square_anchors_to_metadata_container_margins() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_viewport(viewport);
        let anchors = layout.artwork_field_anchors;
        let footer_anchor = layout.footer_anchor;
        let artwork_top_clearance_px = anchors.artwork_top_viewport_y_px;
        let artwork_bottom_clearance_px = viewport.height_px - anchors.artwork_bottom_viewport_y_px;

        assert_eq!(
            anchors.presentation_status_margin_top_px(0),
            artwork_top_clearance_px + anchors.information_rail_inset_px,
            "the metadata container should preserve the status square inset at {viewport:?}",
        );
        assert_eq!(
            viewport.height_px - anchors.information_rail_bottom_viewport_y_px,
            artwork_bottom_clearance_px + anchors.information_rail_inset_px,
            "the information rail should preserve the same square-relative bottom inset at {viewport:?}",
        );
        assert_eq!(
            footer_anchor.margin_bottom_px(0),
            artwork_bottom_clearance_px
                + anchors.information_rail_inset_px
                + layout.footer_optical_raise_px,
            "the metadata container should optically raise the complete footer at {viewport:?}",
        );
    }
}

#[test]
fn replaces_the_determinate_timeline_with_activity_for_indeterminate_playing() {
    let determinate = now_playing("playing.json");
    let indeterminate = now_playing("indeterminate-progress.json");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let determinate = NowPlayingLayout::for_presentation(&determinate, viewport);
        let indeterminate = NowPlayingLayout::for_presentation(&indeterminate, viewport);

        assert!(
            determinate
                .metadata_roles
                .contains(&NowPlayingRole::Progress),
            "determinate progress should retain its hierarchy at {viewport:?}"
        );
        assert!(
            !indeterminate
                .metadata_roles
                .contains(&NowPlayingRole::Progress),
            "indeterminate Playing should omit determinate timing at {viewport:?}"
        );
        assert!(
            indeterminate
                .metadata_roles
                .contains(&NowPlayingRole::Activity),
            "indeterminate Playing should retain visible activity at {viewport:?}"
        );
    }
}

#[test]
fn defensively_ellipsizes_long_identities_without_moving_the_footer() {
    let ordinary = now_playing("playing.json");
    let long_identities = now_playing("long-identities.json");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let ordinary = NowPlayingLayout::for_presentation(&ordinary, viewport);
        let long = NowPlayingLayout::for_presentation(&long_identities, viewport);

        assert_eq!(
            long.identity_line,
            IdentityLineLayout {
                maximum_lines: 1,
                overflow: TextOverflow::EllipsizeEnd,
            }
        );
        assert_eq!(long.identity_placement, IdentityPlacement::BottomRight);
        assert_eq!(
            (long.information, long.identity_row, long.footer_gap_px),
            (
                ordinary.information,
                ordinary.identity_row,
                ordinary.footer_gap_px,
            ),
            "identity content must not move or resize the footer at {viewport:?}"
        );
    }
}

#[test]
fn applies_one_complete_now_playing_policy_to_fixture_and_roon_snapshots() {
    let roon_snapshot = r#"{
      "schemaVersion": 2,
      "revision": 41,
      "availability": "available",
      "playback": "playing",
      "trackedOutput": { "name": "Speaker System" },
      "trackedZone": { "name": "Living Room" },
      "nowPlaying": {
        "title": "A Moment Apart",
        "artist": "ODESZA",
        "album": "A Moment Apart"
      },
      "progress": {
        "positionSeconds": 30,
        "durationSeconds": 234,
        "sampledAt": "2026-08-15T19:20:00.000Z"
      },
      "artwork": { "revision": 41, "path": "artwork/artwork-41.jpg" }
    }"#;
    let expected_roles = vec![
        NowPlayingRole::PresentationStatus,
        NowPlayingRole::Title,
        NowPlayingRole::Artist,
        NowPlayingRole::Album,
        NowPlayingRole::Progress,
    ];
    let presentations = [
        now_playing("playing.json"),
        now_playing("paused.json"),
        now_playing("loading.json"),
        now_playing_from_snapshot(roon_snapshot),
    ];

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        for presentation in &presentations {
            let layout = NowPlayingLayout::for_presentation(presentation, viewport);
            assert_eq!(layout.field, NowPlayingField::Cohesive);
            assert_eq!(layout.metadata_roles, expected_roles);
            assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
        }
    }
}
