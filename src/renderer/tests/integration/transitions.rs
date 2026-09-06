use crate::support;

use std::fs;
use std::path::Path;
use std::time::Duration;

use roonscape_renderer::{
    ArtworkReference, Presentation, PresentationPalette, PresentationTransition, Rgb,
    parse_snapshot, presentation_from_snapshot, resolve_presentation,
};
use tempfile::tempdir;

#[test]
fn replacement_text_retires_before_the_latest_target_appears() {
    use roonscape_renderer::ReplacementFade;
    let mut fade = ReplacementFade::new("Starting");
    assert!(!fade.update("Playing", Duration::ZERO, true));
    assert_eq!(*fade.displayed(), "Starting");
    fade.update("Playing", Duration::from_millis(100), true);
    assert!(fade.opacity() > 0.0 && fade.opacity() < 1.0);
    let interrupted_opacity = fade.opacity();
    fade.update("Paused", Duration::from_millis(100), true);
    assert_eq!(fade.opacity(), interrupted_opacity);
    assert_eq!(*fade.displayed(), "Starting");
    assert!(!fade.update("Paused", Duration::from_millis(225), true));
    assert!(fade.update("Paused", Duration::from_millis(325), true));
    assert_eq!(*fade.displayed(), "Paused");
    assert_eq!(
        fade.opacity(),
        0.0,
        "replacement happens only while invisible"
    );
    fade.update("Paused", Duration::from_millis(550), true);
    assert_eq!(fade.opacity(), 1.0);
    fade.update("Paused", Duration::from_secs(1), true);
    assert_eq!(fade.opacity(), 1.0, "unchanged text must not fade");
    assert!(fade.update("Playing", Duration::from_secs(1), false));
    assert_eq!(*fade.displayed(), "Playing");
    assert_eq!(
        fade.opacity(),
        1.0,
        "reduced animation installs the endpoint"
    );
}

#[test]
fn a_cancelled_text_replacement_restores_the_still_displayed_text() {
    use roonscape_renderer::ReplacementFade;
    let mut fade = ReplacementFade::new("Playing");
    fade.update("Paused", Duration::ZERO, true);
    fade.update("Paused", Duration::from_millis(100), true);
    let dimmed = fade.opacity();
    assert!(!fade.update("Playing", Duration::from_millis(100), true));
    assert_eq!(fade.opacity(), dimmed);
    for millis in [150, 200, 250, 325] {
        assert!(!fade.update("Playing", Duration::from_millis(millis), true));
        assert_eq!(*fade.displayed(), "Playing");
        assert!(
            fade.opacity() >= dimmed,
            "a cancelled replacement must not keep fading out"
        );
    }
    assert_eq!(fade.opacity(), 1.0);
}

#[test]
fn metadata_departure_keeps_its_deadline_and_reveal_retargets_without_a_flash() {
    use roonscape_renderer::ReplacementFade;
    let mut fade = ReplacementFade::new("title only");
    fade.retarget("with artist", Duration::ZERO, true);
    fade.retarget("with album", Duration::from_millis(100), true);
    assert_eq!(*fade.displayed(), "title only");
    assert!(fade.retarget("with album", Duration::from_millis(225), true));
    assert_eq!(*fade.displayed(), "with album");
    assert_eq!(fade.opacity(), 0.0);
    fade.retarget("with album", Duration::from_millis(337), true);
    let visible = fade.opacity();
    assert!(visible > 0.4 && visible < 0.6);
    fade.retarget("latest", Duration::from_millis(337), true);
    assert_eq!(fade.opacity(), visible);
    assert_eq!(*fade.displayed(), "with album");
    assert!(fade.retarget("latest", Duration::from_millis(562), true));
    assert_eq!(fade.opacity(), 0.0);
    assert_eq!(*fade.displayed(), "latest");
    fade.retarget("latest", Duration::from_millis(787), true);
    assert_eq!(fade.opacity(), 1.0);
    assert!(!fade.is_active());
}

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
        Some("src/shared/fixtures/artwork/playing.jpg")
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
        Some("src/shared/fixtures/artwork/revised.jpg")
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
fn static_replacement_releases_prior_layers_without_a_crossfade() {
    let (playing_revision, playing) = coordinated("playing.json");
    let (missing_revision, missing) = coordinated("missing-artwork.json");
    let mut transition = PresentationTransition::new(playing_revision, playing);

    let released = transition.replace_immediately(missing_revision, missing);

    assert_eq!(
        released
            .iter()
            .map(|layer| layer.revision())
            .collect::<Vec<_>>(),
        [7]
    );
    assert_eq!(transition.current().revision(), 8);
    assert!(!transition.is_active());
}

#[test]
fn crossfades_light_now_playing_into_missing_content_as_complete_layers() {
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
        Presentation::FullField(_) => panic!("light artwork should retain Now Playing layout"),
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
        .expect("the complete light Now Playing layout should remain as the outgoing layer");
    assert!(matches!(
        outgoing.value().presentation,
        Presentation::NowPlaying(_)
    ));
    assert!(outgoing.value().artwork_path.is_some());
    assert_ne!(outgoing.value().palette, PresentationPalette::fallback());
}

#[test]
fn now_playing_reveal_completes_after_two_225ms_phases() {
    let mut transition = PresentationTransition::new(0, "Idle");
    transition.begin(1, "Playing", Duration::from_secs(1));
    assert_eq!(transition.duration(), Duration::from_millis(450));
    assert_eq!(transition.progress(Duration::from_millis(1225)), 0.5);
    assert!(transition.finish(Duration::from_millis(1449)).is_none());
    assert!(transition.finish(Duration::from_millis(1450)).is_some());
}
