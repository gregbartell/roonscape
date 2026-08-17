mod support;

use std::time::{Duration, UNIX_EPOCH};

use roonscape_renderer::{
    DiagnosticsConfiguration, InactivityConfiguration, InactivityTransform, Presentation,
    PresentationState, PresentationTime, PresentationTransition, PresentationUpdate,
    inactivity_configuration_from_display_configuration, parse_snapshot,
};

const PLAYING_SAMPLED_AT: u64 = 1_786_821_600;

fn presentation_time(milliseconds: u64) -> PresentationTime {
    PresentationTime::new(
        Duration::from_millis(milliseconds),
        UNIX_EPOCH + Duration::from_secs(PLAYING_SAMPLED_AT) + Duration::from_millis(milliseconds),
    )
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
fn tracked_output_only_display_configuration_remains_valid_with_optional_diagnostics() {
    let inactivity = inactivity_configuration_from_display_configuration(&support::fixture(
        "display-configuration-tracked-output-only.json",
    ))
    .expect("Tracked Output-only configuration should remain valid");

    assert_eq!(inactivity, InactivityConfiguration::default());
    assert!(
        !DiagnosticsConfiguration::from_value(None)
            .expect("absent diagnostics configuration should be valid")
            .enabled()
    );
    assert!(
        DiagnosticsConfiguration::from_value(Some("true"))
            .expect("diagnostics should remain independently opt-in")
            .enabled()
    );
}

#[test]
fn revision_crossfade_enters_inactivity_and_playing_restores_during_the_transition() {
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
        PresentationUpdate::TransitionRequired
    );
    let paused_frame = state
        .frame_at(Duration::from_millis(10))
        .expect("Paused should remain fully visible at first");
    assert_eq!(paused_frame.inactivity, InactivityTransform::default());
    transition.begin(8, paused_frame.presentation, Duration::from_millis(10));

    let inactive_frame = state
        .frame_at(Duration::from_millis(110))
        .expect("Paused should enter inactivity after grace");
    assert_eq!(inactive_frame.inactivity.opacity, 0.3);
    assert!(transition.is_active());

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

    let discarded = transition
        .begin(9, restored.presentation, Duration::from_millis(120))
        .expect("a rapid revision should discard the obsolete outgoing layer");
    assert_eq!(discarded.revision(), 7);
    assert_eq!(transition.current().revision(), 9);
    assert_eq!(transition.outgoing().map(|layer| layer.revision()), Some(8));
}

#[test]
fn local_disconnect_clears_a_transition_and_reconnect_keeps_resources_bounded() {
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
        PresentationUpdate::ReplaceImmediately
    );
    let disconnected = state
        .frame_at(Duration::from_millis(100))
        .expect("local disconnect should be presentable immediately");
    assert_eq!(disconnected.inactivity, InactivityTransform::default());
    assert!(matches!(
        &disconnected.presentation,
        Presentation::FullField(_)
    ));
    let (discarded_current, discarded_outgoing) =
        transition.replace_immediately(state.revision(), disconnected.presentation);
    assert_eq!(discarded_current.revision(), 9);
    assert_eq!(discarded_outgoing.map(|layer| layer.revision()), Some(7));
    assert!(!transition.is_active());

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
