use std::mem;
use std::time::Duration;

const FADE_PHASE_DURATION: Duration = Duration::from_millis(225);
const PRESENTATION_TRANSITION_DURATION: Duration = Duration::from_millis(450);

/// Retains the displayed value until it is invisible, then reveals the latest target.
#[derive(Debug)]
pub struct ReplacementFade<T> {
    displayed: T,
    pending: Option<T>,
    started_at: Option<Duration>,
    from_opacity: f64,
    opacity: f64,
}

impl<T: PartialEq> ReplacementFade<T> {
    pub fn new(displayed: T) -> Self {
        Self {
            displayed,
            pending: None,
            started_at: None,
            from_opacity: 1.0,
            opacity: 1.0,
        }
    }

    /// Returns true when the displayed value changes. A dynamic swap is always invisible.
    pub fn update(&mut self, target: T, now: Duration, animated: bool) -> bool {
        if !animated {
            let changed = self.displayed != target;
            self.displayed = target;
            self.pending = None;
            self.started_at = None;
            self.opacity = 1.0;
            return changed;
        }
        if self.pending.as_ref().unwrap_or(&self.displayed) != &target {
            self.pending = (target != self.displayed).then_some(target);
            self.started_at = Some(now);
            self.from_opacity = self.opacity;
        }
        let Some(started_at) = self.started_at else {
            return false;
        };
        let phase = (now.saturating_sub(started_at).as_secs_f64()
            / FADE_PHASE_DURATION.as_secs_f64())
        .min(1.0);
        let eased = phase * phase * (3.0 - 2.0 * phase);
        if self.pending.is_some() {
            self.opacity = self.from_opacity * (1.0 - eased);
            if phase == 1.0 {
                self.displayed = self.pending.take().expect("pending replacement");
                self.from_opacity = 0.0;
                // Start at this frame so even a delayed tick swaps at zero opacity.
                self.started_at = Some(now);
                return true;
            }
        } else {
            self.opacity = self.from_opacity + (1.0 - self.from_opacity) * eased;
            if phase == 1.0 {
                self.started_at = None;
            }
        }
        false
    }

    pub fn displayed(&self) -> &T {
        &self.displayed
    }

    pub fn opacity(&self) -> f64 {
        self.opacity
    }
}

#[derive(Debug)]
pub struct PresentationRevision<T> {
    revision: u64,
    value: T,
}

impl<T> PresentationRevision<T> {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn value(&self) -> &T {
        &self.value
    }
}

#[derive(Debug)]
pub struct PresentationTransition<T> {
    current: PresentationRevision<T>,
    outgoing: Option<PresentationRevision<T>>,
    started_at: Option<Duration>,
    duration: Duration,
}

impl<T> PresentationTransition<T> {
    pub fn new(revision: u64, value: T) -> Self {
        Self {
            current: PresentationRevision { revision, value },
            outgoing: None,
            started_at: None,
            duration: PRESENTATION_TRANSITION_DURATION,
        }
    }

    pub fn begin(
        &mut self,
        revision: u64,
        value: T,
        started_at: Duration,
    ) -> Option<PresentationRevision<T>> {
        self.begin_with_duration(
            revision,
            value,
            started_at,
            PRESENTATION_TRANSITION_DURATION,
        )
    }

    pub fn begin_with_duration(
        &mut self,
        revision: u64,
        value: T,
        started_at: Duration,
        duration: Duration,
    ) -> Option<PresentationRevision<T>> {
        self.duration = duration;
        let discarded = self.outgoing.take();
        let outgoing = mem::replace(&mut self.current, PresentationRevision { revision, value });
        self.outgoing = Some(outgoing);
        self.started_at = Some(started_at);
        discarded
    }

    pub fn replace_immediately(&mut self, revision: u64, value: T) -> Vec<PresentationRevision<T>> {
        let mut released = self.outgoing.take().into_iter().collect::<Vec<_>>();
        released.push(mem::replace(
            &mut self.current,
            PresentationRevision { revision, value },
        ));
        self.started_at = None;
        released
    }

    pub fn discard_outgoing(&mut self) -> Option<PresentationRevision<T>> {
        self.started_at = None;
        self.outgoing.take()
    }

    pub fn finish(&mut self, now: Duration) -> Option<PresentationRevision<T>> {
        let started_at = self.started_at?;
        if now.saturating_sub(started_at) < self.duration {
            return None;
        }

        self.started_at = None;
        self.outgoing.take()
    }

    pub fn current(&self) -> &PresentationRevision<T> {
        &self.current
    }

    pub fn update_current(&mut self, revision: u64, update: impl FnOnce(&mut T)) {
        self.current.revision = revision;
        update(&mut self.current.value);
    }

    pub fn outgoing(&self) -> Option<&PresentationRevision<T>> {
        self.outgoing.as_ref()
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn progress(&self, now: Duration) -> f64 {
        self.started_at.map_or(1.0, |start| {
            (now.saturating_sub(start).as_secs_f64() / self.duration.as_secs_f64()).min(1.0)
        })
    }

    pub fn is_active(&self) -> bool {
        self.outgoing.is_some()
    }
}
