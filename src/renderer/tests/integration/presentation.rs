use crate::support;

use std::time::{Duration, UNIX_EPOCH};

use roonscape_renderer::{
    InactivityConfiguration, InactivityTransform, LayoutOffset, LyricCue, NowPlaying, Playback,
    Presentation, PresentationIdentity, PresentationState, PresentationTime, PresentationUpdate,
    SynchronizedLyrics, Timing, TimingPosition, classify_presentation_update, parse_snapshot,
    presentation_from_snapshot,
};

const PLAYING_SAMPLED_AT: u64 = 1_786_821_600;

fn presentation_time(monotonic_seconds: u64, utc_seconds: u64) -> PresentationTime {
    PresentationTime::new(
        Duration::from_secs(monotonic_seconds),
        UNIX_EPOCH + Duration::from_secs(utc_seconds),
    )
}

fn snapshot_with_lyrics(
    fixture_name: &str,
    cues: &[(f64, &str)],
) -> roonscape_renderer::PresentationSnapshot {
    let mut snapshot =
        parse_snapshot(&support::fixture(fixture_name)).expect("fixture should be valid");
    snapshot.lyrics = Some(SynchronizedLyrics {
        cues: cues
            .iter()
            .map(|(at_seconds, text)| LyricCue {
                at_seconds: *at_seconds,
                text: (*text).to_owned(),
            })
            .collect(),
    });
    snapshot
}

fn snapshot_with_timing(
    fixture_name: &str,
    revision: u64,
    position_seconds: Option<f64>,
    duration_seconds: Option<f64>,
) -> roonscape_renderer::PresentationSnapshot {
    let mut snapshot =
        parse_snapshot(&support::fixture(fixture_name)).expect("fixture should be valid");
    snapshot.revision = revision;
    snapshot.timing = if position_seconds.is_none() && duration_seconds.is_none() {
        None
    } else {
        Some(Timing {
            position: position_seconds.map(|seconds| TimingPosition {
                seconds,
                sampled_at: "2026-08-15T19:20:00Z".to_owned(),
            }),
            duration_seconds,
        })
    };
    snapshot
}

fn now_playing(
    state: &PresentationState,
    monotonic_seconds: u64,
) -> roonscape_renderer::NowPlayingPresentation {
    let Presentation::NowPlaying(presentation) = state
        .presentation_at(Duration::from_secs(monotonic_seconds))
        .expect("state should remain presentable")
    else {
        panic!("expected Now Playing");
    };
    presentation
}

#[test]
fn selects_synchronized_lyrics_with_lookahead_and_a_bounded_final_hold() {
    let snapshot = snapshot_with_lyrics("playing.json", &[(172.0, "First"), (175.0, "Final")]);
    let state = PresentationState::new(snapshot, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("lyric snapshot should be presentable");

    let Presentation::NowPlaying(before) = state
        .presentation_at(Duration::ZERO)
        .expect("pre-lyric Now Playing should be presentable")
    else {
        panic!("Playing should use Now Playing");
    };
    assert_eq!(
        before.lyrics.as_ref().map(|lyrics| lyrics.current.as_str()),
        Some("")
    );

    let Presentation::NowPlaying(active) = state
        .presentation_at(Duration::from_millis(400))
        .expect("active lyrics should be presentable")
    else {
        panic!("Playing should use Now Playing");
    };
    assert_eq!(
        active.lyrics.as_ref().map(|lyrics| lyrics.current.as_str()),
        Some("First")
    );

    let Presentation::NowPlaying(after) = state
        .presentation_at(Duration::from_millis(7_100))
        .expect("post-lyric Now Playing should be presentable")
    else {
        panic!("Playing should use Now Playing");
    };
    assert_eq!(after.lyrics, None);
    assert_eq!(
        classify_presentation_update(
            &Presentation::NowPlaying(before),
            &Presentation::NowPlaying(active.clone()),
        ),
        PresentationUpdate::InPlace,
    );
    assert_eq!(
        classify_presentation_update(
            &Presentation::NowPlaying(active),
            &Presentation::NowPlaying(after),
        ),
        PresentationUpdate::InPlace,
    );
}

#[test]
fn preserves_intentional_blanks_and_freezes_the_reel_while_paused() {
    let cues = [
        (170.0, "Previous"),
        (171.0, ""),
        (173.0, "   "),
        (175.0, "Upcoming"),
    ];
    let mut snapshot = snapshot_with_lyrics("paused.json", &cues);
    snapshot
        .timing
        .as_mut()
        .expect("Paused fixture has progress")
        .position
        .as_mut()
        .expect("Paused fixture has position")
        .seconds = 171.0;
    let state = PresentationState::new(snapshot, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("lyric snapshot should be presentable");

    for now in [Duration::ZERO, Duration::from_secs(30)] {
        let Presentation::NowPlaying(presentation) = state
            .presentation_at(now)
            .expect("Paused lyrics should be presentable")
        else {
            panic!("Paused should use Now Playing");
        };
        let lyrics = presentation
            .lyrics
            .expect("Intentional Blank should keep the Synchronized Lyric Composition");
        assert_eq!(lyrics.current, "");
        assert_eq!(lyrics.previous.as_deref(), Some("Previous"));
        assert_eq!(lyrics.next.as_deref(), Some("Upcoming"));
    }

    let mut approaching_snapshot = snapshot_with_lyrics("paused.json", &cues);
    approaching_snapshot
        .timing
        .as_mut()
        .expect("Paused fixture has progress")
        .position
        .as_mut()
        .expect("Paused fixture has position")
        .seconds = 173.0;
    approaching_snapshot.revision += 1;
    let approaching = PresentationState::new(
        approaching_snapshot,
        presentation_time(0, PLAYING_SAMPLED_AT),
    )
    .expect("approaching Intentional Blank should be presentable");
    let Presentation::NowPlaying(presentation) = approaching
        .presentation_at(Duration::ZERO)
        .expect("approaching cue should be presentable")
    else {
        panic!("Paused should use Now Playing");
    };
    let lyrics = presentation
        .lyrics
        .expect("Intentional Blank should remain active");
    assert_eq!(lyrics.previous.as_deref(), Some("Previous"));
    assert_eq!(lyrics.current, "   ");
    assert_eq!(lyrics.next.as_deref(), Some("Upcoming"));
}

#[test]
fn classifies_cue_and_same_identity_lyric_composition_changes_in_place() {
    let initial = snapshot_with_lyrics("playing.json", &[(170.0, "First"), (180.0, "Second")]);
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial lyrics should be presentable");
    let mut advanced = snapshot_with_lyrics("playing.json", &[(170.0, "First"), (180.0, "Second")]);
    advanced.revision += 1;
    advanced
        .timing
        .as_mut()
        .expect("Playing fixture has progress")
        .position
        .as_mut()
        .expect("Playing fixture has position")
        .seconds = 180.0;
    assert_eq!(
        state
            .update(advanced, presentation_time(1, PLAYING_SAMPLED_AT))
            .expect("advanced lyrics should be accepted"),
        PresentationUpdate::InPlace
    );

    let mut before =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    before.revision = 20;
    let mut without_lyrics =
        PresentationState::new(before, presentation_time(0, PLAYING_SAMPLED_AT))
            .expect("ordinary Now Playing should be presentable");
    let mut entering = snapshot_with_lyrics("playing.json", &[(170.0, "First")]);
    entering.revision = 21;
    assert_eq!(
        without_lyrics
            .update(entering, presentation_time(1, PLAYING_SAMPLED_AT))
            .expect("lyric entry should be accepted"),
        PresentationUpdate::InPlace
    );
}

#[test]
fn ignores_leading_blanks_and_retains_internal_and_trailing_blanks() {
    let cues = [
        (5.0, ""),
        (7.0, "Opening"),
        (40.0, "Still current"),
        (42.0, " "),
    ];
    let mut snapshot = snapshot_with_lyrics("paused.json", &cues);
    for (seconds, expected) in [
        (4.3, None),
        (5.5, None),
        (20.0, Some("Opening")),
        (41.0, Some("Still current")),
        (42.0, Some(" ")),
        (45.0, Some(" ")),
        (45.1, None),
    ] {
        snapshot
            .timing
            .as_mut()
            .and_then(|timing| timing.position.as_mut())
            .expect("Paused fixture has position")
            .seconds = seconds;
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("every lyric interval position should be presentable");
        let Presentation::NowPlaying(presentation) = presentation else {
            panic!("lyric timeline should retain Now Playing");
        };
        assert_eq!(
            presentation
                .lyrics
                .as_deref()
                .map(|lyrics| lyrics.current.as_str()),
            expected,
            "unexpected lyric state at {seconds}s"
        );
    }
}

#[test]
fn prepares_before_first_cue_and_ignores_all_leading_blanks() {
    for cues in [
        vec![(10.0, "Opening"), (20.0, "Final")],
        vec![(0.0, ""), (4.0, " "), (10.0, "Opening"), (20.0, "Final")],
    ] {
        let mut snapshot = snapshot_with_lyrics("paused.json", &cues);
        for (seconds, expected) in [
            (8.8, None),
            (9.0, Some("")),
            (9.4, Some("Opening")),
            (23.1, None),
        ] {
            snapshot
                .timing
                .as_mut()
                .unwrap()
                .position
                .as_mut()
                .unwrap()
                .seconds = seconds;
            let Presentation::NowPlaying(presentation) =
                presentation_from_snapshot(&snapshot).unwrap()
            else {
                panic!("Now Playing");
            };
            assert_eq!(
                presentation
                    .lyrics
                    .as_ref()
                    .map(|lyrics| lyrics.current.as_str()),
                expected,
                "at {seconds}"
            );
        }
    }
    let snapshot = snapshot_with_lyrics("paused.json", &[(0.0, ""), (180.0, " ")]);
    let Presentation::NowPlaying(presentation) = presentation_from_snapshot(&snapshot).unwrap()
    else {
        panic!("Now Playing");
    };
    assert!(presentation.lyrics.is_none());
}

#[test]
fn short_blanks_preserve_advance_promotion_without_retiring_the_current_cue_early() {
    let mut snapshot = snapshot_with_lyrics(
        "paused.json",
        &[
            (0.0, "Before"),
            (10.0, ""),
            (10.3, "After"),
            (15.0, ""),
            (20.0, "Final"),
        ],
    );
    for (seconds, expected) in [
        (9.4, "Before"),
        (9.7, "After"),
        (10.0, "After"),
        (14.8, "After"),
        (15.0, ""),
        (19.4, "Final"),
    ] {
        snapshot
            .timing
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .seconds = seconds;
        let Presentation::NowPlaying(presentation) = presentation_from_snapshot(&snapshot).unwrap()
        else {
            panic!("Now Playing");
        };
        assert_eq!(
            presentation.lyrics.unwrap().current,
            expected,
            "at {seconds}"
        );
    }
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
    assert_eq!(
        identity,
        &PresentationIdentity::OutputAndZone {
            tracked_output: tracked_output.to_owned(),
            tracked_zone: tracked_zone.to_owned(),
        }
    );
}

#[test]
fn maps_the_playing_snapshot_to_now_playing_content() {
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
    assert_eq!(presentation.tracked_output, "Speaker System");
    assert_eq!(presentation.tracked_zone, "Living Room");
    assert_eq!(presentation.status.label, "PLAYING");
    assert_eq!(presentation.artwork_revision, Some(3));
    assert_eq!(
        presentation.artwork_path.as_deref(),
        Some("src/shared/fixtures/artwork/playing.jpg")
    );

    let progress = presentation
        .progress
        .expect("Playing fixture should include determinate progress");
    assert!((progress.fraction - (171.0 / 266.0)).abs() < f64::EPSILON);
    assert_eq!(progress.elapsed, "2:51");
    assert_eq!(progress.remaining, "−1:35");
}

#[test]
fn carries_artwork_presence_and_presentation_revision_into_now_playing() {
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
            "PAIRING REQUIRED",
            "Enable RoonScape",
            "In a Roon client, open Settings → Extensions and enable RoonScape.",
        ),
        (
            "disconnected.json",
            "DISCONNECTED",
            "Waiting for Roon",
            "Check Roon Server and the network.",
        ),
        (
            "output-unavailable.json",
            "OUTPUT UNAVAILABLE",
            "Check the selected output",
            "Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.",
        ),
    ];

    for (fixture_name, status_label, heading, explanation) in expected {
        let fixture = support::fixture(fixture_name);
        let snapshot = parse_snapshot(&fixture).expect("unavailable fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("unavailable snapshot should produce a presentation");
        let Presentation::FullField(presentation) = presentation else {
            panic!("unavailable snapshot should use the full-field presentation");
        };

        assert_eq!(
            (
                presentation.status.label,
                presentation.heading,
                presentation.explanation,
            ),
            (status_label, heading, Some(explanation)),
        );
        if fixture_name == "output-unavailable.json" {
            assert_eq!(
                presentation.identity,
                Some(PresentationIdentity::OutputOnly {
                    tracked_output: "Speaker System".to_owned(),
                })
            );
        } else {
            assert_eq!(presentation.identity, None);
        }
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

    assert_eq!(presentation.status.label, "IDLE");
    assert_eq!(presentation.heading, "Nothing is playing");
    assert_eq!(presentation.explanation, None);
    assert_full_field_identity(&presentation, "Speaker System", "Living Room");
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

    assert_eq!(presentation.status.label, "STARTING");
    assert_eq!(presentation.heading, "Preparing playback");
    assert_eq!(presentation.explanation, None);
    assert_full_field_identity(&presentation, "Speaker System", "Living Room");
}

#[test]
fn presents_active_playback_without_usable_content_as_a_truthful_full_field() {
    for (fixture_name, status) in [
        ("playing-empty.json", "PLAYING"),
        ("paused-empty.json", "PAUSED"),
    ] {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("content-unavailable playback fixture should be valid");

        let presentation = presentation_from_snapshot(&snapshot)
            .expect("content-unavailable playback should produce a presentation");
        let Presentation::FullField(presentation) = presentation else {
            panic!("{fixture_name} should use the full-field presentation");
        };

        assert_eq!(presentation.status.label, status, "{fixture_name}");
        assert_eq!(
            presentation.heading, "Now Playing details unavailable",
            "{fixture_name}",
        );
        assert_eq!(presentation.explanation, None, "{fixture_name}");
        assert_full_field_identity(&presentation, "Speaker System", "Living Room");
    }
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
fn static_fixture_state_freezes_source_progress_and_inactivity() {
    let snapshot =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let state = PresentationState::new_with_behavior(
        snapshot,
        presentation_time(10, PLAYING_SAMPLED_AT + 60),
        inactivity_configuration(),
        roonscape_renderer::PresentationBehavior::StaticFixture,
    )
    .expect("static Playing should anchor a presentation");

    let frame = state
        .frame_at(Duration::from_secs(600))
        .expect("static Playing should remain presentable");
    assert_eq!(frame.inactivity, InactivityTransform::default());
    let Presentation::NowPlaying(presentation) = frame.presentation else {
        panic!("Playing snapshot should produce Now Playing content");
    };
    assert_eq!(
        presentation.progress.map(|progress| progress.elapsed),
        Some("2:51".to_owned())
    );

    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    let paused_state = PresentationState::new_with_behavior(
        paused,
        presentation_time(10, PLAYING_SAMPLED_AT),
        inactivity_configuration(),
        roonscape_renderer::PresentationBehavior::StaticFixture,
    )
    .expect("static Paused should anchor a presentation");
    assert_eq!(
        paused_state
            .frame_at(Duration::from_secs(600))
            .expect("static Paused should remain presentable")
            .inactivity,
        InactivityTransform::default()
    );
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
    let seeked_position = seeked
        .timing
        .as_mut()
        .expect("Playing fixture should contain timing")
        .position
        .as_mut()
        .expect("Playing fixture should contain position");
    seeked_position.seconds = 30.0;
    seeked_position.sampled_at = "2026-08-15T19:20:05Z".to_owned();

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
            .timing
            .as_mut()
            .expect("Playing fixture should have progress")
            .position
            .as_mut()
            .expect("Playing fixture should have position")
            .seconds = source_position;
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
            "PLAYING",
            Some("Last Light on Phobos"),
            Some(3),
        ),
        (
            "paused.json",
            "PAUSED",
            Some("Last Light on Phobos"),
            Some(3),
        ),
        (
            "loading.json",
            "STARTING",
            Some("Last Light on Phobos"),
            Some(3),
        ),
    ];

    for (fixture_name, status_label, title, artwork_revision) in fixtures {
        let snapshot = parse_snapshot(&support::fixture(fixture_name))
            .expect("playback fixture should be valid");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("playback fixture should produce a presentation");
        let Presentation::NowPlaying(presentation) = presentation else {
            panic!("available playback should produce an available presentation");
        };

        assert_eq!(presentation.status.label, status_label);
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

    assert_eq!(presentation.status.label, "IDLE");
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
fn updates_authoritative_and_provisional_timing_in_place() {
    let initial =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let mut progress_sample =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    progress_sample.revision = 8;
    progress_sample
        .timing
        .as_mut()
        .expect("Playing fixture should include progress")
        .position
        .as_mut()
        .expect("Playing fixture should include position")
        .seconds = 83.0;

    let progress_update = state
        .update(
            progress_sample,
            presentation_time(1, PLAYING_SAMPLED_AT + 1),
        )
        .expect("progress sample should update the presentation");
    assert_eq!(progress_update, PresentationUpdate::InPlace);

    let mut indeterminate =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    indeterminate.revision = 9;
    indeterminate.timing = None;
    let progress_disappeared = state
        .update(indeterminate, presentation_time(2, PLAYING_SAMPLED_AT + 2))
        .expect("indeterminate timing should update the presentation");
    assert_eq!(progress_disappeared, PresentationUpdate::InPlace);

    let mut determinate_again =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    determinate_again.revision = 10;
    let progress_appeared = state
        .update(
            determinate_again,
            presentation_time(3, PLAYING_SAMPLED_AT + 3),
        )
        .expect("determinate timing should update the presentation");
    assert_eq!(progress_appeared, PresentationUpdate::InPlace);

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
fn playback_only_updates_the_current_composition_in_place() {
    let playing =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(playing, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");

    assert_eq!(
        state
            .update(paused, presentation_time(1, PLAYING_SAMPLED_AT + 1))
            .expect("Paused snapshot should update the presentation"),
        PresentationUpdate::InPlace,
    );
}

#[test]
fn simultaneous_playback_and_now_playing_changes_require_a_transition() {
    let playing =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(playing, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let mut paused_track_b =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");
    paused_track_b.revision = 30;
    paused_track_b
        .now_playing
        .as_mut()
        .expect("Paused fixture should contain Now Playing")
        .title = Some("A different track".to_owned());
    let artwork = paused_track_b
        .artwork
        .as_mut()
        .expect("Paused fixture should reference artwork");
    artwork.revision = 12;
    artwork.path = "src/shared/fixtures/artwork/revised.jpg".to_owned();

    assert_eq!(
        state
            .update(paused_track_b, presentation_time(1, PLAYING_SAMPLED_AT + 1),)
            .expect("new paused Now Playing should update the presentation"),
        PresentationUpdate::TransitionRequired,
    );
}

#[test]
fn every_composition_field_requests_one_coordinated_transition() {
    let playing_snapshot = || {
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid")
    };
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
    progress_presence.timing = None;
    let mut artwork = playing_snapshot();
    let artwork_reference = artwork
        .artwork
        .as_mut()
        .expect("Playing fixture should reference artwork");
    artwork_reference.revision = 12;
    artwork_reference.path = "src/shared/fixtures/artwork/revised.jpg".to_owned();

    for (index, (field, mut changed, expected_update)) in [
        ("identity", identity, PresentationUpdate::TransitionRequired),
        ("metadata", metadata, PresentationUpdate::TransitionRequired),
        (
            "timing presence",
            progress_presence,
            PresentationUpdate::InPlace,
        ),
        (
            "artwork and palette",
            artwork,
            PresentationUpdate::TransitionRequired,
        ),
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
            expected_update,
            "{field} should use its intended update path"
        );
    }
}

#[test]
fn unavailable_snapshots_request_a_coordinated_transition() {
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
            PresentationUpdate::TransitionRequired,
            "{fixture_name} should transition from Now Playing content"
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
fn fixture_playback_only_selection_updates_the_current_composition_in_place() {
    let playing =
        parse_snapshot(&support::fixture("playing.json")).expect("Playing fixture should be valid");
    let mut state = PresentationState::new(playing, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("Playing snapshot should anchor a presentation");
    let paused =
        parse_snapshot(&support::fixture("paused.json")).expect("Paused fixture should be valid");

    assert_eq!(
        state
            .update_for_fixture_selection(paused, presentation_time(1, PLAYING_SAMPLED_AT + 1))
            .expect("Paused Fixture Scenario should update the presentation"),
        PresentationUpdate::InPlace,
    );
}

#[test]
fn inactive_conditions_remain_fully_legible_until_the_grace_period_ends() {
    for fixture_name in [
        "paused.json",
        "paused-empty.json",
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
fn loading_and_playing_stay_active_with_now_playing_or_missing_content() {
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
fn repeatedly_repositions_inactive_content_within_now_playing_bounds() {
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
        PresentationUpdate::TransitionRequired
    );
    let Presentation::FullField(disconnected) = state
        .presentation_at(Duration::from_secs(11))
        .expect("disconnection should remain presentable")
    else {
        panic!("disconnection must clear stale Now Playing content");
    };
    assert_eq!(disconnected.status.label, "DISCONNECTED");

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
    assert_eq!(reconnected.status.label, "PAUSED");
}

#[test]
fn continues_from_each_retained_authoritative_timing_dimension_and_reconciles_in_place() {
    let initial = snapshot_with_timing("playing.json", 1, Some(10.0), Some(100.0));
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("complete Authoritative Timing should anchor the state");

    let missing_position = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    assert_eq!(
        state
            .update(
                missing_position,
                presentation_time(1, PLAYING_SAMPLED_AT + 1),
            )
            .expect("partial timing should be accepted"),
        PresentationUpdate::InPlace
    );
    assert_eq!(
        now_playing(&state, 3)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:13".to_owned())
    );

    let mut missing_duration = snapshot_with_timing("playing.json", 3, Some(20.0), None);
    missing_duration
        .timing
        .as_mut()
        .and_then(|timing| timing.position.as_mut())
        .expect("snapshot should carry position")
        .sampled_at = "2026-08-15T19:20:03Z".to_owned();
    state
        .update(
            missing_duration,
            presentation_time(3, PLAYING_SAMPLED_AT + 3),
        )
        .expect("the current position should replace only retained position");
    assert_eq!(
        now_playing(&state, 4)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:21".to_owned())
    );

    let mut reconciled = snapshot_with_timing("playing.json", 4, Some(5.0), Some(100.0));
    reconciled
        .timing
        .as_mut()
        .and_then(|timing| timing.position.as_mut())
        .expect("snapshot should carry position")
        .sampled_at = "2026-08-15T19:20:04Z".to_owned();
    assert_eq!(
        state
            .update(reconciled, presentation_time(4, PLAYING_SAMPLED_AT + 4),)
            .expect("Authoritative Timing should reconcile immediately"),
        PresentationUpdate::InPlace
    );
    assert_eq!(
        now_playing(&state, 4)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:05".to_owned())
    );
}

#[test]
fn zero_anchors_changed_now_playing_and_keeps_the_original_grace_when_duration_arrives_later() {
    for fixture_name in ["playing.json", "paused.json", "loading.json"] {
        let initial = snapshot_with_timing("playing.json", 1, Some(50.0), Some(100.0));
        let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
            .expect("initial timing should be complete");
        let mut changed = snapshot_with_timing(fixture_name, 2, None, None);
        changed
            .now_playing
            .as_mut()
            .expect("fixture should contain Now Playing")
            .title = Some("Changed track".to_owned());
        state
            .update(changed, presentation_time(1, PLAYING_SAMPLED_AT + 1))
            .expect("changed Now Playing should begin grace");
        assert_eq!(now_playing(&state, 2).progress, None);

        let mut duration = snapshot_with_timing(fixture_name, 3, None, Some(80.0));
        duration
            .now_playing
            .as_mut()
            .expect("fixture should contain Now Playing")
            .title = Some("Changed track".to_owned());
        state
            .update(duration, presentation_time(3, PLAYING_SAMPLED_AT + 3))
            .expect("duration should arrive within the original grace");
        let expected = if fixture_name == "playing.json" {
            "0:02"
        } else {
            "0:00"
        };
        assert_eq!(
            now_playing(&state, 3)
                .progress
                .map(|progress| progress.elapsed),
            Some(expected.to_owned())
        );
        assert_eq!(
            now_playing(&state, 6).progress,
            None,
            "{fixture_name} grace should expire five monotonic seconds after changed Now Playing"
        );
    }
}

#[test]
fn compatible_metadata_and_unrelated_snapshots_do_not_restart_grace() {
    let initial = snapshot_with_timing("playing.json", 1, Some(10.0), Some(100.0));
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("complete timing should be valid");
    let mut incomplete = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    incomplete
        .now_playing
        .as_mut()
        .expect("fixture should contain Now Playing")
        .artist = None;
    state
        .update(incomplete, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("compatible missing metadata should start timing grace");

    let mut enriched = snapshot_with_timing("playing.json", 3, None, Some(100.0));
    enriched
        .artwork
        .as_mut()
        .expect("fixture has artwork")
        .revision += 1;
    state
        .update(enriched, presentation_time(4, PLAYING_SAMPLED_AT + 4))
        .expect("metadata enrichment and artwork should not restart grace");
    assert_eq!(now_playing(&state, 6).progress, None);

    let later_incomplete = snapshot_with_timing("playing.json", 4, None, Some(100.0));
    state
        .update(
            later_incomplete,
            presentation_time(7, PLAYING_SAMPLED_AT + 7),
        )
        .expect("post-expiry timing should remain valid");
    assert_eq!(now_playing(&state, 7).progress, None);

    let complete = snapshot_with_timing("playing.json", 5, Some(30.0), Some(100.0));
    state
        .update(complete, presentation_time(8, PLAYING_SAMPLED_AT + 8))
        .expect("complete timing should re-arm grace");
    let missing_again = snapshot_with_timing("playing.json", 6, None, Some(100.0));
    state
        .update(missing_again, presentation_time(9, PLAYING_SAMPLED_AT + 9))
        .expect("a later loss should begin a new grace");
    assert!(now_playing(&state, 9).progress.is_some());
}

#[test]
fn retained_metadata_detects_a_conflict_after_an_absent_field_observation() {
    let initial = snapshot_with_timing("playing.json", 1, Some(40.0), Some(100.0));
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial state should be valid");
    let mut absent = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    absent
        .now_playing
        .as_mut()
        .expect("fixture has Now Playing")
        .title = None;
    state
        .update(absent, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("an absent field remains compatible");

    let mut conflict = snapshot_with_timing("playing.json", 3, None, Some(100.0));
    conflict
        .now_playing
        .as_mut()
        .expect("fixture has Now Playing")
        .title = Some("Conflicting title".to_owned());
    state
        .update(conflict, presentation_time(2, PLAYING_SAMPLED_AT + 2))
        .expect("a later conflict should begin changed Now Playing");
    assert_eq!(
        now_playing(&state, 2)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:00".to_owned())
    );
}

#[test]
fn excludes_zero_based_estimates_on_initial_subscription_reconnection_and_zone_change() {
    let initial_partial = snapshot_with_timing("playing.json", 1, None, Some(100.0));
    let state = PresentationState::new(
        initial_partial.clone(),
        presentation_time(0, PLAYING_SAMPLED_AT),
    )
    .expect("initial partial timing should remain presentable");
    assert_eq!(now_playing(&state, 0).progress, None);

    let mut reconnected = PresentationState::disconnected();
    reconnected
        .update(
            initial_partial,
            presentation_time(1, PLAYING_SAMPLED_AT + 1),
        )
        .expect("reconnection should accept partial timing");
    assert_eq!(now_playing(&reconnected, 1).progress, None);

    let initial = snapshot_with_timing("playing.json", 1, Some(40.0), Some(100.0));
    let mut moved = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial complete timing should be valid");
    let mut new_zone = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    let zone = new_zone
        .tracked_zone
        .as_mut()
        .expect("fixture should identify its Tracked Zone");
    zone.id = "zone-kitchen".to_owned();
    zone.name = "Kitchen".to_owned();
    moved
        .update(new_zone, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("Tracked Zone change should remain presentable");
    assert_eq!(now_playing(&moved, 1).progress, None);
}

#[test]
fn preserves_incoming_authoritative_timing_on_reconnection_and_zone_change() {
    let complete = snapshot_with_timing("playing.json", 1, Some(20.0), Some(100.0));
    let mut reconnected = PresentationState::disconnected();
    reconnected
        .update(complete.clone(), presentation_time(1, PLAYING_SAMPLED_AT))
        .expect("reconnection should retain incoming Authoritative Timing");
    assert!(now_playing(&reconnected, 1).progress.is_some());

    let mut moved = PresentationState::new(complete, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial complete timing should be valid");
    let mut new_zone = snapshot_with_timing("playing.json", 2, Some(30.0), Some(100.0));
    let zone = new_zone
        .tracked_zone
        .as_mut()
        .expect("fixture should identify its Tracked Zone");
    zone.id = "zone-kitchen".to_owned();
    zone.name = "Kitchen".to_owned();
    moved
        .update(new_zone, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("Tracked Zone change should retain incoming Authoritative Timing");
    assert_eq!(
        now_playing(&moved, 1)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:31".to_owned())
    );
}

#[test]
fn combines_complementary_partial_samples_from_initial_subscription() {
    let initial = snapshot_with_timing("playing.json", 1, Some(10.0), None);
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial position-only timing should be valid");
    let duration = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    state
        .update(duration, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("compatible duration should combine within the original grace");
    assert_eq!(
        now_playing(&state, 1)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:11".to_owned())
    );
}

#[test]
fn selects_lyrics_from_authoritative_position_without_duration_and_both_provisional_forms() {
    let cues = [(5.0, "Current"), (20.0, "Later")];
    let mut position_only = snapshot_with_timing("playing.json", 1, Some(5.0), None);
    position_only.lyrics = snapshot_with_lyrics("playing.json", &cues).lyrics;
    let state = PresentationState::new(position_only, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("position-only timing should be presentable");
    assert_eq!(
        now_playing(&state, 0).lyrics.map(|lyrics| lyrics.current),
        Some("Current".to_owned())
    );

    let mut initial = snapshot_with_timing("playing.json", 1, Some(5.0), Some(100.0));
    initial.lyrics = snapshot_with_lyrics("playing.json", &cues).lyrics;
    let mut compatible = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("complete lyric timing should be presentable");
    let mut missing_position = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    missing_position.lyrics = snapshot_with_lyrics("playing.json", &cues).lyrics;
    compatible
        .update(
            missing_position,
            presentation_time(1, PLAYING_SAMPLED_AT + 1),
        )
        .expect("compatible timing loss should retain lyric position");
    assert_eq!(
        now_playing(&compatible, 1)
            .lyrics
            .map(|lyrics| lyrics.current),
        Some("Current".to_owned())
    );

    let mut changed_without_duration = snapshot_with_timing("playing.json", 3, None, None);
    changed_without_duration
        .now_playing
        .as_mut()
        .expect("fixture has Now Playing")
        .title = Some("Changed lyric track".to_owned());
    changed_without_duration.lyrics =
        snapshot_with_lyrics("playing.json", &[(0.0, "Opening")]).lyrics;
    compatible
        .update(
            changed_without_duration,
            presentation_time(2, PLAYING_SAMPLED_AT + 2),
        )
        .expect("changed Now Playing should zero-anchor lyrics");
    assert_eq!(now_playing(&compatible, 2).lyrics, None);

    let mut changed_with_duration = snapshot_with_timing("playing.json", 4, None, Some(100.0));
    changed_with_duration
        .now_playing
        .as_mut()
        .expect("fixture has Now Playing")
        .title = Some("Changed lyric track".to_owned());
    changed_with_duration.lyrics = snapshot_with_lyrics("playing.json", &[(0.0, "Opening")]).lyrics;
    compatible
        .update(
            changed_with_duration,
            presentation_time(3, PLAYING_SAMPLED_AT + 3),
        )
        .expect("authoritative duration should activate the zero anchor");
    assert_eq!(
        now_playing(&compatible, 3)
            .lyrics
            .map(|lyrics| lyrics.current),
        Some("Opening".to_owned())
    );
}

#[test]
fn saturates_provisional_position_then_reconciles_or_expires() {
    let initial = snapshot_with_timing("playing.json", 1, Some(98.0), Some(100.0));
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial state should be valid");
    let missing = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    state
        .update(missing, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("missing position should begin grace");
    let saturated = now_playing(&state, 4)
        .progress
        .expect("Provisional Timing should remain determinate");
    assert_eq!(saturated.fraction, 1.0);
    assert_eq!(saturated.elapsed, "1:40");

    let mut reconciled = snapshot_with_timing("playing.json", 3, Some(50.0), Some(100.0));
    reconciled
        .timing
        .as_mut()
        .and_then(|timing| timing.position.as_mut())
        .expect("snapshot should carry position")
        .sampled_at = "2026-08-15T19:20:04Z".to_owned();
    state
        .update(reconciled, presentation_time(4, PLAYING_SAMPLED_AT + 4))
        .expect("Authoritative Timing should replace saturation");
    assert_eq!(
        now_playing(&state, 4)
            .progress
            .map(|progress| progress.elapsed),
        Some("0:50".to_owned())
    );

    let missing_again = snapshot_with_timing("playing.json", 4, None, Some(100.0));
    state
        .update(missing_again, presentation_time(5, PLAYING_SAMPLED_AT + 5))
        .expect("later loss should begin another grace");
    assert_eq!(now_playing(&state, 10).progress, None);
}

#[test]
fn discards_provisional_timing_on_idle_unavailability_and_absent_now_playing() {
    for discard_fixture in [
        "stopped.json",
        "disconnected.json",
        "output-unavailable.json",
        "playing-empty.json",
    ] {
        let initial = snapshot_with_timing("playing.json", 1, Some(20.0), Some(100.0));
        let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
            .expect("initial state should be valid");
        let missing = snapshot_with_timing("playing.json", 2, None, Some(100.0));
        state
            .update(missing, presentation_time(1, PLAYING_SAMPLED_AT + 1))
            .expect("timing loss should begin grace");
        assert!(now_playing(&state, 1).progress.is_some());

        let mut discarded = parse_snapshot(&support::fixture(discard_fixture))
            .expect("discard fixture should be valid");
        discarded.revision = 3;
        state
            .update(discarded, presentation_time(2, PLAYING_SAMPLED_AT + 2))
            .expect("discard state should be accepted");

        let resumed = snapshot_with_timing("playing.json", 4, None, Some(100.0));
        state
            .update(resumed, presentation_time(3, PLAYING_SAMPLED_AT + 3))
            .expect("resumed partial timing should be accepted");
        assert_eq!(
            now_playing(&state, 3).progress,
            None,
            "{discard_fixture} should discard and block retained timing"
        );
    }
}

#[test]
fn authoritative_reconciliation_stays_in_place_when_a_backward_correction_removes_lyrics() {
    let cues = [(5.0, "First")];
    let mut initial = snapshot_with_timing("playing.json", 1, Some(5.0), Some(100.0));
    initial.lyrics = snapshot_with_lyrics("playing.json", &cues).lyrics;
    let mut state = PresentationState::new(initial, presentation_time(0, PLAYING_SAMPLED_AT))
        .expect("initial lyrics should be presentable");
    let mut missing = snapshot_with_timing("playing.json", 2, None, Some(100.0));
    missing.lyrics = snapshot_with_lyrics("playing.json", &cues).lyrics;
    state
        .update(missing, presentation_time(1, PLAYING_SAMPLED_AT + 1))
        .expect("missing position should retain the lyric cue");
    assert!(now_playing(&state, 1).lyrics.is_some());

    let mut reconciled = snapshot_with_timing("playing.json", 3, Some(0.0), None);
    reconciled.lyrics = snapshot_with_lyrics("playing.json", &cues).lyrics;
    reconciled
        .timing
        .as_mut()
        .and_then(|timing| timing.position.as_mut())
        .expect("snapshot should carry position")
        .sampled_at = "2026-08-15T19:20:02Z".to_owned();
    assert_eq!(
        state
            .update(reconciled, presentation_time(2, PLAYING_SAMPLED_AT + 2),)
            .expect("backward reconciliation should be accepted"),
        PresentationUpdate::InPlace
    );
    assert_eq!(now_playing(&state, 2).lyrics, None);
}
