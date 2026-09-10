/// Selects evenly spaced display refreshes without accumulating delayed work.
#[derive(Debug, Default)]
pub struct FramePacing {
    last_update_micros: Option<i64>,
}

impl FramePacing {
    /// Select from delivered display frames. Below the ceiling, every new
    /// frame is eligible even when CPU scheduling makes callbacks bunch up.
    pub fn update_due(&mut self, frame_micros: i64, refresh_millihertz: u32) -> bool {
        let rate = refresh_millihertz.max(1);
        let stride = rate.div_ceil(60_000);
        // Decide between refreshes, not at a floating-point deadline lying on
        // the expected refresh itself. Sub-refresh timestamp jitter therefore
        // cannot alternate the selected stride.
        if let Some(last) = self.last_update_micros {
            if frame_micros <= last {
                return false;
            }
            if rate > 60_000
                && (i128::from(frame_micros) - i128::from(last)) * i128::from(rate) * 2
                    < i128::from(2 * stride - 1) * 1_000_000_000
            {
                return false;
            }
        }
        self.last_update_micros = Some(frame_micros);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_even_refreshes_under_the_ceiling() {
        for (rate, stride) in [
            (24_000, 1),
            (30_000, 1),
            (50_000, 1),
            (59_940, 1),
            (60_000, 1),
            (60_001, 2),
            (75_000, 2),
            (119_880, 2),
            (120_000, 2),
            (120_001, 3),
            (144_000, 3),
            (165_000, 3),
            (240_000, 4),
        ] {
            let mut pacing = FramePacing::default();
            let delivered: Vec<_> = (0..240)
                .filter(|frame| {
                    let micros =
                        (f64::from(*frame) * 1_000_000_000.0 / f64::from(rate)).round() as i64;
                    pacing.update_due(micros, rate)
                })
                .collect();
            assert_eq!(
                delivered,
                (0..240).step_by(stride).collect::<Vec<_>>(),
                "{rate} mHz"
            );
        }
    }

    #[test]
    fn follows_every_delivered_frame_at_or_below_the_ceiling() {
        for rate in [24_000, 30_000, 50_000, 59_940, 60_000] {
            let mut pacing = FramePacing::default();
            // Display callbacks can arrive late and then close together while
            // queued frames continue reaching consecutive physical refreshes.
            for time in [0, 30_143, 37_842, 50_000] {
                assert!(pacing.update_due(time, rate), "{rate} mHz at {time}");
            }
            assert!(!pacing.update_due(50_000, rate));
        }
    }

    #[test]
    fn timestamp_jitter_does_not_change_the_refresh_stride() {
        let mut pacing = FramePacing::default();
        let delivered: Vec<_> = (0..120)
            .filter(|frame| {
                let jitter = if frame % 2 == 0 { 90 } else { -90 };
                pacing.update_due(frame * 1_000_000 / 120 + jitter, 120_000)
            })
            .collect();
        assert_eq!(delivered, (0..120).step_by(2).collect::<Vec<_>>());
    }

    #[test]
    fn missed_updates_do_not_queue_old_frames_or_replay_the_same_time() {
        let mut pacing = FramePacing::default();
        assert!(pacing.update_due(0, 60_000));
        assert!(pacing.update_due(100_000, 60_000));
        assert!(!pacing.update_due(100_000, 60_000));
        assert!(!pacing.update_due(90_000, 60_000));
        assert!(pacing.update_due(116_667, 60_000));
    }

    #[test]
    fn refresh_changes_adopt_the_new_cadence() {
        let mut pacing = FramePacing::default();
        assert!(pacing.update_due(0, 60_000));
        assert!(pacing.update_due(16_667, 120_000));
        assert!(!pacing.update_due(25_000, 120_000));
        assert!(pacing.update_due(33_333, 120_000));
        assert!(pacing.update_due(50_000, 50_000));
        assert!(pacing.update_due(70_000, 50_000));
    }
}
