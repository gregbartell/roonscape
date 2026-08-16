mod support;

use std::path::Path;

use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkFit, ArtworkLayout, GalleryField, GallerySplitLayout,
    GallerySplitRole, IdentityLineLayout, IdentityPlacement, Presentation, TextOverflow, Viewport,
    parse_snapshot, presentation_from_snapshot,
};

fn now_playing(fixture_name: &str) -> roonscape_renderer::NowPlayingPresentation {
    let snapshot = parse_snapshot(&support::fixture(fixture_name))
        .expect("Gallery split fixture should be a valid shared snapshot");
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
fn lays_out_complete_now_playing_as_the_reference_gallery_field() {
    let layout = GallerySplitLayout::for_presentation(
        &now_playing("playing.json"),
        Viewport::new(3840, 2160),
    );

    assert_eq!(layout.field, GalleryField::Cohesive);
    assert_eq!(layout.outer_gutter_px, 160);
    assert_eq!(layout.column_gap_px, 192);
    assert_eq!(layout.artwork_column_width_px, 1964);
    assert_eq!(layout.metadata_column_width_px, 1364);
    assert_eq!(layout.artwork_field_width_px, 1750);
    assert_eq!(layout.artwork_field_height_px, 1750);
    assert_eq!(layout.status_top_inset_px, 39);
    assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
    assert_eq!(
        layout.metadata_roles,
        vec![
            GallerySplitRole::PlaybackStatus,
            GallerySplitRole::Title,
            GallerySplitRole::Artist,
            GallerySplitRole::Album,
            GallerySplitRole::Progress,
        ]
    );
    assert_eq!(layout.typography.title.preferred_px, 168);
    assert_eq!(layout.typography.artist.preferred_px, 64);
    assert_eq!(layout.typography.album.preferred_px, 45);
    assert_eq!(layout.typography.status_px, 30);
    assert_eq!(layout.typography.status_letter_spacing_px, 4);
}

#[test]
fn scales_the_gallery_composition_at_the_tall_and_windowed_viewports() {
    let presentation = now_playing("playing.json");
    let tall = GallerySplitLayout::for_presentation(&presentation, Viewport::new(3840, 2400));
    let windowed = GallerySplitLayout::for_presentation(&presentation, Viewport::new(1600, 900));

    assert_eq!(tall.artwork_field_width_px, 1944);
    assert_eq!(tall.artwork_field_height_px, 1944);
    assert_eq!(tall.status_top_inset_px, 43);
    assert_eq!(tall.typography.title.preferred_px, 168);

    assert_eq!(windowed.outer_gutter_px, 67);
    assert_eq!(windowed.column_gap_px, 80);
    assert_eq!(windowed.artwork_column_width_px, 818);
    assert_eq!(windowed.metadata_column_width_px, 568);
    assert_eq!(windowed.artwork_field_width_px, 729);
    assert_eq!(windowed.status_top_inset_px, 16);
    assert_eq!(windowed.typography.title.preferred_px, 74);
    assert_eq!(windowed.typography.artist.preferred_px, 28);
    assert_eq!(windowed.typography.album.preferred_px, 20);
    assert_eq!(windowed.typography.status_px, 13);
    assert_eq!(windowed.typography.status_letter_spacing_px, 2);

    for (viewport, layout) in [
        (Viewport::new(3840, 2400), tall),
        (Viewport::new(1600, 900), windowed),
    ] {
        assert_eq!(
            layout.outer_gutter_px * 2
                + layout.artwork_column_width_px
                + layout.column_gap_px
                + layout.metadata_column_width_px,
            viewport.width_px,
            "the composition should use the complete viewport without letterboxing"
        );
    }
}

#[test]
fn keeps_imperfect_artwork_inside_the_stable_square_field() {
    let missing = now_playing("missing-artwork.json");
    let non_square = now_playing("non-square-artwork.json");
    let viewport = Viewport::WINDOWED_FIXTURE;
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/artwork/non-square.svg");
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

    let missing_geometry = GallerySplitLayout::for_presentation(&missing, viewport);
    let non_square_geometry = GallerySplitLayout::for_presentation(&non_square, viewport);
    assert_eq!(
        (
            missing_geometry.artwork_field_width_px,
            missing_geometry.artwork_field_height_px,
        ),
        (
            non_square_geometry.artwork_field_width_px,
            non_square_geometry.artwork_field_height_px,
        ),
        "imperfect artwork must not change the Gallery split geometry"
    );
}

#[test]
fn omits_the_complete_timeline_for_indeterminate_content() {
    let determinate = GallerySplitLayout::for_presentation(
        &now_playing("playing.json"),
        Viewport::WINDOWED_FIXTURE,
    );
    let indeterminate = GallerySplitLayout::for_presentation(
        &now_playing("indeterminate-progress.json"),
        Viewport::WINDOWED_FIXTURE,
    );

    assert!(
        determinate
            .metadata_roles
            .contains(&GallerySplitRole::Progress)
    );
    assert!(
        !indeterminate
            .metadata_roles
            .contains(&GallerySplitRole::Progress)
    );
}

#[test]
fn defensively_ellipsizes_long_identities_without_moving_the_footer() {
    let viewport = Viewport::WINDOWED_FIXTURE;
    let ordinary = GallerySplitLayout::for_presentation(&now_playing("playing.json"), viewport);
    let long = GallerySplitLayout::for_presentation(&now_playing("long-identities.json"), viewport);

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
fn applies_one_complete_gallery_policy_to_fixture_and_roon_snapshots() {
    let roon_snapshot = r#"{
      "schemaVersion": 2,
      "revision": 41,
      "availability": "available",
      "playback": "playing",
      "trackedOutput": { "name": "NUC HDMI" },
      "trackedZone": { "name": "Gallery" },
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
        GallerySplitRole::PlaybackStatus,
        GallerySplitRole::Title,
        GallerySplitRole::Artist,
        GallerySplitRole::Album,
        GallerySplitRole::Progress,
    ];
    let presentations = [
        now_playing("playing.json"),
        now_playing("paused.json"),
        now_playing("loading.json"),
        now_playing_from_snapshot(roon_snapshot),
    ];

    for presentation in presentations {
        let layout =
            GallerySplitLayout::for_presentation(&presentation, Viewport::WINDOWED_FIXTURE);
        assert_eq!(layout.field, GalleryField::Cohesive);
        assert_eq!(layout.metadata_roles, expected_roles);
        assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
    }
}
