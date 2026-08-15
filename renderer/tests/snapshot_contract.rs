mod support;

use roonscape_renderer::{Availability, Playback, parse_snapshot};

#[test]
fn parses_the_shared_playing_fixture_as_a_complete_snapshot() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.revision, 7);
    assert_eq!(snapshot.availability, Availability::Available);
    assert_eq!(snapshot.playback, Some(Playback::Playing));
    assert_eq!(
        snapshot
            .display_zone
            .as_ref()
            .map(|zone| zone.name.as_str()),
        Some("Gallery")
    );
    assert_eq!(
        snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.title.as_deref()),
        Some("A Moment Apart")
    );
    assert_eq!(
        snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.artist.as_deref()),
        Some("ODESZA")
    );
    assert_eq!(
        snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.album.as_deref()),
        Some("A Moment Apart")
    );
    assert_eq!(
        snapshot
            .progress
            .as_ref()
            .map(|progress| (progress.position_seconds, progress.duration_seconds)),
        Some((82.0, 234.0))
    );
    assert_eq!(
        snapshot
            .artwork
            .as_ref()
            .map(|artwork| artwork.path.as_str()),
        Some("fixtures/artwork/playing.svg")
    );
}

#[test]
fn rejects_the_shared_invalid_fixture() {
    let fixture = support::fixture("invalid.json");

    let error = parse_snapshot(&fixture).expect_err("shared invalid fixture should be rejected");

    assert!(error.to_string().contains("violates the schema"));
}
