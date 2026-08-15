use std::error::Error;
use std::fmt;

use crate::contract::{Availability, Playback, PresentationSnapshot, Progress};

#[derive(Debug, PartialEq)]
pub struct Presentation {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub display_zone: String,
    pub playback_state: String,
    pub progress: Option<PresentationProgress>,
    pub artwork_path: Option<String>,
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
        return Err(PresentationError(
            "the Gallery split fixture requires available playback",
        ));
    }

    let playback = snapshot.playback.ok_or(PresentationError(
        "an available snapshot requires playback state",
    ))?;
    let display_zone = snapshot.display_zone.as_ref().ok_or(PresentationError(
        "an available snapshot requires a Display Zone",
    ))?;

    Ok(Presentation {
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
    })
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
