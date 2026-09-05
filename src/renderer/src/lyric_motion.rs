use std::time::Duration;

use roonscape_renderer::{LyricNeighborVisibility, LyricPresentation};

const CUE_HANDOFF_DURATION: Duration = Duration::from_millis(620);
const BLANK_TRANSITION_DURATION: Duration = Duration::from_millis(440);
const COMPOSITION_TRANSITION_DURATION: Duration = Duration::from_millis(580);
// Roon timing is stamped on receipt, so a refreshed anchor can differ from
// local projection without a user seek. Small same-cue seeks are inherently
// indistinguishable from those corrections; preserve the lift within this
// half-second tolerance rather than cut it on ordinary timing jitter.
const SEEK_DISCONTINUITY_SECONDS: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LyricMotionCause {
    Settled,
    NaturalCueHandoff { height_aware: bool },
    IntentionalBlankEntry,
    IntentionalBlankExit,
    IntentionalBlankContinuation,
    SkippedCueDestination,
    InterruptedHandoffDestination,
    ExternalSeek,
    TimelineRevision,
    CompositionEntry,
    CompositionExit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LyricCueSlot {
    Previous,
    Current,
    Next,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LyricColorRole {
    Previous,
    Focal,
    Next,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LyricCueFrame {
    pub slot: LyricCueSlot,
    pub text: String,
    pub position: f64,
    pub emphasis: f64,
    pub opacity: f64,
    pub color_from: LyricColorRole,
    pub color_to: LyricColorRole,
    pub color_progress: f64,
    pub departing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LyricFrame {
    pub composition_progress: f64,
    pub cause: LyricMotionCause,
    pub cues: Vec<LyricCueFrame>,
    pub cue_motion_active: bool,
    pub composition_motion_active: bool,
}

#[derive(Clone, Debug)]
struct ScalarMotion {
    from: f64,
    target: f64,
    started_at: Option<Duration>,
}

impl ScalarMotion {
    fn settled(value: f64) -> Self {
        Self {
            from: value,
            target: value,
            started_at: None,
        }
    }

    fn value_at(&self, now: Duration) -> f64 {
        let Some(started_at) = self.started_at else {
            return self.target;
        };
        let progress = linear_progress(now, started_at, COMPOSITION_TRANSITION_DURATION);
        mix(self.from, self.target, progress)
    }

    fn is_active_at(&self, now: Duration) -> bool {
        self.started_at.is_some_and(|started_at| {
            now.saturating_sub(started_at) < COMPOSITION_TRANSITION_DURATION
        })
    }

    fn retarget(&mut self, target: f64, now: Duration, animate: bool) {
        let current = self.value_at(now);
        if !animate || approximately_equal(current, target) {
            *self = Self::settled(target);
            return;
        }
        self.from = current;
        self.target = target;
        self.started_at = Some(now);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CueMotionKind {
    Natural { height_aware: bool },
    BlankEntry,
    BlankExit,
}

#[derive(Clone, Debug)]
struct CueMotion {
    kind: CueMotionKind,
    source: LyricPresentation,
    target: LyricPresentation,
    target_lines: i32,
    source_lines: i32,
    interrupted_frame: Option<Vec<LyricCueFrame>>,
    started_at: Duration,
}

impl CueMotion {
    fn duration(&self) -> Duration {
        match self.kind {
            CueMotionKind::Natural { .. } | CueMotionKind::BlankExit => CUE_HANDOFF_DURATION,
            CueMotionKind::BlankEntry => BLANK_TRANSITION_DURATION,
        }
    }

    fn is_active_at(&self, now: Duration) -> bool {
        now.saturating_sub(self.started_at) < self.duration()
    }

    fn progress_at(&self, now: Duration) -> f64 {
        linear_progress(now, self.started_at, self.duration())
    }
}

pub(crate) struct LyricMotion {
    revision: u64,
    semantic: Option<LyricPresentation>,
    displayed: Option<LyricPresentation>,
    rendered_lines: i32,
    cause: LyricMotionCause,
    cue_motion: Option<CueMotion>,
    composition: ScalarMotion,
    playback_sample: Option<PlaybackSample>,
    timing_discontinuity: bool,
}

struct PlaybackSample {
    revision: u64,
    position_seconds: Option<f64>,
    advancing: bool,
    observed_at: Duration,
}

impl LyricMotion {
    pub(crate) fn new(
        revision: u64,
        lyrics: Option<&LyricPresentation>,
        rendered_lines: i32,
    ) -> Self {
        Self {
            revision,
            semantic: lyrics.cloned(),
            displayed: lyrics.cloned(),
            rendered_lines,
            cause: LyricMotionCause::Settled,
            cue_motion: None,
            composition: ScalarMotion::settled(f64::from(lyrics.is_some())),
            playback_sample: None,
            timing_discontinuity: false,
        }
    }

    pub(crate) fn observe_playback(
        &mut self,
        revision: u64,
        position_seconds: Option<f64>,
        advancing: bool,
        now: Duration,
    ) {
        self.timing_discontinuity = self.playback_sample.as_ref().is_some_and(|previous| {
            if previous.revision == revision || (previous.advancing && !advancing) {
                // A pause may stop the clock between ticks, but the selected lift
                // still finishes. Subsequent paused seeks remain discontinuities.
                return false;
            }
            previous
                .position_seconds
                .zip(position_seconds)
                .is_some_and(|(before, after)| {
                    let elapsed = if previous.advancing {
                        now.saturating_sub(previous.observed_at).as_secs_f64()
                    } else {
                        0.0
                    };
                    (after - (before + elapsed)).abs() > SEEK_DISCONTINUITY_SECONDS
                })
        });
        self.playback_sample = Some(PlaybackSample {
            revision,
            position_seconds,
            advancing,
            observed_at: now,
        });
    }

    pub(crate) fn update(
        &mut self,
        revision: u64,
        lyrics: Option<&LyricPresentation>,
        rendered_lines: i32,
        now: Duration,
        animations_enabled: bool,
    ) {
        let next = lyrics.cloned();
        let revision_changed = revision != self.revision;
        self.revision = revision;

        if !animations_enabled && self.semantic == next {
            self.displayed.clone_from(&next);
            self.rendered_lines = rendered_lines;
            self.cue_motion = None;
            self.composition = ScalarMotion::settled(f64::from(self.semantic.is_some()));
            return;
        }

        if self.semantic == next {
            if revision_changed && self.timing_discontinuity {
                self.displayed.clone_from(&next);
                self.rendered_lines = rendered_lines;
                self.cue_motion = None;
                self.cause = LyricMotionCause::ExternalSeek;
                return;
            }
            if self
                .cue_motion
                .as_ref()
                .is_none_or(|motion| !motion.is_active_at(now))
            {
                self.rendered_lines = rendered_lines;
            }
            return;
        }

        let presence_changed = self.semantic.is_some() != next.is_some();
        if presence_changed {
            self.cue_motion = None;
            self.composition
                .retarget(f64::from(next.is_some()), now, animations_enabled);
            self.cause = if next.is_some() {
                self.displayed.clone_from(&next);
                LyricMotionCause::CompositionEntry
            } else {
                LyricMotionCause::CompositionExit
            };
            self.semantic = next;
            self.rendered_lines = rendered_lines;
            return;
        }

        let (Some(source), Some(target)) = (self.semantic.as_ref(), next.as_ref()) else {
            self.semantic = next;
            self.displayed = None;
            self.cue_motion = None;
            self.cause = LyricMotionCause::Settled;
            return;
        };
        let active_motion_is_interrupted = self
            .cue_motion
            .as_ref()
            .is_some_and(|motion| motion.is_active_at(now));
        let source_is_blank = source.current.trim().is_empty();
        let target_is_blank = target.current.trim().is_empty();
        let continuing_blank_departure = active_motion_is_interrupted
            && !target_is_blank
            && self
                .cue_motion
                .as_ref()
                .is_some_and(|motion| motion.kind == CueMotionKind::BlankEntry);
        let adjacent = target.current_index == source.current_index.saturating_add(1)
            || (!target_is_blank && target.previous_index == Some(source.current_index))
            || (source_is_blank
                && !target_is_blank
                && target.current_index >= source.current_index
                && target
                    .previous_index
                    .is_none_or(|index| index < source.current_index));

        if !revision_changed && adjacent && source_is_blank && target_is_blank {
            self.cause = LyricMotionCause::IntentionalBlankContinuation;
            if !animations_enabled || !active_motion_is_interrupted {
                self.cue_motion = None;
            }
            self.semantic.clone_from(&next);
            self.displayed.clone_from(&next);
            self.rendered_lines = rendered_lines;
            return;
        }

        let (kind, cause) = if revision_changed {
            let cause = if source.timeline_signature != target.timeline_signature {
                LyricMotionCause::TimelineRevision
            } else {
                LyricMotionCause::ExternalSeek
            };
            (None, cause)
        } else if active_motion_is_interrupted && !continuing_blank_departure {
            (None, LyricMotionCause::InterruptedHandoffDestination)
        } else if !adjacent {
            (None, LyricMotionCause::SkippedCueDestination)
        } else if target_is_blank {
            (
                Some(CueMotionKind::BlankEntry),
                LyricMotionCause::IntentionalBlankEntry,
            )
        } else if source_is_blank {
            (
                Some(CueMotionKind::BlankExit),
                LyricMotionCause::IntentionalBlankExit,
            )
        } else {
            let height_aware = self.rendered_lines >= 3 || rendered_lines >= 3;
            (
                Some(CueMotionKind::Natural { height_aware }),
                LyricMotionCause::NaturalCueHandoff { height_aware },
            )
        };

        let interrupted_frame = continuing_blank_departure.then(|| self.frame_at(now).cues);
        self.cue_motion = kind.filter(|_| animations_enabled).map(|kind| CueMotion {
            kind,
            source: source.clone(),
            target: target.clone(),
            target_lines: rendered_lines,
            source_lines: self.rendered_lines,
            interrupted_frame,
            started_at: now,
        });
        self.cause = cause;
        self.semantic = next;
        self.displayed = lyrics.cloned();
        self.rendered_lines = rendered_lines;
    }

    pub(crate) fn frame_at(&self, now: Duration) -> LyricFrame {
        let cue_motion_active = self
            .cue_motion
            .as_ref()
            .is_some_and(|motion| motion.is_active_at(now));
        let cues = match self
            .cue_motion
            .as_ref()
            .filter(|motion| motion.is_active_at(now))
        {
            Some(motion) => cue_motion_frame(motion, now),
            None => stable_cues(self.displayed.as_ref(), self.rendered_lines),
        };
        LyricFrame {
            composition_progress: self.composition.value_at(now),
            cause: self.cause,
            cues,
            cue_motion_active,
            composition_motion_active: self.composition.is_active_at(now),
        }
    }

    pub(crate) fn reconcile_rendered_lines(&mut self, rendered_lines: i32, now: Duration) {
        if self
            .cue_motion
            .as_ref()
            .is_none_or(|motion| !motion.is_active_at(now))
        {
            self.rendered_lines = rendered_lines;
        }
    }
}

fn cue_motion_frame(motion: &CueMotion, now: Duration) -> Vec<LyricCueFrame> {
    let progress = motion.progress_at(now);
    match motion.kind {
        CueMotionKind::Natural { height_aware } => natural_cue_frame(
            &motion.source,
            &motion.target,
            motion.target_lines,
            progress,
            height_aware,
        ),
        CueMotionKind::BlankEntry => {
            let tall = motion.source_lines >= 3;
            let mut cues = stable_cues(Some(&motion.target), 1);
            cues.retain(|cue| cue.slot != LyricCueSlot::Previous);
            let mut outgoing = cue_frame(
                LyricCueSlot::Previous,
                &motion.source.current,
                if tall {
                    -1.55 * phase(progress, 0.0, 0.6)
                } else {
                    -smoothstep(progress)
                },
                if tall {
                    1.0
                } else {
                    1.0 - smoothstep(progress)
                },
                if tall {
                    1.0 - phase(progress, 0.0, 0.58)
                } else {
                    1.0
                },
                LyricColorRole::Focal,
                LyricColorRole::Previous,
                smoothstep(progress),
            );
            outgoing.departing = tall;
            if tall && progress >= 0.6 {
                outgoing = cue_frame(
                    LyricCueSlot::Previous,
                    &motion.source.current,
                    -1.0,
                    0.0,
                    phase(progress, 0.6, 0.4),
                    LyricColorRole::Previous,
                    LyricColorRole::Previous,
                    1.0,
                );
            }
            cues.push(outgoing);
            cues
        }
        CueMotionKind::BlankExit => {
            let visibility = LyricNeighborVisibility::for_rendered_lines(motion.target_lines);
            let mut cues = stable_cues(Some(&motion.target), motion.target_lines);
            cues.retain(|cue| cue.slot != LyricCueSlot::Current);
            for cue in &mut cues {
                if cue.slot == LyricCueSlot::Next {
                    cue.opacity = phase(progress, 0.58, 0.34);
                }
            }
            if let Some(initial) = motion
                .interrupted_frame
                .as_ref()
                .and_then(|cues| cues.iter().find(|cue| cue.slot == LyricCueSlot::Previous))
            {
                cues.retain(|cue| cue.slot != LyricCueSlot::Previous);
                cues.push(continue_blank_departure(
                    initial,
                    visibility.previous,
                    progress,
                ));
            }
            let arrival = phase(progress, 0.0, 0.78);
            let focus = phase(progress, 0.08, 0.58);
            cues.push(cue_frame(
                LyricCueSlot::Current,
                &motion.target.current,
                mix(
                    if motion.target_lines >= 3 { 0.55 } else { 1.0 },
                    0.0,
                    arrival,
                ),
                focus,
                1.0,
                LyricColorRole::Next,
                LyricColorRole::Focal,
                focus,
            ));
            cues
        }
    }
}

fn continue_blank_departure(
    initial: &LyricCueFrame,
    show_previous: bool,
    progress: f64,
) -> LyricCueFrame {
    let departure = phase(progress, 0.0, 0.6);
    let mut previous = initial.clone();
    if initial.departing {
        if progress < 0.6 {
            previous.position = mix(initial.position, -1.55, departure);
            previous.opacity = initial.opacity * (1.0 - departure);
            return previous;
        }
        return cue_frame(
            LyricCueSlot::Previous,
            &initial.text,
            -1.0,
            0.0,
            if show_previous {
                phase(progress, 0.6, 0.4)
            } else {
                0.0
            },
            LyricColorRole::Previous,
            LyricColorRole::Previous,
            1.0,
        );
    }
    previous.position = mix(initial.position, -1.0, departure);
    previous.emphasis = initial.emphasis * (1.0 - departure);
    previous.opacity = if show_previous {
        1.0
    } else {
        initial.opacity * (1.0 - departure)
    };
    previous.color_progress = mix(initial.color_progress, 1.0, departure);
    previous
}

fn natural_cue_frame(
    source: &LyricPresentation,
    target: &LyricPresentation,
    target_lines: i32,
    progress: f64,
    height_aware: bool,
) -> Vec<LyricCueFrame> {
    let (
        outgoing_position,
        outgoing_emphasis,
        outgoing_opacity,
        incoming_position,
        incoming_emphasis,
        incoming_opacity,
    ) = if height_aware {
        let departure = phase(progress, 0.0, 0.6);
        let arrival = phase(progress, 0.0, 0.55);
        let focus = phase(progress, 0.0, 0.4);
        (
            mix(0.0, -1.55, departure),
            1.0,
            1.0 - phase(progress, 0.0, 0.22),
            mix(0.55, 0.0, arrival),
            mix(0.38, 1.0, focus),
            phase(progress, 0.0, 0.12),
        )
    } else {
        let trajectory = phase(progress, 0.0, 0.78);
        let memory = phase(progress, 0.0, 0.72);
        let focus = phase(progress, 0.08, 0.58);
        (
            mix(0.0, -1.0, memory),
            1.0 - memory,
            1.0,
            mix(1.0, 0.0, trajectory),
            focus,
            1.0,
        )
    };
    let mut cues = vec![
        cue_frame(
            LyricCueSlot::Previous,
            &source.current,
            outgoing_position,
            outgoing_emphasis,
            outgoing_opacity,
            LyricColorRole::Focal,
            LyricColorRole::Previous,
            if height_aware {
                progress
            } else {
                1.0 - outgoing_emphasis
            },
        ),
        cue_frame(
            LyricCueSlot::Current,
            &target.current,
            incoming_position,
            incoming_emphasis,
            incoming_opacity,
            LyricColorRole::Next,
            LyricColorRole::Focal,
            incoming_emphasis,
        ),
    ];
    if height_aware {
        cues[0].departing = true;
        if progress >= 0.58 {
            cues[0] = cue_frame(
                LyricCueSlot::Previous,
                &source.current,
                -1.0,
                0.0,
                if LyricNeighborVisibility::for_rendered_lines(target_lines).previous {
                    phase(progress, 0.58, 0.34)
                } else {
                    0.0
                },
                LyricColorRole::Previous,
                LyricColorRole::Previous,
                1.0,
            );
        }
    }
    if LyricNeighborVisibility::for_rendered_lines(target_lines).next
        && let Some(next) = target.next.as_deref()
    {
        let arrival = phase(progress, 0.58, 0.34);
        cues.push(cue_frame(
            LyricCueSlot::Next,
            next,
            1.0,
            0.0,
            arrival,
            LyricColorRole::Next,
            LyricColorRole::Next,
            arrival,
        ));
    }
    cues
}

fn stable_cues(lyrics: Option<&LyricPresentation>, rendered_lines: i32) -> Vec<LyricCueFrame> {
    let Some(lyrics) = lyrics else {
        return Vec::new();
    };
    let blank = lyrics.current.trim().is_empty();
    let visibility =
        LyricNeighborVisibility::for_rendered_lines(if blank { 1 } else { rendered_lines });
    let mut cues = Vec::with_capacity(3);
    if visibility.previous
        && let Some(previous) = lyrics.previous.as_deref()
    {
        cues.push(cue_frame(
            LyricCueSlot::Previous,
            previous,
            -1.0,
            0.0,
            1.0,
            LyricColorRole::Previous,
            LyricColorRole::Previous,
            1.0,
        ));
    }
    if !blank {
        cues.push(cue_frame(
            LyricCueSlot::Current,
            &lyrics.current,
            0.0,
            1.0,
            1.0,
            LyricColorRole::Focal,
            LyricColorRole::Focal,
            1.0,
        ));
    }
    if visibility.next
        && let Some(next) = lyrics.next.as_deref()
    {
        cues.push(cue_frame(
            LyricCueSlot::Next,
            next,
            1.0,
            0.0,
            1.0,
            LyricColorRole::Next,
            LyricColorRole::Next,
            1.0,
        ));
    }
    cues
}

#[allow(clippy::too_many_arguments)]
fn cue_frame(
    slot: LyricCueSlot,
    text: &str,
    position: f64,
    emphasis: f64,
    opacity: f64,
    color_from: LyricColorRole,
    color_to: LyricColorRole,
    color_progress: f64,
) -> LyricCueFrame {
    LyricCueFrame {
        slot,
        text: text.to_owned(),
        position,
        emphasis,
        opacity,
        color_from,
        color_to,
        color_progress,
        departing: false,
    }
}

fn linear_progress(now: Duration, started_at: Duration, duration: Duration) -> f64 {
    if duration.is_zero() {
        return 1.0;
    }
    let linear = now.saturating_sub(started_at).as_secs_f64() / duration.as_secs_f64();
    linear.clamp(0.0, 1.0)
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn phase(value: f64, start: f64, duration: f64) -> f64 {
    smoothstep(((value - start) / duration).clamp(0.0, 1.0))
}

fn mix(from: f64, to: f64, progress: f64) -> f64 {
    from + (to - from) * progress
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lyrics(
        current_index: usize,
        previous: Option<&str>,
        current: &str,
        next: Option<&str>,
    ) -> LyricPresentation {
        lyrics_on_timeline(1, current_index, previous, current, next)
    }

    fn lyrics_on_timeline(
        timeline_signature: u64,
        current_index: usize,
        previous: Option<&str>,
        current: &str,
        next: Option<&str>,
    ) -> LyricPresentation {
        LyricPresentation {
            timeline_signature,
            current_index,
            previous_index: current_index.checked_sub(1),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
            next: next.map(str::to_owned),
        }
    }

    #[test]
    fn compact_adjacent_cues_transfer_focal_ownership_without_an_opacity_valley() {
        let first = lyrics(0, None, "Again", Some("Again"));
        let second = lyrics(1, Some("Again"), "Again", Some("After"));
        let mut motion = LyricMotion::new(7, Some(&first), 1);

        motion.update(7, Some(&second), 1, Duration::ZERO, true);
        let midpoint = motion.frame_at(CUE_HANDOFF_DURATION / 2);

        assert_eq!(
            midpoint.cause,
            LyricMotionCause::NaturalCueHandoff {
                height_aware: false
            }
        );
        assert_eq!(midpoint.cues.len(), 3);
        let outgoing = midpoint
            .cues
            .iter()
            .find(|cue| cue.slot == LyricCueSlot::Previous)
            .expect("a compact Reel Lift keeps the outgoing cue as visual memory");
        let incoming = midpoint
            .cues
            .iter()
            .find(|cue| cue.slot == LyricCueSlot::Current)
            .expect("a compact Reel Lift promotes the incoming cue");
        assert!(
            incoming.emphasis * incoming.opacity >= 0.7,
            "the incoming cue should visibly own focus by midpoint: {incoming:?}"
        );
        assert!(
            incoming.emphasis * incoming.opacity >= outgoing.emphasis * outgoing.opacity + 0.25,
            "midpoint ownership must be decisive: outgoing={outgoing:?}, incoming={incoming:?}"
        );
        assert!(
            outgoing.position <= -0.65,
            "the outgoing cue should be nearing the Previous Cue tier: {outgoing:?}"
        );
        assert!(
            incoming.position <= 0.35,
            "the anticipation cue should be nearing the focal tier: {incoming:?}"
        );

        let settled = motion.frame_at(CUE_HANDOFF_DURATION);
        assert_eq!(
            settled
                .cues
                .iter()
                .map(|cue| (cue.slot, cue.text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (LyricCueSlot::Previous, "Again"),
                (LyricCueSlot::Current, "Again"),
                (LyricCueSlot::Next, "After"),
            ]
        );
    }

    #[test]
    fn either_tall_endpoint_selects_the_abbreviated_height_aware_path() {
        for (source_lines, target_lines) in [(3, 1), (1, 4)] {
            let first = lyrics(0, None, "Source", Some("Target"));
            let second = lyrics(1, Some("Source"), "Target", Some("After"));
            let mut motion = LyricMotion::new(11, Some(&first), source_lines);

            motion.update(11, Some(&second), target_lines, Duration::ZERO, true);
            let midpoint = motion.frame_at(CUE_HANDOFF_DURATION / 2);

            assert_eq!(
                midpoint.cause,
                LyricMotionCause::NaturalCueHandoff { height_aware: true }
            );
            assert!(midpoint.cues[0].position < -0.5);
            assert!(midpoint.cues[1].position < 0.5);
        }
    }

    #[test]
    fn consecutive_intentional_blanks_form_one_empty_reel_interval() {
        let current = lyrics(0, None, "Before", Some("After"));
        let first_blank = lyrics(1, None, "", None);
        let second_blank = lyrics(2, None, " ", None);
        let mut motion = LyricMotion::new(13, Some(&current), 1);

        motion.update(13, Some(&first_blank), 1, Duration::ZERO, true);
        assert_eq!(
            motion.frame_at(Duration::ZERO).cause,
            LyricMotionCause::IntentionalBlankEntry
        );
        let continued_at = Duration::from_millis(100);
        motion.update(13, Some(&second_blank), 1, continued_at, true);
        let continued = motion.frame_at(continued_at);
        assert_eq!(
            continued.cause,
            LyricMotionCause::IntentionalBlankContinuation
        );
        assert!(continued.cue_motion_active);
        assert_eq!(continued.cues[0].text, "Before");

        let settled = motion.frame_at(BLANK_TRANSITION_DURATION);
        assert!(!settled.cue_motion_active);
        assert!(settled.cues.is_empty());
    }

    #[test]
    fn external_seeks_install_the_destination_without_travel() {
        let current = lyrics(0, None, "First", Some("Second"));
        let destination = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(17, Some(&current), 1);

        motion.update(18, Some(&destination), 1, Duration::ZERO, true);
        let frame = motion.frame_at(Duration::ZERO);

        assert_eq!(frame.cause, LyricMotionCause::ExternalSeek);
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Second");
        assert_eq!(frame.cues[1].position, 0.0);
        assert_eq!(frame.cues[1].emphasis, 1.0);
    }

    #[test]
    fn timeline_revisions_that_change_the_selected_cue_are_distinct_from_external_seeks() {
        let current = lyrics_on_timeline(1, 1, Some("First"), "Second", Some("Third"));
        let corrected = lyrics_on_timeline(2, 2, Some("Corrected"), "Third", Some("Fourth"));
        let mut motion = LyricMotion::new(19, Some(&current), 1);

        motion.update(20, Some(&corrected), 1, Duration::ZERO, true);

        let frame = motion.frame_at(Duration::ZERO);
        assert_eq!(frame.cause, LyricMotionCause::TimelineRevision);
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Third");
    }

    #[test]
    fn delayed_local_progression_is_distinct_from_an_external_seek() {
        let first = lyrics(0, None, "First", Some("Second"));
        let third = lyrics(2, Some("Second"), "Third", Some("Fourth"));
        let mut motion = LyricMotion::new(21, Some(&first), 1);

        motion.update(21, Some(&third), 1, Duration::ZERO, true);

        let frame = motion.frame_at(Duration::ZERO);
        assert_eq!(frame.cause, LyricMotionCause::SkippedCueDestination);
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Third");
    }

    #[test]
    fn interrupted_natural_cue_handoffs_prioritize_the_newest_complete_endpoint() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let third = lyrics(2, Some("Second"), "Third", Some("Fourth"));
        let mut motion = LyricMotion::new(23, Some(&first), 1);
        motion.update(23, Some(&second), 1, Duration::ZERO, true);

        motion.update(23, Some(&third), 1, Duration::from_millis(160), true);

        for now in [Duration::from_millis(160), Duration::from_secs(2)] {
            let frame = motion.frame_at(now);
            assert_eq!(frame.cause, LyricMotionCause::InterruptedHandoffDestination);
            assert!(!frame.cue_motion_active);
            assert_eq!(frame.cues[1].text, "Third");
        }
    }

    #[test]
    fn composition_motion_retargets_from_current_progress_and_reduced_animation_jumps() {
        let cue = lyrics(0, None, "Opening", None);
        let mut motion = LyricMotion::new(29, None, 1);
        motion.update(29, Some(&cue), 1, Duration::ZERO, true);
        let interrupted_at = COMPOSITION_TRANSITION_DURATION / 2;
        let midpoint = motion.frame_at(interrupted_at).composition_progress;
        assert!(midpoint > 0.0 && midpoint < 1.0);

        motion.update(29, None, 1, interrupted_at, true);
        assert_eq!(
            motion.frame_at(interrupted_at).composition_progress,
            midpoint,
            "exit should reverse from the currently rendered geometry"
        );
        assert_eq!(
            motion.frame_at(interrupted_at).cause,
            LyricMotionCause::CompositionExit
        );

        motion.update(30, Some(&cue), 1, interrupted_at, false);
        let reduced = motion.frame_at(interrupted_at);
        assert_eq!(reduced.composition_progress, 1.0);
        assert!(!reduced.composition_motion_active);
        assert_eq!(reduced.cues[0].text, "Opening");
    }

    #[test]
    fn disabling_animation_during_a_natural_cue_handoff_installs_its_semantic_endpoint() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(31, Some(&first), 1);
        motion.update(31, Some(&second), 1, Duration::ZERO, true);

        let disabled_at = Duration::from_millis(100);
        motion.update(31, Some(&second), 1, disabled_at, false);
        let frame = motion.frame_at(disabled_at);

        assert!(!frame.cue_motion_active);
        assert!(!frame.composition_motion_active);
        assert_eq!(frame.cues[1].text, "Second");
        assert_eq!(frame.cues[1].position, 0.0);
        assert_eq!(frame.cues[1].emphasis, 1.0);
    }

    #[test]
    fn reduced_animation_preserves_the_natural_handoff_classification() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(35, Some(&first), 1);

        motion.update(35, Some(&second), 1, Duration::ZERO, false);

        let frame = motion.frame_at(Duration::ZERO);
        assert_eq!(
            frame.cause,
            LyricMotionCause::NaturalCueHandoff {
                height_aware: false
            }
        );
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Second");
        assert_eq!(frame.cues[1].position, 0.0);
    }

    #[test]
    fn an_unchanged_pause_update_allows_the_selected_natural_cue_handoff_to_settle() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(37, Some(&first), 1);
        motion.update(37, Some(&second), 1, Duration::ZERO, true);

        motion.update(38, Some(&second), 1, Duration::from_millis(100), true);

        let settled = motion.frame_at(CUE_HANDOFF_DURATION);
        assert!(!settled.cue_motion_active);
        assert_eq!(settled.cues[1].text, "Second");
        assert_eq!(settled.cues[1].position, 0.0);
    }
}
