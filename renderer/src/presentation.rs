use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::contract::{
    Availability, Playback, PresentationSnapshot, Progress, TrackedOutput, TrackedZone,
};
use crate::display_configuration::InactivityConfiguration;

pub const INACTIVE_HORIZONTAL_BOUND: i32 = 18;
pub const INACTIVE_VERTICAL_BOUND: i32 = 12;

const INACTIVE_POSITIONS: [LayoutOffset; 8] = [
    LayoutOffset {
        x: -INACTIVE_HORIZONTAL_BOUND,
        y: -INACTIVE_VERTICAL_BOUND,
    },
    LayoutOffset { x: 12, y: 8 },
    LayoutOffset {
        x: -8,
        y: INACTIVE_VERTICAL_BOUND,
    },
    LayoutOffset {
        x: INACTIVE_HORIZONTAL_BOUND,
        y: -6,
    },
    LayoutOffset {
        x: 6,
        y: -INACTIVE_VERTICAL_BOUND,
    },
    LayoutOffset {
        x: -INACTIVE_HORIZONTAL_BOUND,
        y: 4,
    },
    LayoutOffset {
        x: 16,
        y: INACTIVE_VERTICAL_BOUND,
    },
    LayoutOffset { x: -4, y: -10 },
];

#[derive(Clone, Debug, PartialEq)]
pub enum Presentation {
    NowPlaying(NowPlayingPresentation),
    FullField(FullFieldPresentation),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowPlayingPresentation {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub tracked_output: String,
    pub tracked_zone: String,
    pub playback: Playback,
    pub progress: Option<PresentationProgress>,
    pub artwork_revision: Option<u64>,
    pub artwork_path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusEmphasis {
    Prominent,
    Quiet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationIdentity {
    pub tracked_output: String,
    pub tracked_zone: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullFieldPresentation {
    pub state_label: &'static str,
    pub heading: &'static str,
    pub explanation: Option<&'static str>,
    pub identity: Option<PresentationIdentity>,
    pub status_emphasis: StatusEmphasis,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PresentationProgress {
    pub fraction: f64,
    pub elapsed: String,
    pub remaining: String,
}

#[derive(Debug, PartialEq)]
pub struct PresentationFrame {
    pub presentation: Presentation,
    pub inactivity: InactivityTransform,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InactivityTransform {
    pub opacity: f64,
    pub offset: LayoutOffset,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PresentationError(&'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationUpdate {
    ProgressOnly,
    ReplaceImmediately,
    TransitionRequired,
}

pub struct PresentationState {
    snapshot: PresentationSnapshot,
    progress_anchored_at: Duration,
    source_sample_age: Duration,
    inactivity_configuration: InactivityConfiguration,
    inactivity_condition: Option<InactivityCondition>,
    inactivity_anchored_at: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InactivityCondition {
    Paused,
    Stopped,
    Unavailable(Availability),
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

impl Default for InactivityTransform {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            offset: LayoutOffset::default(),
        }
    }
}

impl PresentationTime {
    pub fn new(monotonic: Duration, utc: SystemTime) -> Self {
        Self { monotonic, utc }
    }
}

impl PresentationState {
    pub fn disconnected() -> Self {
        Self::disconnected_with_inactivity(Duration::ZERO, InactivityConfiguration::default())
    }

    pub fn disconnected_with_inactivity(
        anchored_at: Duration,
        inactivity_configuration: InactivityConfiguration,
    ) -> Self {
        Self {
            snapshot: disconnected_snapshot(0),
            progress_anchored_at: anchored_at,
            source_sample_age: Duration::ZERO,
            inactivity_configuration,
            inactivity_condition: Some(InactivityCondition::Unavailable(
                Availability::Disconnected,
            )),
            inactivity_anchored_at: anchored_at,
        }
    }

    pub fn new(
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
    ) -> Result<Self, PresentationError> {
        Self::new_with_inactivity(snapshot, anchored_at, InactivityConfiguration::default())
    }

    pub fn new_with_inactivity(
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
        inactivity_configuration: InactivityConfiguration,
    ) -> Result<Self, PresentationError> {
        presentation_from_snapshot(&snapshot)?;
        let source_sample_age = source_sample_age(&snapshot, anchored_at.utc)?;
        let inactivity_condition = inactivity_condition(&snapshot);
        Ok(Self {
            snapshot,
            progress_anchored_at: anchored_at.monotonic,
            source_sample_age,
            inactivity_configuration,
            inactivity_condition,
            inactivity_anchored_at: anchored_at.monotonic,
        })
    }

    pub fn update(
        &mut self,
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
    ) -> Result<PresentationUpdate, PresentationError> {
        presentation_from_snapshot(&snapshot)?;
        let update = if transition_content_changed(&self.snapshot, &snapshot) {
            PresentationUpdate::TransitionRequired
        } else {
            PresentationUpdate::ProgressOnly
        };
        let next_inactivity_condition = inactivity_condition(&snapshot);
        let has_new_source_sample = self.snapshot.playback != snapshot.playback
            || self.snapshot.progress != snapshot.progress;
        if has_new_source_sample {
            self.source_sample_age = source_sample_age(&snapshot, anchored_at.utc)?;
            self.progress_anchored_at = anchored_at.monotonic;
        }
        if self.inactivity_condition != next_inactivity_condition {
            self.inactivity_condition = next_inactivity_condition;
            self.inactivity_anchored_at = anchored_at.monotonic;
        }
        self.snapshot = snapshot;
        Ok(update)
    }

    pub fn disconnect(&mut self, anchored_at: Duration) -> PresentationUpdate {
        let snapshot = disconnected_snapshot(self.snapshot.revision);
        let content_changed = transition_content_changed(&self.snapshot, &snapshot);
        let next_inactivity_condition = inactivity_condition(&snapshot);
        if self.inactivity_condition != next_inactivity_condition {
            self.inactivity_condition = next_inactivity_condition;
            self.inactivity_anchored_at = anchored_at;
        }
        self.snapshot = snapshot;
        self.progress_anchored_at = anchored_at;
        self.source_sample_age = Duration::ZERO;
        if content_changed {
            PresentationUpdate::ReplaceImmediately
        } else {
            PresentationUpdate::ProgressOnly
        }
    }

    pub fn presentation_at(&self, now: Duration) -> Result<Presentation, PresentationError> {
        let elapsed = self
            .source_sample_age
            .saturating_add(now.saturating_sub(self.progress_anchored_at));
        presentation_from_snapshot_after(&self.snapshot, elapsed)
    }

    pub fn revision(&self) -> u64 {
        self.snapshot.revision
    }

    pub fn frame_at(&self, now: Duration) -> Result<PresentationFrame, PresentationError> {
        Ok(PresentationFrame {
            presentation: self.presentation_at(now)?,
            inactivity: self.inactivity_transform_at(now),
        })
    }

    fn inactivity_transform_at(&self, now: Duration) -> InactivityTransform {
        let Some(_) = self.inactivity_condition else {
            return InactivityTransform::default();
        };
        let inactive_for = now.saturating_sub(self.inactivity_anchored_at);
        if inactive_for < self.inactivity_configuration.grace_period() {
            return InactivityTransform::default();
        }

        let repositioning_for =
            inactive_for.saturating_sub(self.inactivity_configuration.grace_period());
        let position_index = (repositioning_for.as_nanos()
            / self
                .inactivity_configuration
                .reposition_cadence()
                .as_nanos())
            % (INACTIVE_POSITIONS.len() as u128);

        InactivityTransform {
            opacity: self.inactivity_configuration.dimmed_opacity(),
            offset: INACTIVE_POSITIONS[position_index as usize],
        }
    }
}

fn transition_content_changed(
    previous: &PresentationSnapshot,
    next: &PresentationSnapshot,
) -> bool {
    previous.availability != next.availability
        || previous.playback != next.playback
        || previous.tracked_output != next.tracked_output
        || previous.tracked_zone != next.tracked_zone
        || previous.now_playing != next.now_playing
        || previous.progress.is_some() != next.progress.is_some()
        || previous.artwork != next.artwork
}

fn inactivity_condition(snapshot: &PresentationSnapshot) -> Option<InactivityCondition> {
    if snapshot.availability != Availability::Available {
        return Some(InactivityCondition::Unavailable(snapshot.availability));
    }

    match snapshot.playback {
        Some(Playback::Paused) => Some(InactivityCondition::Paused),
        Some(Playback::Stopped) => Some(InactivityCondition::Stopped),
        Some(Playback::Playing | Playback::Loading) | None => None,
    }
}

fn disconnected_snapshot(revision: u64) -> PresentationSnapshot {
    PresentationSnapshot {
        schema_version: 2,
        revision,
        availability: Availability::Disconnected,
        playback: None,
        tracked_output: None,
        tracked_zone: None,
        now_playing: None,
        progress: None,
        artwork: None,
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
        return Ok(Presentation::FullField(unavailable_presentation(
            snapshot.availability,
        )));
    }

    let playback = snapshot.playback.ok_or(PresentationError(
        "an available snapshot requires playback state",
    ))?;
    let tracked_output = snapshot.tracked_output.as_ref().ok_or(PresentationError(
        "an available snapshot requires a Tracked Output",
    ))?;
    let tracked_zone = snapshot.tracked_zone.as_ref().ok_or(PresentationError(
        "an available snapshot requires a Tracked Zone",
    ))?;
    if playback == Playback::Stopped {
        return Ok(Presentation::FullField(available_full_field(
            "Idle",
            "Nothing is playing",
            StatusEmphasis::Quiet,
            tracked_output,
            tracked_zone,
        )));
    }
    let now_playing = snapshot.now_playing.as_ref();
    let now_playing = NowPlayingPresentation {
        title: usable_metadata_line(
            now_playing.and_then(|now_playing| now_playing.title.as_deref()),
        ),
        artist: usable_metadata_line(
            now_playing.and_then(|now_playing| now_playing.artist.as_deref()),
        ),
        album: usable_metadata_line(
            now_playing.and_then(|now_playing| now_playing.album.as_deref()),
        ),
        tracked_output: tracked_output.name.clone(),
        tracked_zone: tracked_zone.name.clone(),
        playback,
        progress: snapshot
            .progress
            .as_ref()
            .map(|progress| presentation_progress(progress, playback, elapsed)),
        artwork_revision: snapshot.artwork.as_ref().map(|artwork| artwork.revision),
        artwork_path: snapshot
            .artwork
            .as_ref()
            .map(|artwork| artwork.path.clone()),
    };
    if !now_playing.has_usable_metadata() && now_playing.artwork_path.is_none() {
        return Ok(Presentation::FullField(trackless_full_field(&now_playing)));
    }

    Ok(Presentation::NowPlaying(now_playing))
}

fn usable_metadata_line(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn available_full_field(
    state_label: &'static str,
    heading: &'static str,
    status_emphasis: StatusEmphasis,
    tracked_output: &TrackedOutput,
    tracked_zone: &TrackedZone,
) -> FullFieldPresentation {
    FullFieldPresentation {
        state_label,
        heading,
        explanation: None,
        identity: Some(PresentationIdentity {
            tracked_output: tracked_output.name.clone(),
            tracked_zone: tracked_zone.name.clone(),
        }),
        status_emphasis,
    }
}

impl NowPlayingPresentation {
    pub fn playback_state(&self) -> &'static str {
        playback_label(self.playback)
    }

    pub(crate) fn has_usable_metadata(&self) -> bool {
        [
            self.title.as_deref(),
            self.artist.as_deref(),
            self.album.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| !value.trim().is_empty())
    }
}

pub(crate) fn trackless_full_field(presentation: &NowPlayingPresentation) -> FullFieldPresentation {
    let (state_label, heading, status_emphasis) = match presentation.playback {
        Playback::Loading => ("Loading", "Loading", StatusEmphasis::Quiet),
        Playback::Paused => (
            "Paused",
            "Now Playing details unavailable",
            StatusEmphasis::Quiet,
        ),
        Playback::Playing => (
            "Playing",
            "Now Playing details unavailable",
            StatusEmphasis::Prominent,
        ),
        Playback::Stopped => unreachable!("Idle uses the full-field presentation"),
    };
    FullFieldPresentation {
        state_label,
        heading,
        explanation: None,
        identity: Some(PresentationIdentity {
            tracked_output: presentation.tracked_output.clone(),
            tracked_zone: presentation.tracked_zone.clone(),
        }),
        status_emphasis,
    }
}

fn unavailable_presentation(availability: Availability) -> FullFieldPresentation {
    match availability {
        Availability::PairingRequired => FullFieldPresentation {
            state_label: "Pairing required",
            heading: "Enable RoonScape",
            explanation: Some(
                "Open Settings → Extensions in a Roon client, then enable RoonScape.",
            ),
            identity: None,
            status_emphasis: StatusEmphasis::Prominent,
        },
        Availability::Disconnected => FullFieldPresentation {
            state_label: "Disconnected",
            heading: "Waiting for Roon",
            explanation: Some(
                "Check Roon Server and the network. This display updates when Roon returns.",
            ),
            identity: None,
            status_emphasis: StatusEmphasis::Prominent,
        },
        Availability::OutputUnavailable => FullFieldPresentation {
            state_label: "Output unavailable",
            heading: "Tracked Output unavailable",
            explanation: Some(
                "Configure a Tracked Output on this RoonScape Host, or check that the selected output is available in Roon.",
            ),
            identity: None,
            status_emphasis: StatusEmphasis::Prominent,
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
