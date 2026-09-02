use std::error::Error;
use std::f64::consts::PI;
use std::fmt;
use std::time::{Duration, SystemTime};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::contract::{
    Availability, NowPlaying, Playback, PresentationSnapshot, SynchronizedLyrics, TimingPosition,
    TrackedOutput, TrackedZone,
};
use crate::display_configuration::InactivityConfiguration;

pub const INACTIVE_HORIZONTAL_BOUND: i32 = 18;
pub const INACTIVE_VERTICAL_BOUND: i32 = 12;
const PROVISIONAL_TIMING_GRACE: Duration = Duration::from_secs(5);

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
    pub lyrics: Option<Box<LyricPresentation>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricPresentation {
    pub current_index: usize,
    pub previous: Option<String>,
    pub current: String,
    pub next: Option<String>,
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
    timing: TimingContinuity,
    inactivity_configuration: InactivityConfiguration,
    inactivity_condition: Option<InactivityCondition>,
    inactivity_anchored_at: Duration,
    behavior: PresentationBehavior,
}

struct TimingContinuity {
    retained_position: Option<PositionAnchor>,
    retained_duration_seconds: Option<f64>,
    grace: TimingGrace,
    known_now_playing: Option<KnownNowPlaying>,
}

#[derive(Clone, Copy, Debug)]
struct PositionAnchor {
    seconds: f64,
    anchored_at: Duration,
    basis: PositionBasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PositionBasis {
    Authoritative,
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimingGrace {
    Ready,
    Active { started_at: Duration },
    Expired,
}

#[derive(Clone, Debug)]
struct KnownNowPlaying {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ResolvedTiming {
    position_seconds: Option<f64>,
    duration_seconds: Option<f64>,
}

impl TimingContinuity {
    fn new(
        snapshot: &PresentationSnapshot,
        anchored_at: PresentationTime,
        behavior: PresentationBehavior,
    ) -> Result<Self, PresentationError> {
        let retained_position = authoritative_position_anchor(snapshot, anchored_at, behavior)?;
        let retained_duration_seconds = authoritative_duration(snapshot);
        let grace = if has_complete_authoritative_timing(snapshot) {
            TimingGrace::Ready
        } else if can_observe_timing(snapshot) && behavior == PresentationBehavior::Dynamic {
            TimingGrace::Active {
                started_at: anchored_at.monotonic,
            }
        } else {
            TimingGrace::Expired
        };
        Ok(Self {
            retained_position,
            retained_duration_seconds,
            grace,
            known_now_playing: snapshot.now_playing.as_ref().map(KnownNowPlaying::from),
        })
    }

    fn discarded() -> Self {
        Self {
            retained_position: None,
            retained_duration_seconds: None,
            grace: TimingGrace::Expired,
            known_now_playing: None,
        }
    }

    fn update(
        &mut self,
        previous: &PresentationSnapshot,
        next: &PresentationSnapshot,
        anchored_at: PresentationTime,
        behavior: PresentationBehavior,
    ) -> Result<(), PresentationError> {
        let next_authoritative_position =
            authoritative_position_anchor(next, anchored_at, behavior)?;
        let zone_continues = previous.availability == Availability::Available
            && next.availability == Availability::Available
            && previous.tracked_zone.as_ref().map(|zone| zone.id.as_str())
                == next.tracked_zone.as_ref().map(|zone| zone.id.as_str());
        let next_known = next.now_playing.as_ref().map(KnownNowPlaying::from);
        let now_playing_continues = zone_continues
            && self.known_now_playing.as_ref().is_some_and(|known| {
                next_known
                    .as_ref()
                    .is_some_and(|next| known.is_compatible_with(next))
            });
        let now_playing_changed = zone_continues
            && self.known_now_playing.is_some()
            && next_known.is_some()
            && !now_playing_continues;
        let keeps_timing =
            can_observe_timing(next) && (now_playing_continues || now_playing_changed);

        if self.grace.has_expired_at(anchored_at.monotonic) {
            self.grace = TimingGrace::Expired;
        }

        let retained_position_now = self.retained_position.map(|position| PositionAnchor {
            seconds: projected_position(
                position,
                previous.playback,
                self.retained_duration_seconds,
                anchored_at.monotonic,
                behavior,
            ),
            anchored_at: anchored_at.monotonic,
            basis: position.basis,
        });

        if !keeps_timing {
            self.retained_position = next_authoritative_position;
            self.retained_duration_seconds = authoritative_duration(next);
            self.grace = if has_complete_authoritative_timing(next) {
                TimingGrace::Ready
            } else if can_observe_timing(next) && behavior == PresentationBehavior::Dynamic {
                TimingGrace::Active {
                    started_at: anchored_at.monotonic,
                }
            } else {
                TimingGrace::Expired
            };
        } else {
            if now_playing_changed {
                self.retained_position = None;
                self.retained_duration_seconds = None;
            } else {
                self.retained_position = retained_position_now;
            }

            if let Some(duration_seconds) = authoritative_duration(next) {
                self.retained_duration_seconds = Some(duration_seconds);
            }
            if let Some(position) = next_authoritative_position {
                self.retained_position = Some(position);
            } else if now_playing_changed {
                self.retained_position = Some(PositionAnchor {
                    seconds: 0.0,
                    anchored_at: anchored_at.monotonic,
                    basis: PositionBasis::Zero,
                });
            }

            if let (Some(position), Some(duration_seconds)) =
                (&mut self.retained_position, self.retained_duration_seconds)
            {
                position.seconds = position.seconds.min(duration_seconds);
            }

            if has_complete_authoritative_timing(next) {
                self.grace = TimingGrace::Ready;
            } else if behavior == PresentationBehavior::StaticFixture {
                self.grace = TimingGrace::Expired;
            } else if now_playing_changed || self.grace == TimingGrace::Ready {
                self.grace = TimingGrace::Active {
                    started_at: anchored_at.monotonic,
                };
            }
        }

        self.known_now_playing = match (now_playing_continues, next_known) {
            (true, Some(next)) => self
                .known_now_playing
                .take()
                .map(|known| known.enriched_with(next)),
            (_, next) => next,
        };
        Ok(())
    }

    fn resolved_at(
        &self,
        snapshot: &PresentationSnapshot,
        now: Duration,
        behavior: PresentationBehavior,
    ) -> ResolvedTiming {
        let position_is_authoritative = authoritative_position(snapshot).is_some();
        let duration_is_authoritative = authoritative_duration(snapshot).is_some();
        let provisional_allowed = self.grace.is_active_at(now);
        let provisional_position_allowed = provisional_allowed
            && self.retained_position.is_some_and(|position| {
                position.basis == PositionBasis::Authoritative
                    || self.retained_duration_seconds.is_some()
            });
        let position_seconds = (position_is_authoritative || provisional_position_allowed)
            .then(|| {
                self.retained_position.map(|position| {
                    projected_position(
                        position,
                        snapshot.playback,
                        self.retained_duration_seconds,
                        now,
                        behavior,
                    )
                })
            })
            .flatten();
        let duration_seconds = (duration_is_authoritative || provisional_allowed)
            .then_some(self.retained_duration_seconds)
            .flatten();

        ResolvedTiming {
            position_seconds,
            duration_seconds,
        }
        .clamped()
    }

    fn influences_presentation_at(
        &self,
        snapshot: &PresentationSnapshot,
        now: Duration,
        behavior: PresentationBehavior,
    ) -> bool {
        if !self.grace.is_active_at(now) {
            return false;
        }
        let timing = self.resolved_at(snapshot, now, behavior);
        let progress_depends_on_provisional = timing.position_seconds.is_some()
            && timing.duration_seconds.is_some()
            && (authoritative_position(snapshot).is_none()
                || authoritative_duration(snapshot).is_none());
        let lyrics_depend_on_provisional = snapshot.lyrics.is_some()
            && timing.position_seconds.is_some()
            && authoritative_position(snapshot).is_none();
        progress_depends_on_provisional || lyrics_depend_on_provisional
    }
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
            timing: TimingContinuity::discarded(),
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
        let timing = TimingContinuity::new(&snapshot, anchored_at, behavior)?;
        let inactivity_condition = inactivity_condition(&snapshot);
        Ok(Self {
            snapshot,
            timing,
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
        let previous_presentation = self.presentation_at(anchored_at.monotonic)?;
        let reconciles_provisional_timing =
            self.timing.influences_presentation_at(
                &self.snapshot,
                anchored_at.monotonic,
                self.behavior,
            ) && replaces_provisional_dimension(&self.snapshot, &snapshot)
                && same_composition_except_timing(&self.snapshot, &snapshot);
        presentation_from_snapshot(&snapshot)?;
        let next_inactivity_condition = inactivity_condition(&snapshot);
        self.timing
            .update(&self.snapshot, &snapshot, anchored_at, self.behavior)?;
        if self.inactivity_condition != next_inactivity_condition
            || (restart_inactivity && next_inactivity_condition.is_some())
        {
            self.inactivity_condition = next_inactivity_condition;
            self.inactivity_anchored_at = anchored_at.monotonic;
        }
        self.snapshot = snapshot;
        let next_presentation = self.presentation_at(anchored_at.monotonic)?;
        Ok(if reconciles_provisional_timing {
            PresentationUpdate::InPlace
        } else {
            classify_presentation_update(&previous_presentation, &next_presentation)
        })
    }

    pub fn disconnect(&mut self, anchored_at: Duration) -> PresentationUpdate {
        let snapshot = disconnected_snapshot(self.snapshot.revision);
        let previous_presentation = presentation_from_snapshot(&self.snapshot)
            .expect("PresentationState retains a validated snapshot");
        let next_presentation =
            presentation_from_snapshot(&snapshot).expect("the disconnected snapshot is valid");
        let update = classify_presentation_update(&previous_presentation, &next_presentation);
        let next_inactivity_condition = inactivity_condition(&snapshot);
        if self.inactivity_condition != next_inactivity_condition {
            self.inactivity_condition = next_inactivity_condition;
            self.inactivity_anchored_at = anchored_at;
        }
        self.snapshot = snapshot;
        self.timing = TimingContinuity::discarded();
        update
    }

    pub fn presentation_at(&self, now: Duration) -> Result<Presentation, PresentationError> {
        presentation_from_snapshot_with_timing(
            &self.snapshot,
            self.timing.resolved_at(&self.snapshot, now, self.behavior),
        )
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

impl TimingGrace {
    fn is_active_at(self, now: Duration) -> bool {
        matches!(self, Self::Active { started_at } if now.saturating_sub(started_at) < PROVISIONAL_TIMING_GRACE)
    }

    fn has_expired_at(self, now: Duration) -> bool {
        matches!(self, Self::Active { started_at } if now.saturating_sub(started_at) >= PROVISIONAL_TIMING_GRACE)
    }
}

impl KnownNowPlaying {
    fn is_compatible_with(&self, other: &Self) -> bool {
        [
            (&self.title, &other.title),
            (&self.artist, &other.artist),
            (&self.album, &other.album),
        ]
        .into_iter()
        .all(|(left, right)| left.is_none() || right.is_none() || left == right)
    }

    fn enriched_with(self, other: Self) -> Self {
        Self {
            title: other.title.or(self.title),
            artist: other.artist.or(self.artist),
            album: other.album.or(self.album),
        }
    }
}

impl From<&NowPlaying> for KnownNowPlaying {
    fn from(now_playing: &NowPlaying) -> Self {
        Self {
            title: usable_metadata_line(now_playing.title.as_deref()),
            artist: usable_metadata_line(now_playing.artist.as_deref()),
            album: usable_metadata_line(now_playing.album.as_deref()),
        }
    }
}

impl ResolvedTiming {
    fn clamped(mut self) -> Self {
        if let (Some(position_seconds), Some(duration_seconds)) =
            (self.position_seconds, self.duration_seconds)
        {
            self.position_seconds = Some(position_seconds.min(duration_seconds));
        }
        self
    }
}

fn same_composition_except_timing(
    left: &PresentationSnapshot,
    right: &PresentationSnapshot,
) -> bool {
    left.availability == right.availability
        && left.tracked_output == right.tracked_output
        && left.tracked_zone == right.tracked_zone
        && left.now_playing == right.now_playing
        && left.artwork == right.artwork
        && left.lyrics == right.lyrics
}

pub fn classify_presentation_update(
    previous: &Presentation,
    next: &Presentation,
) -> PresentationUpdate {
    let mut comparable = previous.clone();
    match (&mut comparable, next) {
        (Presentation::NowPlaying(previous), Presentation::NowPlaying(next)) => {
            previous.status = next.status;
            if previous.progress.is_some() && next.progress.is_some() {
                previous.progress.clone_from(&next.progress);
            }
            if previous.lyrics.is_some() && next.lyrics.is_some() {
                previous.lyrics.clone_from(&next.lyrics);
            }
        }
        (Presentation::FullField(previous), Presentation::FullField(next)) => {
            previous.status = next.status;
        }
        _ => return PresentationUpdate::TransitionRequired,
    }
    if comparable == *next {
        PresentationUpdate::InPlace
    } else {
        PresentationUpdate::TransitionRequired
    }
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
        schema_version: 4,
        revision,
        availability: Availability::Disconnected,
        playback: None,
        tracked_output: None,
        tracked_zone: None,
        now_playing: None,
        timing: None,
        artwork: None,
        lyrics: None,
    }
}

fn authoritative_position_anchor(
    snapshot: &PresentationSnapshot,
    received_at: PresentationTime,
    behavior: PresentationBehavior,
) -> Result<Option<PositionAnchor>, PresentationError> {
    let Some(position) = authoritative_position(snapshot) else {
        return Ok(None);
    };
    let sample_age = if behavior == PresentationBehavior::Dynamic
        && snapshot.playback == Some(Playback::Playing)
    {
        source_sample_age(position, received_at.utc)?
    } else {
        Duration::ZERO
    };
    let duration_seconds = authoritative_duration(snapshot);
    let seconds = (position.seconds + sample_age.as_secs_f64())
        .min(duration_seconds.unwrap_or(f64::INFINITY));
    Ok(Some(PositionAnchor {
        seconds,
        anchored_at: received_at.monotonic,
        basis: PositionBasis::Authoritative,
    }))
}

fn source_sample_age(
    position: &TimingPosition,
    received_at: SystemTime,
) -> Result<Duration, PresentationError> {
    let sampled_at = OffsetDateTime::parse(&position.sampled_at, &Rfc3339).map_err(|_| {
        PresentationError("timing position sampledAt must be an RFC 3339 timestamp")
    })?;
    let sample_age = OffsetDateTime::from(received_at) - sampled_at;
    if sample_age.is_negative() {
        return Ok(Duration::ZERO);
    }

    Duration::try_from(sample_age)
        .map_err(|_| PresentationError("timing position sampledAt is outside the supported range"))
}

fn authoritative_position(snapshot: &PresentationSnapshot) -> Option<&TimingPosition> {
    snapshot.timing.as_ref()?.position.as_ref()
}

fn authoritative_duration(snapshot: &PresentationSnapshot) -> Option<f64> {
    snapshot.timing.as_ref()?.duration_seconds
}

fn can_observe_timing(snapshot: &PresentationSnapshot) -> bool {
    snapshot.availability == Availability::Available
        && snapshot.playback != Some(Playback::Stopped)
        && snapshot.now_playing.is_some()
}

fn has_complete_authoritative_timing(snapshot: &PresentationSnapshot) -> bool {
    authoritative_position(snapshot).is_some() && authoritative_duration(snapshot).is_some()
}

fn replaces_provisional_dimension(
    previous: &PresentationSnapshot,
    next: &PresentationSnapshot,
) -> bool {
    (authoritative_position(previous).is_none() && authoritative_position(next).is_some())
        || (authoritative_duration(previous).is_none() && authoritative_duration(next).is_some())
}

fn projected_position(
    position: PositionAnchor,
    playback: Option<Playback>,
    duration_seconds: Option<f64>,
    now: Duration,
    behavior: PresentationBehavior,
) -> f64 {
    let advancement =
        if behavior == PresentationBehavior::Dynamic && playback == Some(Playback::Playing) {
            now.saturating_sub(position.anchored_at).as_secs_f64()
        } else {
            0.0
        };
    (position.seconds + advancement).min(duration_seconds.unwrap_or(f64::INFINITY))
}

pub fn presentation_from_snapshot(
    snapshot: &PresentationSnapshot,
) -> Result<Presentation, PresentationError> {
    let timing = ResolvedTiming {
        position_seconds: authoritative_position(snapshot).map(|position| position.seconds),
        duration_seconds: authoritative_duration(snapshot),
    }
    .clamped();
    presentation_from_snapshot_with_timing(snapshot, timing)
}

fn presentation_from_snapshot_with_timing(
    snapshot: &PresentationSnapshot,
    timing: ResolvedTiming,
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
    let lyric_presentation = timing.position_seconds.and_then(|position_seconds| {
        snapshot
            .lyrics
            .as_ref()
            .and_then(|lyrics| lyric_presentation(lyrics, position_seconds))
    });
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
        progress: timing.position_seconds.zip(timing.duration_seconds).map(
            |(position_seconds, duration_seconds)| {
                presentation_progress(position_seconds, duration_seconds)
            },
        ),
        activity: (playback == Playback::Playing
            && (timing.position_seconds.is_none() || timing.duration_seconds.is_none()))
        .then(|| Box::new(indeterminate_activity())),
        artwork_revision: snapshot.artwork.as_ref().map(|artwork| artwork.revision),
        artwork_path: snapshot
            .artwork
            .as_ref()
            .map(|artwork| artwork.path.clone()),
        lyrics: lyric_presentation.map(Box::new),
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

fn presentation_progress(position_seconds: f64, duration_seconds: f64) -> PresentationProgress {
    let remaining = duration_seconds - position_seconds;

    PresentationProgress {
        fraction: position_seconds / duration_seconds,
        elapsed: format_duration(position_seconds),
        remaining: format!("−{}", format_duration(remaining)),
    }
}

fn lyric_presentation(
    lyrics: &SynchronizedLyrics,
    position_seconds: f64,
) -> Option<LyricPresentation> {
    const LOOK_AHEAD_SECONDS: f64 = 0.7;
    const BLANK_PREPARATION_SECONDS: f64 = 2.0;
    const FINAL_HOLD_SECONDS: f64 = 3.0;

    let first = lyrics.cues.first()?;
    let last = lyrics.cues.last()?;
    let selection_position = position_seconds + LOOK_AHEAD_SECONDS;
    if selection_position < first.at_seconds
        || position_seconds > last.at_seconds + FINAL_HOLD_SECONDS
    {
        return None;
    }
    let current_index = lyrics
        .cues
        .partition_point(|cue| cue.at_seconds <= selection_position)
        .saturating_sub(1);
    let current = lyrics.cues.get(current_index)?;
    let previous = lyrics.cues[..current_index]
        .iter()
        .rev()
        .find(|cue| !cue.text.trim().is_empty())
        .map(|cue| cue.text.clone());
    let next = lyrics.cues[current_index + 1..]
        .iter()
        .find(|cue| !cue.text.trim().is_empty())
        .filter(|cue| {
            !current.text.trim().is_empty()
                || cue.at_seconds - position_seconds <= BLANK_PREPARATION_SECONDS
        })
        .map(|cue| cue.text.clone());
    Some(LyricPresentation {
        current_index,
        previous,
        current: current.text.clone(),
        next,
    })
}

fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}
