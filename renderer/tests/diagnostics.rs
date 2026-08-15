mod support;

use std::path::Path;
use std::time::Duration;

use roonscape_renderer::{ConnectionState, Diagnostics, DiagnosticsConfiguration, parse_snapshot};

#[test]
fn diagnostics_are_disabled_by_default_and_require_an_explicit_host_flag() {
    assert!(
        !DiagnosticsConfiguration::from_value(None)
            .expect("absent configuration should be valid")
            .enabled()
    );
    assert!(
        !DiagnosticsConfiguration::from_value(Some("false"))
            .expect("false should be valid")
            .enabled()
    );
    assert!(
        DiagnosticsConfiguration::from_value(Some("1"))
            .expect("1 should enable diagnostics")
            .enabled()
    );
    assert!(
        DiagnosticsConfiguration::from_value(Some("true"))
            .expect("true should enable diagnostics")
            .enabled()
    );
    assert!(DiagnosticsConfiguration::from_value(Some("occasionally")).is_err());
}

#[test]
fn reports_bounded_observations_without_changing_the_presentation_snapshot() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("renderer should be inside the repository");
    let snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let original_revision = snapshot.revision;
    let original_playback = snapshot.playback;
    let mut diagnostics = Diagnostics::default();

    diagnostics.observe_connection(ConnectionState::Connected);
    diagnostics.observe_snapshot(&snapshot, repository_root);
    diagnostics.observe_frame(Duration::from_millis(10));
    diagnostics.observe_frame(Duration::from_millis(27));

    assert_eq!(
        diagnostics.overlay_text(Some(48 * 1024 * 1024)),
        "Memory  48.0 MiB\nFrame   17.0 ms\nArtwork 1200 × 1200\nConnection connected\nRevision 7"
    );
    assert_eq!(snapshot.revision, original_revision);
    assert_eq!(snapshot.playback, original_playback);
}
