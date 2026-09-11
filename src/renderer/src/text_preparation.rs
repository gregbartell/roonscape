use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use gtk::{cairo, glib::Unichar, pango};
use pango::prelude::*;
use roonscape_renderer::{MetadataTypography, TypographySelection};

use crate::qt_window::{Color, Rect, Sprite, SpriteGeometry, SpriteKind, Texture, Uploader};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Face {
    Title,
    Artist,
    Album,
    Masthead,
    Utility,
    UtilityStrong,
    Status,
    IdentityLabel,
    IdentityName,
    Time,
    FullHeading,
    FullUtility,
    FullStrong,
    FullStatus,
}

impl From<MetadataTypography> for Face {
    fn from(role: MetadataTypography) -> Self {
        match role {
            MetadataTypography::EditorialSerif => Self::Title,
            MetadataTypography::ArtistSans => Self::Artist,
            MetadataTypography::AlbumSans => Self::Album,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreparedText<'window> {
    texture: Texture<'window>,
    foreground: Option<Texture<'window>>,
    pub bounds: Rect,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    #[cfg(test)]
    pub lines: Vec<String>,
    #[cfg(test)]
    pub ellipsized: bool,
}

impl<'window> PreparedText<'window> {
    pub fn sprite(&self, x: f32, y: f32, color: Color, clip: Rect) -> Sprite<'window> {
        Sprite {
            texture: Some(self.texture.clone()),
            foreground: self.foreground.clone(),
            geometry: SpriteGeometry {
                bounds: Rect::new(
                    x + self.bounds.x,
                    y + self.bounds.y,
                    self.bounds.width,
                    self.bounds.height,
                ),
                uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                clip,
                color,
                kind: if self.foreground.is_some() {
                    SpriteKind::TextWithForeground
                } else {
                    SpriteKind::AlphaMask
                },
                ..SpriteGeometry::default()
            },
        }
    }
}

/// A word occurrence drawn from the endpoint's already-shaped glyphs. Byte
/// coverage stops at native cluster boundaries when Pango retains a prefix.
#[derive(Clone)]
pub(crate) struct PreparedWord<'window> {
    pub occurrence: Option<usize>,
    pub text: PreparedText<'window>,
    pub x: f32,
    pub baseline: f32,
    pub visible_bytes: usize,
    pub clusters: Vec<(usize, Rect)>,
}

impl PreparedWord<'_> {
    pub fn retained_bounds(&self, bytes: usize) -> Rect {
        self.clusters
            .iter()
            .filter(|(end, _)| *end <= bytes)
            .map(|(_, bounds)| *bounds)
            .reduce(|a, b| union(Some(a), b))
            .unwrap_or(Rect::new(self.x, self.baseline, 0.0, 0.0))
    }
}

struct WordGlyphs {
    occurrence: Option<usize>,
    runs: Vec<(pango::Font, pango::GlyphString, f32, f32)>,
    ink: Option<Rect>,
    logical: Option<Rect>,
    baseline: f32,
    visible_bytes: usize,
    clusters: Vec<(usize, Rect)>,
}

fn union(a: Option<Rect>, b: Rect) -> Rect {
    let Some(a) = a else { return b };
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect::new(
        x,
        y,
        (a.x + a.width).max(b.x + b.width) - x,
        (a.y + a.height).max(b.y + b.height) - y,
    )
}

fn pango_rect(rect: pango::Rectangle, x: f32, y: f32) -> Rect {
    let unit = pango::SCALE as f32;
    Rect::new(
        x + rect.x() as f32 / unit,
        y + rect.y() as f32 / unit,
        rect.width() as f32 / unit,
        rect.height() as f32 / unit,
    )
}

fn pixel_rect(rect: Rect, x: f32, y: f32) -> pango::Rectangle {
    let left = (rect.x - x).floor() as i32;
    let top = (rect.y - y).floor() as i32;
    pango::Rectangle::new(
        left,
        top,
        (rect.x + rect.width - x).ceil() as i32 - left,
        (rect.y + rect.height - y).ceil() as i32 - top,
    )
}

pub(crate) struct TextPreparation<'window> {
    context: pango::Context,
    typography: TypographySelection,
    uploader: Uploader<'window>,
    scale: f64,
    drawings: RefCell<VecDeque<(String, PreparedText<'window>)>>,
    word_drawings: RefCell<VecDeque<(String, Arc<[PreparedWord<'window>]>)>>,
    measurements: HashMap<(Face, String, u32), (u32, u32)>,
}

impl<'window> TextPreparation<'window> {
    pub fn new(typography: TypographySelection, uploader: Uploader<'window>, scale: f64) -> Self {
        // Each preparation thread owns its font map and all Pango objects.
        // PreparedText contains only immutable, shareable graphics resources.
        let context = pangocairo::FontMap::new().create_context();
        pangocairo::functions::context_set_resolution(&context, 96.0);
        let mut options = cairo::FontOptions::new().expect("Cairo font options");
        options.set_antialias(cairo::Antialias::Gray);
        options.set_hint_metrics(cairo::HintMetrics::Off);
        options.set_hint_style(cairo::HintStyle::Slight);
        context.set_round_glyph_positions(false);
        pangocairo::functions::context_set_font_options(&context, Some(&options));
        Self {
            context,
            typography,
            uploader,
            scale,
            drawings: RefCell::new(VecDeque::new()),
            word_drawings: RefCell::new(VecDeque::new()),
            measurements: HashMap::new(),
        }
    }

    pub fn layout(&self, text: &str, face: Face, size: u32, spacing: u32) -> pango::Layout {
        let layout = pango::Layout::new(&self.context);
        layout.set_text(text);
        let (family, weight) = match face {
            Face::Title | Face::Masthead => (self.typography.now_playing_title_family(), 700),
            Face::FullHeading => (self.typography.full_field_editorial_family(), 400),
            Face::FullUtility => (self.typography.full_field_utility_family(), 400),
            Face::FullStrong => (self.typography.full_field_utility_family(), 600),
            Face::FullStatus => (self.typography.full_field_utility_family(), 700),
            Face::Album => (self.typography.now_playing_supporting_family(), 400),
            Face::Utility | Face::IdentityName | Face::Time => {
                (self.typography.now_playing_supporting_family(), 560)
            }
            Face::Status => (self.typography.now_playing_supporting_family(), 700),
            _ => (self.typography.now_playing_supporting_family(), 600),
        };
        let mut font = pango::FontDescription::new();
        let family = if face == Face::Title && family != "Libre Baskerville" {
            format!("{family}, Libre Baskerville, serif")
        } else {
            family.to_owned()
        };
        font.set_family(&family);
        font.set_absolute_size(f64::from(size) * f64::from(pango::SCALE));
        // Pango accepts intermediate numeric weights, matching the authored
        // 560-weight CSS without rounding to a named 500/600 face.
        font.set_weight(unsafe { gtk::glib::translate::from_glib(weight) });
        if matches!(
            face,
            Face::Status | Face::IdentityLabel | Face::IdentityName | Face::Time
        ) {
            font.set_variations(Some("wdth=96"));
        }
        layout.set_font_description(Some(&font));
        let attrs = pango::AttrList::new();
        if spacing > 0 {
            attrs.insert(pango::AttrInt::new_letter_spacing(
                (spacing as i32) * pango::SCALE,
            ));
        }
        if face == Face::Time {
            attrs.insert(pango::AttrFontFeatures::new("tnum=1"));
        }
        layout.set_attributes(Some(&attrs));
        layout
    }

    pub fn measure(&mut self, face: Face, text: &str, size: u32) -> (u32, u32) {
        let key = (face, text.to_owned(), size);
        if let Some(result) = self.measurements.get(&key) {
            return *result;
        }
        let (width, height) = self.layout(text, face, size, 0).pixel_size();
        let result = (width.max(0) as u32, height.max(0) as u32);
        if self.measurements.len() >= 512 {
            self.measurements.clear();
        }
        self.measurements.insert(key, result);
        result
    }

    pub fn line(
        &self,
        text: &str,
        face: Face,
        size: u32,
        width: u32,
        spacing: u32,
    ) -> Result<PreparedText<'window>, String> {
        let layout = self.layout(text, face, size, spacing);
        layout.set_width((width.max(1) as i32) * pango::SCALE);
        layout.set_single_paragraph_mode(true);
        layout.set_ellipsize(pango::EllipsizeMode::End);
        self.rasterize(&layout, 1.0)
    }

    pub fn rasterize(
        &self,
        layout: &pango::Layout,
        fitting_scale: f64,
    ) -> Result<PreparedText<'window>, String> {
        let key = format!(
            "{:?}|{:?}|{:?}|{}|{}|{:?}|{:?}|{}|{}|{}|{}",
            layout.text(),
            layout.font_description().map(|font| font.to_string()),
            layout.attributes().map(|a| a.to_string()),
            layout.width(),
            layout.height(),
            layout.wrap(),
            layout.ellipsize(),
            layout.spacing(),
            layout.line_spacing(),
            layout.is_single_paragraph_mode(),
            fitting_scale
        );
        let mut drawings = self.drawings.borrow_mut();
        if let Some(index) = drawings.iter().position(|(cached, _)| cached == &key) {
            let entry = drawings.remove(index).unwrap();
            let result = entry.1.clone();
            drawings.push_back(entry);
            return Ok(result);
        }
        let (ink, logical) = layout.pixel_extents();
        #[allow(unused_mut)]
        let mut prepared = self.rasterize_drawing(
            ink,
            logical,
            fitting_scale,
            layout.baseline() as f32 / pango::SCALE as f32,
            |context| pangocairo::functions::show_layout(context, layout),
        )?;
        #[cfg(test)]
        {
            prepared.lines = layout
                .lines_readonly()
                .iter()
                .map(|line| {
                    let text = layout.text();
                    text[line.start_index() as usize..(line.start_index() + line.length()) as usize]
                        .to_owned()
                })
                .collect();
            prepared.ellipsized = layout.is_ellipsized();
        }
        if drawings.len() >= 128 {
            drawings.pop_front();
        }
        drawings.push_back((key, prepared.clone()));
        Ok(prepared)
    }

    fn rasterize_drawing(
        &self,
        ink: pango::Rectangle,
        logical: pango::Rectangle,
        fitting_scale: f64,
        baseline: f32,
        draw: impl Fn(&cairo::Context),
    ) -> Result<PreparedText<'window>, String> {
        let left = ink.x().min(logical.x()) - 1;
        let top = ink.y().min(logical.y()) - 1;
        let right = (ink.x() + ink.width()).max(logical.x() + logical.width()) + 1;
        let bottom = (ink.y() + ink.height()).max(logical.y() + logical.height()) + 1;
        let scale = self.scale * fitting_scale;
        let width = (f64::from(right - left) * scale).ceil().max(1.0) as u32;
        let height = (f64::from(bottom - top) * scale).ceil().max(1.0) as u32;
        let paint = |foreground: f64| -> Result<Vec<u8>, String> {
            let surface =
                cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32)
                    .map_err(|e| e.to_string())?;
            surface.set_device_scale(scale, scale);
            let context = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
            context.translate(-f64::from(left), -f64::from(top));
            context.set_source_rgba(foreground, foreground, foreground, 1.0);
            draw(&context);
            drop(context);
            let stride = surface.stride() as usize;
            let data = surface.take_data().map_err(|e| e.to_string())?;
            let mut result = Vec::with_capacity(width as usize * height as usize * 4);
            for row in data.chunks_exact(stride).take(height as usize) {
                for pixel in row[..width as usize * 4].chunks_exact(4) {
                    let argb = u32::from_ne_bytes(pixel.try_into().unwrap());
                    result.extend_from_slice(&[
                        (argb >> 16) as u8,
                        (argb >> 8) as u8,
                        argb as u8,
                        (argb >> 24) as u8,
                    ]);
                }
            }
            Ok(result)
        };
        let white = paint(1.0)?;
        let monochrome = white
            .chunks_exact(4)
            .all(|p| p[0] == p[3] && p[1] == p[3] && p[2] == p[3]);
        let (texture, foreground) = if monochrome {
            let mask: Vec<_> = white.chunks_exact(4).map(|p| p[3]).collect();
            (self.uploader.mask(width, height, &mask)?, None)
        } else {
            // Separate color-font pixels from tintable text without changing
            // glyph shapes. This also preserves mixed text/emoji in one cue.
            let black = paint(0.0)?;
            let delta: Vec<_> = white
                .iter()
                .zip(&black)
                .map(|(w, b)| w.saturating_sub(*b))
                .collect();
            (
                self.uploader.rgba(width, height, &black)?,
                Some(self.uploader.rgba(width, height, &delta)?),
            )
        };
        let prepared = PreparedText {
            texture,
            foreground,
            bounds: Rect::new(
                left as f32 * fitting_scale as f32,
                top as f32 * fitting_scale as f32,
                width as f32 / self.scale as f32,
                height as f32 / self.scale as f32,
            ),
            width: logical.width() as f32 * fitting_scale as f32,
            height: logical.height() as f32 * fitting_scale as f32,
            #[cfg(test)]
            lines: Vec::new(),
            #[cfg(test)]
            ellipsized: false,
            baseline: baseline * fitting_scale as f32,
        };
        Ok(prepared)
    }

    pub fn words(
        &self,
        layout: &pango::Layout,
        synthetic_ellipsis: bool,
    ) -> Result<Arc<[PreparedWord<'window>]>, String> {
        let text = layout.text();
        let key = format!(
            "{:?}|{:?}|{}|{:?}|{}",
            text,
            layout.font_description().map(|font| font.to_string()),
            layout.width(),
            layout.ellipsize(),
            synthetic_ellipsis
        );
        let mut drawings = self.word_drawings.borrow_mut();
        if let Some(index) = drawings.iter().position(|(cached, _)| cached == &key) {
            let entry = drawings.remove(index).unwrap();
            let result = entry.1.clone();
            drawings.push_back(entry);
            return Ok(result);
        }
        let cutoff = if synthetic_ellipsis && text.ends_with('…') {
            text.len() - '…'.len_utf8()
        } else {
            text.len()
        };
        let mut ranges = Vec::new();
        let mut offset = 0;
        for token in text[..cutoff].split_inclusive(char::is_whitespace) {
            let word = token.trim();
            if !word.is_empty() {
                let start = offset + token.find(word).unwrap();
                ranges.push(start..start + word.len());
            }
            offset += token.len();
        }
        let mut groups: Vec<WordGlyphs> = Vec::new();
        let mut iter = layout.iter();
        loop {
            if let Some(run) = iter.run_readonly() {
                let item = run.item();
                let font = item.analysis().font();
                let glyphs = run.glyph_string();
                let (_, logical) = iter.run_extents();
                let mut x = logical.x() as f32 / pango::SCALE as f32;
                let baseline = iter.run_baseline() as f32 / pango::SCALE as f32;
                let ellipsis =
                    item.analysis().flags() & pango::ANALYSIS_FLAG_IS_ELLIPSIS as u8 != 0;
                let identity = |glyph: usize| {
                    let index = (item.offset() + glyphs.log_clusters()[glyph]) as usize;
                    if ellipsis || index >= cutoff {
                        Some(None)
                    } else {
                        ranges
                            .iter()
                            .position(|range| range.contains(&index))
                            .map(Some)
                    }
                };
                let mut start = 0;
                while start < glyphs.num_glyphs() as usize {
                    let id = identity(start);
                    let mut end = start + 1;
                    while end < glyphs.num_glyphs() as usize && identity(end) == id {
                        end += 1;
                    }
                    let advance = glyphs.glyph_info()[start..end]
                        .iter()
                        .map(|glyph| glyph.geometry().width())
                        .sum::<i32>() as f32
                        / pango::SCALE as f32;
                    if let Some(occurrence) = id {
                        let mut selected = pango::GlyphString::new();
                        selected.set_size((end - start) as i32);
                        selected
                            .glyph_info_mut()
                            .clone_from_slice(&glyphs.glyph_info()[start..end]);
                        selected
                            .log_clusters_mut()
                            .copy_from_slice(&glyphs.log_clusters()[start..end]);
                        let (ink, logical) = selected.extents(&font);
                        let group_index = groups
                            .iter()
                            .position(|group| group.occurrence == occurrence)
                            .unwrap_or_else(|| {
                                groups.push(WordGlyphs {
                                    occurrence,
                                    runs: Vec::new(),
                                    ink: None,
                                    logical: None,
                                    baseline,
                                    visible_bytes: 0,
                                    clusters: Vec::new(),
                                });
                                groups.len() - 1
                            });
                        let group = &mut groups[group_index];
                        group.ink = Some(union(group.ink, pango_rect(ink, x, baseline)));
                        group.logical =
                            Some(union(group.logical, pango_rect(logical, x, baseline)));
                        if let Some(occurrence) = occurrence {
                            let mut cluster_start = start;
                            let mut cluster_x = x;
                            while cluster_start < end {
                                let cluster = glyphs.log_clusters()[cluster_start];
                                let mut cluster_end = cluster_start + 1;
                                while cluster_end < end
                                    && glyphs.log_clusters()[cluster_end] == cluster
                                {
                                    cluster_end += 1;
                                }
                                let mut cluster_glyphs = pango::GlyphString::new();
                                cluster_glyphs.set_size((cluster_end - cluster_start) as i32);
                                cluster_glyphs.glyph_info_mut().clone_from_slice(
                                    &glyphs.glyph_info()[cluster_start..cluster_end],
                                );
                                let (ink, logical) = cluster_glyphs.extents(&font);
                                let next = glyphs
                                    .log_clusters()
                                    .iter()
                                    .copied()
                                    .filter(|index| *index > cluster)
                                    .min()
                                    .unwrap_or(item.length());
                                let end_byte = ((item.offset() + next) as usize)
                                    .min(ranges[occurrence].end)
                                    - ranges[occurrence].start;
                                group.visible_bytes = group.visible_bytes.max(end_byte);
                                group.clusters.push((
                                    end_byte,
                                    union(
                                        Some(pango_rect(ink, cluster_x, baseline)),
                                        pango_rect(logical, cluster_x, baseline),
                                    ),
                                ));
                                cluster_x += glyphs.glyph_info()[cluster_start..cluster_end]
                                    .iter()
                                    .map(|glyph| glyph.geometry().width())
                                    .sum::<i32>()
                                    as f32
                                    / pango::SCALE as f32;
                                cluster_start = cluster_end;
                            }
                        }
                        group.runs.push((font.clone(), selected, x, baseline));
                    }
                    x += advance;
                    start = end;
                }
            }
            if !iter.next_run() {
                break;
            }
        }
        groups.sort_by_key(|group| group.occurrence.unwrap_or(usize::MAX));
        let prepared: Arc<[PreparedWord<'window>]> = groups
            .into_iter()
            .map(|group| {
                let logical = group.logical.unwrap();
                let x = logical.x;
                let y = group.baseline;
                let prepared = self.rasterize_drawing(
                    pixel_rect(group.ink.unwrap(), 0.0, 0.0),
                    pixel_rect(logical, 0.0, 0.0),
                    1.0,
                    0.0,
                    |context| {
                        for (font, glyphs, run_x, run_y) in &group.runs {
                            context.move_to(f64::from(*run_x), f64::from(*run_y));
                            pangocairo::functions::show_glyph_string(
                                context,
                                font,
                                &mut glyphs.clone(),
                            );
                        }
                    },
                )?;
                Ok(PreparedWord {
                    occurrence: group.occurrence,
                    text: prepared,
                    x,
                    baseline: y,
                    visible_bytes: group.visible_bytes,
                    clusters: group.clusters,
                })
            })
            .collect::<Result<Vec<_>, String>>()?
            .into();
        // Cache complete endpoints so dense credits cannot evict their own
        // earliest words, and cue preparation does not reshape unchanged text.
        if drawings.len() >= 8 {
            drawings.pop_front();
        }
        drawings.push_back((key, prepared.clone()));
        Ok(prepared)
    }

    pub fn cue(
        &self,
        text: &str,
        width: u32,
        size: u32,
        height: f64,
    ) -> Result<PreparedText<'window>, String> {
        let layout = self.layout(
            if text.is_empty() { " " } else { text },
            Face::UtilityStrong,
            size,
            0,
        );
        layout.set_width((width.max(1) as i32) * pango::SCALE);
        layout.set_wrap(pango::WrapMode::WordChar);
        layout.set_line_spacing(1.04);
        protect_hyphenated_words(&layout);
        let mut fitting = (height / f64::from(layout.pixel_size().1).max(1.0)).min(1.0);
        let mut fitted = layout;
        if fitting < 1.0 {
            let mut upper = 1.0;
            for _ in 0..12 {
                let candidate_scale = (fitting + upper) / 2.0;
                let candidate = fitted.copy();
                candidate.set_width(
                    (f64::from(width.max(1)) * f64::from(pango::SCALE) / candidate_scale) as i32,
                );
                protect_hyphenated_words(&candidate);
                if f64::from(candidate.pixel_size().1) * candidate_scale <= height {
                    fitted = candidate;
                    fitting = candidate_scale;
                } else {
                    upper = candidate_scale;
                }
            }
        }
        self.rasterize(&fitted, fitting.max(f64::EPSILON))
    }
}

fn protect_hyphenated_words(layout: &pango::Layout) {
    let text = layout.text();
    let attributes = pango::AttrList::new();
    let measurement = layout.copy();
    measurement.set_attributes(None);
    measurement.set_width(-1);
    let word_character = |ch: char| {
        ch.is_alphanumeric()
            || matches!(
                ch.unicode_type(),
                gtk::glib::UnicodeType::SpacingMark
                    | gtk::glib::UnicodeType::EnclosingMark
                    | gtk::glib::UnicodeType::NonSpacingMark
            )
    };
    let mut offset = 0;
    for token in text.split_inclusive(|ch| !word_character(ch) && ch != '-' && ch != '‐') {
        let word = token.trim_matches(|ch| !word_character(ch));
        let start = offset + token.find(word).unwrap();
        offset += token.len();
        if !word.contains(['-', '‐']) || word.split(['-', '‐']).any(str::is_empty) {
            continue;
        }
        measurement.set_text(word);
        if measurement.size().0 > layout.width() {
            continue;
        }
        for (index, ch) in word
            .char_indices()
            .filter(|(_, ch)| matches!(ch, '-' | '‐'))
        {
            let following = index + ch.len_utf8();
            let end = following + word[following..].chars().next().unwrap().len_utf8();
            let mut attribute = pango::AttrInt::new_allow_breaks(false);
            attribute.set_start_index((start + index) as u32);
            attribute.set_end_index((start + end) as u32);
            attributes.insert(attribute);
        }
    }
    layout.set_attributes(Some(&attributes));
}
