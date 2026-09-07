use std::time::Duration;

use roonscape_renderer::LyricPresentation;

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
    NaturalCueHandoff,
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
pub(crate) enum LyricColorRole {
    Earlier,
    Focal,
    Upcoming,
}

impl LyricColorRole {
    fn weights(self) -> [f64; 3] {
        match self {
            Self::Earlier => [1.0, 0.0, 0.0],
            Self::Focal => [0.0, 1.0, 0.0],
            Self::Upcoming => [0.0, 0.0, 1.0],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LyricCueFrame {
    pub role: LyricColorRole,
    pub index: i64,
    pub extent: f64,
    pub text: String,
    pub emphasis: f64,
    pub opacity: f64,
    pub color_weights: [f64; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LyricFrame {
    pub composition_progress: f64,
    pub cause: LyricMotionCause,
    pub cues: Vec<LyricCueFrame>,
    pub anchors: Vec<(i64, f64)>,
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
    Natural,
    BlankEntry,
    BlankExit,
}

#[derive(Clone, Debug)]
struct CueMotion {
    kind: CueMotionKind,
    source: LyricPresentation,
    target: LyricPresentation,
    interrupted_frame: Option<LyricFrame>,
    started_at: Duration,
}

impl CueMotion {
    fn duration(&self) -> Duration {
        match self.kind {
            CueMotionKind::Natural | CueMotionKind::BlankExit => CUE_HANDOFF_DURATION,
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
    pub(crate) fn new(revision: u64, lyrics: Option<&LyricPresentation>) -> Self {
        Self {
            revision,
            semantic: lyrics.cloned(),
            displayed: lyrics.cloned(),
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
        now: Duration,
        animations_enabled: bool,
    ) {
        let next = lyrics.cloned();
        let revision_changed = revision != self.revision;
        self.revision = revision;

        if !animations_enabled && self.semantic == next {
            self.displayed.clone_from(&next);

            self.cue_motion = None;
            self.composition = ScalarMotion::settled(f64::from(self.semantic.is_some()));
            return;
        }

        if self.semantic == next {
            if revision_changed && self.timing_discontinuity {
                self.displayed.clone_from(&next);

                self.cue_motion = None;
                self.cause = LyricMotionCause::ExternalSeek;
                return;
            }
            return;
        }

        let presence_changed = self.semantic.is_some() != next.is_some();
        if presence_changed {
            self.cue_motion = None;
            self.composition
                .retarget(f64::from(next.is_some()), now, animations_enabled);
            // A timing relocation owns its lyric destination immediately, even
            // while artwork and metadata finish the composition transition.
            // Only continuous exits retain the old reel for its normal fade.
            if next.is_some()
                || (revision_changed && self.timing_discontinuity)
                || !animations_enabled
            {
                self.displayed.clone_from(&next);
            }
            self.cause = if next.is_some() {
                LyricMotionCause::CompositionEntry
            } else {
                LyricMotionCause::CompositionExit
            };
            self.semantic = next;

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
        let source_is_blank = source.current().trim().is_empty();
        let target_is_blank = target.current().trim().is_empty();
        let continuing_blank_departure = active_motion_is_interrupted
            && !target_is_blank
            && self
                .cue_motion
                .as_ref()
                .is_some_and(|motion| motion.kind == CueMotionKind::BlankEntry);
        let adjacent = target.current_index == source.current_index.saturating_add(1)
            || (!target_is_blank && target.previous_index() == Some(source.current_index))
            || (source_is_blank
                && !target_is_blank
                && target.current_index >= source.current_index
                && target
                    .previous_index()
                    .is_none_or(|index| index < source.current_index));
        let timeline_changed = source.timeline_signature != target.timeline_signature;
        // Snapshot revisions also advance for ordinary Roon timing updates.
        // Only a timing relocation should turn an adjacent cue into a seek.
        let external_seek =
            revision_changed && (self.timing_discontinuity || self.playback_sample.is_none());

        if !timeline_changed && !external_seek && adjacent && source_is_blank && target_is_blank {
            self.cause = LyricMotionCause::IntentionalBlankContinuation;
            if !animations_enabled || !active_motion_is_interrupted {
                self.cue_motion = None;
            }
            self.semantic.clone_from(&next);
            self.displayed.clone_from(&next);

            return;
        }

        let (kind, cause) = if timeline_changed {
            (None, LyricMotionCause::TimelineRevision)
        } else if external_seek {
            (None, LyricMotionCause::ExternalSeek)
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
            (
                Some(CueMotionKind::Natural),
                LyricMotionCause::NaturalCueHandoff,
            )
        };

        let interrupted_frame = continuing_blank_departure.then(|| self.frame_at(now));
        self.cue_motion = kind.filter(|_| animations_enabled).map(|kind| CueMotion {
            kind,
            source: source.clone(),
            target: target.clone(),
            interrupted_frame,
            started_at: now,
        });
        self.cause = cause;
        self.semantic = next;
        self.displayed = lyrics.cloned();
    }

    pub(crate) fn frame_at(&self, now: Duration) -> LyricFrame {
        let cue_motion_active = self
            .cue_motion
            .as_ref()
            .is_some_and(|motion| motion.is_active_at(now));
        let (cues, anchors) = match self
            .cue_motion
            .as_ref()
            .filter(|motion| motion.is_active_at(now))
        {
            Some(motion) => cue_motion_frame(motion, now),
            None => (
                stable_cues(self.displayed.as_ref()),
                self.displayed
                    .as_ref()
                    .map(|lyrics| vec![(anchor_index(lyrics), 1.0)])
                    .unwrap_or_default(),
            ),
        };
        LyricFrame {
            composition_progress: self.composition.value_at(now),
            cause: self.cause,
            cues,
            anchors,
            cue_motion_active,
            composition_motion_active: self.composition.is_active_at(now),
        }
    }
}

// A blank run shares its first entry's identity, including on direct seeks.
// Preparation uses the position immediately before the first nonblank cue.
fn anchor_index(lyrics: &LyricPresentation) -> i64 {
    if lyrics.preparing {
        lyrics.current_index as i64 - 1
    } else if lyrics.current().trim().is_empty() {
        lyrics.previous_index().map_or(0, |index| index + 1) as i64
    } else {
        lyrics.current_index as i64
    }
}

fn cue_motion_frame(motion: &CueMotion, now: Duration) -> (Vec<LyricCueFrame>, Vec<(i64, f64)>) {
    let progress = phase(motion.progress_at(now), 0.0, 0.78);
    let source = motion
        .interrupted_frame
        .as_ref()
        .map(|frame| frame.cues.clone())
        .unwrap_or_else(|| stable_cues(Some(&motion.source)));
    let target = stable_cues(Some(&motion.target));
    let mut indices: Vec<_> = source.iter().chain(&target).map(|cue| cue.index).collect();
    indices.sort_unstable();
    indices.dedup();
    let cues = indices
        .into_iter()
        .map(|index| {
            let before = source.iter().find(|cue| cue.index == index);
            let after = target.iter().find(|cue| cue.index == index);
            let mut cue = after.or(before).unwrap().clone();
            cue.emphasis = mix(
                before.map_or(0.0, |cue| cue.emphasis),
                after.map_or(0.0, |cue| cue.emphasis),
                progress,
            );
            cue.opacity = mix(
                before.map_or(0.0, |cue| cue.opacity),
                after.map_or(0.0, |cue| cue.opacity),
                progress,
            );
            // Timed blank rows keep their extent in both endpoints. Only the
            // synthetic preparation anchor yields its space as lyrics begin.
            cue.extent = if cue.text.trim().is_empty() {
                mix(
                    before.map_or(0.0, |cue| cue.extent),
                    after.map_or(0.0, |cue| cue.extent),
                    progress,
                )
            } else {
                1.0
            };
            let from = before.map_or(cue.color_weights, |cue| cue.color_weights);
            let to = after.map_or(cue.color_weights, |cue| cue.color_weights);
            cue.color_weights = std::array::from_fn(|index| mix(from[index], to[index], progress));
            cue
        })
        .collect();
    let mut anchors = motion
        .interrupted_frame
        .as_ref()
        .map(|frame| frame.anchors.clone())
        .unwrap_or_else(|| vec![(anchor_index(&motion.source), 1.0)]);
    for (_, weight) in &mut anchors {
        *weight *= 1.0 - progress;
    }
    anchors.push((anchor_index(&motion.target), progress));
    (cues, anchors)
}

fn stable_cues(lyrics: Option<&LyricPresentation>) -> Vec<LyricCueFrame> {
    let Some(lyrics) = lyrics else {
        return Vec::new();
    };
    let anchor = anchor_index(lyrics);
    let mut cues = Vec::new();
    let mut previous_was_blank = true;
    for (index, text) in lyrics.timeline.iter().enumerate() {
        let index = index as i64;
        let blank = text.trim().is_empty();
        let skip = blank && previous_was_blank;
        previous_was_blank = blank;
        // Ignore leading blanks and retain just the first row of each run.
        if skip {
            continue;
        }
        let role = if index < anchor {
            LyricColorRole::Earlier
        } else if index == anchor {
            LyricColorRole::Focal
        } else {
            LyricColorRole::Upcoming
        };
        cues.push(LyricCueFrame {
            role,
            index,
            text: if blank { String::new() } else { text.clone() },
            emphasis: f64::from(index == anchor),
            opacity: f64::from(!blank),
            extent: 1.0,
            color_weights: role.weights(),
        });
    }
    if lyrics.preparing {
        cues.push(LyricCueFrame {
            role: LyricColorRole::Focal,
            index: anchor,
            text: String::new(),
            emphasis: 1.0,
            opacity: 0.0,
            extent: 1.0,
            color_weights: LyricColorRole::Focal.weights(),
        });
    }
    cues.sort_by_key(|cue| cue.index);
    cues
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
            timeline: (0..current_index)
                .map(|index| {
                    if index + 1 == current_index {
                        previous.unwrap_or("").to_owned()
                    } else {
                        String::new()
                    }
                })
                .chain(std::iter::once(current.to_owned()))
                .chain(next.map(str::to_owned))
                .collect(),
            current_index,
            preparing: false,
        }
    }

    #[test]
    fn continuous_snapshot_at_cue_boundary_preserves_reel_lift() {
        let first = lyrics(0, None, "First", Some("Second"));
        let mut second = first.clone();
        second.current_index = 1;
        let mut motion = LyricMotion::new(1, Some(&first));
        motion.observe_playback(1, Some(8.95), true, Duration::ZERO);
        motion.update(1, Some(&first), Duration::ZERO, true);

        let boundary = Duration::from_millis(50);
        motion.observe_playback(2, Some(9.0), true, boundary);
        motion.update(2, Some(&second), boundary, true);
        let frame = motion.frame_at(boundary + Duration::from_millis(150));
        assert_eq!(frame.cause, LyricMotionCause::NaturalCueHandoff);
        assert!(frame.cue_motion_active);
        assert!(frame.cues[1].emphasis > 0.0 && frame.cues[1].emphasis < 1.0);
    }

    #[test]
    fn reel_keeps_context_and_timed_identity_during_a_tall_handoff() {
        let mut first = lyrics(2, Some("Again"), "Short", Some("One\nTwo\nThree\nFour"));
        first.timeline = [
            "Earlier",
            "Again",
            "Short",
            "One\nTwo\nThree\nFour",
            "Again",
            "Later",
        ]
        .map(str::to_owned)
        .to_vec();
        let mut second = first.clone();
        second.current_index = 3;
        let mut motion = LyricMotion::new(1, Some(&first));
        assert!(motion.frame_at(Duration::ZERO).cues.len() > 3);
        motion.update(1, Some(&second), Duration::ZERO, true);
        for millis in [0, 100, 310, 500, 620] {
            let frame = motion.frame_at(Duration::from_millis(millis));
            let outgoing = frame.cues.iter().find(|cue| cue.text == "Short").unwrap();
            assert_eq!(outgoing.opacity, 1.0, "outgoing must leave by geometry");
            assert_eq!(
                frame.cues.iter().filter(|cue| cue.text == "Again").count(),
                2
            );
        }
    }

    #[test]
    fn compact_adjacent_cues_transfer_focal_ownership_without_an_opacity_valley() {
        let first = lyrics(0, None, "Again", Some("Again"));
        let second = lyrics(1, Some("Again"), "Again", Some("After"));
        let mut motion = LyricMotion::new(7, Some(&first));

        motion.update(7, Some(&second), Duration::ZERO, true);
        let midpoint = motion.frame_at(CUE_HANDOFF_DURATION / 2);

        assert_eq!(midpoint.cause, LyricMotionCause::NaturalCueHandoff);
        assert_eq!(midpoint.cues.len(), 3);
        let outgoing = midpoint
            .cues
            .iter()
            .find(|cue| cue.role == LyricColorRole::Earlier)
            .expect("a compact Reel Lift keeps the outgoing cue as visual memory");
        let incoming = midpoint
            .cues
            .iter()
            .find(|cue| cue.role == LyricColorRole::Focal)
            .expect("a compact Reel Lift promotes the incoming cue");
        assert!(
            incoming.emphasis * incoming.opacity >= 0.7,
            "the incoming cue should visibly own focus by midpoint: {incoming:?}"
        );
        assert!(
            incoming.emphasis * incoming.opacity >= outgoing.emphasis * outgoing.opacity + 0.25,
            "midpoint ownership must be decisive: outgoing={outgoing:?}, incoming={incoming:?}"
        );
        let settled = motion.frame_at(CUE_HANDOFF_DURATION);
        assert_eq!(
            settled
                .cues
                .iter()
                .map(|cue| (cue.role, cue.text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (LyricColorRole::Earlier, "Again"),
                (LyricColorRole::Focal, "Again"),
                (LyricColorRole::Upcoming, "After"),
            ]
        );
    }

    #[test]
    fn multiline_endpoints_use_the_same_continuous_handoff() {
        for (source, target) in [
            ("One\nTwo\nThree", "Short"),
            ("Short", "One\nTwo\nThree\nFour"),
        ] {
            let first = lyrics(0, None, source, Some(target));
            let second = lyrics(1, Some(source), target, Some("After"));
            let mut motion = LyricMotion::new(11, Some(&first));

            motion.update(11, Some(&second), Duration::ZERO, true);
            let midpoint = motion.frame_at(CUE_HANDOFF_DURATION / 2);

            assert_eq!(midpoint.cause, LyricMotionCause::NaturalCueHandoff);
            assert!(midpoint.cues[0].emphasis < 0.5);
            assert!(midpoint.cues[1].emphasis > 0.5);
        }
    }

    #[test]
    fn intentional_blank_context_keeps_one_row_throughout_handoffs() {
        let mut presentation = LyricPresentation {
            timeline_signature: 1,
            timeline: ["Before", "", " \n ", "After", "", "Later"]
                .map(str::to_owned)
                .to_vec(),
            current_index: 0,
            preparing: false,
        };
        let mut motion = LyricMotion::new(1, Some(&presentation));
        for current_index in 0..presentation.timeline.len() {
            presentation.current_index = current_index;
            let now = Duration::from_secs(current_index as u64);
            motion.update(1, Some(&presentation), now, true);
            for offset in [0, 100, 310, 620] {
                let frame = motion.frame_at(now + Duration::from_millis(offset));
                let blanks: Vec<_> = frame
                    .cues
                    .iter()
                    .filter(|cue| cue.text.trim().is_empty())
                    .map(|cue| (cue.index, cue.extent, cue.opacity))
                    .collect();
                assert_eq!(blanks, vec![(1, 1.0, 0.0), (4, 1.0, 0.0)]);
            }
            let direct = LyricMotion::new(2, Some(&presentation)).frame_at(now);
            assert_eq!(
                direct.cues,
                motion.frame_at(now + CUE_HANDOFF_DURATION).cues
            );
            assert_eq!(
                direct.anchors,
                vec![(
                    if current_index == 2 {
                        1
                    } else {
                        current_index as i64
                    },
                    1.0
                )]
            );
        }
    }

    #[test]
    fn preparation_keeps_a_separate_anchor_and_ignores_leading_blanks() {
        for leading_blanks in [0, 2] {
            let mut presentation = LyricPresentation {
                timeline_signature: 1,
                timeline: vec![String::new(); leading_blanks],
                current_index: leading_blanks,
                preparing: true,
            };
            presentation
                .timeline
                .extend(["First", "", " \n "].map(str::to_owned));
            let mut motion = LyricMotion::new(1, Some(&presentation));
            let prepared = motion.frame_at(Duration::ZERO);
            let synthetic_index = leading_blanks as i64 - 1;
            assert_eq!(prepared.anchors, vec![(synthetic_index, 1.0)]);
            assert_eq!(prepared.cues.len(), 3);
            assert_eq!(prepared.cues[0].index, synthetic_index);
            assert_eq!(prepared.cues[0].extent, 1.0);
            assert_eq!(prepared.cues[0].opacity, 0.0);

            presentation.preparing = false;
            motion.update(1, Some(&presentation), Duration::ZERO, true);
            let midpoint = motion.frame_at(CUE_HANDOFF_DURATION / 2);
            let synthetic = &midpoint.cues[0];
            assert_eq!(synthetic.index, synthetic_index);
            assert!(synthetic.extent > 0.0 && synthetic.extent < 1.0);
            let settled = motion.frame_at(CUE_HANDOFF_DURATION);
            assert_eq!(settled.cues.len(), 2);
            assert_eq!(settled.cues[0].text, "First");
            assert_eq!(settled.cues[1].text, "");
            assert_eq!(settled.cues[1].extent, 1.0);
            assert_eq!(settled.cues[1].opacity, 0.0);
        }
    }

    #[test]
    fn consecutive_intentional_blanks_form_one_empty_reel_interval() {
        let current = lyrics(0, None, "Before", Some("After"));
        let first_blank = lyrics(1, None, "", None);
        let second_blank = lyrics(2, None, " ", None);
        let mut motion = LyricMotion::new(13, Some(&current));

        motion.update(13, Some(&first_blank), Duration::ZERO, true);
        assert_eq!(
            motion.frame_at(Duration::ZERO).cause,
            LyricMotionCause::IntentionalBlankEntry
        );
        let continued_at = Duration::from_millis(100);
        motion.update(13, Some(&second_blank), continued_at, true);
        let continued = motion.frame_at(continued_at);
        assert_eq!(
            continued.cause,
            LyricMotionCause::IntentionalBlankContinuation
        );
        assert!(continued.cue_motion_active);
        assert_eq!(continued.cues[0].text, "Before");

        let settled = motion.frame_at(BLANK_TRANSITION_DURATION);
        assert!(!settled.cue_motion_active);
        assert!(settled.cues.iter().all(|cue| cue.opacity == 0.0));
    }

    #[test]
    fn authoritative_restart_clears_the_reel_without_cutting_composition_motion() {
        let current = lyrics(1, Some("Earlier"), "Later passage", Some("Upcoming"));
        let mut motion = LyricMotion::new(1, Some(&current));
        motion.observe_playback(1, Some(30.0), true, Duration::ZERO);
        let reset_at = Duration::from_millis(100);
        motion.observe_playback(2, Some(0.0), true, reset_at);
        motion.update(2, None, reset_at, true);

        for offset in [0, 100, 290, 579, 580] {
            let frame = motion.frame_at(reset_at + Duration::from_millis(offset));
            assert!(
                frame.cues.is_empty(),
                "the entire old reel must clear at reset"
            );
            assert!(frame.anchors.is_empty());
            assert!(!frame.cue_motion_active);
            assert_eq!(frame.composition_motion_active, offset < 580);
        }
        assert_eq!(motion.frame_at(reset_at).composition_progress, 1.0);
        assert_eq!(
            motion
                .frame_at(reset_at + Duration::from_millis(290))
                .composition_progress,
            0.5
        );
        assert_eq!(
            motion
                .frame_at(reset_at + Duration::from_millis(580))
                .composition_progress,
            0.0
        );
    }

    #[test]
    fn relocation_interrupts_handoffs_and_retargets_composition_from_current_geometry() {
        let first = lyrics(0, None, "Opening", Some("Later"));
        let mut later = first.clone();
        later.current_index = 1;
        let mut motion = LyricMotion::new(1, Some(&first));
        motion.observe_playback(1, Some(10.0), true, Duration::ZERO);
        motion.update(1, Some(&later), Duration::ZERO, true);
        let reset_at = Duration::from_millis(100);
        assert!(motion.frame_at(reset_at).cue_motion_active);
        motion.observe_playback(2, Some(0.0), true, reset_at);
        motion.update(2, None, reset_at, true);
        assert!(motion.frame_at(reset_at).cues.is_empty());
        assert!(!motion.frame_at(reset_at).cue_motion_active);

        let reversed_at = reset_at + Duration::from_millis(290);
        motion.observe_playback(3, Some(10.0), true, reversed_at);
        motion.update(3, Some(&later), reversed_at, true);
        let reversed = motion.frame_at(reversed_at);
        assert_eq!(reversed.composition_progress, 0.5);
        assert_eq!(reversed.anchors, vec![(1, 1.0)]);
        assert_eq!(reversed.cues[1].emphasis, 1.0);
        assert!(!reversed.cue_motion_active);

        let interrupted_at = reversed_at + Duration::from_millis(100);
        let before = motion.frame_at(interrupted_at).composition_progress;
        motion.observe_playback(4, Some(0.0), true, interrupted_at);
        motion.update(4, None, interrupted_at, true);
        assert_eq!(motion.frame_at(interrupted_at).composition_progress, before);
        assert!(motion.frame_at(interrupted_at).cues.is_empty());
        // A second relocation while the semantic destination remains absent
        // must neither resurrect lyrics nor restart the geometry clock.
        let absent_seek_at = interrupted_at + Duration::from_millis(100);
        motion.observe_playback(5, Some(50.0), true, absent_seek_at);
        motion.update(5, None, absent_seek_at, true);
        let settled = motion.frame_at(interrupted_at + COMPOSITION_TRANSITION_DURATION);
        assert!(settled.cues.is_empty());
        assert_eq!(settled.composition_progress, 0.0);
        assert!(!settled.composition_motion_active);
    }

    #[test]
    fn continuous_exit_and_unavailable_inputs_preserve_the_normal_reel_fade() {
        let current = lyrics(1, Some("Earlier"), "Final", None);
        for (before, after) in [
            (Some(30.0), Some(30.1)),
            (Some(30.0), None),
            (None, Some(0.0)),
        ] {
            let mut motion = LyricMotion::new(1, Some(&current));
            motion.observe_playback(1, before, true, Duration::ZERO);
            let exit_at = Duration::from_millis(100);
            motion.observe_playback(2, after, true, exit_at);
            motion.update(2, None, exit_at, true);
            let frame = motion.frame_at(exit_at);
            assert_eq!(frame.cues[1].text, "Final");
            assert_eq!(frame.cues[1].emphasis, 1.0);
            assert!(frame.composition_motion_active);
            assert_eq!(frame.composition_progress, 1.0);
            assert_eq!(
                motion
                    .frame_at(exit_at + COMPOSITION_TRANSITION_DURATION)
                    .composition_progress,
                0.0
            );
        }
    }

    #[test]
    fn external_seeks_install_the_destination_without_travel() {
        let current = lyrics(0, None, "First", Some("Second"));
        let destination = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(17, Some(&current));

        motion.update(18, Some(&destination), Duration::ZERO, true);
        let frame = motion.frame_at(Duration::ZERO);

        assert_eq!(frame.cause, LyricMotionCause::ExternalSeek);
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Second");
        assert_eq!(frame.cues[1].emphasis, 1.0);
        assert_eq!(frame.cues[1].emphasis, 1.0);
    }

    #[test]
    fn timeline_revisions_that_change_the_selected_cue_are_distinct_from_external_seeks() {
        let current = lyrics_on_timeline(1, 1, Some("First"), "Second", Some("Third"));
        let corrected = lyrics_on_timeline(2, 2, Some("Corrected"), "Third", Some("Fourth"));
        let mut motion = LyricMotion::new(19, Some(&current));

        motion.update(20, Some(&corrected), Duration::ZERO, true);

        let frame = motion.frame_at(Duration::ZERO);
        assert_eq!(frame.cause, LyricMotionCause::TimelineRevision);
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Third");
    }

    #[test]
    fn delayed_local_progression_is_distinct_from_an_external_seek() {
        let first = lyrics(0, None, "First", Some("Second"));
        let third = lyrics(2, Some("Second"), "Third", Some("Fourth"));
        let mut motion = LyricMotion::new(21, Some(&first));

        motion.update(21, Some(&third), Duration::ZERO, true);

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
        let mut motion = LyricMotion::new(23, Some(&first));
        motion.update(23, Some(&second), Duration::ZERO, true);

        motion.update(23, Some(&third), Duration::from_millis(160), true);

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
        let mut motion = LyricMotion::new(29, None);
        motion.update(29, Some(&cue), Duration::ZERO, true);
        let interrupted_at = COMPOSITION_TRANSITION_DURATION / 2;
        let midpoint = motion.frame_at(interrupted_at).composition_progress;
        assert!(midpoint > 0.0 && midpoint < 1.0);

        motion.update(29, None, interrupted_at, true);
        assert_eq!(
            motion.frame_at(interrupted_at).composition_progress,
            midpoint,
            "exit should reverse from the currently rendered geometry"
        );
        assert_eq!(
            motion.frame_at(interrupted_at).cause,
            LyricMotionCause::CompositionExit
        );

        motion.update(30, Some(&cue), interrupted_at, false);
        let reduced = motion.frame_at(interrupted_at);
        assert_eq!(reduced.composition_progress, 1.0);
        assert!(!reduced.composition_motion_active);
        assert_eq!(reduced.cues[0].text, "Opening");
    }

    #[test]
    fn disabling_animation_during_a_natural_cue_handoff_installs_its_semantic_endpoint() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(31, Some(&first));
        motion.update(31, Some(&second), Duration::ZERO, true);

        let disabled_at = Duration::from_millis(100);
        motion.update(31, Some(&second), disabled_at, false);
        let frame = motion.frame_at(disabled_at);

        assert!(!frame.cue_motion_active);
        assert!(!frame.composition_motion_active);
        assert_eq!(frame.cues[1].text, "Second");
        assert_eq!(frame.cues[1].emphasis, 1.0);
        assert_eq!(frame.cues[1].emphasis, 1.0);
    }

    #[test]
    fn reduced_animation_preserves_the_natural_handoff_classification() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(35, Some(&first));

        motion.update(35, Some(&second), Duration::ZERO, false);

        let frame = motion.frame_at(Duration::ZERO);
        assert_eq!(frame.cause, LyricMotionCause::NaturalCueHandoff);
        assert!(!frame.cue_motion_active);
        assert_eq!(frame.cues[1].text, "Second");
        assert_eq!(frame.cues[1].emphasis, 1.0);
    }

    #[test]
    fn an_unchanged_pause_update_allows_the_selected_natural_cue_handoff_to_settle() {
        let first = lyrics(0, None, "First", Some("Second"));
        let second = lyrics(1, Some("First"), "Second", Some("Third"));
        let mut motion = LyricMotion::new(37, Some(&first));
        motion.update(37, Some(&second), Duration::ZERO, true);

        motion.update(38, Some(&second), Duration::from_millis(100), true);

        let settled = motion.frame_at(CUE_HANDOFF_DURATION);
        assert!(!settled.cue_motion_active);
        assert_eq!(settled.cues[1].text, "Second");
        assert_eq!(settled.cues[1].emphasis, 1.0);
    }
}
