use std::time::Duration;

use roonscape_renderer::{
    NowPlayingLayout, Presentation, PresentationActivity, PresentationPalette, PresentationStatus,
    PresentationStatusSymbol, ReplacementFade,
};

use crate::lyric_motion::LyricMotion;
use crate::prepared_presentation::{PreparedContent, PreparedPresentation};
use crate::qt_window::{Graphic, Scene};
use crate::scene::{self, Foreground};

#[path = "selective_foreground.rs"]
mod selective_foreground;
use selective_foreground::SelectiveForeground;

const PHASE: Duration = Duration::from_millis(225);

type Metadata = (Option<String>, Option<String>, Option<String>);

fn metadata(presentation: &Presentation) -> Metadata {
    match presentation {
        Presentation::NowPlaying(value) => (
            value.title.clone(),
            value.artist.clone(),
            value.album.clone(),
        ),
        Presentation::FullField(_) => (None, None, None),
    }
}

fn status(presentation: &Presentation) -> PresentationStatus {
    match presentation {
        Presentation::NowPlaying(value) => value.status,
        Presentation::FullField(value) => value.status,
    }
}

#[derive(Clone, PartialEq)]
enum Timing {
    Progress,
    Activity(Box<PresentationActivity>),
    Quiet,
}

fn timing(presentation: &Presentation) -> Timing {
    match presentation {
        Presentation::NowPlaying(value) if value.progress.is_some() => Timing::Progress,
        Presentation::NowPlaying(value) => value
            .activity
            .clone()
            .map_or(Timing::Quiet, Timing::Activity),
        Presentation::FullField(_) => Timing::Quiet,
    }
}

/// An interrupted artwork fade retains its live blend, including the previous
/// fade's clock. Completed branches disappear as soon as they become occluded.
#[derive(Clone)]
enum Graphics<'window> {
    Single(Box<PreparedPresentation<'window>>),
    Blend {
        previous: Box<Self>,
        incoming: Box<Self>,
        started: Duration,
        duration: Duration,
        previous_composition: Option<f64>,
    },
}

impl<'window> Graphics<'window> {
    fn current(&self) -> &PreparedPresentation<'window> {
        match self {
            Self::Single(value) => value,
            Self::Blend { incoming, .. } => incoming.current(),
        }
    }

    fn palette(&self, now: Duration) -> PresentationPalette {
        match self {
            Self::Single(value) => value.palette,
            Self::Blend {
                previous,
                incoming,
                started,
                duration,
                ..
            } => previous
                .palette(now)
                .mix(incoming.palette(now), phase(now, *started, *duration)),
        }
    }

    fn append(
        &mut self,
        output: &mut Vec<Graphic<'window>>,
        progress: f64,
        weight: f32,
        now: Duration,
    ) {
        if let Self::Blend {
            incoming,
            started,
            duration,
            ..
        } = self
            && now.saturating_sub(*started) >= *duration
        {
            *self = (**incoming).clone();
        }
        if weight <= 0.0 {
            return;
        }
        match self {
            Self::Single(prepared) => {
                let layout = match &prepared.presentation {
                    Presentation::NowPlaying(value) => {
                        Some(NowPlayingLayout::for_composition_progress(
                            value,
                            prepared.viewport,
                            scene::composition_geometry(progress),
                        ))
                    }
                    Presentation::FullField(_) => None,
                };
                output.push(prepared.graphic(layout.as_ref(), weight));
            }
            Self::Blend {
                previous,
                incoming,
                started,
                duration,
                previous_composition,
            } => {
                let amount = phase(now, *started, *duration) as f32;
                previous.append(
                    output,
                    previous_composition.unwrap_or(progress),
                    weight * (1.0 - amount),
                    now,
                );
                incoming.append(output, progress, weight * amount, now);
            }
        }
    }

    fn active(&self) -> bool {
        matches!(self, Self::Blend { .. })
    }
}

fn phase(now: Duration, start: Duration, duration: Duration) -> f64 {
    scene::motion_phase(
        now.saturating_sub(start).as_secs_f64() / duration.as_secs_f64(),
        0.0,
        1.0,
    )
}

struct Departure {
    started: Duration,
    opacity: f32,
}

struct Reveal<'window> {
    previous: Graphics<'window>,
    previous_composition: f64,
    started: Duration,
    full_field: bool,
    same_art: bool,
}

pub(crate) struct NativeView<'window> {
    prepared: PreparedPresentation<'window>,
    latest: Presentation,
    fields: SelectiveForeground<'window>,
    pending_activity: Option<
        Option<(
            crate::text_preparation::PreparedText<'window>,
            crate::text_preparation::PreparedText<'window>,
        )>,
    >,
    graphics: Graphics<'window>,
    status: ReplacementFade<PresentationStatus>,
    timing: ReplacementFade<Timing>,
    last_progress: Option<roonscape_renderer::PresentationProgress>,
    lyrics: LyricMotion,
    departure: Option<Departure>,
    reveal: Option<Reveal<'window>>,
    last_frame: Duration,
    text_opacity: f32,
    revision: u64,
}

impl<'window> NativeView<'window> {
    pub fn new(prepared: PreparedPresentation<'window>, revision: u64) -> Self {
        let lyrics = match &prepared.presentation {
            Presentation::NowPlaying(value) => value.lyrics.as_deref(),
            _ => None,
        };
        Self {
            latest: prepared.presentation.clone(),
            fields: SelectiveForeground::new(&prepared.presentation),
            pending_activity: None,
            graphics: Graphics::Single(Box::new(prepared.clone())),
            status: ReplacementFade::new(status(&prepared.presentation)),
            timing: ReplacementFade::new(timing(&prepared.presentation)),
            last_progress: None,
            lyrics: LyricMotion::new(revision, lyrics),
            prepared,
            departure: None,
            reveal: None,
            last_frame: Duration::ZERO,
            text_opacity: 1.0,
            revision,
        }
    }

    pub fn begin_departure(&mut self, destination: &Presentation, now: Duration, animated: bool) {
        if matches!(self.latest, Presentation::NowPlaying(_)) {
            self.fields.request(destination, now, animated);
        }
        if self.departure.is_none() {
            self.departure = Some(Departure {
                started: now,
                opacity: if animated { self.text_opacity } else { 0.0 },
            });
        }
    }

    pub fn departure_complete(&self, now: Duration, animated: bool) -> bool {
        !animated || now >= self.reveal_at(Duration::ZERO)
    }

    pub fn reveal_at(&self, prepared_at: Duration) -> Duration {
        let deadline = self.departure.as_ref().map_or(prepared_at, |departure| {
            prepared_at.max(departure.started + PHASE)
        });
        deadline.max(self.fields.departure_deadline().unwrap_or(deadline))
    }

    pub fn replace(
        &mut self,
        prepared: PreparedPresentation<'window>,
        presentation: &Presentation,
        revision: u64,
        now: Duration,
        animated: bool,
    ) {
        let inherited = self.lyrics.inherit_composition(self.last_frame, now);
        let previous_composition = self.lyrics.frame_at(self.last_frame).composition_progress;
        let selective = matches!(
            (&self.latest, presentation),
            (Presentation::NowPlaying(_), Presentation::NowPlaying(_))
        );
        let same_art = same_artwork(
            &self.graphics.current().presentation,
            &prepared.presentation,
        );
        let mut incoming = Self::new(prepared, revision);
        if selective {
            incoming.fields =
                std::mem::replace(&mut self.fields, SelectiveForeground::new(presentation));
            incoming.fields.request(presentation, now, animated);
            incoming.fields.reflow();
            incoming.fields.reveal_prepared(presentation, now);
        }
        if animated && !selective && matches!(presentation, Presentation::NowPlaying(_)) {
            incoming.fields.reveal_all(now);
        }
        incoming.latest = presentation.clone();
        if animated {
            incoming.lyrics = inherited;
            let previous = std::mem::replace(
                &mut self.graphics,
                Graphics::Single(Box::new(incoming.prepared.clone())),
            );
            // A previous reveal can still be visible. Preserve its weighted
            // appearance as a continuing graphics fade before starting this one.
            let previous = if let Some(reveal) = self.reveal.take() {
                Graphics::Blend {
                    previous: Box::new(reveal.previous),
                    incoming: Box::new(previous),
                    started: reveal.started,
                    duration: PHASE,
                    previous_composition: Some(reveal.previous_composition),
                }
            } else {
                previous
            };
            if same_art {
                incoming.graphics = previous.clone();
            }
            incoming.reveal = Some(Reveal {
                previous,
                previous_composition,
                started: now,
                full_field: matches!(presentation, Presentation::FullField(_)),
                same_art,
            });
            incoming.text_opacity = 0.0;
        }
        *self = incoming;
    }

    pub fn install(
        &mut self,
        mut prepared: PreparedPresentation<'window>,
        now: Duration,
        animated: bool,
    ) {
        self.prepared.diagnostics = prepared.diagnostics.clone();
        let previous = self.graphics.current();
        let same_art = same_artwork(&previous.presentation, &prepared.presentation);
        if !same_art {
            let previous = std::mem::replace(
                &mut self.graphics,
                Graphics::Single(Box::new(prepared.clone())),
            );
            if animated {
                self.graphics = Graphics::Blend {
                    previous: Box::new(previous),
                    incoming: Box::new(Graphics::Single(Box::new(prepared.clone()))),
                    started: now,
                    duration: PHASE,
                    previous_composition: None,
                };
            }
        } else if previous.viewport != prepared.viewport {
            self.graphics = Graphics::Single(Box::new(prepared.clone()));
        }
        if animated
            && let (PreparedContent::NowPlaying(current), PreparedContent::NowPlaying(incoming)) =
                (&self.prepared.content, &mut prepared.content)
            && incoming.reel.is_none()
        {
            incoming.reel = current.reel.clone();
        }
        if timing(&prepared.presentation) != *self.timing.displayed()
            && let (PreparedContent::NowPlaying(current), PreparedContent::NowPlaying(incoming)) =
                (&self.prepared.content, &mut prepared.content)
        {
            self.pending_activity = Some(incoming.activity.take());
            incoming.activity = current.activity.clone();
        }
        if displayed_fields_changed(&prepared.presentation, &self.prepared.presentation) {
            self.fields.reflow();
        }
        self.prepared = prepared;
    }

    pub fn update(&mut self, presentation: &Presentation, revision: u64) {
        self.latest = presentation.clone();
        self.revision = revision;
    }

    pub fn render(&mut self, now: Duration, animated: bool) -> Scene<'window> {
        if !animated {
            if matches!(self.graphics, Graphics::Blend { .. }) {
                self.graphics = Graphics::Single(Box::new(self.graphics.current().clone()));
            }
            self.reveal = None;
            if let Some(departure) = &mut self.departure {
                departure.opacity = 0.0;
            }
        }
        self.status.retarget(status(&self.latest), now, animated);
        if self.departure.is_none() {
            self.fields.request(&self.latest, now, animated);
        }
        if self.timing.update(
            timing(&self.latest),
            now,
            animated && !matches!(timing(&self.latest), Timing::Progress),
        ) && let Some(activity) = self.pending_activity.take()
            && let PreparedContent::NowPlaying(current) = &mut self.prepared.content
        {
            current.activity = activity;
        }
        if let Presentation::NowPlaying(value) = &self.latest {
            if let Some(progress) = &value.progress {
                self.last_progress = Some(progress.clone());
            }
            self.lyrics.observe_playback(
                self.revision,
                value.playback_position_seconds,
                value.status.symbol == PresentationStatusSymbol::Playing,
                now,
            );
            self.lyrics.update_with_information(
                self.revision,
                value.lyrics.as_deref(),
                value.lyrics_known,
                now,
                animated,
            );
        }
        let lyric_frame = self.lyrics.frame_at(now);
        let mut scene = Scene::default();
        let mut weight = 1.0;
        let mut opacity = 1.0;
        if let Some(reveal) = &mut self.reveal {
            weight = phase(now, reveal.started, PHASE) as f32;
            if !reveal.same_art {
                reveal.previous.append(
                    &mut scene.graphics,
                    reveal.previous_composition,
                    1.0 - weight,
                    now,
                );
            }
            opacity = if reveal.full_field {
                phase(now, reveal.started + PHASE, PHASE) as f32
            } else {
                weight
            };
            if reveal.same_art {
                weight = 1.0;
            }
            if now.saturating_sub(reveal.started)
                >= if reveal.full_field { PHASE * 2 } else { PHASE }
            {
                self.reveal = None;
            }
        }
        self.graphics.append(
            &mut scene.graphics,
            lyric_frame.composition_progress,
            weight,
            now,
        );
        if let Some(departure) = &self.departure {
            opacity = departure.opacity * (1.0 - phase(now, departure.started, PHASE) as f32);
        }
        self.text_opacity = opacity;
        let mut presentation = self.latest.clone();
        if let Presentation::NowPlaying(value) = &mut presentation {
            match self.timing.displayed() {
                Timing::Progress => {
                    value.progress = self.last_progress.clone();
                    value.activity = None;
                }
                Timing::Activity(activity) => {
                    value.progress = None;
                    value.activity = Some(activity.clone());
                }
                Timing::Quiet => {
                    value.progress = None;
                    value.activity = None;
                }
            }
        }
        scene::foreground(
            &mut scene,
            &self.prepared,
            &Foreground {
                presentation: &presentation,
                palette: self.palette(now),
                now,
                animated,
                opacity,
                status: if matches!(presentation, Presentation::NowPlaying(_)) {
                    status(&self.latest)
                } else {
                    *self.status.displayed()
                },
                status_opacity: self.status.opacity() as f32,
                timing_opacity: self.timing.opacity() as f32,
                lyrics: Some(&lyric_frame),
            },
        );
        if matches!(presentation, Presentation::NowPlaying(_)) {
            let palette = self.palette(now);
            self.fields
                .apply(&mut scene, &presentation, now, animated, palette);
        }
        if let Some(text) = &self.prepared.diagnostics {
            scene::diagnostics(&mut scene, text, self.palette(now), self.prepared.viewport);
        }
        self.last_frame = now;
        scene
    }

    fn palette(&self, now: Duration) -> PresentationPalette {
        let current = self.graphics.palette(now);
        self.reveal.as_ref().map_or(current, |reveal| {
            reveal
                .previous
                .palette(now)
                .mix(current, phase(now, reveal.started, PHASE))
        })
    }

    pub fn active(&self, now: Duration) -> bool {
        self.departure.is_some()
            || self.reveal.is_some()
            || self.graphics.active()
            || self.fields.active()
            || self.status.is_active()
            || self.timing.is_active()
            || self.lyrics.is_active_at(now)
    }

    pub fn lyrics_ready(&self, presentation: &Presentation) -> bool {
        let Presentation::NowPlaying(value) = presentation else {
            return true;
        };
        let Some(lyrics) = &value.lyrics else {
            return true;
        };
        let PreparedContent::NowPlaying(content) = &self.prepared.content else {
            return false;
        };
        let Some(reel) = &content.reel else {
            return false;
        };
        reel.timeline_signature == lyrics.timeline_signature
            && [
                Some(lyrics.current_index),
                lyrics.previous_index(),
                lyrics.next_index(),
            ]
            .into_iter()
            .flatten()
            .all(|index| lyrics.timeline[index].trim().is_empty() || reel.cues.contains_key(&index))
    }

    pub fn visible_lyrics(&self) -> bool {
        let frame = self.lyrics.frame_at(self.last_frame);
        self.text_opacity > 0.0
            && frame.composition_progress > 0.35
            && frame
                .cues
                .iter()
                .any(|cue| !cue.text.trim().is_empty() && cue.opacity > 0.0)
    }
}

fn same_artwork(a: &Presentation, b: &Presentation) -> bool {
    match (a, b) {
        (Presentation::NowPlaying(a), Presentation::NowPlaying(b)) => {
            a.artwork_path == b.artwork_path && a.artwork_revision == b.artwork_revision
        }
        (Presentation::FullField(_), Presentation::FullField(_)) => true,
        _ => false,
    }
}

fn displayed_fields_changed(a: &Presentation, b: &Presentation) -> bool {
    metadata(a) != metadata(b)
        || matches!((a,b), (Presentation::NowPlaying(a), Presentation::NowPlaying(b)) if a.tracked_output != b.tracked_output || a.tracked_zone != b.tracked_zone)
}
