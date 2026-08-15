use std::mem;
use std::time::Duration;

const CROSSFADE_DURATION: Duration = Duration::from_millis(450);

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
}

impl<T> PresentationTransition<T> {
    pub fn new(revision: u64, value: T) -> Self {
        Self {
            current: PresentationRevision { revision, value },
            outgoing: None,
            started_at: None,
        }
    }

    pub fn begin(
        &mut self,
        revision: u64,
        value: T,
        started_at: Duration,
    ) -> Option<PresentationRevision<T>> {
        let discarded = self.outgoing.take();
        let outgoing = mem::replace(&mut self.current, PresentationRevision { revision, value });
        self.outgoing = Some(outgoing);
        self.started_at = Some(started_at);
        discarded
    }

    pub fn discard_outgoing(&mut self) -> Option<PresentationRevision<T>> {
        self.started_at = None;
        self.outgoing.take()
    }

    pub fn finish(&mut self, now: Duration) -> Option<PresentationRevision<T>> {
        let started_at = self.started_at?;
        if now.saturating_sub(started_at) < CROSSFADE_DURATION {
            return None;
        }

        self.started_at = None;
        self.outgoing.take()
    }

    pub fn current(&self) -> &PresentationRevision<T> {
        &self.current
    }

    pub fn outgoing(&self) -> Option<&PresentationRevision<T>> {
        self.outgoing.as_ref()
    }

    pub fn duration(&self) -> Duration {
        CROSSFADE_DURATION
    }

    pub fn is_active(&self) -> bool {
        self.outgoing.is_some()
    }
}
