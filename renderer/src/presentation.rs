use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::contract::{Availability, Playback, PresentationSnapshot, Progress};

#[derive(Debug, PartialEq)]
pub enum Presentation {
    NowPlaying(NowPlayingPresentation),
    Unavailable(UnavailablePresentation),
}

#[derive(Debug, PartialEq)]
pub struct NowPlayingPresentation {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub display_zone: String,
    pub playback_state: String,
    pub progress: Option<PresentationProgress>,
    pub artwork_revision: Option<u64>,
    pub artwork_path: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnavailablePresentation {
    pub state_label: &'static str,
    pub heading: &'static str,
    pub explanation: &'static str,
}

#[derive(Debug, PartialEq)]
pub struct PresentationProgress {
    pub fraction: f64,
    pub elapsed: String,
    pub remaining: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PresentationError(&'static str);

pub struct PresentationState {
    snapshot: PresentationSnapshot,
    progress_anchored_at: Duration,
    source_sample_age: Duration,
}

#[derive(Clone, Copy, Debug)]
pub struct PresentationTime {
    monotonic: Duration,
    utc: SystemTime,
}

impl fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for PresentationError {}

impl PresentationTime {
    pub fn new(monotonic: Duration, utc: SystemTime) -> Self {
        Self { monotonic, utc }
    }
}

impl PresentationState {
    pub fn new(
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
    ) -> Result<Self, PresentationError> {
        presentation_from_snapshot(&snapshot)?;
        let source_sample_age = source_sample_age(&snapshot, anchored_at.utc)?;
        Ok(Self {
            snapshot,
            progress_anchored_at: anchored_at.monotonic,
            source_sample_age,
        })
    }

    pub fn update(
        &mut self,
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
    ) -> Result<(), PresentationError> {
        presentation_from_snapshot(&snapshot)?;
        let has_new_source_sample = self.snapshot.playback != snapshot.playback
            || self.snapshot.progress != snapshot.progress;
        if has_new_source_sample {
            self.source_sample_age = source_sample_age(&snapshot, anchored_at.utc)?;
            self.progress_anchored_at = anchored_at.monotonic;
        }
        self.snapshot = snapshot;
        Ok(())
    }

    pub fn presentation_at(&self, now: Duration) -> Result<Presentation, PresentationError> {
        let elapsed = self
            .source_sample_age
            .saturating_add(now.saturating_sub(self.progress_anchored_at));
        presentation_from_snapshot_after(&self.snapshot, elapsed)
    }
}

fn source_sample_age(
    snapshot: &PresentationSnapshot,
    received_at: SystemTime,
) -> Result<Duration, PresentationError> {
    if snapshot.playback != Some(Playback::Playing) {
        return Ok(Duration::ZERO);
    }
    let Some(progress) = snapshot.progress.as_ref() else {
        return Ok(Duration::ZERO);
    };

    let sampled_at = OffsetDateTime::parse(&progress.sampled_at, &Rfc3339)
        .map_err(|_| PresentationError("progress sampledAt must be an RFC 3339 timestamp"))?;
    let sample_age = OffsetDateTime::from(received_at) - sampled_at;
    if sample_age.is_negative() {
        return Ok(Duration::ZERO);
    }

    Duration::try_from(sample_age)
        .map_err(|_| PresentationError("progress sampledAt is outside the supported range"))
}

pub fn presentation_from_snapshot(
    snapshot: &PresentationSnapshot,
) -> Result<Presentation, PresentationError> {
    presentation_from_snapshot_after(snapshot, Duration::ZERO)
}

fn presentation_from_snapshot_after(
    snapshot: &PresentationSnapshot,
    elapsed: Duration,
) -> Result<Presentation, PresentationError> {
    if snapshot.availability != Availability::Available {
        return Ok(Presentation::Unavailable(unavailable_presentation(
            snapshot.availability,
        )));
    }

    let playback = snapshot.playback.ok_or(PresentationError(
        "an available snapshot requires playback state",
    ))?;
    let display_zone = snapshot.display_zone.as_ref().ok_or(PresentationError(
        "an available snapshot requires a Display Zone",
    ))?;
    let retains_now_playing = playback != Playback::Stopped;
    let now_playing = if retains_now_playing {
        snapshot.now_playing.as_ref()
    } else {
        None
    };

    Ok(Presentation::NowPlaying(NowPlayingPresentation {
        title: now_playing.and_then(|now_playing| now_playing.title.clone()),
        artist: now_playing.and_then(|now_playing| now_playing.artist.clone()),
        album: now_playing.and_then(|now_playing| now_playing.album.clone()),
        display_zone: display_zone.name.clone(),
        playback_state: playback_label(playback).to_owned(),
        progress: if retains_now_playing {
            snapshot
                .progress
                .as_ref()
                .map(|progress| presentation_progress(progress, playback, elapsed))
        } else {
            None
        },
        artwork_revision: if retains_now_playing {
            snapshot.artwork.as_ref().map(|artwork| artwork.revision)
        } else {
            None
        },
        artwork_path: if retains_now_playing {
            snapshot
                .artwork
                .as_ref()
                .map(|artwork| artwork.path.clone())
        } else {
            None
        },
    }))
}

fn unavailable_presentation(availability: Availability) -> UnavailablePresentation {
    match availability {
        Availability::PairingRequired => UnavailablePresentation {
            state_label: "Pairing required",
            heading: "Enable RoonScape",
            explanation: "Open Settings → Extensions in a Roon client, then enable RoonScape.",
        },
        Availability::Disconnected => UnavailablePresentation {
            state_label: "Disconnected",
            heading: "Waiting for Roon",
            explanation: "Check Roon Server and the network. This display updates when Roon returns.",
        },
        Availability::OutputUnavailable => UnavailablePresentation {
            state_label: "Output unavailable",
            heading: "Display Output unavailable",
            explanation: "Configure a Display Output on this RoonScape Host, or check that the selected output is available in Roon.",
        },
        Availability::Available => unreachable!("available snapshots use Now Playing"),
    }
}

fn playback_label(playback: Playback) -> &'static str {
    match playback {
        Playback::Playing => "Playing",
        Playback::Paused => "Paused",
        Playback::Loading => "Loading",
        Playback::Stopped => "Stopped",
    }
}

fn presentation_progress(
    progress: &Progress,
    playback: Playback,
    elapsed: Duration,
) -> PresentationProgress {
    let advancement = if playback == Playback::Playing {
        elapsed.as_secs_f64()
    } else {
        0.0
    };
    let position = (progress.position_seconds + advancement).clamp(0.0, progress.duration_seconds);
    let remaining = progress.duration_seconds - position;

    PresentationProgress {
        fraction: position / progress.duration_seconds,
        elapsed: format_duration(position),
        remaining: format!("−{}", format_duration(remaining)),
    }
}

fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}
