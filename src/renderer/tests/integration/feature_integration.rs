use crate::support;

use std::time::{Duration, UNIX_EPOCH};

use roonscape_renderer::{
    InactivityConfiguration, InactivityTransform, Presentation, PresentationBehavior,
    PresentationState, PresentationTime, PresentationTransition, PresentationUpdate,
    parse_snapshot,
};

const PLAYING_SAMPLED_AT: u64 = 1_786_821_600;

fn presentation_time(milliseconds: u64) -> PresentationTime {
    PresentationTime::new(
        Duration::from_millis(milliseconds),
        UNIX_EPOCH + Duration::from_secs(PLAYING_SAMPLED_AT) + Duration::from_millis(milliseconds),
    )
}

#[test]
fn static_fixture_behavior_uses_reduced_animation_reference_states() {
    let behavior = PresentationBehavior::StaticFixture;
    let animations_enabled = behavior.animations_enabled(true);
    assert!(!animations_enabled);

    let starting = snapshot("loading.json", 7);
    let Presentation::NowPlaying(starting) =
        roonscape_renderer::presentation_from_snapshot(&starting)
            .expect("Starting should be presentable")
    else {
        panic!("Starting with content should use Now Playing");
    };
    assert_eq!(
        starting
            .status
            .motion
            .rotation_at(Duration::from_millis(900), animations_enabled),
        0.0
    );

    let indeterminate = snapshot("indeterminate-progress.json", 8);
    let Presentation::NowPlaying(indeterminate) =
        roonscape_renderer::presentation_from_snapshot(&indeterminate)
            .expect("indeterminate activity should be presentable")
    else {
        panic!("indeterminate activity should use Now Playing");
    };
    assert_eq!(
        indeterminate
            .activity
            .expect("indeterminate Playing should have activity")
            .waveform
            .bar_scales_at(Duration::from_millis(550), animations_enabled),
        [1.0; 7]
    );
}

fn inactivity_configuration() -> InactivityConfiguration {
    InactivityConfiguration::new(Duration::from_millis(100), 0.3, Duration::from_millis(100))
        .expect("integration inactivity configuration should be valid")
}

fn snapshot(fixture_name: &str, revision: u64) -> roonscape_renderer::PresentationSnapshot {
    let mut snapshot = parse_snapshot(&support::fixture(fixture_name))
        .expect("integration fixture should be a valid snapshot");
    snapshot.revision = revision;
    snapshot
}

#[test]
fn playback_updates_in_place_and_a_new_composition_crossfades_from_its_latest_revision() {
    let playing = snapshot("playing.json", 7);
    let mut state = PresentationState::new_with_inactivity(
        playing,
        presentation_time(0),
        inactivity_configuration(),
    )
    .expect("Playing should initialize the presentation state");
    let initial = state
        .frame_at(Duration::ZERO)
        .expect("initial Playing frame should be valid");
    let mut transition = PresentationTransition::new(7, initial.presentation);

    let paused = snapshot("paused.json", 8);
    assert_eq!(
        state
            .update(paused, presentation_time(10))
            .expect("Paused should update the presentation state"),
        PresentationUpdate::InPlace
    );
    let paused_frame = state
        .frame_at(Duration::from_millis(10))
        .expect("Paused should remain fully visible at first");
    assert_eq!(paused_frame.inactivity, InactivityTransform::default());
    transition.update_current(8, |presentation| {
        *presentation = paused_frame.presentation;
    });

    let inactive_frame = state
        .frame_at(Duration::from_millis(110))
        .expect("Paused should enter inactivity after grace");
    assert_eq!(inactive_frame.inactivity.opacity, 0.3);
    assert!(!transition.is_active());

    let resumed = snapshot("artwork-revision-changed.json", 9);
    assert_eq!(
        state
            .update(resumed, presentation_time(120))
            .expect("Playing should resume during the active transition"),
        PresentationUpdate::TransitionRequired
    );
    let restored = state
        .frame_at(Duration::from_millis(120))
        .expect("resumed Playing should render immediately");
    assert_eq!(restored.inactivity, InactivityTransform::default());
    let Presentation::NowPlaying(now_playing) = &restored.presentation else {
        panic!("Playing should restore Now Playing content");
    };
    assert_eq!(now_playing.status.label, "PLAYING");

    let discarded = transition.begin(9, restored.presentation, Duration::from_millis(120));
    assert!(discarded.is_none());
    assert_eq!(transition.current().revision(), 9);
    assert_eq!(transition.outgoing().map(|layer| layer.revision()), Some(8));
}

#[test]
fn local_disconnect_transitions_and_reconnect_keeps_resources_bounded() {
    let playing = snapshot("playing.json", 7);
    let mut state = PresentationState::new_with_inactivity(
        playing,
        presentation_time(0),
        inactivity_configuration(),
    )
    .expect("Playing should initialize the presentation state");
    let initial = state
        .frame_at(Duration::ZERO)
        .expect("initial Playing frame should be valid");
    let mut transition = PresentationTransition::new(7, initial.presentation);

    let revised = snapshot("artwork-revision-changed.json", 9);
    state
        .update(revised, presentation_time(20))
        .expect("a revision should update presentation state");
    let revised_frame = state
        .frame_at(Duration::from_millis(20))
        .expect("revised Playing frame should be valid");
    transition.begin(9, revised_frame.presentation, Duration::from_millis(20));
    assert!(transition.is_active());

    assert_eq!(
        state.disconnect(Duration::from_millis(100)),
        PresentationUpdate::TransitionRequired
    );
    let disconnected = state
        .frame_at(Duration::from_millis(100))
        .expect("local disconnect should be presentable immediately");
    assert_eq!(disconnected.inactivity, InactivityTransform::default());
    assert!(matches!(
        &disconnected.presentation,
        Presentation::FullField(_)
    ));
    let discarded = transition.begin(
        state.revision(),
        disconnected.presentation,
        Duration::from_millis(100),
    );
    assert_eq!(discarded.map(|layer| layer.revision()), Some(7));
    assert_eq!(transition.current().revision(), 9);
    assert_eq!(transition.outgoing().map(|layer| layer.revision()), Some(9));
    assert!(transition.is_active());

    assert_eq!(
        state
            .frame_at(Duration::from_millis(199))
            .expect("disconnect grace should remain fully visible")
            .inactivity,
        InactivityTransform::default()
    );
    assert_eq!(
        state
            .frame_at(Duration::from_millis(200))
            .expect("disconnect grace should be anchored at local loss")
            .inactivity
            .opacity,
        0.3
    );

    let replayed = snapshot("playing.json", 10);
    assert_eq!(
        state
            .update(replayed, presentation_time(250))
            .expect("replayed current state should be accepted"),
        PresentationUpdate::TransitionRequired
    );
    let replayed_frame = state
        .frame_at(Duration::from_millis(250))
        .expect("replayed Playing should render at full luminance");
    assert_eq!(replayed_frame.inactivity, InactivityTransform::default());
    transition.begin(10, replayed_frame.presentation, Duration::from_millis(250));

    let rapid_revision = snapshot("artwork-revision-changed.json", 11);
    state
        .update(rapid_revision, presentation_time(260))
        .expect("rapid replay revision should be accepted");
    let rapid_frame = state
        .frame_at(Duration::from_millis(260))
        .expect("rapid revision should remain presentable");
    let discarded = transition
        .begin(11, rapid_frame.presentation, Duration::from_millis(260))
        .expect("rapid replay should discard the disconnected outgoing layer");

    assert_eq!(discarded.revision(), 9);
    assert_eq!(transition.current().revision(), 11);
    assert_eq!(
        transition.outgoing().map(|layer| layer.revision()),
        Some(10)
    );
}
