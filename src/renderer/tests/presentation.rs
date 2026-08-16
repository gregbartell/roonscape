mod support;

use std::time::{Duration, UNIX_EPOCH};

use roonscape_renderer::{
    InactivityConfiguration, InactivityTransform, LayoutOffset, NowPlaying, Playback, Presentation,
    PresentationState, PresentationTime, PresentationUpdate, parse_snapshot,
    presentation_from_snapshot,
};

const PLAYING_SAMPLED_AT: u64 = 1_786_821_600;

fn presentation_time(monotonic_seconds: u64, utc_seconds: u64) -> PresentationTime {
    PresentationTime::new(
        Duration::from_secs(monotonic_seconds),
        UNIX_EPOCH + Duration::from_secs(utc_seconds),
    )
}

fn inactivity_configuration() -> InactivityConfiguration {
    InactivityConfiguration::new(Duration::from_secs(5), 0.3, Duration::from_secs(2))
        .expect("test inactivity configuration should be valid")
}

fn assert_full_field_identity(
    presentation: &roonscape_renderer::FullFieldPresentation,
    tracked_output: &str,
    tracked_zone: &str,
) {
    let identity = presentation
        .identity
        .as_ref()
        .expect("available full-field presentation should retain authoritative identities");
    assert_eq!(identity.tracked_output, tracked_output);
    assert_eq!(identity.tracked_zone, tracked_zone);
}

#[test]
fn maps_the_playing_snapshot_to_gallery_split_content() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Playing snapshot should produce a presentation");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };

    assert_eq!(presentation.title.as_deref(), Some("Last Light on Phobos"));
    assert_eq!(
        presentation.artist.as_deref(),
        Some("Evelyn Lark & The Orbital Choir")
    );
    assert_eq!(
        presentation.album.as_deref(),
        Some("Signals from the Quiet Sea")
    );
    assert_eq!(presentation.tracked_output, "AudioDevice");
    assert_eq!(presentation.tracked_zone, "Living Room");
    assert_eq!(presentation.playback_state(), "Playing");
    assert_eq!(presentation.artwork_revision, Some(3));
    assert_eq!(
        presentation.artwork_path.as_deref(),
        Some("src/shared/fixtures/artwork/playing.svg")
    );

    let progress = presentation
        .progress
        .expect("Playing fixture should include determinate progress");
    assert!((progress.fraction - (171.0 / 266.0)).abs() < f64::EPSILON);
    assert_eq!(progress.elapsed, "2:51");
    assert_eq!(progress.remaining, "−1:35");
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
            "Tracked Output unavailable",
            "Configure a Tracked Output on this RoonScape Host, or check that the selected output is available in Roon.",
        ),
    ];

    for (fixture_name, state_label, heading, explanation) in expected {
        let fixture = support::fixture(fixture_name);
        let snapshot = parse_snapshot(&fixture).expect("unavailable fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("unavailable snapshot should produce a presentation");
        let Presentation::FullField(presentation) = presentation else {
            panic!("unavailable snapshot should use the full-field presentation");
        };

        assert_eq!(
            (
                presentation.state_label,
                presentation.heading,
                presentation.explanation,
            ),
            (state_label, heading, Some(explanation)),
        );
        assert_eq!(presentation.identity, None);
    }
}

#[test]
fn presents_stopped_playback_as_idle_full_field_copy_with_authoritative_identities() {
    let snapshot =
        parse_snapshot(&support::fixture("stopped.json")).expect("Stopped fixture should be valid");

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Stopped playback should produce a presentation");
    let Presentation::FullField(presentation) = presentation else {
        panic!("Stopped playback should use the full-field presentation");
    };

    assert_eq!(presentation.state_label, "Idle");
    assert_eq!(presentation.heading, "Nothing is playing");
    assert_eq!(presentation.explanation, None);
    assert_full_field_identity(&presentation, "AudioDevice", "Living Room");
}

#[test]
fn presents_empty_loading_as_full_field_copy_with_authoritative_identities() {
    let snapshot = parse_snapshot(&support::fixture("loading-empty.json"))
        .expect("empty Loading fixture should be valid");

    let presentation =
        presentation_from_snapshot(&snapshot).expect("empty Loading should produce a presentation");
    let Presentation::FullField(presentation) = presentation else {
        panic!("empty Loading should use the full-field presentation");
    };

    assert_eq!(presentation.state_label, "Loading");
    assert_eq!(presentation.heading, "Loading");
    assert_eq!(presentation.explanation, None);
    assert_full_field_identity(&presentation, "AudioDevice", "Living Room");
}

#[test]
fn presents_playing_without_usable_content_as_a_truthful_full_field() {
    let snapshot = parse_snapshot(&support::fixture("playing-empty.json"))
        .expect("trackless Playing fixture should be valid");

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("trackless Playing should produce a presentation");
    let Presentation::FullField(presentation) = presentation else {
        panic!("trackless Playing should use the full-field presentation");
    };

    assert_eq!(presentation.state_label, "Playing");
    assert_eq!(presentation.heading, "Now Playing details unavailable");
    assert_eq!(presentation.explanation, None);
    assert_full_field_identity(&presentation, "AudioDevice", "Living Room");
}

#[test]
fn treats_whitespace_only_now_playing_as_unusable_content() {
    let mut snapshot = parse_snapshot(&support::fixture("playing-empty.json"))
        .expect("trackless Playing fixture should be valid");
    snapshot.now_playing = Some(NowPlaying {
        title: Some("  ".to_owned()),
        artist: Some("\t".to_owned()),
        album: Some("\n".to_owned()),
    });

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("whitespace-only Now Playing should produce a presentation");

    assert!(matches!(presentation, Presentation::FullField(_)));
}

#[test]
fn advances_playing_progress_from_the_latest_local_anchor() {
    let snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let state = PresentationState::new(snapshot, presentation_time(10, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");

    let presentation = state
        .presentation_at(Duration::from_secs(15))
        .expect("anchored Playing presentation should render");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };
    let progress = presentation
        .progress
        .expect("Playing fixture should include progress");

    assert!((progress.fraction - (176.0 / 266.0)).abs() < f64::EPSILON);
    assert_eq!(progress.elapsed, "2:56");
    assert_eq!(progress.remaining, "−1:30");
}

#[test]
fn freezes_paused_and_loading_progress_at_the_source_sample() {
    for fixture_name in ["paused.json", "loading.json"] {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("inactive playback fixture should be valid");
        let state =
            PresentationState::new(snapshot, presentation_time(10, PLAYING_SAMPLED_AT + 60))
                .expect("inactive playback should anchor a presentation");

        let presentation = state
            .presentation_at(Duration::from_secs(70))
            .expect("inactive presentation should render");
        let Presentation::NowPlaying(presentation) = presentation else {
            panic!("available playback should produce Now Playing content");
        };

        assert_eq!(
            presentation.progress.map(|progress| progress.elapsed),
            Some("2:51".to_owned())
        );
    }
}

#[test]
fn reanchors_playing_progress_when_a_new_source_sample_arrives() {
    let initial =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let mut seeked =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    seeked.revision = 8;
    let seeked_progress = seeked
        .progress
        .as_mut()
        .expect("Playing fixture should contain progress");
    seeked_progress.position_seconds = 30.0;
    seeked_progress.sampled_at = "2026-08-15T19:20:05Z".to_owned();

    state
        .update(seeked, presentation_time(5, PLAYING_SAMPLED_AT + 5))
        .expect("seek sample should replace the local anchor");
    let presentation = state
        .presentation_at(Duration::from_secs(7))
        .expect("re-anchored presentation should render");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };

    assert_eq!(
        presentation.progress.map(|progress| progress.elapsed),
        Some("0:32".to_owned())
    );
}

#[test]
fn clamps_source_and_locally_advanced_progress_at_duration() {
    for (source_position, now) in [(300.0, Duration::ZERO), (171.0, Duration::from_secs(1_000))] {
        let mut snapshot = parse_snapshot(&support::fixture("playing.json"))
            .expect("Playing fixture should be valid");
        snapshot
            .progress
            .as_mut()
            .expect("Playing fixture should have progress")
            .position_seconds = source_position;
        let state = PresentationState::new(snapshot, presentation_time(0, PLAYING_SAMPLED_AT))
            .expect("clamping fixture should anchor a presentation");
        let presentation = state
            .presentation_at(now)
            .expect("clamped presentation should render");
        let Presentation::NowPlaying(presentation) = presentation else {
            panic!("Playing snapshot should produce Now Playing content");
        };
        let progress = presentation.progress.expect("fixture should show progress");

        assert_eq!(progress.fraction, 1.0);
        assert_eq!(progress.elapsed, "4:26");
        assert_eq!(progress.remaining, "−0:00");
    }
}

#[test]
fn presents_each_playback_state_without_inventing_now_playing() {
    let fixtures = [
        (
            "playing.json",
            "Playing",
            Some("Last Light on Phobos"),
            Some(3),
        ),
        (
            "paused.json",
            "Paused",
            Some("Last Light on Phobos"),
            Some(3),
        ),
        (
            "loading.json",
            "Loading",
            Some("Last Light on Phobos"),
            Some(3),
        ),
    ];

    for (fixture_name, state_label, title, artwork_revision) in fixtures {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("playback fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("playback fixture should produce a presentation");
        let Presentation::NowPlaying(presentation) = presentation else {
            panic!("available playback should produce an available presentation");
        };

        assert_eq!(presentation.playback_state(), state_label);
        assert_eq!(presentation.title.as_deref(), title);
        assert_eq!(presentation.artwork_revision, artwork_revision);
    }
}

#[test]
fn clears_now_playing_from_a_stopped_presentation() {
    let mut snapshot =
        parse_snapshot(&support::fixture("loading.json")).expect("Loading fixture should be valid");
    snapshot.playback = Some(Playback::Stopped);

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Stopped playback should produce a presentation");
    let Presentation::FullField(presentation) = presentation else {
        panic!("Stopped playback should replace stale content with a full-field presentation");
    };

    assert_eq!(presentation.state_label, "Idle");
    assert_eq!(presentation.heading, "Nothing is playing");
    assert_eq!(presentation.explanation, None);
}

#[test]
fn accounts_for_source_sample_age_when_anchoring_playing_progress() {
    let snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let state = PresentationState::new(snapshot, presentation_time(10, PLAYING_SAMPLED_AT + 5))
        .expect("Playing snapshot should anchor a presentation");

    let presentation = state
        .presentation_at(Duration::from_secs(10))
        .expect("source-aged Playing presentation should render");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };

    assert_eq!(
        presentation.progress.map(|progress| progress.elapsed),
        Some("2:56".to_owned())
    );
}

#[test]
fn preserves_the_progress_anchor_for_a_presentation_only_revision() {
    let initial =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let mut presentation_only =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    presentation_only.revision = 8;
    presentation_only
        .now_playing
        .as_mut()
        .expect("Playing fixture should contain Now Playing")
        .title = Some("Updated opaque title".to_owned());

    state
        .update(
            presentation_only,
            presentation_time(5, PLAYING_SAMPLED_AT + 5),
        )
        .expect("presentation-only revision should be accepted");
    let presentation = state
        .presentation_at(Duration::from_secs(7))
        .expect("presentation-only revision should render");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };

    assert_eq!(presentation.title.as_deref(), Some("Updated opaque title"));
    assert_eq!(
        presentation.progress.map(|progress| progress.elapsed),
        Some("2:58".to_owned())
    );
}

#[test]
fn hides_progress_for_indeterminate_now_playing() {
    let snapshot = parse_snapshot(&support::fixture("indeterminate-progress.json"))
        .expect("indeterminate progress fixture should be valid");
    let presentation = presentation_from_snapshot(&snapshot)
        .expect("indeterminate progress should produce a presentation");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("available playback should produce an available presentation");
    };

    assert_eq!(presentation.title.as_deref(), Some("Radio Paradise"));
    assert_eq!(presentation.progress, None);
}

#[test]
fn distinguishes_progress_samples_from_visual_revision_changes() {
    let initial =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let mut progress_sample =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    progress_sample.revision = 8;
    progress_sample
        .progress
        .as_mut()
        .expect("Playing fixture should include progress")
        .position_seconds = 83.0;

    let progress_update = state
        .update(
            progress_sample,
            presentation_time(1, PLAYING_SAMPLED_AT + 1),
        )
        .expect("progress sample should update the presentation");
    assert_eq!(progress_update, PresentationUpdate::ProgressOnly);

    let mut indeterminate =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    indeterminate.revision = 9;
    indeterminate.progress = None;
    let progress_disappeared = state
        .update(indeterminate, presentation_time(2, PLAYING_SAMPLED_AT + 2))
        .expect("indeterminate timing should update the presentation");
    assert_eq!(progress_disappeared, PresentationUpdate::TransitionRequired);

    let mut determinate_again =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    determinate_again.revision = 10;
    let progress_appeared = state
        .update(
            determinate_again,
            presentation_time(3, PLAYING_SAMPLED_AT + 3),
        )
        .expect("determinate timing should update the presentation");
    assert_eq!(progress_appeared, PresentationUpdate::TransitionRequired);

    let mut revised = parse_snapshot(&support::fixture("artwork-revision-changed.json"))
        .expect("artwork revision fixture should be valid");
    revised.revision = 11;
    let visual_update = state
        .update(revised, presentation_time(4, PLAYING_SAMPLED_AT + 4))
        .expect("visual revision should update the presentation");
    assert_eq!(visual_update, PresentationUpdate::TransitionRequired);
    assert_eq!(state.revision(), 11);
}

#[test]
fn every_visual_snapshot_field_requests_one_coordinated_transition() {
    let playing_snapshot = || {
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid")
    };
    let mut playback = playing_snapshot();
    playback.playback = Some(Playback::Paused);
    let mut identity = playing_snapshot();
    identity
        .tracked_zone
        .as_mut()
        .expect("Playing fixture should identify the Tracked Zone")
        .name = "Kitchen".to_owned();
    let mut metadata = playing_snapshot();
    metadata
        .now_playing
        .as_mut()
        .expect("Playing fixture should contain Now Playing")
        .title = Some("A different track".to_owned());
    let mut progress_presence = playing_snapshot();
    progress_presence.progress = None;
    let mut artwork = playing_snapshot();
    let artwork_reference = artwork
        .artwork
        .as_mut()
        .expect("Playing fixture should reference artwork");
    artwork_reference.revision = 12;
    artwork_reference.path = "src/shared/fixtures/artwork/revised.svg".to_owned();

    for (index, (field, mut changed)) in [
        ("playback", playback),
        ("identity", identity),
        ("metadata", metadata),
        ("progress presence", progress_presence),
        ("artwork and palette", artwork),
    ]
    .into_iter()
    .enumerate()
    {
        let mut state =
            PresentationState::new(playing_snapshot(), presentation_time(0, PLAYING_SAMPLED_AT))
                .expect("Playing snapshot should anchor a presentation");
        changed.revision = 30 + index as u64;

        assert_eq!(
            state
                .update(
                    changed,
                    presentation_time(index as u64 + 1, PLAYING_SAMPLED_AT + index as u64 + 1),
                )
                .expect("visual snapshot change should be accepted"),
            PresentationUpdate::TransitionRequired,
            "a {field} change should replace the complete presentation once"
        );
    }
}

#[test]
fn unavailable_snapshots_replace_now_playing_immediately() {
    for (index, fixture_name) in [
        "pairing-required.json",
        "disconnected.json",
        "output-unavailable.json",
    ]
    .into_iter()
    .enumerate()
    {
        let playing = parse_snapshot(&support::fixture("playing.json"))
            .expect("Playing fixture should be valid");
        let mut state = PresentationState::new(playing, presentation_time(0, PLAYING_SAMPLED_AT))
            .expect("Playing snapshot should anchor a presentation");
        let mut unavailable = parse_snapshot(&support::fixture(fixture_name))
            .expect("unavailable fixture should be valid");
        unavailable.revision = 20 + index as u64;

        let update = state
            .update(
                unavailable,
                presentation_time(index as u64 + 1, PLAYING_SAMPLED_AT + index as u64 + 1),
            )
            .expect("unavailable snapshot should replace Playing");

        assert_eq!(
            update,
            PresentationUpdate::ReplaceImmediately,
            "{fixture_name} must not retain outgoing Now Playing content"
        );
        assert!(matches!(
            state
                .presentation_at(Duration::from_secs(index as u64 + 1))
                .expect("unavailable snapshot should remain presentable"),
            Presentation::FullField(_)
        ));
    }
}

#[test]
fn inactive_conditions_remain_fully_legible_until_the_grace_period_ends() {
    for fixture_name in [
        "paused.json",
        "stopped.json",
        "pairing-required.json",
        "disconnected.json",
        "output-unavailable.json",
    ] {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("inactive fixture should be valid");
        let state = PresentationState::new_with_inactivity(
            snapshot,
            presentation_time(10, PLAYING_SAMPLED_AT),
            inactivity_configuration(),
        )
        .expect("inactive fixture should anchor a presentation");

        assert_eq!(
            state
                .frame_at(Duration::from_millis(14_999))
                .expect("grace-period presentation should render")
                .inactivity,
            InactivityTransform::default(),
            "{fixture_name} should remain fully legible before grace expires"
        );
        assert_eq!(
            state
                .frame_at(Duration::from_secs(15))
                .expect("inactive presentation should render")
                .inactivity,
            InactivityTransform {
                opacity: 0.3,
                offset: LayoutOffset { x: -18, y: -12 },
            },
            "{fixture_name} should dim and reposition at the grace boundary"
        );
    }
}

#[test]
fn loading_and_playing_stay_active_with_gallery_or_missing_content() {
    for fixture_name in [
        "playing.json",
        "loading.json",
        "playing-empty.json",
        "loading-empty.json",
    ] {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("active fixture should be valid");
        let state = PresentationState::new_with_inactivity(
            snapshot,
            presentation_time(0, PLAYING_SAMPLED_AT),
            inactivity_configuration(),
        )
        .expect("active fixture should anchor a presentation");

        assert_eq!(
            state
                .frame_at(Duration::from_secs(60))
                .expect("active fixture should remain presentable")
                .inactivity,
            InactivityTransform::default(),
            "{fixture_name} must not enter OLED inactivity"
        );
    }
}

#[test]
fn repeatedly_repositions_inactive_content_within_gallery_split_bounds() {
    let snapshot =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    let state = PresentationState::new_with_inactivity(
        snapshot,
        presentation_time(0, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
    )
    .expect("Paused fixture should anchor a presentation");

    let offsets = [5, 7, 9, 11, 13].map(|seconds| {
        state
            .frame_at(Duration::from_secs(seconds))
            .expect("inactive presentation should render")
            .inactivity
            .offset
    });

    assert_eq!(
        offsets,
        [
            LayoutOffset { x: -18, y: -12 },
            LayoutOffset { x: 12, y: 8 },
            LayoutOffset { x: -8, y: 12 },
            LayoutOffset { x: 18, y: -6 },
            LayoutOffset { x: 6, y: -12 },
        ]
    );
    assert!(offsets.iter().all(|offset| offset.x.abs() <= 18));
    assert!(offsets.iter().all(|offset| offset.y.abs() <= 12));
}

#[test]
fn changing_inactive_condition_restarts_the_grace_period() {
    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    let mut state = PresentationState::new_with_inactivity(
        paused,
        presentation_time(0, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
    )
    .expect("Paused fixture should anchor a presentation");
    assert_eq!(
        state
            .frame_at(Duration::from_secs(6))
            .expect("dimmed Paused presentation should render")
            .inactivity
            .opacity,
        0.3
    );

    let disconnected = parse_snapshot(&support::fixture("disconnected.json"))
        .expect("Disconnected fixture should be valid");
    state
        .update(disconnected, presentation_time(6, PLAYING_SAMPLED_AT + 6))
        .expect("Disconnected snapshot should replace Paused");

    let changed = state
        .frame_at(Duration::from_secs(6))
        .expect("changed presentation should render");
    assert_eq!(changed.inactivity, InactivityTransform::default());
    assert!(matches!(changed.presentation, Presentation::FullField(_)));
    assert_eq!(
        state
            .frame_at(Duration::from_millis(10_999))
            .expect("new grace period should remain legible")
            .inactivity,
        InactivityTransform::default()
    );
    assert_eq!(
        state
            .frame_at(Duration::from_secs(11))
            .expect("new inactive treatment should render")
            .inactivity
            .opacity,
        0.3
    );
}

#[test]
fn fixture_selection_restarts_inactivity_without_changing_live_mode_updates() {
    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    let mut live_paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    live_paused.revision += 1;
    let mut fixture_paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    fixture_paused.revision += 1;
    let mut live = PresentationState::new_with_inactivity(
        paused,
        presentation_time(0, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
    )
    .expect("Live Mode Paused state should be presentable");
    let mut fixture = PresentationState::new_with_inactivity(
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid"),
        presentation_time(0, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
    )
    .expect("Fixture Mode Paused state should be presentable");

    live.update(live_paused, presentation_time(6, PLAYING_SAMPLED_AT + 6))
        .expect("Live Mode update should remain valid");
    fixture
        .update_for_fixture_selection(fixture_paused, presentation_time(6, PLAYING_SAMPLED_AT + 6))
        .expect("Fixture Scenario selection should remain valid");

    assert_eq!(
        live.frame_at(Duration::from_secs(6))
            .expect("Live Mode frame should render")
            .inactivity
            .opacity,
        0.3
    );
    assert_eq!(
        fixture
            .frame_at(Duration::from_secs(6))
            .expect("Fixture Mode frame should render")
            .inactivity,
        InactivityTransform::default()
    );
    assert_eq!(
        fixture
            .frame_at(Duration::from_secs(11))
            .expect("fresh Fixture Mode grace period should expire")
            .inactivity
            .opacity,
        0.3
    );
}

#[test]
fn playing_cancels_a_stale_inactivity_deadline_before_it_fires() {
    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    let mut state = PresentationState::new_with_inactivity(
        paused,
        presentation_time(0, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
    )
    .expect("Paused fixture should anchor a presentation");
    let playing =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");

    state
        .update(playing, presentation_time(4, PLAYING_SAMPLED_AT + 4))
        .expect("Playing snapshot should replace Paused");

    assert_eq!(
        state
            .frame_at(Duration::from_secs(5))
            .expect("Playing presentation should render past the stale deadline")
            .inactivity,
        InactivityTransform::default()
    );
}

#[test]
fn playing_immediately_restores_luminance_position_and_advancing_progress() {
    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    let mut state = PresentationState::new_with_inactivity(
        paused,
        presentation_time(0, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
    )
    .expect("Paused fixture should anchor a presentation");
    assert_ne!(
        state
            .frame_at(Duration::from_secs(6))
            .expect("inactive Paused presentation should render")
            .inactivity,
        InactivityTransform::default()
    );

    let playing =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    state
        .update(playing, presentation_time(6, PLAYING_SAMPLED_AT + 6))
        .expect("Playing snapshot should replace Paused");

    let restored = state
        .frame_at(Duration::from_secs(6))
        .expect("restored Playing presentation should render");
    assert_eq!(restored.inactivity, InactivityTransform::default());
    let Presentation::NowPlaying(now_playing) = restored.presentation else {
        panic!("Playing should restore Now Playing content");
    };
    assert_eq!(
        now_playing.progress.map(|progress| progress.elapsed),
        Some("2:57".to_owned())
    );

    let advanced = state
        .frame_at(Duration::from_secs(8))
        .expect("Playing progress should advance locally");
    let Presentation::NowPlaying(now_playing) = advanced.presentation else {
        panic!("Playing should retain Now Playing content");
    };
    assert_eq!(
        now_playing.progress.map(|progress| progress.elapsed),
        Some("2:59".to_owned())
    );
}

#[test]
fn clears_now_playing_while_the_bridge_is_disconnected_and_recovers_after_reconnect() {
    let mut state = PresentationState::disconnected();
    let playing =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    state
        .update(playing, presentation_time(10, PLAYING_SAMPLED_AT))
        .expect("current state should be accepted after connection");
    assert!(matches!(
        state
            .presentation_at(Duration::from_secs(10))
            .expect("Playing state should remain presentable"),
        Presentation::NowPlaying(_)
    ));

    assert_eq!(
        state.disconnect(Duration::from_secs(11)),
        PresentationUpdate::ReplaceImmediately
    );
    let Presentation::FullField(disconnected) = state
        .presentation_at(Duration::from_secs(11))
        .expect("disconnection should remain presentable")
    else {
        panic!("disconnection must clear stale Now Playing content");
    };
    assert_eq!(disconnected.state_label, "Disconnected");

    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    state
        .update(paused, presentation_time(12, PLAYING_SAMPLED_AT + 2))
        .expect("current state should be accepted after reconnect");
    let Presentation::NowPlaying(reconnected) = state
        .presentation_at(Duration::from_secs(12))
        .expect("reconnected state should remain presentable")
    else {
        panic!("reconnected current state should replace the Disconnected presentation");
    };
    assert_eq!(reconnected.playback_state(), "Paused");
}
