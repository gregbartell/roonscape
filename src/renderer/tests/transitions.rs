mod support;

use std::fs;
use std::path::Path;
use std::time::Duration;

use roonscape_renderer::{
    ArtworkReference, Presentation, PresentationPalette, PresentationTransition, Rgb,
    parse_snapshot, presentation_from_snapshot, resolve_presentation,
};
use tempfile::tempdir;

struct CoordinatedPresentation {
    presentation: Presentation,
    artwork_path: Option<String>,
    palette: PresentationPalette,
}

fn coordinated(fixture_name: &str) -> (u64, CoordinatedPresentation) {
    let snapshot = parse_snapshot(&support::fixture(fixture_name))
        .expect("transition fixture should be a valid shared snapshot");
    let revision = snapshot.revision;
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("transition fixture should produce a presentation");
    let artwork_path = match &presentation {
        Presentation::NowPlaying(now_playing) => now_playing.artwork_path.clone(),
        Presentation::FullField(_) => None,
    };
    let resolved_artwork = artwork_path.as_deref().map(|path| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    });
    let palette = PresentationPalette::for_artwork(resolved_artwork.as_deref());

    (
        revision,
        CoordinatedPresentation {
            presentation,
            artwork_path,
            palette,
        },
    )
}

fn title(presentation: &CoordinatedPresentation) -> Option<&str> {
    let Presentation::NowPlaying(now_playing) = &presentation.presentation else {
        return None;
    };
    now_playing.title.as_deref()
}

#[test]
fn revisions_transition_artwork_metadata_and_the_complete_palette_together() {
    let (playing_revision, playing) = coordinated("playing.json");
    let (missing_revision, missing) = coordinated("missing-artwork.json");
    let mut transition = PresentationTransition::new(playing_revision, playing);

    transition.begin(missing_revision, missing, Duration::from_millis(100));

    let current = transition.current();
    let outgoing = transition
        .outgoing()
        .expect("a changed revision should retain one outgoing presentation");
    assert_eq!((current.revision(), outgoing.revision()), (8, 7));
    assert_eq!(title(current.value()), Some("Last Light on Phobos"));
    assert_eq!(current.value().artwork_path, None);
    assert_eq!(current.value().palette, PresentationPalette::fallback());
    assert_eq!(title(outgoing.value()), Some("Last Light on Phobos"));
    assert_eq!(
        outgoing.value().artwork_path.as_deref(),
        Some("src/shared/fixtures/artwork/playing.svg")
    );
    assert_ne!(outgoing.value().palette, current.value().palette);
}

#[test]
fn rapid_revisions_keep_only_current_and_one_outgoing_presentation() {
    let (playing_revision, playing) = coordinated("playing.json");
    let (missing_revision, missing) = coordinated("missing-artwork.json");
    let (revised_revision, revised) = coordinated("artwork-revision-changed.json");
    let mut transition = PresentationTransition::new(playing_revision, playing);

    assert!(
        transition
            .begin(missing_revision, missing, Duration::from_millis(100))
            .is_none()
    );
    let discarded = transition
        .begin(revised_revision, revised, Duration::from_millis(200))
        .expect("a rapid revision should discard the superseded outgoing presentation");

    assert_eq!(discarded.revision(), 7);
    assert_eq!(transition.current().revision(), 9);
    assert_eq!(transition.outgoing().map(|layer| layer.revision()), Some(8));
    assert_eq!(
        title(transition.current().value()),
        Some("Last Light on Phobos")
    );
    assert_eq!(
        transition.current().value().artwork_path.as_deref(),
        Some("src/shared/fixtures/artwork/revised.svg")
    );
    assert_ne!(
        transition.current().value().palette,
        transition
            .outgoing()
            .expect("rapid transition should retain an outgoing presentation")
            .value()
            .palette
    );
}

#[test]
fn completed_transition_releases_the_outgoing_presentation_and_becomes_stable() {
    let (playing_revision, playing) = coordinated("playing.json");
    let (revised_revision, revised) = coordinated("artwork-revision-changed.json");
    let mut transition = PresentationTransition::new(playing_revision, playing);
    transition.begin(revised_revision, revised, Duration::from_millis(100));
    let completion = Duration::from_millis(100) + transition.duration();

    assert!(
        transition
            .finish(completion - Duration::from_millis(1))
            .is_none()
    );
    assert!(transition.is_active());

    let released = transition
        .finish(completion)
        .expect("transition completion should release the outgoing presentation");
    assert_eq!(released.revision(), 7);
    assert!(transition.outgoing().is_none());
    assert!(!transition.is_active());
}

#[test]
fn crossfades_light_gallery_split_into_missing_content_as_complete_layers() {
    let artwork_directory = tempdir().expect("temporary artwork directory should be available");
    let light_artwork = artwork_directory.path().join("light.svg");
    fs::write(
        &light_artwork,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
            <rect width="64" height="64" fill="#f4e7c5"/>
            <circle cx="48" cy="16" r="10" fill="#e59a73"/>
        </svg>"##,
    )
    .expect("light artwork should be writable");
    let mut snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    snapshot.revision = 18;
    snapshot.artwork = Some(ArtworkReference {
        revision: 18,
        path: light_artwork.to_string_lossy().into_owned(),
    });
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("light artwork snapshot should produce a presentation");
    let resolved = resolve_presentation(&presentation, Path::new(""));
    let artwork_path = match &resolved.presentation {
        Presentation::NowPlaying(now_playing) => now_playing.artwork_path.clone(),
        Presentation::FullField(_) => panic!("light artwork should retain Gallery split"),
    };
    let light = CoordinatedPresentation {
        presentation: resolved.presentation,
        artwork_path,
        palette: resolved.palette,
    };
    assert!(
        light.palette.background.contrast_ratio(Rgb {
            red: 0,
            green: 0,
            blue: 0,
        }) >= 7.0,
        "light artwork should retain its readable light palette"
    );

    let (missing_revision, missing) = coordinated("playing-empty.json");
    let mut transition = PresentationTransition::new(snapshot.revision, light);
    transition.begin(missing_revision, missing, Duration::from_millis(100));

    assert!(matches!(
        transition.current().value().presentation,
        Presentation::FullField(_)
    ));
    assert_eq!(
        transition.current().value().palette,
        PresentationPalette::fallback()
    );
    let outgoing = transition
        .outgoing()
        .expect("the complete light Gallery split should remain as the outgoing layer");
    assert!(matches!(
        outgoing.value().presentation,
        Presentation::NowPlaying(_)
    ));
    assert!(outgoing.value().artwork_path.is_some());
    assert_ne!(outgoing.value().palette, PresentationPalette::fallback());
}
