#[path = "support/representative_viewports.rs"]
mod representative_viewports;
mod support;

use std::{fs, path::Path};

use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFit,
    ArtworkLayout, ArtworkReference, IdentityLineLayout, IdentityPlacement, NowPlayingField,
    NowPlayingLayout, NowPlayingRole, Presentation, TextOverflow, parse_snapshot,
    presentation_from_snapshot, resolve_presentation,
};

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
                + layout.metadata_column_width_px,
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
        assert!(layout.metadata_right_inset_px < layout.metadata_column_width_px);
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
fn contains_the_artwork_and_its_depth_at_representative_landscape_viewports() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = NowPlayingLayout::for_viewport(viewport);
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
                ),
                expected_reservation,
                "{condition} artwork must not change the reserved square geometry at {viewport:?}",
            );
        }
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
            (
                long.metadata_column_width_px,
                long.metadata_right_inset_px,
                long.identity_gap_px,
            ),
            (
                ordinary.metadata_column_width_px,
                ordinary.metadata_right_inset_px,
                ordinary.identity_gap_px,
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
