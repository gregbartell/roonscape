mod support;

use std::time::{Duration, UNIX_EPOCH};

use roonscape_renderer::{
    InactivityConfiguration, InactivityTransform, LayoutOffset, Playback, Presentation,
    PresentationState, PresentationTime, parse_snapshot, presentation_from_snapshot,
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

#[test]
fn maps_the_playing_snapshot_to_gallery_split_content() {
    let fixture = support::fixture("playing.json");
    let snapshot = parse_snapshot(&fixture).expect("shared Playing fixture should be valid");

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Playing snapshot should produce a presentation");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };

    assert_eq!(presentation.title.as_deref(), Some("A Moment Apart"));
    assert_eq!(presentation.artist.as_deref(), Some("ODESZA"));
    assert_eq!(presentation.album.as_deref(), Some("A Moment Apart"));
    assert_eq!(presentation.display_zone, "Gallery");
    assert_eq!(presentation.playback_state, "Playing");
    assert_eq!(presentation.artwork_revision, Some(3));
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
            "Display Output unavailable",
            "Configure a Display Output on this RoonScape Host, or check that the selected output is available in Roon.",
        ),
    ];

    for (fixture_name, state_label, heading, explanation) in expected {
        let fixture = support::fixture(fixture_name);
        let snapshot = parse_snapshot(&fixture).expect("unavailable fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("unavailable snapshot should produce a presentation");
        let Presentation::Unavailable(presentation) = presentation else {
            panic!("unavailable snapshot must not retain Now Playing content");
        };

        assert_eq!(
            (
                presentation.state_label,
                presentation.heading,
                presentation.explanation,
            ),
            (state_label, heading, explanation),
        );
    }
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

    assert!((progress.fraction - (87.0 / 234.0)).abs() < f64::EPSILON);
    assert_eq!(progress.elapsed, "1:27");
    assert_eq!(progress.remaining, "−2:27");
}

#[test]
fn freezes_paused_and_loading_progress_at_the_source_sample() {
    for (fixture_name, elapsed) in [("paused.json", "1:30"), ("loading.json", "1:31")] {
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
            Some(elapsed.to_owned())
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
    for (fixture_name, now) in [
        ("playing-past-duration.json", Duration::ZERO),
        ("playing.json", Duration::from_secs(1_000)),
    ] {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("clamping fixture should be valid");
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
        assert_eq!(progress.elapsed, "3:54");
        assert_eq!(progress.remaining, "−0:00");
    }
}

#[test]
fn presents_each_playback_state_without_inventing_now_playing() {
    let fixtures = [
        ("playing.json", "Playing", Some("A Moment Apart"), Some(3)),
        ("paused.json", "Paused", Some("A Moment Apart"), Some(3)),
        ("loading.json", "Loading", Some("A Moment Apart"), Some(3)),
        ("loading-empty.json", "Loading", None, None),
        ("stopped.json", "Stopped", None, None),
    ];

    for (fixture_name, state_label, title, artwork_revision) in fixtures {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("playback fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("playback fixture should produce a presentation");
        let Presentation::NowPlaying(presentation) = presentation else {
            panic!("available playback should produce an available presentation");
        };

        assert_eq!(presentation.playback_state, state_label);
        assert_eq!(presentation.title.as_deref(), title);
        assert_eq!(presentation.artwork_revision, artwork_revision);
        if state_label == "Stopped" {
            assert_eq!(presentation.artist, None);
            assert_eq!(presentation.album, None);
            assert_eq!(presentation.progress, None);
            assert_eq!(presentation.artwork_path, None);
        }
    }
}

#[test]
fn clears_now_playing_from_a_stopped_presentation() {
    let mut snapshot =
        parse_snapshot(&support::fixture("loading.json")).expect("Loading fixture should be valid");
    snapshot.playback = Some(Playback::Stopped);

    let presentation = presentation_from_snapshot(&snapshot)
        .expect("Stopped playback should produce a presentation");
    let Presentation::NowPlaying(presentation) = presentation else {
        panic!("available playback should produce an available presentation");
    };

    assert_eq!(presentation.playback_state, "Stopped");
    assert_eq!(presentation.title, None);
    assert_eq!(presentation.artist, None);
    assert_eq!(presentation.album, None);
    assert_eq!(presentation.progress, None);
    assert_eq!(presentation.artwork_revision, None);
    assert_eq!(presentation.artwork_path, None);
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
        Some("1:27".to_owned())
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
        Some("1:29".to_owned())
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
    assert!(matches!(changed.presentation, Presentation::Unavailable(_)));
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
        Some("1:28".to_owned())
    );

    let advanced = state
        .frame_at(Duration::from_secs(8))
        .expect("Playing progress should advance locally");
    let Presentation::NowPlaying(now_playing) = advanced.presentation else {
        panic!("Playing should retain Now Playing content");
    };
    assert_eq!(
        now_playing.progress.map(|progress| progress.elapsed),
        Some("1:30".to_owned())
    );
}
