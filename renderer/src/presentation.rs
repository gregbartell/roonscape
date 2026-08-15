use std::error::Error;
use std::fmt;

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

impl fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for PresentationError {}

pub fn presentation_from_snapshot(
    snapshot: &PresentationSnapshot,
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

    Ok(Presentation::NowPlaying(NowPlayingPresentation {
        title: snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.title.clone()),
        artist: snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.artist.clone()),
        album: snapshot
            .now_playing
            .as_ref()
            .and_then(|now_playing| now_playing.album.clone()),
        display_zone: display_zone.name.clone(),
        playback_state: playback_label(playback).to_owned(),
        progress: snapshot.progress.as_ref().map(presentation_progress),
        artwork_path: snapshot
            .artwork
            .as_ref()
            .map(|artwork| artwork.path.clone()),
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

fn presentation_progress(progress: &Progress) -> PresentationProgress {
    let position = progress
        .position_seconds
        .clamp(0.0, progress.duration_seconds);
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
