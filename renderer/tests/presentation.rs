mod support;

use roonscape_renderer::{parse_snapshot, presentation_from_snapshot};

#[test]
fn maps_the_playing_snapshot_to_gallery_split_content() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Playing snapshot should produce a presentation");

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
