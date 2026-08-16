#[path = "support/representative_viewports.rs"]
mod representative_viewports;
mod support;

use std::path::Path;

use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkFit, ArtworkLayout, IdentityLineLayout,
    IdentityPlacement, NowPlayingField, NowPlayingLayout, NowPlayingRole, Presentation,
    TextOverflow, Viewport, parse_snapshot, presentation_from_snapshot,
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
                NowPlayingRole::PlaybackStatus,
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
        assert!(layout.typography.status_px >= 12);
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
fn keeps_imperfect_artwork_inside_the_stable_square_field() {
    let missing = now_playing("missing-artwork.json");
    let non_square = now_playing("non-square-artwork.json");
    let viewport = Viewport::WINDOWED_FIXTURE;
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/non-square.svg");
    let artwork = gdk_pixbuf::Pixbuf::from_file(artwork_path)
        .expect("the non-square artwork fixture should be decodable");

    assert_ne!(artwork.width(), artwork.height());

    assert_eq!(
        ArtworkLayout::for_presentation(&missing),
        ArtworkLayout {
            content: ArtworkContent::QuietField,
            fit: ArtworkFit::Contain,
            alignment: ArtworkAlignment::Center,
        }
    );
    assert_eq!(
        ArtworkLayout::for_presentation(&non_square),
        ArtworkLayout {
            content: ArtworkContent::Supplied,
            fit: ArtworkFit::Contain,
            alignment: ArtworkAlignment::Center,
        }
    );

    let missing_geometry = NowPlayingLayout::for_presentation(&missing, viewport);
    let non_square_geometry = NowPlayingLayout::for_presentation(&non_square, viewport);
    assert_eq!(
        (
            missing_geometry.artwork_field_width_px,
            missing_geometry.artwork_field_height_px,
        ),
        (
            non_square_geometry.artwork_field_width_px,
            non_square_geometry.artwork_field_height_px,
        ),
        "imperfect artwork must not change the Now Playing layout geometry"
    );
}

#[test]
fn omits_the_complete_timeline_for_indeterminate_content() {
    let determinate = NowPlayingLayout::for_presentation(
        &now_playing("playing.json"),
        Viewport::WINDOWED_FIXTURE,
    );
    let indeterminate = NowPlayingLayout::for_presentation(
        &now_playing("indeterminate-progress.json"),
        Viewport::WINDOWED_FIXTURE,
    );

    assert!(
        determinate
            .metadata_roles
            .contains(&NowPlayingRole::Progress)
    );
    assert!(
        !indeterminate
            .metadata_roles
            .contains(&NowPlayingRole::Progress)
    );
}

#[test]
fn defensively_ellipsizes_long_identities_without_moving_the_footer() {
    let viewport = Viewport::WINDOWED_FIXTURE;
    let ordinary = NowPlayingLayout::for_presentation(&now_playing("playing.json"), viewport);
    let long = NowPlayingLayout::for_presentation(&now_playing("long-identities.json"), viewport);

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
        "identity content must not move or resize the footer"
    );
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
        NowPlayingRole::PlaybackStatus,
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

    for presentation in presentations {
        let layout = NowPlayingLayout::for_presentation(&presentation, Viewport::WINDOWED_FIXTURE);
        assert_eq!(layout.field, NowPlayingField::Cohesive);
        assert_eq!(layout.metadata_roles, expected_roles);
        assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
    }
}
