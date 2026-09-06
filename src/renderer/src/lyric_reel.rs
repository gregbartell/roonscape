use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::{cairo, pango, prelude::*};
use roonscape_renderer::{NowPlayingTypography, PresentationPalette, Rgb};

use crate::lyric_motion::{LyricCueFrame, LyricFrame};

// Pango fits each cue before motion. Painting scales that same layout,
// so a cue never rewraps as it changes size during a Natural Cue Handoff.
pub(crate) struct LyricReel {
    pub widget: gtk::DrawingArea,
    frame: RefCell<Option<LyricFrame>>,
    metrics: Cell<Option<(i32, i32, u32)>>,
    layouts: RefCell<HashMap<String, FittedCue>>,
    typography: Cell<NowPlayingTypography>,
    palette: Cell<PresentationPalette>,
    supporting_family: &'static str,
}

#[derive(Clone)]
struct FittedCue {
    layout: pango::Layout,
    scale: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct PositionedCue {
    pub index: i64,
    pub y: f64,
    pub height: f64,
    pub scale: f64,
    pub cue: LyricCueFrame,
    pub color: Rgb,
    pub layout: pango::Layout,
    fitted_scale: f64,
}

impl LyricReel {
    pub fn new(
        typography: NowPlayingTypography,
        palette: PresentationPalette,
        supporting_family: &'static str,
    ) -> Rc<Self> {
        let widget = gtk::DrawingArea::new();
        widget.add_css_class("lyric-reel");
        widget.add_css_class("utility-text");
        widget.set_hexpand(true);
        widget.set_vexpand(true);
        widget.set_overflow(gtk::Overflow::Hidden);
        let reel = Rc::new(Self {
            widget,
            frame: RefCell::new(None),
            metrics: Cell::new(None),
            layouts: RefCell::new(HashMap::new()),
            typography: Cell::new(typography),
            palette: Cell::new(palette),
            supporting_family,
        });
        let weak = Rc::downgrade(&reel);
        reel.widget.set_draw_func(move |_, context, _, height| {
            if let Some(reel) = weak.upgrade() {
                reel.paint(context, height);
            }
        });
        reel
    }

    pub fn update(
        &self,
        frame: &LyricFrame,
        width: i32,
        height: i32,
        typography: NowPlayingTypography,
    ) {
        let metrics = (width, height, typography.lyric_current_px);
        if self.metrics.get() == Some(metrics)
            && self.typography.get() == typography
            && self.frame.borrow().as_ref() == Some(frame)
        {
            return;
        }
        if self.metrics.replace(Some(metrics)) != Some(metrics) {
            self.layouts.borrow_mut().clear();
        }
        // Only the current bounded timeline and its in-flight departure are kept.
        self.layouts
            .borrow_mut()
            .retain(|text, _| frame.cues.iter().any(|cue| &cue.text == text));
        self.typography.set(typography);
        self.frame.replace(Some(frame.clone()));
        self.widget.queue_draw();
    }

    pub fn set_palette(&self, palette: PresentationPalette) {
        if self.palette.replace(palette) != palette {
            self.widget.queue_draw();
        }
    }

    fn shape(&self, text: &str, width: i32, font_px: u32) -> pango::Layout {
        let layout =
            self.widget
                .create_pango_layout(Some(if text.is_empty() { " " } else { text }));
        let mut font = self
            .widget
            .pango_context()
            .font_description()
            .unwrap_or_default();
        font.set_family(self.supporting_family);
        font.set_absolute_size(f64::from(font_px) * f64::from(pango::SCALE));
        font.set_weight(pango::Weight::Semibold);
        layout.set_font_description(Some(&font));
        layout.set_width(width.max(1).saturating_mul(pango::SCALE));
        layout.set_wrap(pango::WrapMode::WordChar);
        layout.set_line_spacing(1.04);
        layout
    }

    fn positioned_cues(&self) -> Vec<PositionedCue> {
        let frame = self.frame.borrow();
        let Some(frame) = frame.as_ref() else {
            return Vec::new();
        };
        let typography = self.typography.get();
        let (width, fitting_height, font_px) =
            self.metrics
                .get()
                .unwrap_or((1, 1, typography.lyric_current_px));
        let neighbor_scale = f64::from(typography.lyric_neighbor_px) / f64::from(font_px);
        let gap = f64::from(typography.lyric_neighbor_px) * 0.42;
        let area_height = f64::from(self.widget.height());
        let primary_y = area_height / 3.0;
        // Allocation can change during composition travel. Fit against the
        // destination budget while placement and clipping follow the allocation.
        let fitting_height = f64::from(fitting_height);
        let available_height =
            (fitting_height * 2.0 / 3.0 - self.fade_height(fitting_height)).max(0.0);
        let mut cues = Vec::with_capacity(frame.cues.len());
        for cue in &frame.cues {
            let fitted = {
                let mut layouts = self.layouts.borrow_mut();
                layouts
                    .entry(cue.text.clone())
                    .or_insert_with(|| self.fit(&cue.text, width, font_px, available_height))
                    .clone()
            };
            let scale = fitted.scale * (neighbor_scale + (1.0 - neighbor_scale) * cue.emphasis);
            let height = f64::from(fitted.layout.pixel_size().1) * scale * cue.extent;
            cues.push(PositionedCue {
                color: self.color(cue),
                index: cue.index,
                y: 0.0,
                height,
                scale,
                cue: cue.clone(),
                layout: fitted.layout,
                fitted_scale: fitted.scale,
            });
        }
        // Interpolate complete packed endpoints, not an anchor against already
        // interpolated heights. The latter multiplies the easing curves and can
        // make shrinking outgoing cues reverse direction near the end of a lift.
        for (anchor, weight) in &frame.anchors {
            let mut y = 0.0;
            let tops: Vec<_> = cues
                .iter()
                .map(|cue| {
                    let top = y;
                    let scale = cue.fitted_scale
                        * if cue.index == *anchor {
                            1.0
                        } else {
                            neighbor_scale
                        };
                    y += (f64::from(cue.layout.pixel_size().1) * scale + gap) * cue.cue.extent;
                    top
                })
                .collect();
            let anchor_y = cues
                .iter()
                .position(|cue| cue.index == *anchor)
                .map_or(0.0, |index| tops[index]);
            for (cue, top) in cues.iter_mut().zip(tops) {
                cue.y += (top - anchor_y) * weight;
            }
        }
        for cue in &mut cues {
            cue.y += primary_y;
        }
        cues
    }

    fn fit(&self, text: &str, width: i32, font_px: u32, available_height: f64) -> FittedCue {
        let layout = self.shape(text, width, font_px);
        let scale = (available_height / f64::from(layout.pixel_size().1).max(1.0)).min(1.0);
        let mut fitted = FittedCue { layout, scale };
        if scale <= 0.0 || scale == 1.0 {
            return fitted;
        }
        // Find the largest fitting scale using the whole displayed width.
        // Shape at focal font size in inverse-scaled coordinates, then freeze
        // those line breaks for both focal and surrounding presentations.
        let mut upper = 1.0;
        for _ in 0..12 {
            let candidate_scale = (fitted.scale + upper) / 2.0;
            let candidate = fitted.layout.copy();
            candidate.set_width(
                (f64::from(width.max(1)) * f64::from(pango::SCALE) / candidate_scale) as i32,
            );
            if f64::from(candidate.pixel_size().1) * candidate_scale <= available_height {
                fitted = FittedCue {
                    layout: candidate,
                    scale: candidate_scale,
                };
            } else {
                upper = candidate_scale;
            }
        }
        fitted
    }

    pub fn visible_cues(&self) -> Vec<PositionedCue> {
        let height = f64::from(self.widget.height());
        self.positioned_cues()
            .into_iter()
            .filter(|cue| {
                cue.cue.opacity > 0.0
                    && !cue.cue.text.trim().is_empty()
                    && cue.y < height
                    && cue.y + cue.height > 0.0
            })
            .collect()
    }

    fn paint(&self, context: &cairo::Context, height: i32) {
        if height <= 0 {
            return;
        }
        context.push_group();
        for positioned in self.visible_cues() {
            let cue = &positioned.cue;
            let color = positioned.color;
            context.save().expect("save lyric drawing state");
            context.translate(0.0, positioned.y);
            context.scale(positioned.scale, positioned.scale);
            context.set_source_rgba(
                f64::from(color.red) / 255.0,
                f64::from(color.green) / 255.0,
                f64::from(color.blue) / 255.0,
                cue.opacity,
            );
            pangocairo::functions::show_layout(context, &positioned.layout);
            context.restore().expect("restore lyric drawing state");
        }
        context
            .pop_group_to_source()
            .expect("finish lyric drawing group");
        let fade = self.fade_height(f64::from(height)) / f64::from(height);
        let mask = cairo::LinearGradient::new(0.0, 0.0, 0.0, f64::from(height));
        for (offset, alpha) in [(0.0, 0.0), (fade, 1.0), (1.0 - fade, 1.0), (1.0, 0.0)] {
            mask.add_color_stop_rgba(offset, 1.0, 1.0, 1.0, alpha);
        }
        context.mask(&mask).expect("paint lyric edge fades");
    }

    fn fade_height(&self, height: f64) -> f64 {
        (f64::from(self.typography.get().lyric_neighbor_px) * 0.65).min(height / 4.0)
    }

    fn color(&self, cue: &LyricCueFrame) -> Rgb {
        let palette = self.palette.get();
        let colors = [
            palette.muted_text,
            palette.primary_text,
            palette.secondary_text,
        ];
        let component = |channel: fn(Rgb) -> u8| {
            colors
                .iter()
                .zip(cue.color_weights)
                .map(|(color, weight)| f64::from(channel(*color)) * weight)
                .sum::<f64>()
                .round() as u8
        };
        Rgb {
            red: component(|color| color.red),
            green: component(|color| color.green),
            blue: component(|color| color.blue),
        }
    }
}
