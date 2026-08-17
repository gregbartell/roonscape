use std::error::Error;
use std::fmt;
use std::path::Path;

use gdk_pixbuf::Pixbuf;

const SAMPLE_SIZE: i32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }

    pub fn contrast_ratio(self, other: Self) -> f64 {
        let first = self.relative_luminance();
        let second = other.relative_luminance();
        let (lighter, darker) = if first > second {
            (first, second)
        } else {
            (second, first)
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    fn relative_luminance(self) -> f64 {
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.red) + 0.7152 * channel(self.green) + 0.0722 * channel(self.blue)
    }

    fn mix(self, other: Self, amount: f64) -> Self {
        let mix_channel = |first: u8, second: u8| {
            (f64::from(first) * (1.0 - amount) + f64::from(second) * amount).round() as u8
        };
        Self::new(
            mix_channel(self.red, other.red),
            mix_channel(self.green, other.green),
            mix_channel(self.blue, other.blue),
        )
    }

    fn hsl(self) -> Hsl {
        let red = f64::from(self.red) / 255.0;
        let green = f64::from(self.green) / 255.0;
        let blue = f64::from(self.blue) / 255.0;
        let maximum = red.max(green).max(blue);
        let minimum = red.min(green).min(blue);
        let chroma = maximum - minimum;
        let lightness = (maximum + minimum) / 2.0;
        let saturation = if chroma == 0.0 {
            0.0
        } else {
            chroma / (1.0 - (2.0 * lightness - 1.0).abs())
        };
        let hue = if chroma == 0.0 {
            0.0
        } else if maximum == red {
            60.0 * ((green - blue) / chroma).rem_euclid(6.0)
        } else if maximum == green {
            60.0 * ((blue - red) / chroma + 2.0)
        } else {
            60.0 * ((red - green) / chroma + 4.0)
        };

        Hsl {
            hue,
            saturation,
            lightness,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Hsl {
    hue: f64,
    saturation: f64,
    lightness: f64,
}

impl Hsl {
    fn rgb(self) -> Rgb {
        let chroma = (1.0 - (2.0 * self.lightness - 1.0).abs()) * self.saturation;
        let segment = self.hue / 60.0;
        let x = chroma * (1.0 - (segment.rem_euclid(2.0) - 1.0).abs());
        let (red, green, blue) = match segment as u8 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let offset = self.lightness - chroma / 2.0;
        let channel = |value: f64| ((value + offset).clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgb::new(channel(red), channel(green), channel(blue))
    }

    fn with_saturation_and_lightness(self, saturation: f64, lightness: f64) -> Self {
        Self {
            hue: self.hue,
            saturation,
            lightness,
        }
    }
}

#[derive(Clone, Copy)]
enum PaletteTone {
    Dark,
    Light,
}

#[derive(Clone, Copy)]
struct ToneProfile {
    background_lightness: f64,
    metadata_lightness: f64,
    artwork_lightness: f64,
    primary_lightness: f64,
    secondary_lightness: f64,
    accent_lightness: f64,
    contrast_step: f64,
}

impl PaletteTone {
    fn profile(self) -> ToneProfile {
        match self {
            Self::Dark => ToneProfile {
                background_lightness: 0.065,
                metadata_lightness: 0.19,
                artwork_lightness: 0.3,
                primary_lightness: 0.88,
                secondary_lightness: 0.68,
                accent_lightness: 0.58,
                contrast_step: 0.02,
            },
            Self::Light => ToneProfile {
                background_lightness: 0.92,
                metadata_lightness: 0.82,
                artwork_lightness: 0.72,
                primary_lightness: 0.14,
                secondary_lightness: 0.28,
                accent_lightness: 0.32,
                contrast_step: -0.02,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationPalette {
    pub background: Rgb,
    pub artwork_field: Rgb,
    pub metadata_field: Rgb,
    pub primary_text: Rgb,
    pub secondary_text: Rgb,
    pub muted_text: Rgb,
    pub accent: Rgb,
    pub status_muted_accent: Rgb,
    pub progress_track: Rgb,
    pub progress_fill: Rgb,
    pub diagnostics_field: Rgb,
    pub diagnostics_text: Rgb,
    pub diagnostics_border: Rgb,
}

impl PresentationPalette {
    pub const fn fallback() -> Self {
        Self {
            background: Rgb::new(0x07, 0x15, 0x22),
            artwork_field: Rgb::new(0x14, 0x28, 0x56),
            metadata_field: Rgb::new(0x0a, 0x14, 0x29),
            primary_text: Rgb::new(0xf3, 0xea, 0xd7),
            secondary_text: Rgb::new(0xc9, 0xc5, 0xbd),
            muted_text: Rgb::new(0x92, 0x99, 0xa8),
            accent: Rgb::new(0xff, 0x70, 0x51),
            status_muted_accent: Rgb::new(0xc3, 0x87, 0x81),
            progress_track: Rgb::new(0x92, 0x99, 0xa8),
            progress_fill: Rgb::new(0xff, 0x70, 0x51),
            diagnostics_field: Rgb::new(0x0a, 0x14, 0x29),
            diagnostics_text: Rgb::new(0xf3, 0xea, 0xd7),
            diagnostics_border: Rgb::new(0xff, 0x70, 0x51),
        }
    }

    pub fn from_artwork(path: &Path) -> Result<Self, PaletteError> {
        let pixbuf = Pixbuf::from_file_at_scale(path, SAMPLE_SIZE, SAMPLE_SIZE, true)
            .map_err(PaletteError::Load)?;
        let swatches = swatches(&pixbuf);
        let dominant = swatches
            .iter()
            .max_by_key(|swatch| swatch.count)
            .ok_or(PaletteError::NoVisiblePixels)?
            .color;
        let vibrant = swatches
            .iter()
            .max_by(|first, second| {
                swatch_score(first)
                    .total_cmp(&swatch_score(second))
                    .then_with(|| first.count.cmp(&second.count))
            })
            .map_or(dominant, |swatch| swatch.color);
        let light = swatches
            .iter()
            .max_by(|first, second| {
                text_score(first)
                    .total_cmp(&text_score(second))
                    .then_with(|| first.count.cmp(&second.count))
            })
            .map_or(dominant, |swatch| swatch.color);

        let dominant_hsl = dominant.hsl();
        let vibrant_hsl = vibrant.hsl();
        let light_hsl = light.hsl();
        let profile = palette_tone(&swatches).profile();
        let background = dominant_hsl
            .with_saturation_and_lightness(
                dominant_hsl.saturation.clamp(0.12, 0.52),
                profile.background_lightness,
            )
            .rgb();
        let metadata_field = background.mix(
            dominant_hsl
                .with_saturation_and_lightness(
                    dominant_hsl.saturation.clamp(0.14, 0.58),
                    profile.metadata_lightness,
                )
                .rgb(),
            0.34,
        );
        let artwork_field = background.mix(
            vibrant_hsl
                .with_saturation_and_lightness(
                    vibrant_hsl.saturation.clamp(0.24, 0.72),
                    profile.artwork_lightness,
                )
                .rgb(),
            0.42,
        );
        let presentation_fields = [background, artwork_field, metadata_field];
        let primary_text = readable_tint(
            light_hsl.with_saturation_and_lightness(
                light_hsl.saturation.clamp(0.08, 0.24),
                profile.primary_lightness,
            ),
            &presentation_fields,
            7.0,
            profile.contrast_step,
        );
        let secondary_text = readable_tint(
            dominant_hsl.with_saturation_and_lightness(
                dominant_hsl.saturation.clamp(0.12, 0.3),
                profile.secondary_lightness,
            ),
            &presentation_fields,
            4.5,
            profile.contrast_step,
        );
        let muted_text = readable_tint(
            dominant_hsl.with_saturation_and_lightness(
                dominant_hsl.saturation.clamp(0.08, 0.2),
                profile.secondary_lightness,
            ),
            &presentation_fields,
            4.5,
            profile.contrast_step,
        );
        let accent = readable_tint(
            vibrant_hsl.with_saturation_and_lightness(
                vibrant_hsl.saturation.clamp(0.48, 0.86),
                profile.accent_lightness,
            ),
            &presentation_fields,
            4.5,
            profile.contrast_step,
        );
        let status_muted_accent = accent.mix(muted_text, 0.55);

        Ok(Self {
            background,
            artwork_field,
            metadata_field,
            primary_text,
            secondary_text,
            muted_text,
            accent,
            status_muted_accent,
            progress_track: muted_text,
            progress_fill: accent,
            diagnostics_field: metadata_field,
            diagnostics_text: primary_text,
            diagnostics_border: accent,
        })
    }

    pub fn for_artwork(path: Option<&Path>) -> Self {
        match path {
            Some(path) => Self::from_artwork(path).unwrap_or_else(|_| Self::fallback()),
            None => Self::fallback(),
        }
    }
}

#[derive(Debug)]
pub enum PaletteError {
    Load(gtk::glib::Error),
    NoVisiblePixels,
}

impl fmt::Display for PaletteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(error) => write!(formatter, "could not load artwork for palette: {error}"),
            Self::NoVisiblePixels => formatter.write_str("artwork has no visible pixels"),
        }
    }
}

impl Error for PaletteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Load(error) => Some(error),
            Self::NoVisiblePixels => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Swatch {
    color: Rgb,
    count: u32,
}

#[derive(Clone, Copy, Default)]
struct Bucket {
    red: u64,
    green: u64,
    blue: u64,
    count: u32,
}

fn swatches(pixbuf: &Pixbuf) -> Vec<Swatch> {
    let pixels = pixbuf.read_pixel_bytes();
    let pixels = pixels.as_ref();
    let channels = pixbuf.n_channels() as usize;
    let row_stride = pixbuf.rowstride() as usize;
    let mut buckets = [Bucket::default(); 4096];

    for row in 0..pixbuf.height() as usize {
        for column in 0..pixbuf.width() as usize {
            let offset = row * row_stride + column * channels;
            if channels == 4 && pixels[offset + 3] < 128 {
                continue;
            }
            let red = pixels[offset];
            let green = pixels[offset + 1];
            let blue = pixels[offset + 2];
            let index = (usize::from(red >> 4) << 8)
                | (usize::from(green >> 4) << 4)
                | usize::from(blue >> 4);
            let bucket = &mut buckets[index];
            bucket.red += u64::from(red);
            bucket.green += u64::from(green);
            bucket.blue += u64::from(blue);
            bucket.count += 1;
        }
    }

    buckets
        .into_iter()
        .filter(|bucket| bucket.count > 0)
        .map(|bucket| Swatch {
            color: Rgb::new(
                (bucket.red / u64::from(bucket.count)) as u8,
                (bucket.green / u64::from(bucket.count)) as u8,
                (bucket.blue / u64::from(bucket.count)) as u8,
            ),
            count: bucket.count,
        })
        .collect()
}

fn swatch_score(swatch: &Swatch) -> f64 {
    let hsl = swatch.color.hsl();
    let useful_lightness = 1.0 - (hsl.lightness - 0.55).abs();
    hsl.saturation.powi(2) * useful_lightness.max(0.2) * (1.0 + f64::from(swatch.count).ln())
}

fn text_score(swatch: &Swatch) -> f64 {
    let hsl = swatch.color.hsl();
    swatch.color.relative_luminance() + hsl.saturation * 0.18 + f64::from(swatch.count).ln() * 0.01
}

fn palette_tone(swatches: &[Swatch]) -> PaletteTone {
    let (weighted_luminance, pixel_count) =
        swatches
            .iter()
            .fold((0.0, 0_u64), |(weighted_luminance, pixel_count), swatch| {
                (
                    weighted_luminance
                        + swatch.color.relative_luminance() * f64::from(swatch.count),
                    pixel_count + u64::from(swatch.count),
                )
            });
    if weighted_luminance / pixel_count as f64 >= 0.55 {
        PaletteTone::Light
    } else {
        PaletteTone::Dark
    }
}

fn readable_tint(mut tint: Hsl, fields: &[Rgb], minimum_contrast: f64, contrast_step: f64) -> Rgb {
    let mut color = tint.rgb();
    while fields
        .iter()
        .any(|field| color.contrast_ratio(*field) < minimum_contrast)
    {
        let next_lightness = (tint.lightness + contrast_step).clamp(0.02, 0.98);
        if next_lightness == tint.lightness {
            break;
        }
        tint.lightness = next_lightness;
        color = tint.rgb();
    }
    color
}
