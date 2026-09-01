use std::error::Error;
use std::f64::consts::PI;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationStatus {
    pub label: &'static str,
    pub symbol: PresentationStatusSymbol,
    pub motion: PresentationStatusMotion,
    pub emphasis: PresentationStatusEmphasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationStatusSymbol {
    Playing,
    Paused,
    Starting,
    Idle,
    PairingRequired,
    Disconnected,
    OutputUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationStatusMotion {
    Static,
    ContinuousRotation { period: Duration },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationStatusEmphasis {
    FullAccent,
    MutedAccent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowPlayingPresentation {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub tracked_output: String,
    pub tracked_zone: String,
    pub status: PresentationStatus,
    pub progress: Option<PresentationProgress>,
    pub activity: Option<Box<PresentationActivity>>,
    pub artwork_revision: Option<u64>,
    pub artwork_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationIdentity {
    OutputAndZone {
        tracked_output: String,
        tracked_zone: String,
    },
    OutputOnly {
        tracked_output: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullFieldPresentation {
    pub status: PresentationStatus,
    pub heading: &'static str,
    pub explanation: Option<&'static str>,
    pub identity: Option<PresentationIdentity>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PresentationProgress {
    pub fraction: f64,
    pub elapsed: String,
    pub remaining: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationActivity {
    pub waveform: PresentationActivityWaveform,
    pub heading: &'static str,
    pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationActivityWaveform {
    pub reference_heights_percent: [u8; 7],
    pub minimum_scale_percent: u8,
    pub phase_offsets: [Duration; 7],
    pub motion: PresentationActivityMotion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationActivityMotion {
    AlternatingEaseInOut { period: Duration },
}

impl PresentationActivityWaveform {
    pub fn bar_scales_at(self, elapsed: Duration, animations_enabled: bool) -> [f64; 7] {
        let PresentationActivityMotion::AlternatingEaseInOut { period } = self.motion;
        if !animations_enabled || period.is_zero() {
            return [1.0; 7];
        }

        let period_seconds = period.as_secs_f64();
        let minimum_scale = f64::from(self.minimum_scale_percent) / 100.0;
        self.phase_offsets.map(|offset| {
            let phase = elapsed
                .saturating_add(offset)
                .as_secs_f64()
                .rem_euclid(period_seconds)
                / period_seconds;
            let alternating_progress = 1.0 - (phase * 2.0 - 1.0).abs();
            let eased = (1.0 - (alternating_progress * PI).cos()) / 2.0;
            1.0 - eased * (1.0 - minimum_scale)
        })
    }
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
    InPlace,
    TransitionRequired,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresentationBehavior {
    #[default]
    Dynamic,
    StaticFixture,
}

impl PresentationBehavior {
    pub fn animations_enabled(self, system_animations_enabled: bool) -> bool {
        self == Self::Dynamic && system_animations_enabled
    }
}

impl PresentationStatusMotion {
    pub fn rotation_at(self, elapsed: Duration, animations_enabled: bool) -> f64 {
        let Self::ContinuousRotation { period } = self else {
            return 0.0;
        };
        if !animations_enabled || period.is_zero() {
            return 0.0;
        }
        elapsed.as_secs_f64().rem_euclid(period.as_secs_f64()) / period.as_secs_f64() * (2.0 * PI)
    }
}

pub struct PresentationState {
    snapshot: PresentationSnapshot,
    progress_anchored_at: Duration,
    source_sample_age: Duration,
    inactivity_configuration: InactivityConfiguration,
    inactivity_condition: Option<InactivityCondition>,
    inactivity_anchored_at: Duration,
    behavior: PresentationBehavior,
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
        Self::disconnected_with_behavior(
            anchored_at,
            inactivity_configuration,
            PresentationBehavior::Dynamic,
        )
    }

    pub fn disconnected_with_behavior(
        anchored_at: Duration,
        inactivity_configuration: InactivityConfiguration,
        behavior: PresentationBehavior,
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
            behavior,
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
        Self::new_with_behavior(
            snapshot,
            anchored_at,
            inactivity_configuration,
            PresentationBehavior::Dynamic,
        )
    }

    pub fn new_with_behavior(
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
        inactivity_configuration: InactivityConfiguration,
        behavior: PresentationBehavior,
    ) -> Result<Self, PresentationError> {
        presentation_from_snapshot(&snapshot)?;
        let source_sample_age = match behavior {
            PresentationBehavior::Dynamic => source_sample_age(&snapshot, anchored_at.utc)?,
            PresentationBehavior::StaticFixture => Duration::ZERO,
        };
        let inactivity_condition = inactivity_condition(&snapshot);
        Ok(Self {
            snapshot,
            progress_anchored_at: anchored_at.monotonic,
            source_sample_age,
            inactivity_configuration,
            inactivity_condition,
            inactivity_anchored_at: anchored_at.monotonic,
            behavior,
        })
    }

    pub fn update(
        &mut self,
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
    ) -> Result<PresentationUpdate, PresentationError> {
        self.update_snapshot(snapshot, anchored_at, false)
    }

    pub fn update_for_fixture_selection(
        &mut self,
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
    ) -> Result<PresentationUpdate, PresentationError> {
        self.update_snapshot(snapshot, anchored_at, true)
    }

    fn update_snapshot(
        &mut self,
        snapshot: PresentationSnapshot,
        anchored_at: PresentationTime,
        restart_inactivity: bool,
    ) -> Result<PresentationUpdate, PresentationError> {
        let previous_presentation = presentation_from_snapshot(&self.snapshot)
            .expect("PresentationState retains a validated snapshot");
        let next_presentation = presentation_from_snapshot(&snapshot)?;
        let update =
            if !presentation_composition_changed(&previous_presentation, &next_presentation) {
                PresentationUpdate::InPlace
            } else {
                PresentationUpdate::TransitionRequired
            };
        let next_inactivity_condition = inactivity_condition(&snapshot);
        let has_new_source_sample = self.snapshot.playback != snapshot.playback
            || self.snapshot.progress != snapshot.progress;
        if has_new_source_sample {
            self.source_sample_age = match self.behavior {
                PresentationBehavior::Dynamic => source_sample_age(&snapshot, anchored_at.utc)?,
                PresentationBehavior::StaticFixture => Duration::ZERO,
            };
            self.progress_anchored_at = anchored_at.monotonic;
        }
        if self.inactivity_condition != next_inactivity_condition
            || (restart_inactivity && next_inactivity_condition.is_some())
        {
            self.inactivity_condition = next_inactivity_condition;
            self.inactivity_anchored_at = anchored_at.monotonic;
        }
        self.snapshot = snapshot;
        Ok(update)
    }

    pub fn disconnect(&mut self, anchored_at: Duration) -> PresentationUpdate {
        let snapshot = disconnected_snapshot(self.snapshot.revision);
        let previous_presentation = presentation_from_snapshot(&self.snapshot)
            .expect("PresentationState retains a validated snapshot");
        let next_presentation =
            presentation_from_snapshot(&snapshot).expect("the disconnected snapshot is valid");
        let content_changed =
            presentation_composition_changed(&previous_presentation, &next_presentation);
        let next_inactivity_condition = inactivity_condition(&snapshot);
        if self.inactivity_condition != next_inactivity_condition {
            self.inactivity_condition = next_inactivity_condition;
            self.inactivity_anchored_at = anchored_at;
        }
        self.snapshot = snapshot;
        self.progress_anchored_at = anchored_at;
        self.source_sample_age = Duration::ZERO;
        if content_changed {
            PresentationUpdate::TransitionRequired
        } else {
            PresentationUpdate::InPlace
        }
    }

    pub fn presentation_at(&self, now: Duration) -> Result<Presentation, PresentationError> {
        let elapsed = match self.behavior {
            PresentationBehavior::Dynamic => self
                .source_sample_age
                .saturating_add(now.saturating_sub(self.progress_anchored_at)),
            PresentationBehavior::StaticFixture => Duration::ZERO,
        };
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
        if self.behavior == PresentationBehavior::StaticFixture {
            return InactivityTransform::default();
        }
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

fn presentation_composition_changed(previous: &Presentation, next: &Presentation) -> bool {
    let mut comparable = previous.clone();
    match (&mut comparable, next) {
        (Presentation::NowPlaying(previous), Presentation::NowPlaying(next)) => {
            previous.status = next.status;
            if previous.progress.is_some() && next.progress.is_some() {
                previous.progress.clone_from(&next.progress);
            }
        }
        (Presentation::FullField(previous), Presentation::FullField(next)) => {
            previous.status = next.status;
        }
        _ => return true,
    }
    comparable != *next
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
            snapshot.tracked_output.as_ref(),
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
            presentation_status_for_playback(playback),
            "Nothing is playing",
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
        status: presentation_status_for_playback(playback),
        progress: snapshot
            .progress
            .as_ref()
            .map(|progress| presentation_progress(progress, playback, elapsed)),
        activity: (playback == Playback::Playing && snapshot.progress.is_none())
            .then(|| Box::new(indeterminate_activity())),
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

fn indeterminate_activity() -> PresentationActivity {
    PresentationActivity {
        waveform: PresentationActivityWaveform {
            reference_heights_percent: [30, 70, 100, 48, 100, 70, 30],
            minimum_scale_percent: 28,
            phase_offsets: [0, 90, 180, 270, 360, 450, 540].map(Duration::from_millis),
            motion: PresentationActivityMotion::AlternatingEaseInOut {
                period: Duration::from_millis(1_100),
            },
        },
        heading: "Audio active",
        detail: "Timing unavailable",
    }
}

fn usable_metadata_line(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| is_usable_metadata_line(value))
        .map(str::to_owned)
}

fn is_usable_metadata_line(value: &str) -> bool {
    !value.trim().is_empty()
}

fn available_full_field(
    status: PresentationStatus,
    heading: &'static str,
    tracked_output: &TrackedOutput,
    tracked_zone: &TrackedZone,
) -> FullFieldPresentation {
    FullFieldPresentation {
        status,
        heading,
        explanation: None,
        identity: Some(PresentationIdentity::OutputAndZone {
            tracked_output: tracked_output.name.clone(),
            tracked_zone: tracked_zone.name.clone(),
        }),
    }
}

impl NowPlayingPresentation {
    pub(crate) fn has_usable_metadata(&self) -> bool {
        [
            self.title.as_deref(),
            self.artist.as_deref(),
            self.album.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(is_usable_metadata_line)
    }
}

pub(crate) fn trackless_full_field(presentation: &NowPlayingPresentation) -> FullFieldPresentation {
    let heading = match presentation.status.symbol {
        PresentationStatusSymbol::Starting => "Preparing playback",
        PresentationStatusSymbol::Paused | PresentationStatusSymbol::Playing => {
            "Now Playing details unavailable"
        }
        _ => unreachable!("Now Playing fallback requires a playback Presentation Status"),
    };
    FullFieldPresentation {
        status: presentation.status,
        heading,
        explanation: None,
        identity: Some(PresentationIdentity::OutputAndZone {
            tracked_output: presentation.tracked_output.clone(),
            tracked_zone: presentation.tracked_zone.clone(),
        }),
    }
}

fn unavailable_presentation(
    availability: Availability,
    tracked_output: Option<&TrackedOutput>,
) -> FullFieldPresentation {
    let status = presentation_status_for_availability(availability);
    match availability {
        Availability::PairingRequired => FullFieldPresentation {
            status,
            heading: "Enable RoonScape",
            explanation: Some("In a Roon client, open Settings → Extensions and enable RoonScape."),
            identity: None,
        },
        Availability::Disconnected => FullFieldPresentation {
            status,
            heading: "Waiting for Roon",
            explanation: Some("Check Roon Server and the network."),
            identity: None,
        },
        Availability::OutputUnavailable => FullFieldPresentation {
            status,
            heading: "Check the selected output",
            explanation: Some(
                "Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.",
            ),
            identity: tracked_output.map(|tracked_output| PresentationIdentity::OutputOnly {
                tracked_output: tracked_output.name.clone(),
            }),
        },
        Availability::Available => unreachable!("available snapshots use Now Playing"),
    }
}

fn presentation_status_for_playback(playback: Playback) -> PresentationStatus {
    match playback {
        Playback::Playing => PresentationStatus {
            label: "PLAYING",
            symbol: PresentationStatusSymbol::Playing,
            motion: PresentationStatusMotion::Static,
            emphasis: PresentationStatusEmphasis::FullAccent,
        },
        Playback::Paused => PresentationStatus {
            label: "PAUSED",
            symbol: PresentationStatusSymbol::Paused,
            motion: PresentationStatusMotion::Static,
            emphasis: PresentationStatusEmphasis::MutedAccent,
        },
        Playback::Loading => PresentationStatus {
            label: "STARTING",
            symbol: PresentationStatusSymbol::Starting,
            motion: PresentationStatusMotion::ContinuousRotation {
                period: Duration::from_millis(1_800),
            },
            emphasis: PresentationStatusEmphasis::FullAccent,
        },
        Playback::Stopped => PresentationStatus {
            label: "IDLE",
            symbol: PresentationStatusSymbol::Idle,
            motion: PresentationStatusMotion::Static,
            emphasis: PresentationStatusEmphasis::MutedAccent,
        },
    }
}

fn presentation_status_for_availability(availability: Availability) -> PresentationStatus {
    let (label, symbol) = match availability {
        Availability::PairingRequired => (
            "PAIRING REQUIRED",
            PresentationStatusSymbol::PairingRequired,
        ),
        Availability::Disconnected => ("DISCONNECTED", PresentationStatusSymbol::Disconnected),
        Availability::OutputUnavailable => (
            "OUTPUT UNAVAILABLE",
            PresentationStatusSymbol::OutputUnavailable,
        ),
        Availability::Available => {
            unreachable!("available snapshots use playback Presentation Status")
        }
    };
    PresentationStatus {
        label,
        symbol,
        motion: PresentationStatusMotion::Static,
        emphasis: PresentationStatusEmphasis::FullAccent,
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
