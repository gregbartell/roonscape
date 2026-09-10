use std::time::Duration;

use roonscape_renderer::{Presentation, PresentationError, PresentationFrame, PresentationState};

/// Retain the visible song's authoritative timeline while a replacement's
/// graphics are unavailable. Same-song live updates remain independent of art.
pub(crate) struct DisplayedState {
    state: PresentationState,
    timing: PresentationState,
    artwork: Option<(String, Option<u64>)>,
}

pub(crate) struct DisplayedFrame {
    pub(crate) frame: PresentationFrame,
    pub(crate) revision: u64,
    pub(crate) generation: u64,
}

impl DisplayedState {
    pub(crate) fn new(state: &PresentationState, presentation: &Presentation) -> Self {
        Self {
            state: state.clone(),
            timing: state.clone(),
            artwork: artwork(presentation),
        }
    }

    pub(crate) fn frame_at(
        &mut self,
        incoming: &PresentationState,
        now: Duration,
        prepare_assets: impl FnOnce(&Presentation) -> (bool, bool),
    ) -> Result<DisplayedFrame, PresentationError> {
        let mut frame = incoming.frame_at(now)?;
        let (assets_ready, timing_ready) = prepare_assets(&frame.presentation);
        let display_incoming = assets_ready
            || (incoming.now_playing_generation() == self.state.now_playing_generation()
                && matches!(&frame.presentation, Presentation::NowPlaying(_))
                && matches!(
                    self.state.frame_at(now)?.presentation,
                    Presentation::NowPlaying(_)
                ));
        if display_incoming {
            if incoming.revision() != self.state.revision()
                || incoming.now_playing_generation() != self.state.now_playing_generation()
            {
                self.state = incoming.clone();
            }
            if assets_ready || timing_ready {
                self.timing = incoming.clone();
            } else {
                self.timing.synchronize_playback(incoming, now);
            }
            if assets_ready {
                self.artwork = artwork(&frame.presentation);
            }
        } else {
            frame = self.state.frame_at(now)?;
        }
        if self.timing.revision() != self.state.revision()
            && let (Presentation::NowPlaying(value), Presentation::NowPlaying(timing)) = (
                &mut frame.presentation,
                self.timing.frame_at(now)?.presentation,
            )
        {
            value.lyrics = timing.lyrics;
            value.lyrics_known = timing.lyrics_known;
            value.progress = timing.progress;
            value.playback_position_seconds = timing.playback_position_seconds;
        }
        if !assets_ready && let Presentation::NowPlaying(presentation) = &mut frame.presentation {
            presentation.artwork_path = self.artwork.as_ref().map(|(path, _)| path.clone());
            presentation.artwork_revision =
                self.artwork.as_ref().and_then(|(_, revision)| *revision);
        }
        Ok(DisplayedFrame {
            frame,
            revision: self.timing.revision(),
            generation: self.state.now_playing_generation(),
        })
    }
}

fn artwork(presentation: &Presentation) -> Option<(String, Option<u64>)> {
    match presentation {
        Presentation::NowPlaying(presentation) => presentation
            .artwork_path
            .as_ref()
            .map(|path| (path.clone(), presentation.artwork_revision)),
        Presentation::FullField(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roonscape_renderer::{
        Playback, PresentationSnapshot, PresentationStatusSymbol, PresentationTime, parse_snapshot,
    };
    use std::time::UNIX_EPOCH;

    fn anchor(seconds: u64) -> PresentationTime {
        PresentationTime::new(
            Duration::from_secs(seconds),
            UNIX_EPOCH + Duration::from_secs(1_786_821_600 + seconds),
        )
    }

    fn snapshot() -> PresentationSnapshot {
        parse_snapshot(include_str!("../../shared/fixtures/playing.json")).unwrap()
    }

    #[test]
    fn waiting_for_another_song_preserves_live_updates_already_displayed() {
        let mut snapshot = snapshot();
        let mut incoming = PresentationState::new(snapshot.clone(), anchor(0)).unwrap();
        let initial = incoming.frame_at(Duration::ZERO).unwrap();
        let mut displayed = DisplayedState::new(&incoming, &initial.presentation);
        snapshot.revision += 1;
        snapshot.playback = Some(Playback::Paused);
        snapshot.artwork.as_mut().unwrap().path = "new-artwork.jpg".into();
        incoming.update(snapshot.clone(), anchor(1)).unwrap();
        let paused = displayed
            .frame_at(&incoming, Duration::from_secs(1), |_| (false, true))
            .unwrap();
        let Presentation::NowPlaying(paused) = paused.frame.presentation else {
            panic!("displayed song");
        };
        assert_eq!(paused.status.symbol, PresentationStatusSymbol::Paused);
        assert_eq!(
            paused.artwork_path.as_deref(),
            Some("src/shared/fixtures/artwork/playing.jpg")
        );

        snapshot.revision += 1;
        snapshot.playback = Some(Playback::Playing);
        snapshot.now_playing.as_mut().unwrap().title = Some("Incoming song".into());
        incoming.update(snapshot, anchor(2)).unwrap();
        let waiting = displayed
            .frame_at(&incoming, Duration::from_secs(3), |_| (false, true))
            .unwrap();
        let Presentation::NowPlaying(waiting) = waiting.frame.presentation else {
            panic!("outgoing song");
        };
        assert_eq!(waiting.status.symbol, PresentationStatusSymbol::Paused);
        assert_eq!(waiting.progress, paused.progress);
        assert_eq!(waiting.artwork_path, paused.artwork_path);
        assert_eq!(waiting.title, paused.title);
        let ready = displayed
            .frame_at(&incoming, Duration::from_secs(3), |_| (true, true))
            .unwrap();
        let Presentation::NowPlaying(ready) = ready.frame.presentation else {
            panic!("incoming song");
        };
        assert_eq!(ready.title.as_deref(), Some("Incoming song"));
    }

    #[test]
    fn reconnect_waits_on_the_displayed_disconnected_state() {
        let mut snapshot = snapshot();
        let mut incoming = PresentationState::new(snapshot.clone(), anchor(0)).unwrap();
        let initial = incoming.frame_at(Duration::ZERO).unwrap();
        let mut displayed = DisplayedState::new(&incoming, &initial.presentation);
        incoming.disconnect(Duration::from_secs(1));
        let disconnected = displayed
            .frame_at(&incoming, Duration::from_secs(1), |_| (true, true))
            .unwrap();
        assert_eq!(disconnected.revision, snapshot.revision);
        snapshot.revision += 1;
        incoming.update(snapshot, anchor(2)).unwrap();
        let waiting = displayed
            .frame_at(&incoming, Duration::from_secs(2), |_| (false, true))
            .unwrap();
        assert_eq!(waiting.frame.presentation, disconnected.frame.presentation);
        assert!(matches!(
            waiting.frame.presentation,
            Presentation::FullField(_)
        ));
    }

    #[test]
    fn unprepared_seek_retains_an_advancing_timeline_until_ready() {
        let mut snapshot = parse_snapshot(include_str!(
            "../../shared/fixtures/lyrics-reel-capacity.json"
        ))
        .unwrap();
        let mut incoming = PresentationState::new(snapshot.clone(), anchor(0)).unwrap();
        let initial = incoming.frame_at(Duration::ZERO).unwrap();
        let mut displayed = DisplayedState::new(&incoming, &initial.presentation);
        snapshot.revision += 1;
        snapshot
            .timing
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .seconds = 230.0;
        snapshot
            .timing
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .sampled_at = "2026-08-15T19:20:01Z".into();
        incoming.update(snapshot, anchor(1)).unwrap();
        let position = |frame: DisplayedFrame| {
            let Presentation::NowPlaying(value) = frame.frame.presentation else {
                panic!("Now Playing")
            };
            (
                value.playback_position_seconds.unwrap(),
                value.lyrics.unwrap().current_index,
            )
        };
        let before = position(
            displayed
                .frame_at(&incoming, Duration::from_millis(1010), |_| (false, false))
                .unwrap(),
        );
        let after = position(
            displayed
                .frame_at(&incoming, Duration::from_millis(1050), |_| (false, false))
                .unwrap(),
        );
        assert!((after.0 - before.0 - 0.04).abs() < 0.001);
        assert_eq!(after.1, before.1);
        assert!(after.0 < 200.0);
        let ready = position(
            displayed
                .frame_at(&incoming, Duration::from_millis(1060), |_| (true, true))
                .unwrap(),
        );
        assert!(ready.0 >= 230.0);
        assert_ne!(ready.1, before.1);
    }

    #[test]
    fn unprepared_seek_keeps_its_revision_and_playback_when_another_song_arrives() {
        let mut snapshot = parse_snapshot(include_str!(
            "../../shared/fixtures/lyrics-reel-capacity.json"
        ))
        .unwrap();
        let original_revision = snapshot.revision;
        let mut incoming = PresentationState::new(snapshot.clone(), anchor(0)).unwrap();
        let initial = incoming.frame_at(Duration::ZERO).unwrap();
        let mut displayed = DisplayedState::new(&incoming, &initial.presentation);
        snapshot.revision += 1;
        snapshot.playback = Some(Playback::Paused);
        snapshot
            .timing
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .seconds = 230.0;
        incoming.update(snapshot.clone(), anchor(1)).unwrap();
        let waiting = displayed
            .frame_at(&incoming, Duration::from_secs(1), |_| (false, false))
            .unwrap();
        assert_eq!(waiting.revision, original_revision);
        let Presentation::NowPlaying(waiting) = waiting.frame.presentation else {
            panic!("Now Playing")
        };
        assert_eq!(waiting.status.symbol, PresentationStatusSymbol::Paused);
        assert!(waiting.playback_position_seconds.unwrap() < 200.0);
        let paused = displayed
            .frame_at(&incoming, Duration::from_secs(2), |_| (false, false))
            .unwrap();
        let Presentation::NowPlaying(paused) = paused.frame.presentation else {
            panic!("Now Playing")
        };
        assert_eq!(
            paused.playback_position_seconds,
            waiting.playback_position_seconds
        );
        snapshot.revision += 1;
        snapshot.now_playing.as_mut().unwrap().title = Some("Next song".into());
        snapshot.lyrics = None;
        incoming.update(snapshot, anchor(3)).unwrap();
        let outgoing = displayed
            .frame_at(&incoming, Duration::from_secs(3), |_| (false, true))
            .unwrap();
        assert_eq!(outgoing.revision, original_revision);
        let Presentation::NowPlaying(outgoing) = outgoing.frame.presentation else {
            panic!("Now Playing")
        };
        assert_eq!(
            outgoing.playback_position_seconds,
            paused.playback_position_seconds
        );
        assert_eq!(outgoing.lyrics, paused.lyrics);
        assert_eq!(outgoing.title, paused.title);
    }

    #[test]
    fn outgoing_playback_advances_without_replaying_a_pending_destination() {
        let mut snapshot = snapshot();
        let mut incoming = PresentationState::new(snapshot.clone(), anchor(0)).unwrap();
        let initial = incoming.frame_at(Duration::ZERO).unwrap();
        let mut displayed = DisplayedState::new(&incoming, &initial.presentation);
        snapshot.revision += 1;
        snapshot.now_playing.as_mut().unwrap().title = Some("Unprepared song".into());
        incoming.update(snapshot, anchor(1)).unwrap();
        let waiting = displayed
            .frame_at(&incoming, Duration::from_secs(2), |_| (false, true))
            .unwrap();
        let Presentation::NowPlaying(waiting) = waiting.frame.presentation else {
            panic!("outgoing song");
        };
        let Presentation::NowPlaying(initial) = initial.presentation else {
            unreachable!()
        };
        assert_eq!(waiting.title, initial.title);
        assert!(waiting.progress.unwrap().fraction > initial.progress.unwrap().fraction);
    }
}
