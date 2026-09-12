use std::time::Duration;

use roonscape_renderer::{
    Presentation, PresentationPalette, PresentationStatus, PresentationStatusEmphasis,
};

use crate::qt_window::{Color, Field, Rect, Scene, Sprite};

use super::{PHASE, phase};

#[derive(Clone, PartialEq)]
enum FieldValue {
    Text(Option<String>),
    Status(PresentationStatus),
    Furniture,
    Absent,
}

impl Field {
    const ALL: [Self; 9] = [
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::Status,
        Self::OutputLabel,
        Self::Output,
        Self::ZoneLabel,
        Self::Zone,
        Self::Separator,
    ];

    fn value(self, presentation: &Presentation) -> FieldValue {
        let Presentation::NowPlaying(value) = presentation else {
            return FieldValue::Absent;
        };
        match self {
            Self::Title => FieldValue::Text(value.title.clone()),
            Self::Artist => FieldValue::Text(value.artist.clone()),
            Self::Album => FieldValue::Text(value.album.clone()),
            Self::Status => FieldValue::Status(value.status),
            Self::Output => FieldValue::Text(Some(value.tracked_output.clone())),
            Self::Zone => FieldValue::Text(Some(value.tracked_zone.clone())),
            Self::OutputLabel | Self::ZoneLabel | Self::Separator => FieldValue::Furniture,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Reflow {
    None,
    Retained,
    Replacement,
}

struct FieldState<'window> {
    field: Field,
    displayed: FieldValue,
    target: FieldValue,
    departure: Option<(Duration, f32)>,
    reveal: Option<(Duration, f32)>,
    opacity: f32,
    visible: Vec<Sprite<'window>>,
    movement: Option<(Duration, Vec<Rect>)>,
    reflow: Reflow,
}

pub(crate) struct SelectiveForeground<'window> {
    fields: Vec<FieldState<'window>>,
}

impl<'window> SelectiveForeground<'window> {
    pub fn new(presentation: &Presentation) -> Self {
        Self {
            fields: Field::ALL
                .into_iter()
                .map(|field| FieldState {
                    field,
                    displayed: field.value(presentation),
                    target: field.value(presentation),
                    departure: None,
                    reveal: None,
                    opacity: 1.0,
                    visible: Vec::new(),
                    movement: None,
                    reflow: Reflow::None,
                })
                .collect(),
        }
    }

    /// Receives actual incoming values even while their graphics are preparing.
    pub fn request(&mut self, presentation: &Presentation, now: Duration, animated: bool) {
        for state in &mut self.fields {
            let target = state.field.value(presentation);
            if !animated {
                state.displayed = target.clone();
                state.target = target;
                state.departure = None;
                state.reveal = None;
                state.movement = None;
                state.opacity = 1.0;
            } else if state.target != target {
                state.target = target;
                if state.target == state.displayed {
                    state.departure = None;
                    state.reveal = Some((now, state.opacity));
                } else if state.departure.is_none() {
                    state.departure = Some((now, state.opacity));
                    state.reveal = None;
                }
            }
        }
    }

    pub fn reveal_prepared(&mut self, presentation: &Presentation, now: Duration) {
        // A prepared destination uses one reveal clock for all changed fields.
        if self.fields.iter().any(|state| {
            state.departure.is_some_and(|(start, _)| {
                now.saturating_sub(start) < PHASE || state.field.value(presentation) != state.target
            })
        }) {
            return;
        }
        for state in &mut self.fields {
            if state
                .departure
                .is_some_and(|(start, _)| now.saturating_sub(start) >= PHASE)
                && state.field.value(presentation) == state.target
            {
                state.displayed.clone_from(&state.target);
                state.departure = None;
                state.reveal = Some((now, 0.0));
                state.opacity = 0.0;
                state.movement = None;
                // Incoming and retained bounds follow the same layout motion,
                // preserving the gaps between them while wrapping changes.
                state.reflow = Reflow::Replacement;
            }
        }
    }

    pub fn reveal_all(&mut self, now: Duration) {
        for state in &mut self.fields {
            state.opacity = 0.0;
            state.reveal = Some((now, 0.0));
        }
    }

    pub fn departure_deadline(&self) -> Option<Duration> {
        self.fields
            .iter()
            .filter_map(|state| state.departure.map(|(start, _)| start + PHASE))
            .max()
    }

    pub fn reflow(&mut self) {
        for state in &mut self.fields {
            state.reflow = Reflow::Retained;
        }
    }

    /// The supplied scene contains only prepared content. A departure may finish
    /// before preparation; hold its zero-opacity source until this scene matches
    /// the latest destination. No obsolete destination can pass that gate.
    pub fn apply(
        &mut self,
        scene: &mut Scene<'window>,
        presentation: &Presentation,
        now: Duration,
        animated: bool,
        palette: PresentationPalette,
    ) {
        self.reveal_prepared(presentation, now);
        let departing = self.fields.iter().any(|state| state.departure.is_some());
        for state in &mut self.fields {
            let mut incoming = Vec::new();
            scene.sprites.retain(|sprite| {
                if sprite.field == Some(state.field) {
                    incoming.push(sprite.clone());
                    false
                } else {
                    true
                }
            });
            if let Some((start, from)) = state.departure {
                state.opacity = from * (1.0 - phase(now, start, PHASE) as f32);
            }
            if let Some((start, from)) = state.reveal {
                let amount = phase(now, start, PHASE) as f32;
                state.opacity = from + (1.0 - from) * amount;
                if amount == 1.0 {
                    state.reveal = None;
                }
            }
            let holding = state.departure.is_some()
                || state.field.value(presentation) != state.displayed
                || (departing && state.reflow == Reflow::Retained);
            if holding {
                // Retain the shaped source with its intrinsic alpha. Colors
                // still follow the live graphics blend during departure.
                incoming = state.visible.clone();
            } else if state.reflow != Reflow::None {
                if animated && state.visible.len() == incoming.len() {
                    state.movement = Some((
                        now,
                        state
                            .visible
                            .iter()
                            .zip(&incoming)
                            .map(|(source, destination)| {
                                let source = source.geometry.bounds;
                                if state.reflow == Reflow::Replacement {
                                    fit_replacement(source, destination.geometry.bounds)
                                } else {
                                    source
                                }
                            })
                            .collect(),
                    ));
                }
                state.reflow = Reflow::None;
            }
            if !holding && let Some((start, bounds)) = &state.movement {
                let amount = phase(now, *start, PHASE) as f32;
                for (sprite, source) in incoming.iter_mut().zip(bounds) {
                    sprite.geometry.bounds = mix_rect(*source, sprite.geometry.bounds, amount);
                }
                if amount == 1.0 {
                    state.movement = None;
                }
            }
            let rgb = match state.field {
                Field::Title => palette.primary_text,
                Field::Artist | Field::Album => palette.secondary_text,
                Field::Output | Field::Zone => palette.identity_name_text(),
                Field::OutputLabel | Field::ZoneLabel | Field::Separator => palette.muted_text,
                Field::Status => match &state.displayed {
                    FieldValue::Status(status)
                        if status.emphasis == PresentationStatusEmphasis::MutedAccent =>
                    {
                        palette.status_muted_accent
                    }
                    _ => palette.accent,
                },
            };
            for sprite in &mut incoming {
                sprite.geometry.color = Color::new(rgb, sprite.geometry.color.alpha);
            }
            state.visible = incoming.clone();
            for sprite in &mut incoming {
                sprite.geometry.color.alpha *= state.opacity;
            }
            scene.sprites.extend(incoming);
        }
    }

    pub fn active(&self) -> bool {
        self.fields.iter().any(|state| {
            state.departure.is_some() || state.reveal.is_some() || state.movement.is_some()
        })
    }
}

fn mix_rect(a: Rect, b: Rect, amount: f32) -> Rect {
    let mix = |a, b| a + (b - a) * amount;
    Rect::new(
        mix(a.x, b.x),
        mix(a.y, b.y),
        mix(a.width, b.width),
        mix(a.height, b.height),
    )
}

// Fit a replacement into its source slot without stretching its letterforms.
// Keeping the lower edge preserves clearance from the next retained field.
fn fit_replacement(source: Rect, destination: Rect) -> Rect {
    let scale = (source.width / destination.width.max(1.0))
        .min(source.height / destination.height.max(1.0));
    Rect::new(
        source.x,
        source.y + source.height - destination.height * scale,
        destination.width * scale,
        destination.height * scale,
    )
}
