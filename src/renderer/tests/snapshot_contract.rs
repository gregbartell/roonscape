mod support;

use roonscape_renderer::{Availability, Playback, parse_snapshot};

#[test]
fn parses_the_shared_playing_fixture_as_a_complete_snapshot() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    assert_eq!(snapshot.schema_version, 2);
    assert_eq!(snapshot.revision, 7);
    assert_eq!(snapshot.availability, Availability::Available);
    assert_eq!(snapshot.playback, Some(Playback::Playing));
    assert_eq!(
        snapshot
            .tracked_output
            .as_ref()
            .map(|output| output.name.as_str()),
        Some("AudioDevice")
    );
    assert_eq!(
        snapshot
            .tracked_zone
            .as_ref()
            .map(|zone| zone.name.as_str()),
        Some("Living Room")
    );
    assert_eq!(
        snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.title.as_deref()),
        Some("Last Light on Phobos")
    );
    assert_eq!(
        snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.artist.as_deref()),
        Some("Evelyn Lark & The Orbital Choir")
    );
    assert_eq!(
        snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.album.as_deref()),
        Some("Signals from the Quiet Sea")
    );
    assert_eq!(
        snapshot
            .progress
            .as_ref()
            .map(|progress| (progress.position_seconds, progress.duration_seconds)),
        Some((171.0, 266.0))
    );
    assert_eq!(
        snapshot
            .artwork
            .as_ref()
            .map(|artwork| artwork.path.as_str()),
        Some("src/shared/fixtures/artwork/playing.svg")
    );
}

#[test]
fn parses_missing_artwork_and_artwork_revision_fixtures() {
    let missing = parse_snapshot(&support::fixture("missing-artwork.json"))
        .expect("missing artwork fixture should be valid");
    let revised = parse_snapshot(&support::fixture("artwork-revision-changed.json"))
        .expect("artwork revision fixture should be valid");

    assert_eq!(missing.artwork, None);
    assert_eq!(revised.revision, 9);
    assert_eq!(
        revised
            .artwork
            .as_ref()
            .map(|artwork| (artwork.revision, artwork.path.as_str())),
        Some((9, "src/shared/fixtures/artwork/revised.svg"))
    );
}

#[test]
fn parses_the_light_artwork_visual_acceptance_fixture() {
    let snapshot = parse_snapshot(&support::fixture("light-artwork.json"))
        .expect("light artwork fixture should be valid");

    assert_eq!(
        snapshot
            .artwork
            .map(|artwork| (artwork.revision, artwork.path)),
        Some((20, "src/shared/fixtures/artwork/light.svg".to_owned()))
    );
    assert_eq!(
        snapshot
            .now_playing
            .and_then(|now_playing| now_playing.title),
        Some("Last Light on Phobos".to_owned())
    );
}

#[test]
fn parses_the_typography_glyph_fallback_visual_acceptance_fixture() {
    let snapshot = parse_snapshot(&support::fixture("glyph-fallback.json"))
        .expect("glyph fallback fixture should be valid");

    assert_eq!(
        snapshot
            .now_playing
            .and_then(|now_playing| now_playing.album),
        Some("Signals from the Quiet Sea — 月".to_owned())
    );
    assert_eq!(
        snapshot.artwork.map(|artwork| artwork.path),
        Some("src/shared/fixtures/artwork/playing.svg".to_owned())
    );
}

#[test]
fn parses_missing_long_and_extreme_metadata_fixtures() {
    let missing = parse_snapshot(&support::fixture("missing-metadata.json"))
        .expect("missing metadata fixture should be valid");
    let missing_artist = parse_snapshot(&support::fixture("missing-artist.json"))
        .expect("missing Artist fixture should be valid");
    let missing_album = parse_snapshot(&support::fixture("missing-album.json"))
        .expect("missing Album fixture should be valid");
    let long = parse_snapshot(&support::fixture("long-metadata.json"))
        .expect("long metadata fixture should be valid");
    let extreme = parse_snapshot(&support::fixture("extreme-metadata.json"))
        .expect("extreme metadata fixture should be valid");

    let missing = missing
        .now_playing
        .expect("missing metadata fixture should contain Now Playing");
    assert_eq!(missing.title.as_deref(), Some("Last Light on Phobos"));
    assert_eq!(missing.artist, None);
    assert_eq!(missing.album, None);
    assert_eq!(
        missing_artist
            .now_playing
            .and_then(|now_playing| now_playing.artist),
        None
    );
    assert_eq!(
        missing_album
            .now_playing
            .and_then(|now_playing| now_playing.album),
        None
    );
    assert!(
        long.now_playing
            .and_then(|now_playing| now_playing.title)
            .is_some_and(|title| title.len() > 80)
    );
    assert!(
        extreme
            .now_playing
            .and_then(|now_playing| now_playing.title)
            .is_some_and(|title| title.len() > 250)
    );
}

#[test]
fn parses_non_square_artwork_and_long_identity_fixtures() {
    let non_square = parse_snapshot(&support::fixture("non-square-artwork.json"))
        .expect("non-square artwork fixture should be valid");
    let long_identities = parse_snapshot(&support::fixture("long-identities.json"))
        .expect("long identity fixture should be valid");

    assert_eq!(
        non_square
            .artwork
            .map(|artwork| (artwork.revision, artwork.path)),
        Some((19, "src/shared/fixtures/artwork/non-square.svg".to_owned()))
    );
    assert!(
        long_identities
            .tracked_output
            .is_some_and(|output| output.name.len() > 80)
    );
    assert!(
        long_identities
            .tracked_zone
            .is_some_and(|zone| zone.name.len() > 80)
    );
}

#[test]
fn parses_blank_optional_metadata_for_renderer_normalization() {
    let snapshot = parse_snapshot(&support::fixture("blank-optional-metadata.json"))
        .expect("blank optional metadata fixture should satisfy the shared contract");
    let now_playing = snapshot
        .now_playing
        .expect("blank optional metadata fixture should carry Now Playing");

    assert_eq!(now_playing.title.as_deref(), Some("Last Light on Phobos"));
    assert_eq!(now_playing.artist.as_deref(), Some("   "));
    assert_eq!(now_playing.album.as_deref(), Some("\t"));
}

#[test]
fn rejects_the_shared_invalid_fixture() {
    let fixture = support::fixture("invalid.json");

    let error = parse_snapshot(&fixture).expect_err("shared invalid fixture should be rejected");

    assert!(error.to_string().contains("violates the schema"));
}

#[test]
fn parses_every_shared_unavailable_fixture_without_stale_content() {
    let fixtures = [
        ("pairing-required.json", Availability::PairingRequired),
        ("disconnected.json", Availability::Disconnected),
        ("output-unavailable.json", Availability::OutputUnavailable),
    ];

    for (fixture_name, availability) in fixtures {
        let fixture = support::fixture(fixture_name);
        let snapshot = parse_snapshot(&fixture).expect("unavailable fixture should be valid");

        assert_eq!(snapshot.availability, availability);
        assert_eq!(snapshot.playback, None);
        assert_eq!(snapshot.tracked_output, None);
        assert_eq!(snapshot.tracked_zone, None);
        assert_eq!(snapshot.now_playing, None);
        assert_eq!(snapshot.progress, None);
        assert_eq!(snapshot.artwork, None);
    }
}

#[test]
fn parses_every_shared_playback_state_with_truthful_now_playing() {
    let fixtures = [
        ("playing.json", Playback::Playing, true, true),
        ("playing-empty.json", Playback::Playing, false, false),
        ("paused.json", Playback::Paused, true, true),
        ("loading.json", Playback::Loading, true, true),
        ("loading-empty.json", Playback::Loading, false, false),
        ("stopped.json", Playback::Stopped, false, false),
    ];

    for (fixture_name, playback, has_now_playing, has_progress) in fixtures {
        let fixture = support::fixture(fixture_name);
        let snapshot = parse_snapshot(&fixture).expect("playback fixture should be valid");

        assert_eq!(snapshot.availability, Availability::Available);
        assert_eq!(snapshot.playback, Some(playback));
        assert_eq!(snapshot.now_playing.is_some(), has_now_playing);
        assert_eq!(snapshot.progress.is_some(), has_progress);
        if playback == Playback::Stopped {
            assert_eq!(snapshot.artwork, None);
        }
    }
}

#[test]
fn parses_absent_and_past_duration_progress_samples() {
    let indeterminate = parse_snapshot(&support::fixture("indeterminate-progress.json"))
        .expect("indeterminate fixture should be valid");
    let past_duration = parse_snapshot(&support::fixture("playing-past-duration.json"))
        .expect("past-duration fixture should be valid");

    assert_eq!(indeterminate.progress, None);
    assert_eq!(
        past_duration
            .progress
            .map(|progress| (progress.position_seconds, progress.duration_seconds)),
        Some((300.0, 266.0))
    );
}

#[test]
fn rejects_invalid_timing_and_stopped_snapshots_with_stale_now_playing() {
    for fixture_name in ["invalid-progress.json", "invalid-stopped-now-playing.json"] {
        let error = parse_snapshot(&support::fixture(fixture_name))
            .expect_err("invalid playback fixture should be rejected");

        assert!(error.to_string().contains("violates the schema"));
    }
}

#[test]
fn rejects_the_removed_display_zone_snapshot_field() {
    let error = parse_snapshot(
        r#"{"schemaVersion":2,"revision":1,"availability":"available","playback":"playing","trackedOutput":{"name":"NUC HDMI"},"trackedZone":{"name":"Gallery"},"displayZone":{"name":"Gallery"},"nowPlaying":null,"progress":null,"artwork":null}"#,
    )
    .expect_err("the removed displayZone field should be rejected");

    assert!(error.to_string().contains("violates the schema"));
}
