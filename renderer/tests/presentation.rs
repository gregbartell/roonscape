mod support;

use roonscape_renderer::{Presentation, parse_snapshot, presentation_from_snapshot};

#[test]
fn maps_the_playing_snapshot_to_gallery_split_content() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Playing snapshot should produce a presentation");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };

    assert_eq!(presentation.title.as_deref(), Some("A Moment Apart"));
    assert_eq!(presentation.artist.as_deref(), Some("ODESZA"));
    assert_eq!(presentation.album.as_deref(), Some("A Moment Apart"));
    assert_eq!(presentation.display_zone, "Gallery");
    assert_eq!(presentation.playback_state, "Playing");
    assert_eq!(
        presentation.artwork_path.as_deref(),
        Some("fixtures/artwork/playing.svg")
    );

    let progress = presentation
        .progress
        .expect("Playing fixture should include determinate progress");
    assert!((progress.fraction - (82.0 / 234.0)).abs() < f64::EPSILON);
    assert_eq!(progress.elapsed, "1:22");
    assert_eq!(progress.remaining, "−2:32");
}

#[test]
fn carries_artwork_presence_and_presentation_revision_into_gallery_split() {
    let missing_fixture = support::fixture("missing-artwork.json");
    let missing_snapshot =
        parse_snapshot(&missing_fixture).expect("missing artwork fixture should be valid");
    let Presentation::NowPlaying(missing) = presentation_from_snapshot(&missing_snapshot)
        .expect("missing artwork should retain the neutral Now Playing presentation")
    else {
        panic!("available content should produce Now Playing");
    };

    assert_eq!(missing.artwork_path, None);
    assert_eq!(missing.artwork_revision, None);

    let revised_fixture = support::fixture("artwork-revision-changed.json");
    let revised_snapshot =
        parse_snapshot(&revised_fixture).expect("artwork revision fixture should be valid");
    let Presentation::NowPlaying(revised) = presentation_from_snapshot(&revised_snapshot)
        .expect("revised artwork should produce Now Playing")
    else {
        panic!("available content should produce Now Playing");
    };

    assert_eq!(revised.artwork_revision, Some(9));
}

#[test]
fn maps_unavailable_snapshots_to_distinct_explanations() {
    let expected = [
        (
            "pairing-required.json",
            "Pairing required",
            "Enable RoonScape",
            "Open Settings → Extensions in a Roon client, then enable RoonScape.",
        ),
        (
            "disconnected.json",
            "Disconnected",
            "Waiting for Roon",
            "Check Roon Server and the network. This display updates when Roon returns.",
        ),
        (
            "output-unavailable.json",
            "Output unavailable",
            "Display Output unavailable",
            "Configure a Display Output on this RoonScape Host, or check that the selected output is available in Roon.",
        ),
    ];

    for (fixture_name, state_label, heading, explanation) in expected {
        let fixture = support::fixture(fixture_name);
        let snapshot = parse_snapshot(&fixture).expect("unavailable fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("unavailable snapshot should produce a presentation");
        let Presentation::Unavailable(presentation) = presentation else {
            panic!("unavailable snapshot must not retain Now Playing content");
        };

        assert_eq!(
            (
                presentation.state_label,
                presentation.heading,
                presentation.explanation,
            ),
            (state_label, heading, explanation),
        );
    }
}
