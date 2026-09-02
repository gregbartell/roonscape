mod support;

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use roonscape_renderer::{Availability, Playback, parse_snapshot};
use serde_json::Value;

#[test]
fn fixture_scenarios_reference_committed_srgb_jpegs_fitted_within_live_mode_artwork_bounds() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog: Value = serde_json::from_str(&support::fixture("fixture-scenario-catalog.json"))
        .expect("Fixture Scenario catalog should be valid JSON");
    let fixture_paths = catalog["scenarios"]
        .as_array()
        .expect("Fixture Scenario catalog should contain scenarios")
        .iter()
        .map(|scenario| {
            scenario["fixture"]
                .as_str()
                .expect("Fixture Scenario should name a fixture")
        });
    let artwork_paths = fixture_paths
        .filter_map(|fixture_path| {
            let snapshot = parse_snapshot(
                &fs::read_to_string(repository_root.join(fixture_path))
                    .expect("catalogued Fixture Scenario should be committed"),
            )
            .expect("catalogued Fixture Scenario should satisfy the shared contract");
            snapshot.artwork.map(|artwork| artwork.path)
        })
        .collect::<HashSet<_>>();

    assert!(!artwork_paths.is_empty());
    for artwork_path in artwork_paths {
        assert_eq!(
            Path::new(&artwork_path)
                .extension()
                .and_then(|value| value.to_str()),
            Some("jpg")
        );
        let committed_artwork_path = repository_root.join(&artwork_path);
        let encoded_artwork_bytes = fs::read(&committed_artwork_path)
            .expect("Fixture Scenario artwork should be committed");
        assert!(
            encoded_artwork_bytes
                .windows(b"ICC_PROFILE\0".len())
                .any(|window| window == b"ICC_PROFILE\0"),
            "{} should embed its sRGB profile",
            committed_artwork_path.display()
        );
        let decoded_artwork = gdk_pixbuf::Pixbuf::from_file(&committed_artwork_path)
            .expect("Fixture Scenario JPEG should decode");
        assert!(decoded_artwork.width() <= 1_600);
        assert!(decoded_artwork.height() <= 1_600);
        let canonical_source =
            gdk_pixbuf::Pixbuf::from_file(committed_artwork_path.with_extension("svg"))
                .expect("Fixture Scenario JPEG should retain a canonical SVG source");
        assert_eq!(
            canonical_source.width() * decoded_artwork.height(),
            canonical_source.height() * decoded_artwork.width(),
            "{} should preserve its canonical source aspect ratio",
            committed_artwork_path.display()
        );
    }
}

#[test]
fn parses_the_shared_playing_fixture_as_a_complete_snapshot() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    assert_eq!(snapshot.schema_version, 4);
    assert_eq!(snapshot.revision, 7);
    assert_eq!(snapshot.availability, Availability::Available);
    assert_eq!(snapshot.playback, Some(Playback::Playing));
    assert_eq!(
        snapshot
            .tracked_output
            .as_ref()
            .map(|output| output.name.as_str()),
        Some("Speaker System")
    );
    assert_eq!(
        snapshot
            .tracked_zone
            .as_ref()
            .map(|zone| (zone.id.as_str(), zone.name.as_str())),
        Some(("zone-living-room", "Living Room"))
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
        snapshot.timing.as_ref().map(|timing| (
            timing.position.as_ref().map(|position| position.seconds),
            timing.duration_seconds,
        )),
        Some((Some(171.0), Some(266.0)))
    );
    assert_eq!(
        snapshot
            .artwork
            .as_ref()
            .map(|artwork| artwork.path.as_str()),
        Some("src/shared/fixtures/artwork/playing.jpg")
    );
    assert_eq!(snapshot.lyrics, None);
}

#[test]
fn accepts_every_version_4_timing_shape_and_rejects_retired_or_invalid_timing() {
    let base: Value =
        serde_json::from_str(&support::fixture("playing.json")).expect("fixture should be JSON");
    for timing in [
        Value::Null,
        serde_json::json!({
            "position": {"seconds": 42, "sampledAt": "2026-09-02T20:15:00Z"},
            "durationSeconds": 180
        }),
        serde_json::json!({
            "position": {"seconds": 42, "sampledAt": "2026-09-02T20:15:00Z"},
            "durationSeconds": null
        }),
        serde_json::json!({"position": null, "durationSeconds": 180}),
    ] {
        let mut candidate = base.clone();
        candidate["timing"] = timing;
        parse_snapshot(&candidate.to_string()).expect("version 4 timing shape should be accepted");
    }

    let mut retired = base.clone();
    retired["schemaVersion"] = 3.into();
    assert!(parse_snapshot(&retired.to_string()).is_err());

    for timing in [
        serde_json::json!({"position": null, "durationSeconds": null}),
        serde_json::json!({
            "position": {"seconds": -1, "sampledAt": "2026-09-02T20:15:00Z"},
            "durationSeconds": 180
        }),
        serde_json::json!({
            "position": {"seconds": 1, "sampledAt": "not-a-date"},
            "durationSeconds": 180
        }),
        serde_json::json!({"position": null, "durationSeconds": 0}),
    ] {
        let mut candidate = base.clone();
        candidate["timing"] = timing;
        assert!(parse_snapshot(&candidate.to_string()).is_err());
    }
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
        Some((9, "src/shared/fixtures/artwork/revised.jpg"))
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
        Some((20, "src/shared/fixtures/artwork/light.jpg".to_owned()))
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
        Some("src/shared/fixtures/artwork/playing.jpg".to_owned())
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
        Some((19, "src/shared/fixtures/artwork/non-square.jpg".to_owned()))
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
        assert_eq!(
            snapshot
                .tracked_output
                .as_ref()
                .map(|output| output.name.as_str()),
            (availability == Availability::OutputUnavailable).then_some("Speaker System")
        );
        assert_eq!(snapshot.tracked_zone, None);
        assert_eq!(snapshot.now_playing, None);
        assert_eq!(snapshot.timing, None);
        assert_eq!(snapshot.artwork, None);
    }
}

#[test]
fn parses_every_shared_playback_state_with_truthful_now_playing() {
    let fixtures = [
        ("playing.json", Playback::Playing, true, true),
        ("playing-empty.json", Playback::Playing, false, false),
        ("paused.json", Playback::Paused, true, true),
        ("paused-empty.json", Playback::Paused, false, false),
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
        assert_eq!(snapshot.timing.is_some(), has_progress);
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

    assert_eq!(indeterminate.timing, None);
    assert_eq!(
        past_duration.timing.map(|timing| (
            timing.position.map(|position| position.seconds),
            timing.duration_seconds,
        )),
        Some((Some(300.0), Some(266.0)))
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
fn rejects_unordered_duplicate_or_excessive_lyric_text() {
    let fixture: serde_json::Value =
        serde_json::from_str(&support::fixture("lyrics-one-line.json")).expect("fixture is JSON");

    for cues in [
        serde_json::json!([
            {"atSeconds": 2.0, "text": "Second"},
            {"atSeconds": 1.0, "text": "First"}
        ]),
        serde_json::json!([
            {"atSeconds": 1.0, "text": "First"},
            {"atSeconds": 1.0, "text": "Replacement"}
        ]),
    ] {
        let mut candidate = fixture.clone();
        candidate["lyrics"]["cues"] = cues;
        let error = parse_snapshot(&candidate.to_string())
            .expect_err("normalized cue timestamps should be strictly increasing");
        assert!(error.to_string().contains("strictly increasing"));
    }

    let mut excessive = fixture;
    excessive["lyrics"]["cues"] = serde_json::json!(
        (0..33)
            .map(|index| serde_json::json!({
                "atSeconds": index,
                "text": "x".repeat(512)
            }))
            .collect::<Vec<_>>()
    );
    let error =
        parse_snapshot(&excessive.to_string()).expect_err("total lyric text should remain bounded");
    assert!(error.to_string().contains("total lyric text"));
}

#[test]
fn rejects_the_removed_display_zone_snapshot_field() {
    let error = parse_snapshot(
        r#"{"schemaVersion":4,"revision":1,"availability":"available","playback":"playing","trackedOutput":{"name":"Speaker System"},"trackedZone":{"id":"zone-living-room","name":"Living Room"},"displayZone":{"name":"Living Room"},"nowPlaying":null,"timing":null,"artwork":null,"lyrics":null}"#,
    )
    .expect_err("the removed displayZone field should be rejected");

    assert!(error.to_string().contains("violates the schema"));
}

#[test]
fn bounds_roon_supplied_display_strings_by_unicode_code_points() {
    let fixture: serde_json::Value =
        serde_json::from_str(&support::fixture("playing.json")).expect("fixture should be JSON");
    let cases = [
        (vec!["trackedOutput", "name"], 256_usize),
        (vec!["trackedZone", "name"], 256_usize),
        (vec!["nowPlaying", "title"], 1_024_usize),
        (vec!["nowPlaying", "artist"], 1_024_usize),
        (vec!["nowPlaying", "album"], 1_024_usize),
    ];

    for (path, limit) in cases {
        let mut bounded = fixture.clone();
        let mut oversized = fixture.clone();
        set_string(&mut bounded, &path, &"🌌".repeat(limit));
        set_string(&mut oversized, &path, &"🌌".repeat(limit + 1));

        parse_snapshot(&bounded.to_string())
            .expect("the field limit should count Unicode code points");
        let error = parse_snapshot(&oversized.to_string())
            .expect_err("one Unicode code point beyond the field limit should fail");
        assert!(error.to_string().contains("violates the schema"));
    }
}

fn set_string(candidate: &mut serde_json::Value, path: &[&str], value: &str) {
    let mut target = candidate;
    for segment in &path[..path.len() - 1] {
        target = &mut target[*segment];
    }
    target[path[path.len() - 1]] = value.into();
}
