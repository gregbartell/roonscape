use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;

use gtk::cairo;
use roonscape_renderer::{
    ArtworkDimensions, ArtworkLayout, ArtworkReference, FullFieldLayout, FullFieldPresentation,
    IdentityRowLayout, NowPlayingGradientLookup, NowPlayingLayout, NowPlayingPresentation,
    Presentation, PresentationIdentity, PresentationPalette, PresentationStatusDecoration,
    PresentationStatusLayout, PresentationStatusSymbol, TypographySelection, Viewport,
    metadata_layout, resolve_presentation_with_palette,
};

use crate::qt_window::{Color, Graphic, GraphicGeometry, Rect, Texture, Uploader};
use crate::text_preparation::{Face, PreparedText, PreparedWord, TextPreparation};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextRole {
    Primary,
    Secondary,
    Muted,
}

impl TextRole {
    pub fn color(self, palette: PresentationPalette) -> roonscape_renderer::Rgb {
        match self {
            Self::Primary => palette.primary_text,
            Self::Secondary => palette.secondary_text,
            Self::Muted => palette.muted_text,
        }
    }
}

#[derive(Clone)]
pub(crate) struct TextAt<'window> {
    pub text: PreparedText<'window>,
    pub x: f32,
    pub y: f32,
    pub role: TextRole,
}

#[derive(Clone)]
pub(crate) struct PreparedStatus<'window> {
    pub label: PreparedText<'window>,
    pub glyph: Texture<'window>,
    pub circle: Option<Texture<'window>>,
    pub layout: PresentationStatusLayout,
}

#[derive(Clone)]
pub(crate) struct PreparedIdentity<'window> {
    pub text: Vec<TextAt<'window>>,
    pub separator: Option<Rect>,
    pub height: f32,
}

#[derive(Clone)]
pub(crate) struct PreparedMetadata<'window> {
    pub ordinary: Vec<TextAt<'window>>,
    pub masthead: Vec<TextAt<'window>>,
    pub masthead_height: f32,
    pub movement: Vec<MetadataMovement<'window>>,
    pub album_index: Option<usize>,
    pub album_displacement: f32,
}

#[derive(Clone)]
pub(crate) struct MetadataMovement<'window> {
    pub ordinary: MetadataEndpoint<'window>,
    pub compact: MetadataEndpoint<'window>,
    pub role: TextRole,
    pub paths: Vec<MetadataWordPath<'window>>,
    pub normalized_extents: [f32; 2],
}

#[derive(Clone)]
pub(crate) struct MetadataEndpoint<'window> {
    pub words: Arc<[PreparedWord<'window>]>,
    pub size: f32,
    pub y: f32,
    pub width: f32,
}

impl MetadataEndpoint<'_> {
    fn normalized_extent(&self) -> f32 {
        self.words
            .iter()
            .map(|word| (word.text.bounds.x + word.text.bounds.width) / self.size)
            .fold(0.0, f32::max)
            // An unbroken token may be wider than the native endpoint's
            // clip. Hidden glyphs must not force a smaller moving size.
            .min((self.width + 2.0) / self.size)
    }
}

#[derive(Clone)]
pub(crate) struct MetadataWordPath<'window> {
    pub text: PreparedText<'window>,
    pub bounds: Rect,
    pub source: [f32; 2],
    pub destination: [f32; 2],
    pub visible: [bool; 2],
    pub clipping: Option<[Rect; 2]>,
    pub omitted_spread: [f32; 2],
}

impl<'window> MetadataMovement<'window> {
    fn new(
        ordinary: MetadataEndpoint<'window>,
        compact: MetadataEndpoint<'window>,
        role: TextRole,
    ) -> Self {
        let mut movement = Self {
            normalized_extents: [ordinary.normalized_extent(), compact.normalized_extent()],
            ordinary,
            compact,
            role,
            paths: Vec::new(),
        };
        movement.prepare_paths();
        movement
    }

    fn prepare_paths(&mut self) {
        let mut identities: Vec<_> = self
            .ordinary
            .words
            .iter()
            .chain(self.compact.words.iter())
            .map(|word| word.occurrence)
            .collect();
        identities.sort();
        identities.dedup();
        let source_ellipsis = self
            .ordinary
            .words
            .iter()
            .find(|word| word.occurrence.is_none())
            .or_else(|| self.ordinary.words.last());
        let destination_ellipsis = self
            .compact
            .words
            .iter()
            .find(|word| word.occurrence.is_none())
            .or_else(|| self.compact.words.last());
        let first_source_only = self.ordinary.words.iter().find(|word| {
            word.occurrence.is_some()
                && !self
                    .compact
                    .words
                    .iter()
                    .any(|other| other.occurrence == word.occurrence)
        });
        let first_destination_only = self.compact.words.iter().find(|word| {
            word.occurrence.is_some()
                && !self
                    .ordinary
                    .words
                    .iter()
                    .any(|other| other.occurrence == word.occurrence)
        });
        for identity in identities {
            let source = self
                .ordinary
                .words
                .iter()
                .find(|word| word.occurrence == identity);
            let destination = self
                .compact
                .words
                .iter()
                .find(|word| word.occurrence == identity);
            let (Some(a), Some(b)) = (
                source.or(source_ellipsis),
                destination.or(destination_ellipsis),
            ) else {
                continue;
            };
            let (word, native_size) = match (source, destination) {
                (Some(a), Some(b))
                    if b.visible_bytes > a.visible_bytes
                        || (b.visible_bytes == a.visible_bytes
                            && self.compact.size > self.ordinary.size) =>
                {
                    (b, self.compact.size)
                }
                (Some(a), _) => (a, self.ordinary.size),
                (_, Some(b)) => (b, self.compact.size),
                _ => continue,
            };
            let relative = |bounds: Rect| {
                Rect::new(
                    (bounds.x - word.x) / native_size,
                    (bounds.y - word.baseline) / native_size,
                    bounds.width / native_size,
                    bounds.height / native_size,
                )
            };
            let clipping = source
                .zip(destination)
                .filter(|(a, b)| a.visible_bytes != b.visible_bytes)
                .map(|(a, b)| {
                    [
                        relative(word.retained_bounds(a.visible_bytes)),
                        relative(word.retained_bounds(b.visible_bytes)),
                    ]
                });
            // Keep omitted words as an intact tail while they fade. They only
            // converge on the ellipsis after becoming invisible, avoiding a
            // pile of opaque words at the truncation point.
            let omitted_spread = match (source, destination) {
                (Some(word), None) if word.occurrence.is_some() => first_source_only.map(|first| {
                    [
                        (word.x - first.x) / self.ordinary.size,
                        (word.baseline - first.baseline) / self.ordinary.size,
                    ]
                }),
                (None, Some(word)) if word.occurrence.is_some() => {
                    first_destination_only.map(|first| {
                        [
                            (word.x - first.x) / self.compact.size,
                            (word.baseline - first.baseline) / self.compact.size,
                        ]
                    })
                }
                _ => None,
            }
            .unwrap_or([0.0; 2]);
            self.paths.push(MetadataWordPath {
                text: word.text.clone(),
                bounds: relative(word.text.bounds),
                source: [a.x / self.ordinary.size, self.ordinary.y + a.baseline],
                destination: [b.x / self.compact.size, self.compact.y + b.baseline],
                visible: [source.is_some(), destination.is_some()],
                omitted_spread,
                clipping,
            });
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreparedReel<'window> {
    pub timeline_signature: u64,
    pub width: f32,
    pub cues: BTreeMap<usize, PreparedText<'window>>,
    pub blank: PreparedText<'window>,
    pub primary_y: f32,
    pub top: f32,
    pub bottom: f32,
    pub fade: f32,
    pub gap: f32,
}

#[derive(Clone)]
pub(crate) struct PreparedNowPlaying<'window> {
    pub metadata: PreparedMetadata<'window>,
    pub ordinary_identity: PreparedIdentity<'window>,
    pub lyric_identity: PreparedIdentity<'window>,
    pub statuses: Vec<(PresentationStatusSymbol, PreparedStatus<'window>)>,
    pub digits: HashMap<char, PreparedText<'window>>,
    pub activity: Option<(PreparedText<'window>, PreparedText<'window>)>,
    pub reel: Option<PreparedReel<'window>>,
    pub time_height: f32,
}

#[derive(Clone)]
pub(crate) struct PreparedFullField<'window> {
    pub text: Vec<TextAt<'window>>,
    pub statuses: Vec<(PresentationStatusSymbol, PreparedStatus<'window>)>,
    pub identity: Option<PreparedIdentity<'window>>,
}

#[derive(Clone)]
pub(crate) enum PreparedContent<'window> {
    NowPlaying(Box<PreparedNowPlaying<'window>>),
    FullField(PreparedFullField<'window>),
}

#[derive(Clone)]
pub(crate) struct PreparedPresentation<'window> {
    pub presentation: Presentation,
    pub palette: PresentationPalette,
    pub artwork: Option<Texture<'window>>,
    pub artwork_dimensions: Option<ArtworkDimensions>,
    pub gradient: Option<Texture<'window>>,
    pub gradient_steps: [u32; 3],
    pub noise: Texture<'window>,
    pub content: PreparedContent<'window>,
    pub diagnostics: Option<PreparedText<'window>>,
    pub viewport: Viewport,
}

impl<'window> PreparedPresentation<'window> {
    pub fn graphic(&self, layout: Option<&NowPlayingLayout>, weight: f32) -> Graphic<'window> {
        let palette = self.palette;
        let mut geometry = GraphicGeometry {
            canvas: Rect::viewport(self.viewport),
            background: Color::new(palette.background, 1.0),
            quiet: Color::new(palette.artwork_field, 1.0),
            muted: Color::new(palette.muted_text, 1.0),
            plate: Color::new(palette.accent, 1.0),
            origin: self.gradient_steps[0],
            step_x: self.gradient_steps[1],
            step_y: self.gradient_steps[2],
            weight,
            ..GraphicGeometry::default()
        };
        if let (Some(layout), Presentation::NowPlaying(presentation)) = (layout, &self.presentation)
        {
            let artwork = ArtworkLayout::for_presentation(presentation, self.artwork_dimensions);
            let reservation = ArtworkDimensions::new(
                layout.artwork_field_width_px,
                layout.artwork_field_height_px,
            );
            let footprint =
                artwork.visible_decoration_with_border(reservation, layout.artwork_border_width_px);
            let x = layout.outer_gutter_px + (reservation.width_px - footprint.width_px) / 2;
            let y = (self.viewport.height_px - footprint.height_px) / 2;
            geometry.artwork_bounds = Rect::new(
                x as f32,
                y as f32,
                footprint.width_px as f32,
                footprint.height_px as f32,
            );
            geometry.plate_bounds = Rect::new(
                (x + layout.artwork_print_plate.offset_x_px) as f32,
                (y + layout.artwork_print_plate.offset_y_px) as f32,
                footprint.width_px as f32,
                footprint.height_px as f32,
            );
            geometry.border_width = layout.artwork_border_width_px as f32;
            geometry.border = if self.artwork.is_some() {
                Color::new(palette.primary_text, 0.16)
            } else {
                Color::new(palette.muted_text, 0.22)
            };
            geometry.shadow_radius = layout.artwork_shadow_blur_px as f32;
            geometry.shadow_y = layout.artwork_shadow_offset_px as f32;
            geometry.shadow_alpha = 0.38;
        }
        Graphic {
            artwork: self.artwork.clone(),
            gradient: self.gradient.clone(),
            noise: Some(self.noise.clone()),
            geometry,
        }
    }
}

#[derive(Clone)]
struct PreparedArtwork<'window> {
    palette: PresentationPalette,
    texture: Texture<'window>,
    size: Viewport,
}

struct PreparedGradient<'window> {
    palette: PresentationPalette,
    viewport: Viewport,
    texture: Texture<'window>,
    steps: [u32; 3],
}

pub(crate) struct PresentationPreparation<'window> {
    uploader: Uploader<'window>,
    text: TextPreparation<'window>,
    noise: Texture<'window>,
    scale: f64,
    recent: VecDeque<PreparedPresentation<'window>>,
    gradients: VecDeque<PreparedGradient<'window>>,
    statuses: RefCell<VecDeque<(String, PreparedStatus<'window>)>>,
    arriving_artwork: Option<(ArtworkReference, Result<PreparedArtwork<'window>, String>)>,
}

impl<'window> PresentationPreparation<'window> {
    pub fn new(
        uploader: Uploader<'window>,
        typography: TypographySelection,
        scale: f64,
    ) -> Result<Self, String> {
        Ok(Self {
            uploader,
            text: TextPreparation::new(typography, uploader, scale),
            noise: uploader.noise(NowPlayingGradientLookup::noise())?,
            scale,
            recent: VecDeque::new(),
            gradients: VecDeque::new(),
            statuses: RefCell::new(VecDeque::new()),
            arriving_artwork: None,
        })
    }

    pub fn prepare_arriving_artwork(&mut self, artwork: ArtworkReference, repository: &Path) {
        let key = Some((artwork.path.as_str(), Some(artwork.revision)));
        if self
            .arriving_artwork
            .as_ref()
            .is_some_and(|(previous, _)| *previous == artwork)
            || self
                .recent
                .iter()
                .any(|previous| artwork_key(&previous.presentation) == key)
        {
            return;
        }
        let result = self.image(&repository.join(&artwork.path));
        self.arriving_artwork = Some((artwork, result));
    }

    pub fn prepare(
        &mut self,
        presentation: &Presentation,
        viewport: Viewport,
        repository: &Path,
        strict_artwork: bool,
    ) -> Result<PreparedPresentation<'window>, String> {
        let previous_art = self
            .recent
            .iter()
            .rev()
            .find(|previous| artwork_key(&previous.presentation) == artwork_key(presentation));
        let mut artwork = None;
        let mut dimensions = None;
        let mut artwork_palette = None;
        if let Presentation::NowPlaying(now_playing) = presentation
            && let Some(path) = &now_playing.artwork_path
        {
            let path = repository.join(path);
            let result = if let Some(previous) = previous_art {
                previous
                    .artwork
                    .clone()
                    .zip(previous.artwork_dimensions)
                    .map(|(texture, size)| PreparedArtwork {
                        palette: previous.palette,
                        texture,
                        size: Viewport::new(size.width_px, size.height_px),
                    })
                    .ok_or_else(|| "Artwork could not be decoded".to_owned())
            } else if let Some((_, result)) =
                self.arriving_artwork.as_ref().filter(|(reference, _)| {
                    artwork_key(presentation)
                        == Some((reference.path.as_str(), Some(reference.revision)))
                })
            {
                result.clone()
            } else {
                self.image(&path)
            };
            match result {
                Ok(PreparedArtwork {
                    palette,
                    texture,
                    size,
                }) => {
                    artwork_palette = Some(palette);
                    artwork = Some(texture);
                    dimensions = Some(ArtworkDimensions::new(size.width_px, size.height_px));
                }
                Err(error) if strict_artwork => {
                    return Err(format!(
                        "could not decode or derive a palette from artwork at {}: {error}",
                        path.display()
                    ));
                }
                Err(_) => {}
            }
        }
        let resolved = resolve_presentation_with_palette(presentation, artwork_palette);
        let (gradient, gradient_steps) =
            if matches!(resolved.presentation, Presentation::NowPlaying(_)) {
                let physical = Viewport::new(
                    (viewport.width_px as f64 * self.scale).round() as u32,
                    (viewport.height_px as f64 * self.scale).round() as u32,
                );
                if let Some(index) = self.gradients.iter().position(|previous| {
                    previous.palette == resolved.palette && previous.viewport == physical
                }) {
                    let previous = self.gradients.remove(index).unwrap();
                    let result = (Some(previous.texture.clone()), previous.steps);
                    self.gradients.push_back(previous);
                    result
                } else {
                    let lookup = NowPlayingGradientLookup::new(resolved.palette, physical);
                    let texture = self.uploader.lookup(&lookup.colors)?;
                    let steps = [lookup.origin, lookup.step_x, lookup.step_y];
                    // Keep palette reuse independent of metadata churn. Each
                    // fixed-size lookup occupies about one MiB on the device.
                    if self.gradients.len() >= 4 {
                        self.gradients.pop_front();
                    }
                    self.gradients.push_back(PreparedGradient {
                        palette: resolved.palette,
                        viewport: physical,
                        texture: texture.clone(),
                        steps,
                    });
                    (Some(texture), steps)
                }
            } else {
                (None, [0; 3])
            };

        let content_key = content_key(&resolved.presentation);
        let content = if let Some(previous) = self.recent.iter().rev().find(|previous| {
            previous.viewport == viewport
                && content_key == self::content_key(&previous.presentation)
        }) {
            previous.content.clone()
        } else {
            match &resolved.presentation {
                Presentation::NowPlaying(value) => {
                    PreparedContent::NowPlaying(Box::new(self.now_playing(value, viewport)?))
                }
                Presentation::FullField(value) => {
                    PreparedContent::FullField(self.full_field(value, viewport)?)
                }
            }
        };

        let prepared = PreparedPresentation {
            presentation: resolved.presentation,
            palette: resolved.palette,
            artwork,
            artwork_dimensions: dimensions,
            gradient,
            gradient_steps,
            noise: self.noise.clone(),
            content,
            diagnostics: None,
            viewport,
        };
        if self.recent.len() >= 3 {
            self.recent.pop_front();
        }
        self.recent.push_back(prepared.clone());
        Ok(prepared)
    }

    pub fn diagnostics(&self, text: &str) -> Result<PreparedText<'window>, String> {
        self.text
            .rasterize(&self.text.layout(text, Face::FullUtility, 15, 0), 1.0)
    }

    fn image(&self, path: &Path) -> Result<PreparedArtwork<'window>, String> {
        crate::content_evidence::record(
            || serde_json::json!({"event":"artwork-started","path":path}),
        );
        let result = self.decode_image(path);
        crate::content_evidence::record(
            || serde_json::json!({"event":"artwork-completed","path":path,"resource":result.as_ref().ok().map(|image|image.texture.identity()),"success":result.is_ok()}),
        );
        result
    }

    fn decode_image(&self, path: &Path) -> Result<PreparedArtwork<'window>, String> {
        if let Ok(image) = crate::qt_window::DecodedImage::open(path) {
            let size = image.size;
            let pending = self.uploader.start_image(&image)?;
            let sample = image.palette_image()?;
            let has_alpha = sample.has_alpha;
            let stride = sample.stride;
            let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
                &gtk::glib::Bytes::from_owned(sample),
                gdk_pixbuf::Colorspace::Rgb,
                has_alpha,
                8,
                size.width_px as i32,
                size.height_px as i32,
                stride as i32,
            );
            let palette = PresentationPalette::from_pixbuf(&pixbuf).map_err(|e| e.to_string())?;
            let texture = pending.finish()?;
            return Ok(PreparedArtwork {
                palette,
                texture,
                size,
            });
        }
        // Fixture artwork includes vector formats supported by the existing
        // image loader even when a Qt image-format plugin is unavailable.
        let image = gdk_pixbuf::Pixbuf::from_file(path).map_err(|e| e.to_string())?;
        let rgba = roonscape_renderer::pixbuf_rgba(&image);
        let viewport = Viewport::new(image.width() as u32, image.height() as u32);
        Ok(PreparedArtwork {
            palette: PresentationPalette::from_pixbuf(&image).map_err(|e| e.to_string())?,
            texture: self
                .uploader
                .rgba(viewport.width_px, viewport.height_px, &rgba)?,
            size: viewport,
        })
    }

    fn now_playing(
        &mut self,
        presentation: &NowPlayingPresentation,
        viewport: Viewport,
    ) -> Result<PreparedNowPlaying<'window>, String> {
        let ordinary = NowPlayingLayout::for_composition_progress(presentation, viewport, 0.0);
        let lyrics = NowPlayingLayout::for_composition_progress(presentation, viewport, 1.0);
        let metadata = self.metadata(presentation, viewport, &ordinary, &lyrics)?;

        let ordinary_identity = self.identity(
            &presentation.tracked_output,
            Some(&presentation.tracked_zone),
            ordinary.identity_row,
            ordinary.typography.identity_px,
            false,
        )?;
        let lyric_identity = self.identity(
            &presentation.tracked_output,
            Some(&presentation.tracked_zone),
            lyrics.identity_row,
            lyrics.typography.identity_px,
            false,
        )?;

        let statuses = self.all_statuses(
            ordinary.presentation_status,
            ordinary.information.utility_width_px,
            false,
        )?;
        let mut digits = HashMap::new();
        for ch in "0123456789:-−".chars() {
            let layout =
                self.text
                    .layout(&ch.to_string(), Face::Time, ordinary.typography.time_px, 0);
            digits.insert(ch, self.text.rasterize(&layout, 1.0)?);
        }
        let time_height = digits[&'0'].height;
        let identity_height = ordinary_identity.height;
        let activity = if let Some(activity) = &presentation.activity {
            let width = ordinary.information.utility_width_px.saturating_sub(
                ordinary.activity_waveform_width_px + ordinary.activity_copy_gap_px,
            );
            Some((
                self.text.line(
                    activity.heading,
                    Face::UtilityStrong,
                    ordinary.typography.activity_heading_px,
                    width,
                    0,
                )?,
                self.text.line(
                    activity.detail,
                    Face::Utility,
                    ordinary.typography.activity_detail_px,
                    width,
                    0,
                )?,
            ))
        } else {
            None
        };

        let reel = if let Some(lyric_presentation) = &presentation.lyrics {
            let margin = (lyrics.typography.lyric_cue_px as f64 * 0.52).round() as f32;
            let upper_extent = metadata.masthead_height + margin;
            let primary_y = (lyrics.metadata_height_budget_px as f32 - upper_extent).max(0.0) / 3.0;
            let progress_height =
                (lyrics.progress_fill_height_px + lyrics.time_spacing_px) as f32 + time_height;
            let timing_height = lyrics.timing_height_px() as f32;
            let footer_top = lyrics.footer_anchor.bottom_viewport_y_px as f32
                - timing_height
                - lyrics.footer_gap_px as f32
                - identity_height;
            let progress_top = footer_top + (timing_height - progress_height) / 2.0;
            let top = lyrics.metadata_region_top_viewport_y_px as f32 + upper_extent;
            let bottom = progress_top - margin;
            let height = (bottom - top).max(1.0);
            let fade = (lyrics.typography.lyric_spacing_px as f32 * 0.65).min(height / 4.0);
            let available = (height - primary_y - fade).max(1.0) as f64;
            let blank = self.text.cue(
                "",
                lyrics.lyric_width_px,
                lyrics.typography.lyric_cue_px,
                available,
            )?;
            let gap = lyrics.typography.lyric_spacing_px as f32 * 0.42;
            let mut cues = BTreeMap::new();
            let current = lyric_presentation
                .current_index
                .min(lyric_presentation.timeline.len().saturating_sub(1));
            // Prepare enough complete cues on either side to cover two lyric
            // areas. Natural movement stays inside prepared context; seeks can
            // request a new neighborhood without rasterizing the entire song.
            for direction in [-1i32, 1] {
                let mut distance = 0.0;
                let mut index = current as i32 + if direction < 0 { -1 } else { 0 };
                while index >= 0
                    && (index as usize) < lyric_presentation.timeline.len()
                    && distance < height * 2.0
                {
                    let text = &lyric_presentation.timeline[index as usize];
                    let cue = if text.trim().is_empty() {
                        blank.clone()
                    } else {
                        self.text.cue(
                            text,
                            lyrics.lyric_width_px,
                            lyrics.typography.lyric_cue_px,
                            available,
                        )?
                    };
                    // LyricMotion collapses an entire blank run into one row.
                    if !text.trim().is_empty()
                        || index == current as i32
                        || (index > 0
                            && !lyric_presentation.timeline[index as usize - 1]
                                .trim()
                                .is_empty())
                    {
                        distance += cue.height + gap;
                    }
                    cues.insert(index as usize, cue);
                    index += direction;
                }
            }
            Some(PreparedReel {
                timeline_signature: lyric_presentation.timeline_signature,
                width: lyrics.lyric_width_px as f32,
                cues,
                blank,
                primary_y,
                top,
                bottom,
                fade,
                gap,
            })
        } else {
            None
        };

        Ok(PreparedNowPlaying {
            metadata,
            ordinary_identity,
            lyric_identity,
            statuses,
            digits,
            activity,
            reel,
            time_height,
        })
    }

    fn metadata(
        &mut self,
        presentation: &NowPlayingPresentation,
        viewport: Viewport,
        ordinary: &NowPlayingLayout,
        lyrics: &NowPlayingLayout,
    ) -> Result<PreparedMetadata<'window>, String> {
        let metadata = metadata_layout(presentation, viewport);
        let plan = metadata.fitting_group_plan(
            ordinary.information.musical_metadata_width_px,
            ordinary.metadata_height_budget_px,
            ordinary.metadata_fitting,
            |role, text, size| self.text.measure(role.into(), text, size),
        );
        let mut result = Vec::new();
        let mut ordinary_endpoints = Vec::new();
        let mut album_index = None;
        let mut top = (ordinary.metadata_region_top_viewport_y_px
            + ordinary.metadata_group_offset_px(plan.height_px)) as f32;
        let mut previous = None;
        for (line, face, role) in [
            (&plan.title, Face::Title, TextRole::Primary),
            (&plan.artist, Face::Artist, TextRole::Secondary),
            (&plan.album, Face::Album, TextRole::Secondary),
        ] {
            let Some(line) = line else {
                continue;
            };
            if let Some(previous) = previous {
                top += if previous == Face::Title {
                    plan.title_to_credit_gap_px
                } else {
                    plan.album_gap_px
                } as f32;
            }
            let native = self
                .text
                .layout(&line.lines.join("\n"), face, line.font_size_px, 0);
            let prepared = self.text.rasterize(&native, 1.0)?;
            if face == Face::Album {
                album_index = Some(result.len());
            } else {
                ordinary_endpoints.push((
                    role,
                    MetadataEndpoint {
                        words: self.text.words(&native, line.ellipsized)?,
                        size: line.font_size_px as f32,
                        y: top,
                        width: ordinary.information.musical_metadata_width_px as f32,
                    },
                ));
            }
            let height = prepared.height;
            result.push(TextAt {
                text: prepared,
                x: 0.0,
                y: top,
                role,
            });
            top += height;
            previous = Some(face);
        }
        let mut masthead = Vec::new();
        let mut movement = Vec::new();
        let mut top = 0.0;
        for (text, face, size, role) in [
            (
                &presentation.title,
                Face::Masthead,
                lyrics.typography.lyric_masthead_title_px,
                TextRole::Primary,
            ),
            (
                &presentation.artist,
                Face::Artist,
                lyrics.typography.lyric_masthead_artist_px,
                TextRole::Secondary,
            ),
        ] {
            if let Some(text) = text {
                if !masthead.is_empty() {
                    top +=
                        (lyrics.typography.lyric_masthead_artist_px as f64 * 0.25).round() as f32;
                }
                let native = self.text.layout(text, face, size, 0);
                native.set_width(lyrics.lyric_width_px.max(1) as i32 * gtk::pango::SCALE);
                native.set_single_paragraph_mode(true);
                native.set_ellipsize(gtk::pango::EllipsizeMode::End);
                let prepared = self.text.rasterize(&native, 1.0)?;
                let index = ordinary_endpoints
                    .iter()
                    .position(|(ordinary_role, _)| *ordinary_role == role)
                    .expect("ordinary metadata has the same Title and Artist roles");
                let (_, ordinary) = ordinary_endpoints.remove(index);
                movement.push(MetadataMovement::new(
                    ordinary,
                    MetadataEndpoint {
                        words: self.text.words(&native, false)?,
                        size: size as f32,
                        y: lyrics.metadata_region_top_viewport_y_px as f32 + top,
                        width: lyrics.lyric_width_px as f32,
                    },
                    role,
                ));
                let height = prepared.height;
                masthead.push(TextAt {
                    text: prepared,
                    x: 0.0,
                    y: top,
                    role,
                });
                top += height;
            }
        }
        Ok(PreparedMetadata {
            ordinary: result,
            masthead,
            masthead_height: top,
            album_index,
            album_displacement: movement
                .last()
                .map_or(0.0, |movement| movement.compact.y - movement.ordinary.y),
            movement,
        })
    }

    fn identity(
        &self,
        output: &str,
        zone: Option<&str>,
        layout: IdentityRowLayout,
        size: u32,
        full: bool,
    ) -> Result<PreparedIdentity<'window>, String> {
        let label_face = if full {
            Face::FullStrong
        } else {
            Face::IdentityLabel
        };
        let name_face = if full {
            Face::FullStrong
        } else {
            Face::IdentityName
        };
        let mut text = Vec::new();
        let mut x = 0.0;
        let mut height = 0.0f32;
        let mut separator = None;
        for (label, name, budget) in [
            ("OUTPUT", Some(output), layout.output_phrase_max_width_px),
            ("ZONE", zone, layout.zone_phrase_max_width_px),
        ] {
            let Some(name) = name else {
                continue;
            };
            if !text.is_empty() {
                x += layout.phrase_gap_px as f32;
                separator = Some(Rect::new(
                    x,
                    0.0,
                    layout.separator_size_px as f32,
                    layout.separator_size_px as f32,
                ));
                x += (layout.separator_size_px + layout.phrase_gap_px) as f32;
            }
            let label_layout = self.text.layout(
                label,
                label_face,
                layout.label_px,
                layout.label_letter_spacing_px,
            );
            let label = self.text.rasterize(&label_layout, 1.0)?;
            let name_width = budget
                .saturating_sub(label.width.ceil() as u32 + layout.label_gap_px)
                .max(1);
            let name = self.text.line(name, name_face, size, name_width, 0)?;
            let baseline = label.baseline.max(name.baseline);
            let label_y = baseline - label.baseline;
            let name_y = baseline - name.baseline;
            height = height.max(label_y + label.height).max(name_y + name.height);
            let name_x = x + label.width + layout.label_gap_px as f32;
            text.push(TextAt {
                text: label,
                x,
                y: label_y,
                role: TextRole::Muted,
            });
            x = name_x + name.width;
            text.push(TextAt {
                text: name,
                x: name_x,
                y: name_y,
                role: TextRole::Secondary,
            });
        }
        if let Some(separator) = &mut separator {
            separator.y = (height - separator.height) / 2.0;
        }
        Ok(PreparedIdentity {
            text,
            separator,
            height,
        })
    }

    fn all_statuses(
        &self,
        layout: PresentationStatusLayout,
        width: u32,
        full: bool,
    ) -> Result<Vec<(PresentationStatusSymbol, PreparedStatus<'window>)>, String> {
        let mut statuses = Vec::new();
        for (symbol, label) in [
            (PresentationStatusSymbol::Playing, "PLAYING"),
            (PresentationStatusSymbol::Paused, "PAUSED"),
            (PresentationStatusSymbol::Starting, "STARTING"),
            (PresentationStatusSymbol::Idle, "IDLE"),
            (
                PresentationStatusSymbol::PairingRequired,
                "PAIRING REQUIRED",
            ),
            (PresentationStatusSymbol::Disconnected, "DISCONNECTED"),
            (
                PresentationStatusSymbol::OutputUnavailable,
                "OUTPUT UNAVAILABLE",
            ),
        ] {
            statuses.push((symbol, self.status(symbol, label, layout, width, full)?));
        }
        Ok(statuses)
    }

    fn status(
        &self,
        symbol: PresentationStatusSymbol,
        label: &str,
        layout: PresentationStatusLayout,
        width: u32,
        full: bool,
    ) -> Result<PreparedStatus<'window>, String> {
        let key = format!("{symbol:?}|{label}|{layout:?}|{width}|{full}");
        let mut statuses = self.statuses.borrow_mut();
        if let Some(index) = statuses.iter().position(|(cached, _)| cached == &key) {
            let entry = statuses.remove(index).unwrap();
            let result = entry.1.clone();
            statuses.push_back(entry);
            return Ok(result);
        }
        let face = if full { Face::FullStatus } else { Face::Status };
        let label = self.text.line(
            label,
            face,
            layout.font_px,
            width.saturating_sub(layout.symbol_size_px + layout.symbol_gap_px),
            layout.letter_spacing_px,
        )?;
        let circular = layout.decoration == PresentationStatusDecoration::Circle;
        let glyph = self.mask(layout.symbol_size_px, |context| {
            crate::status_glyph::paint_glyph(
                context,
                layout.symbol_size_px as i32,
                layout.symbol_size_px as i32,
                symbol,
                0.0,
                if circular { 0.44 } else { 1.0 },
            );
            Ok(())
        })?;
        let circle = if circular {
            Some(self.mask(layout.symbol_size_px, |context| {
                let size = layout.symbol_size_px as f64;
                context.arc(
                    size / 2.0,
                    size / 2.0,
                    size / 2.0 - 0.5,
                    0.0,
                    std::f64::consts::TAU,
                );
                context.set_source_rgba(1.0, 1.0, 1.0, 0.08);
                context.fill_preserve().map_err(|e| e.to_string())?;
                context.set_source_rgba(1.0, 1.0, 1.0, 0.48);
                context.set_line_width(1.0);
                context.stroke().map_err(|e| e.to_string())
            })?)
        } else {
            None
        };
        let prepared = PreparedStatus {
            label,
            glyph,
            circle,
            layout,
        };
        if statuses.len() >= 28 {
            statuses.pop_front();
        }
        statuses.push_back((key, prepared.clone()));
        Ok(prepared)
    }

    fn mask(
        &self,
        size: u32,
        paint: impl FnOnce(&cairo::Context) -> Result<(), String>,
    ) -> Result<Texture<'window>, String> {
        let pixels = (size as f64 * self.scale).ceil() as i32;
        let surface = cairo::ImageSurface::create(cairo::Format::A8, pixels, pixels)
            .map_err(|e| e.to_string())?;
        surface.set_device_scale(self.scale, self.scale);
        let context = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
        context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        paint(&context)?;
        drop(context);
        let stride = surface.stride() as usize;
        let data = surface.take_data().map_err(|e| e.to_string())?;
        let mask: Vec<_> = data
            .chunks_exact(stride)
            .flat_map(|row| row[..pixels as usize].iter().copied())
            .collect();
        self.uploader.mask(pixels as u32, pixels as u32, &mask)
    }

    fn full_field(
        &mut self,
        presentation: &FullFieldPresentation,
        viewport: Viewport,
    ) -> Result<PreparedFullField<'window>, String> {
        let layout = FullFieldLayout::for_viewport(viewport);
        let mut text = Vec::new();
        for (value, face, sizes, slot, role) in [
            (
                Some(presentation.heading),
                Face::FullHeading,
                layout.heading_font,
                layout.heading_slot,
                TextRole::Primary,
            ),
            (
                presentation.explanation,
                Face::FullUtility,
                layout.explanation_font,
                layout.explanation_slot,
                TextRole::Muted,
            ),
        ] {
            if let Some(value) = value {
                let size = sizes.fitting_font_size(|size| {
                    self.text.measure(face, value, size).0 <= layout.text_width_px()
                });
                let prepared = self
                    .text
                    .line(value, face, size, layout.text_width_px(), 0)?;
                let top = slot.top_viewport_y_px as f32;
                text.push(TextAt {
                    text: prepared,
                    x: layout.text_left_viewport_x_px as f32,
                    y: top,
                    role,
                });
            }
        }
        let statuses =
            self.all_statuses(layout.presentation_status, layout.text_width_px(), true)?;
        let identity = if let Some(identity) = &presentation.identity {
            let (output, zone) = match identity {
                PresentationIdentity::OutputAndZone {
                    tracked_output,
                    tracked_zone,
                } => (tracked_output.as_str(), Some(tracked_zone.as_str())),
                PresentationIdentity::OutputOnly { tracked_output } => {
                    (tracked_output.as_str(), None)
                }
            };
            let label_px = (layout.identity_px as f64 * 0.84).round() as u32;
            let separator_size_px = IdentityRowLayout::separator_diameter_px(layout.identity_px);
            let gap = layout.identity_gap_px.div_ceil(2);
            let width = layout
                .identity_width_px
                .saturating_sub(separator_size_px + gap * 2)
                / 2;
            let row = IdentityRowLayout {
                output_phrase_max_width_px: if zone.is_some() {
                    width
                } else {
                    layout.identity_width_px
                },
                zone_phrase_max_width_px: width,
                phrase_gap_px: gap,
                label_px,
                label_letter_spacing_px: IdentityRowLayout::tracked_label_letter_spacing_px(
                    label_px,
                ),
                label_gap_px: label_px / 2,
                separator_size_px,
            };
            Some(self.identity(output, zone, row, layout.identity_px, true)?)
        } else {
            None
        };
        Ok(PreparedFullField {
            text,
            statuses,
            identity,
        })
    }
}

fn artwork_key(presentation: &Presentation) -> Option<(&str, Option<u64>)> {
    match presentation {
        Presentation::NowPlaying(value) => value
            .artwork_path
            .as_deref()
            .map(|path| (path, value.artwork_revision)),
        Presentation::FullField(_) => None,
    }
}

/// Only fields that require new text or geometry participate in preparation.
/// Live timing and status use already prepared numeric and status sprites.
pub(crate) fn content_key(presentation: &Presentation) -> Presentation {
    let mut key = presentation.clone();
    if let Presentation::NowPlaying(value) = &mut key {
        value.artwork_path = None;
        value.artwork_revision = None;
        value.progress = None;
        value.playback_position_seconds = None;
        value.status = roonscape_renderer::PresentationStatus {
            label: "PLAYING",
            symbol: PresentationStatusSymbol::Playing,
            motion: roonscape_renderer::PresentationStatusMotion::Static,
            emphasis: roonscape_renderer::PresentationStatusEmphasis::FullAccent,
        };
        value.lyrics_known = false;
        if let Some(lyrics) = &mut value.lyrics {
            lyrics.preparing = false;
        }
    }
    key
}

pub(crate) fn preparation_key(presentation: &Presentation) -> Presentation {
    let mut key = content_key(presentation);
    if let (Presentation::NowPlaying(target), Presentation::NowPlaying(source)) =
        (&mut key, presentation)
    {
        target.artwork_path.clone_from(&source.artwork_path);
        target.artwork_revision = source.artwork_revision;
    }
    key
}
