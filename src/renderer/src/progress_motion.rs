use std::time::Duration;

use roonscape_renderer::NowPlayingPresentation;

/// Visual fill travel only. Playback position, numerals and lyrics stay with
/// the Presentation Snapshot's timeline, including while travel is unfinished.
#[derive(Default)]
pub(super) struct ProgressMotion {
    destination: f64,
    motion: Option<Travel>,
    sample: Option<Sample>,
}

struct Travel {
    from: f64,
    started: Duration,
    duration: Duration,
}

struct Sample {
    revision: u64,
    position: Option<f64>,
    fraction: Option<f64>,
    advancing: bool,
    at: Duration,
}

impl ProgressMotion {
    pub fn fraction(&self, now: Duration) -> f64 {
        self.motion.as_ref().map_or(self.destination, |travel| {
            let phase = (now.saturating_sub(travel.started).as_secs_f64()
                / travel.duration.as_secs_f64())
            .clamp(0.0, 1.0);
            let eased = phase * phase * (3.0 - 2.0 * phase);
            travel.from + (self.destination - travel.from) * eased
        })
    }

    pub fn active(&self, now: Duration) -> bool {
        self.motion
            .as_ref()
            .is_some_and(|travel| now < travel.started + travel.duration)
    }

    pub fn replace(
        &mut self,
        value: &NowPlayingPresentation,
        revision: u64,
        now: Duration,
        animated: bool,
    ) {
        self.observe(value, revision, now, animated, true);
    }

    pub fn update(
        &mut self,
        value: &NowPlayingPresentation,
        revision: u64,
        now: Duration,
        animated: bool,
    ) {
        self.observe(value, revision, now, animated, false);
    }

    fn observe(
        &mut self,
        value: &NowPlayingPresentation,
        revision: u64,
        now: Duration,
        animated: bool,
        replacement: bool,
    ) {
        let fraction = value
            .progress
            .as_ref()
            .map(|progress| progress.fraction.clamp(0.0, 1.0));
        let discontinuity = self.sample.as_ref().is_some_and(|previous| {
            if previous.fraction.is_some() != fraction.is_some() {
                return true;
            }
            if previous.revision == revision {
                return false;
            }
            let elapsed = if previous.advancing {
                now.saturating_sub(previous.at).as_secs_f64()
            } else {
                0.0
            };
            let positions = previous.position.zip(value.playback_position_seconds);
            let position_jump =
                positions.is_some_and(|(before, after)| (after - before - elapsed).abs() > 0.5);
            // A duration correction can move the visual destination without
            // changing position. Ordinary clock advancement is not a correction.
            let fraction_correction = previous.fraction.zip(fraction).zip(positions).is_some_and(
                |((before, after), (position_before, position_after))| {
                    position_before > 0.0
                        && (after - before * position_after / position_before).abs() > 0.000_001
                },
            );
            position_jump || fraction_correction
        });
        let from = self.fraction(now);
        self.destination = fraction.unwrap_or(0.0);
        if !animated || self.sample.is_none() && !replacement {
            self.motion = None;
        } else if replacement || discontinuity {
            let duration = Duration::from_secs_f64(0.225 * (self.destination - from).abs());
            self.motion = (!duration.is_zero()).then_some(Travel {
                from,
                started: now,
                duration,
            });
        } else if !self.active(now) {
            self.motion = None;
        }
        self.sample = Some(Sample {
            revision,
            position: value.playback_position_seconds,
            fraction,
            advancing: value.status.symbol == roonscape_renderer::PresentationStatusSymbol::Playing,
            at: now,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roonscape_renderer::{Presentation, parse_snapshot, presentation_from_snapshot};

    fn value(fraction: f64) -> NowPlayingPresentation {
        let snapshot = parse_snapshot(include_str!("../../shared/fixtures/playing.json")).unwrap();
        let Presentation::NowPlaying(mut value) = presentation_from_snapshot(&snapshot).unwrap()
        else {
            panic!()
        };
        value.progress.as_mut().unwrap().fraction = fraction;
        value.playback_position_seconds = Some(fraction * 100.0);
        value
    }
    fn ms(value: f64) -> Duration {
        Duration::from_secs_f64(value / 1000.0)
    }
    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn replacement_uses_visual_distance_and_easing() {
        let mut motion = ProgressMotion::default();
        motion.update(&value(0.8), 1, ms(0.0), true);
        motion.replace(&value(0.3), 2, ms(10.0), true);
        close(motion.fraction(ms(10.0)), 0.8);
        close(motion.fraction(ms(38.125)), 0.721875);
        close(motion.fraction(ms(66.25)), 0.55);
        close(motion.fraction(ms(122.5)), 0.3);
        assert!(!motion.active(ms(122.5)));
        motion.replace(&value(0.3), 3, ms(123.0), true);
        assert!(!motion.active(ms(123.0)));
    }

    #[test]
    fn missing_timing_retracts_and_retargets_from_visible_fraction() {
        let mut motion = ProgressMotion::default();
        motion.update(&value(0.8), 1, ms(0.0), true);
        let mut missing = value(0.0);
        missing.progress = None;
        motion.replace(&missing, 2, ms(0.0), true);
        close(motion.fraction(ms(90.0)), 0.4);
        motion.update(&value(0.6), 3, ms(90.0), true);
        close(motion.fraction(ms(90.0)), 0.4);
        close(motion.fraction(ms(112.5)), 0.5);
        close(motion.fraction(ms(135.0)), 0.6);
        motion.replace(&missing, 4, ms(140.0), true);
        close(motion.fraction(ms(275.0)), 0.0);
    }

    #[test]
    fn seeks_adopt_live_destinations_without_restarting_on_ordinary_updates() {
        let mut motion = ProgressMotion::default();
        motion.update(&value(0.2), 1, ms(0.0), true);
        motion.update(&value(0.8), 2, ms(10.0), true);
        close(motion.fraction(ms(10.0)), 0.2);
        motion.update(&value(0.8005), 3, ms(60.0), true);
        motion.update(&value(0.80135), 3, ms(145.0), true);
        close(motion.fraction(ms(145.0)), 0.80135);
        assert!(!motion.active(ms(145.0)));
        let mut paused = value(0.1);
        paused.status.symbol = roonscape_renderer::PresentationStatusSymbol::Paused;
        motion.update(&paused, 4, ms(150.0), true);
        assert!(motion.active(ms(150.0)));
        let visible = motion.fraction(ms(190.0));
        paused.progress.as_mut().unwrap().fraction = 0.9;
        paused.playback_position_seconds = Some(90.0);
        motion.update(&paused, 5, ms(190.0), true);
        close(motion.fraction(ms(190.0)), visible);
        paused.progress.as_mut().unwrap().fraction = 0.0;
        paused.playback_position_seconds = Some(0.0);
        motion.update(&paused, 6, ms(200.0), false);
        close(motion.fraction(ms(200.0)), 0.0);
        assert!(!motion.active(ms(200.0)));
    }

    #[test]
    fn expected_advancement_and_half_second_tolerance_do_not_start_travel() {
        let mut motion = ProgressMotion::default();
        motion.update(&value(0.2), 1, ms(0.0), true);
        motion.update(&value(0.21), 2, ms(1000.0), true);
        assert!(!motion.active(ms(1000.0)));
        motion.update(&value(0.225), 3, ms(2000.0), true);
        assert!(!motion.active(ms(2000.0)));
        motion.update(&value(0.0), 4, ms(2100.0), true);
        close(motion.fraction(ms(2100.0)), 0.225);
        close(motion.fraction(ms(2150.625)), 0.0);
    }
}
