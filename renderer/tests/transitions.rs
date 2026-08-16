mod support;

use std::path::Path;
use std::time::Duration;

use roonscape_renderer::{
    Presentation, PresentationPalette, PresentationTransition, parse_snapshot,
    presentation_from_snapshot,
};

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
        Presentation::Unavailable(_) => None,
    };
    let resolved_artwork = artwork_path
        .as_deref()
        .map(|path| Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path));
    let palette = PresentationPalette::for_artwork(resolved_artwork.as_deref())
        .expect("transition artwork should produce a palette");

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
        Some("fixtures/artwork/playing.svg")
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
        Some("fixtures/artwork/revised.svg")
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
