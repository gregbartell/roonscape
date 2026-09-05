use crate::support;

use std::fs;
use std::time::Duration;

use roonscape_renderer::{
    ArtworkContent, ArtworkDecoration, ArtworkLayout, ArtworkReference, Presentation,
    PresentationActivityMotion, PresentationPalette, PresentationStatusEmphasis,
    PresentationStatusMotion, PresentationStatusSymbol, parse_snapshot, presentation_from_snapshot,
    resolve_presentation,
};

#[test]
fn resolves_canonical_presentation_status_for_every_fixture_condition_and_form() {
    let expected = [
        (
            "playing.json",
            "PLAYING",
            PresentationStatusSymbol::Playing,
            PresentationStatusMotion::Static,
            PresentationStatusEmphasis::FullAccent,
            false,
        ),
        (
            "paused.json",
            "PAUSED",
            PresentationStatusSymbol::Paused,
            PresentationStatusMotion::Static,
            PresentationStatusEmphasis::MutedAccent,
            false,
        ),
        (
            "loading.json",
            "STARTING",
            PresentationStatusSymbol::Starting,
            PresentationStatusMotion::ContinuousRotation {
                period: Duration::from_millis(1_800),
            },
            PresentationStatusEmphasis::FullAccent,
            false,
        ),
        (
            "loading-empty.json",
            "STARTING",
            PresentationStatusSymbol::Starting,
            PresentationStatusMotion::ContinuousRotation {
                period: Duration::from_millis(1_800),
            },
            PresentationStatusEmphasis::FullAccent,
            true,
        ),
        (
            "stopped.json",
            "IDLE",
            PresentationStatusSymbol::Idle,
            PresentationStatusMotion::Static,
            PresentationStatusEmphasis::MutedAccent,
            true,
        ),
        (
            "pairing-required.json",
            "PAIRING REQUIRED",
            PresentationStatusSymbol::PairingRequired,
            PresentationStatusMotion::Static,
            PresentationStatusEmphasis::FullAccent,
            true,
        ),
        (
            "disconnected.json",
            "DISCONNECTED",
            PresentationStatusSymbol::Disconnected,
            PresentationStatusMotion::Static,
            PresentationStatusEmphasis::FullAccent,
            true,
        ),
        (
            "output-unavailable.json",
            "OUTPUT UNAVAILABLE",
            PresentationStatusSymbol::OutputUnavailable,
            PresentationStatusMotion::Static,
            PresentationStatusEmphasis::FullAccent,
            true,
        ),
    ];
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for (fixture_name, label, symbol, motion, emphasis, is_full_field) in expected {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("Presentation Status fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("Presentation Status fixture should produce a presentation");

        let resolved = resolve_presentation(&presentation, &repository_root);

        assert_eq!(resolved.status().label, label, "{fixture_name}");
        assert_eq!(resolved.status().symbol, symbol, "{fixture_name}");
        assert_eq!(resolved.status().motion, motion, "{fixture_name}");
        assert_eq!(resolved.status().emphasis, emphasis, "{fixture_name}");
        assert_eq!(
            matches!(resolved.presentation, Presentation::FullField(_)),
            is_full_field,
            "{fixture_name}",
        );
    }
}

#[test]
fn resolves_every_full_field_snapshot_with_truthful_copy_identity_and_fallback_palette() {
    let expected = [
        ("stopped.json", "IDLE", "Nothing is playing", None, true),
        (
            "loading-empty.json",
            "STARTING",
            "Preparing playback",
            None,
            true,
        ),
        (
            "playing-empty.json",
            "PLAYING",
            "Now Playing details unavailable",
            None,
            true,
        ),
        (
            "paused-empty.json",
            "PAUSED",
            "Now Playing details unavailable",
            None,
            true,
        ),
        (
            "pairing-required.json",
            "PAIRING REQUIRED",
            "Enable RoonScape",
            Some("In a Roon client, open Settings → Extensions and enable RoonScape."),
            false,
        ),
        (
            "disconnected.json",
            "DISCONNECTED",
            "Waiting for Roon",
            Some("Check Roon Server and the network."),
            false,
        ),
        (
            "output-unavailable.json",
            "OUTPUT UNAVAILABLE",
            "Check the selected output",
            Some(
                "Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.",
            ),
            true,
        ),
    ];
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for (fixture_name, status, heading, explanation, has_identity) in expected {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("full-field fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("full-field fixture should produce a presentation");

        let resolved = resolve_presentation(&presentation, &repository_root);
        let Presentation::FullField(full_field) = resolved.presentation else {
            panic!("{fixture_name} should resolve to a full-field presentation");
        };

        assert_eq!(full_field.status.label, status, "{fixture_name}");
        assert_eq!(full_field.heading, heading, "{fixture_name}");
        assert_eq!(full_field.explanation, explanation, "{fixture_name}");
        assert_eq!(
            full_field.identity.is_some(),
            has_identity,
            "{fixture_name}",
        );
        assert_eq!(
            resolved.palette,
            PresentationPalette::fallback(),
            "{fixture_name}",
        );
    }
}

#[test]
fn unreadable_artwork_without_metadata_resolves_to_details_unavailable() {
    let repository_root = tempfile::tempdir().expect("temporary repository root should be created");
    fs::write(repository_root.path().join("broken.jpg"), b"not an image")
        .expect("unreadable artwork fixture should be written");
    let mut snapshot = parse_snapshot(&support::fixture("playing-empty.json"))
        .expect("trackless Playing fixture should be valid");
    snapshot.artwork = Some(ArtworkReference {
        revision: 18,
        path: "broken.jpg".to_owned(),
    });
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("artwork-referencing snapshot should produce a presentation");

    let resolved = resolve_presentation(&presentation, repository_root.path());
    let Presentation::FullField(full_field) = resolved.presentation else {
        panic!("unreadable artwork without metadata should not retain Now Playing layout");
    };

    assert_eq!(full_field.status.label, "PLAYING");
    assert_eq!(full_field.heading, "Now Playing details unavailable");
    assert!(full_field.identity.is_some());
    assert_eq!(resolved.palette, PresentationPalette::fallback());
}

#[test]
fn metadata_with_unreadable_artwork_resolves_to_the_quiet_artwork_field() {
    let repository_root = tempfile::tempdir().expect("temporary repository root should be created");
    fs::write(repository_root.path().join("broken.jpg"), b"not an image")
        .expect("unreadable artwork fixture should be written");
    let mut snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    snapshot.artwork = Some(ArtworkReference {
        revision: 21,
        path: "broken.jpg".to_owned(),
    });
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("artwork-referencing snapshot should produce a presentation");

    let resolved = resolve_presentation(&presentation, repository_root.path());
    let Presentation::NowPlaying(now_playing) = resolved.presentation else {
        panic!("usable metadata should retain Now Playing layout");
    };

    assert_eq!(now_playing.artwork_revision, None);
    assert_eq!(now_playing.artwork_path, None);
    let artwork_layout = ArtworkLayout::for_presentation(&now_playing, None);
    assert_eq!(artwork_layout.content, ArtworkContent::QuietField);
    assert_eq!(
        artwork_layout.decoration,
        ArtworkDecoration::QuietSquareField,
    );
    assert_eq!(resolved.palette, PresentationPalette::fallback());
}

#[test]
fn resolves_indeterminate_playing_as_artwork_backed_audio_activity() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let snapshot = parse_snapshot(&support::fixture("indeterminate-progress.json"))
        .expect("Indeterminate progress Fixture Scenario should be valid");
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("indeterminate Playing should produce a presentation");

    let resolved = resolve_presentation(&presentation, &repository_root);
    let Presentation::NowPlaying(indeterminate) = resolved.presentation else {
        panic!("indeterminate Playing should retain Now Playing content");
    };
    let activity = indeterminate
        .activity
        .expect("indeterminate Playing should expose audio activity");

    assert!(indeterminate.artwork_path.is_some());
    assert_eq!(indeterminate.progress, None);
    assert_eq!(activity.heading, "Audio active");
    assert_eq!(activity.detail, "Timing unavailable");
    assert_eq!(
        activity.waveform.reference_heights_percent,
        [30, 70, 100, 48, 100, 70, 30]
    );
    assert_eq!(activity.waveform.minimum_scale_percent, 28);
    assert_eq!(
        activity.waveform.motion,
        PresentationActivityMotion::AlternatingEaseInOut {
            period: Duration::from_millis(1_100),
        }
    );
    assert!(
        activity
            .waveform
            .phase_offsets
            .windows(2)
            .all(|phases| phases[0] < phases[1]),
        "activity bars should use staggered phases"
    );
    assert_eq!(
        activity.waveform.bar_scales_at(Duration::ZERO, true)[0],
        1.0
    );
    assert_eq!(
        activity
            .waveform
            .bar_scales_at(Duration::from_millis(550), true)[0],
        0.28
    );
    assert_eq!(
        activity
            .waveform
            .bar_scales_at(Duration::from_millis(1_100), true)[0],
        1.0
    );
    assert_eq!(
        activity
            .waveform
            .bar_scales_at(Duration::from_millis(275), false),
        [1.0; 7],
        "reduced animation should retain the reference-height waveform"
    );

    let determinate = parse_snapshot(&support::fixture("playing.json"))
        .expect("determinate Playing Fixture Scenario should be valid");
    let determinate = presentation_from_snapshot(&determinate)
        .expect("determinate Playing should produce a presentation");
    let Presentation::NowPlaying(determinate) = determinate else {
        panic!("determinate Playing should retain Now Playing content");
    };

    assert!(determinate.progress.is_some());
    assert_eq!(determinate.activity, None);
}
