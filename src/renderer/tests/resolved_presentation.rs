mod support;

use std::fs;

use roonscape_renderer::{
    ArtworkContent, ArtworkLayout, ArtworkReference, Presentation, PresentationPalette,
    StatusEmphasis, parse_snapshot, presentation_from_snapshot, resolve_presentation,
};

#[test]
fn resolves_every_full_field_snapshot_with_truthful_copy_identity_and_fallback_palette() {
    let expected = [
        (
            "stopped.json",
            "Idle",
            "Nothing is playing",
            None,
            true,
            StatusEmphasis::Quiet,
        ),
        (
            "loading-empty.json",
            "Loading",
            "Loading",
            None,
            true,
            StatusEmphasis::Quiet,
        ),
        (
            "playing-empty.json",
            "Playing",
            "Now Playing details unavailable",
            None,
            true,
            StatusEmphasis::Prominent,
        ),
        (
            "pairing-required.json",
            "Pairing required",
            "Enable RoonScape",
            Some("Open Settings → Extensions in a Roon client, then enable RoonScape."),
            false,
            StatusEmphasis::Prominent,
        ),
        (
            "disconnected.json",
            "Disconnected",
            "Waiting for Roon",
            Some("Check Roon Server and the network. This display updates when Roon returns."),
            false,
            StatusEmphasis::Prominent,
        ),
        (
            "output-unavailable.json",
            "Output unavailable",
            "Tracked Output unavailable",
            Some(
                "Configure a Tracked Output on this RoonScape Host, or check that the selected output is available in Roon.",
            ),
            false,
            StatusEmphasis::Prominent,
        ),
    ];
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for (fixture_name, state_label, heading, explanation, has_identity, status_emphasis) in expected
    {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("full-field fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("full-field fixture should produce a presentation");

        let resolved = resolve_presentation(&presentation, &repository_root);
        let Presentation::FullField(full_field) = resolved.presentation else {
            panic!("{fixture_name} should resolve to a full-field presentation");
        };

        assert_eq!(full_field.state_label, state_label);
        assert_eq!(full_field.heading, heading);
        assert_eq!(full_field.explanation, explanation);
        assert_eq!(full_field.identity.is_some(), has_identity);
        assert_eq!(full_field.status_emphasis, status_emphasis);
        assert_eq!(resolved.palette, PresentationPalette::fallback());
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
        panic!("unreadable artwork without metadata should not retain Gallery split");
    };

    assert_eq!(full_field.state_label, "Playing");
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
        panic!("usable metadata should retain Gallery split");
    };

    assert_eq!(now_playing.artwork_revision, None);
    assert_eq!(now_playing.artwork_path, None);
    assert_eq!(
        ArtworkLayout::for_presentation(&now_playing).content,
        ArtworkContent::QuietField,
    );
    assert_eq!(resolved.palette, PresentationPalette::fallback());
}
