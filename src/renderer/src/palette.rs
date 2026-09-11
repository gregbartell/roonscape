use std::error::Error;
use std::fmt;
use std::path::Path;

use gdk_pixbuf::Pixbuf;

const SAMPLE_SIZE: i32 = 64;
// Perceptual distances in OKLab, independent of the gradient geometry.
const MINIMUM_ENDPOINT_FIELD_SEPARATION: f64 = 0.12;
const MINIMUM_DARK_ADJACENT_FIELD_SEPARATION: f64 = 0.05;
const MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION: f64 = 0.045;
const FAMILY_CLUSTER_DISTANCE: f64 = 0.045;
const MINIMUM_CHROMATIC_FAMILY_CHROMA: f64 = 0.025;
const MINIMUM_FIELD_FAMILY_SHARE: f64 = 0.08;
const MINIMUM_ACCENT_DETAIL_SHARE: f64 = 0.008;

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

    fn oklab(self) -> Oklab {
        let channel = |value: u8| {
            let encoded = f64::from(value) / 255.0;
            if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        };
        let red = channel(self.red);
        let green = channel(self.green);
        let blue = channel(self.blue);
        let l = (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
        let m = (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
        let s = (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();

        Oklab {
            lightness: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
            a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
            b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
        }
    }

    fn perceptual_distance(self, other: Self) -> f64 {
        let first = self.oklab();
        let second = other.oklab();
        first.distance(second)
    }

    fn oklch(self) -> Oklch {
        self.oklab().oklch()
    }
}

#[derive(Clone, Copy, Debug)]
struct Oklab {
    lightness: f64,
    a: f64,
    b: f64,
}

impl Oklab {
    fn distance(self, other: Self) -> f64 {
        ((self.lightness - other.lightness).powi(2)
            + (self.a - other.a).powi(2)
            + (self.b - other.b).powi(2))
        .sqrt()
    }

    fn oklch(self) -> Oklch {
        Oklch {
            lightness: self.lightness,
            chroma: (self.a * self.a + self.b * self.b).sqrt(),
            hue: self.b.atan2(self.a),
        }
    }

    fn weighted_average(self, self_count: u32, sample: Self, sample_count: u32) -> Self {
        let combined_count = f64::from(self_count + sample_count);
        let average = |current, sampled| {
            (current * f64::from(self_count) + sampled * f64::from(sample_count)) / combined_count
        };
        Self {
            lightness: average(self.lightness, sample.lightness),
            a: average(self.a, sample.a),
            b: average(self.b, sample.b),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Oklch {
    lightness: f64,
    chroma: f64,
    hue: f64,
}

impl Oklch {
    fn rgb(self) -> Rgb {
        let mut chroma = self.chroma;
        loop {
            let a = chroma * self.hue.cos();
            let b = chroma * self.hue.sin();
            let l = (self.lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
            let m = (self.lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
            let s = (self.lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
            let linear = [
                4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
                -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
                -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
            ];
            if linear.iter().all(|channel| (0.0..=1.0).contains(channel)) || chroma <= 0.001 {
                let encode = |channel: f64| {
                    let encoded = if channel <= 0.003_130_8 {
                        12.92 * channel
                    } else {
                        1.055 * channel.powf(1.0 / 2.4) - 0.055
                    };
                    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
                };
                return Rgb::new(encode(linear[0]), encode(linear[1]), encode(linear[2]));
            }
            chroma *= 0.96;
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
    primary_lightness: f64,
    secondary_lightness: f64,
    accent_lightness: f64,
    contrast_step: f64,
}

impl PaletteTone {
    fn profile(self) -> ToneProfile {
        match self {
            Self::Dark => ToneProfile {
                primary_lightness: 0.9,
                secondary_lightness: 0.68,
                accent_lightness: 0.8,
                contrast_step: 0.02,
            },
            Self::Light => ToneProfile {
                primary_lightness: 0.12,
                secondary_lightness: 0.26,
                accent_lightness: 0.38,
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

fn sample_artwork(pixbuf: &Pixbuf) -> Option<Pixbuf> {
    let scale = f64::from(SAMPLE_SIZE) / f64::from(pixbuf.width().max(pixbuf.height()));
    let width = (f64::from(pixbuf.width()) * scale).round().max(1.0) as i32;
    let height = (f64::from(pixbuf.height()) * scale).round().max(1.0) as i32;
    if height < 2 || i64::from(pixbuf.width()) * i64::from(pixbuf.height()) <= 512 * 512 {
        return pixbuf.scale_simple(width, height, gdk_pixbuf::InterpType::Bilinear);
    }
    let bytes = pixbuf.read_pixel_bytes();
    let source_width = pixbuf.width();
    let source_height = pixbuf.height();
    let source_stride = pixbuf.rowstride();
    let alpha = pixbuf.has_alpha();
    let channels = pixbuf.n_channels();
    // Each worker owns its Pixbuf objects and reads the same immutable bytes.
    // Offsets preserve the original filter coordinates across the strips.
    let strip = |start: i32, rows: i32| {
        let source = Pixbuf::from_bytes(
            &bytes,
            gdk_pixbuf::Colorspace::Rgb,
            alpha,
            8,
            source_width,
            source_height,
            source_stride,
        );
        if let Some(sample) = sample_integer_strip(&source, width, height, start, rows) {
            return Some(sample);
        }
        let output = Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, alpha, 8, width, rows)?;
        source.scale(
            &output,
            0,
            0,
            width,
            rows,
            0.0,
            -f64::from(start),
            f64::from(width) / f64::from(source_width),
            f64::from(height) / f64::from(source_height),
            gdk_pixbuf::InterpType::Bilinear,
        );
        Some((output.read_pixel_bytes(), output.rowstride()))
    };
    let workers = height.min(4);
    let ranges: Vec<_> = (0..workers)
        .map(|index| {
            let start = index * height / workers;
            (start, (index + 1) * height / workers - start)
        })
        .collect();
    let samples = std::thread::scope(|scope| {
        let pending: Vec<_> = ranges
            .iter()
            .skip(1)
            .map(|&(start, rows)| scope.spawn(move || strip(start, rows)))
            .collect();
        std::iter::once(strip(ranges[0].0, ranges[0].1))
            .chain(pending.into_iter().map(|worker| worker.join().unwrap()))
            .collect::<Option<Vec<_>>>()
    })?;
    let stride = (width * channels + 3) & !3;
    let mut pixels = vec![0; stride as usize * height as usize];
    for ((start, rows), (bytes, source_stride)) in ranges.into_iter().zip(samples) {
        for row in 0..rows {
            let source = (row * source_stride) as usize;
            let target = ((start + row) * stride) as usize;
            let count = (width * channels) as usize;
            pixels[target..target + count].copy_from_slice(&bytes[source..source + count]);
        }
    }
    Some(Pixbuf::from_mut_slice(
        pixels,
        gdk_pixbuf::Colorspace::Rgb,
        alpha,
        8,
        width,
        height,
        stride,
    ))
}

fn sample_integer_strip(
    source: &Pixbuf,
    width: i32,
    height: i32,
    start: i32,
    rows: i32,
) -> Option<(gtk::glib::Bytes, i32)> {
    let factor = source.width() / width;
    // Larger reductions use GdkPixbuf's two-stage scaler. Alpha and fractional
    // reductions also retain its general sampling path.
    if source.has_alpha()
        || source.n_channels() != 3
        || !(2..=30).contains(&factor)
        || source.width() != width * factor
        || source.height() != height * factor
    {
        return None;
    }
    let factor = factor as usize;
    let count = factor * factor;
    let unit = ((65_536 + count / 2) / count) as i64;
    let correction = 65_536 - unit * count as i64;
    let mut adjustments = Vec::new();
    // Integer box weights are equal before normalization. Sum their pixels
    // once, then apply the sparse correction that makes the weights total 2^16.
    // Match GdkPixbuf's reverse-order correction without building phase tables.
    if correction > 0 {
        adjustments.push((factor, factor, correction));
    } else if correction < 0 {
        let divisor = (-correction + unit - 1) / unit;
        let chunk = correction / divisor;
        let mut remaining = correction;
        for index in (0..count).rev() {
            let amount = remaining.max(chunk);
            adjustments.push((index % factor, index / factor, amount));
            remaining -= amount;
            if remaining == 0 {
                break;
            }
        }
        debug_assert_eq!(remaining, 0);
    }
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let source_stride = source.rowstride() as usize;
    let bytes = source.read_pixel_bytes();
    let stride = (width * 3 + 3) & !3;
    let mut output = vec![0; (stride * rows) as usize];
    for row in 0..rows as usize {
        let y = start as usize + row;
        for x in 0..width as usize {
            let mut sums = [0_u32; 3];
            for dy in 0..factor {
                let at = (y * factor + dy) * source_stride + x * factor * 3;
                for pixel in bytes[at..at + factor * 3].chunks_exact(3) {
                    sums[0] += u32::from(pixel[0]);
                    sums[1] += u32::from(pixel[1]);
                    sums[2] += u32::from(pixel[2]);
                }
            }
            let mut weighted = sums.map(|sum| i64::from(sum) * unit);
            for &(dx, dy, weight) in &adjustments {
                let at = (y * factor + dy).min(source_height - 1) * source_stride
                    + (x * factor + dx).min(source_width - 1) * 3;
                for channel in 0..3 {
                    weighted[channel] += weight * i64::from(bytes[at + channel]);
                }
            }
            for channel in 0..3 {
                let value = weighted[channel] as u64;
                // GdkPixbuf's clamped right-edge path uses an 8-bit opacity
                // factor; retain that rounding as well as interior rounding.
                output[row * stride as usize + x * 3 + channel] = if x + 1 == width as usize {
                    ((value * 255 + 0xff_ffff) >> 24) as u8
                } else {
                    ((value + 0xffff) >> 16) as u8
                };
            }
        }
    }
    Some((gtk::glib::Bytes::from_owned(output), stride))
}

impl PresentationPalette {
    /// Blend artwork fields without letting text converge with a dark/light
    /// midpoint. Exact endpoint palettes retain their calibrated role hierarchy.
    pub fn mix(self, other: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        if amount == 0.0 || self == other {
            return self;
        }
        if amount == 1.0 {
            return other;
        }
        let background = self.background.mix(other.background, amount);
        let metadata_field = self.metadata_field.mix(other.metadata_field, amount);
        let fields = [background, metadata_field];
        let minimum_contrast = |color: Rgb, fields: &[Rgb]| {
            fields
                .iter()
                .map(|field| color.contrast_ratio(*field))
                .fold(f64::INFINITY, f64::min)
        };
        let readable_mix = |first: Rgb, second: Rgb| {
            let color = first.mix(second, amount);
            let target = minimum_contrast(first, &[self.background, self.metadata_field])
                .min(minimum_contrast(
                    second,
                    &[other.background, other.metadata_field],
                ))
                .min(7.0);
            if minimum_contrast(color, &fields) >= target {
                return color;
            }
            // Mid-gray cannot support 7:1 even with black or white. Search both
            // polarities and retain the best feasible contrast, using the same
            // hue-preserving lightness adjustment as settled palette selection.
            [-0.02, 0.02]
                .into_iter()
                .map(|step| readable_tint(color.hsl(), &fields, target, step))
                .max_by(|left, right| {
                    minimum_contrast(*left, &fields).total_cmp(&minimum_contrast(*right, &fields))
                })
                .expect("two lightness directions")
        };
        Self {
            background,
            artwork_field: self.artwork_field.mix(other.artwork_field, amount),
            metadata_field,
            primary_text: readable_mix(self.primary_text, other.primary_text),
            secondary_text: readable_mix(self.secondary_text, other.secondary_text),
            muted_text: readable_mix(self.muted_text, other.muted_text),
            accent: readable_mix(self.accent, other.accent),
            status_muted_accent: readable_mix(self.status_muted_accent, other.status_muted_accent),
            progress_track: self.progress_track.mix(other.progress_track, amount),
            progress_fill: readable_mix(self.progress_fill, other.progress_fill),
            diagnostics_field: self.diagnostics_field.mix(other.diagnostics_field, amount),
            diagnostics_text: readable_mix(self.diagnostics_text, other.diagnostics_text),
            diagnostics_border: readable_mix(self.diagnostics_border, other.diagnostics_border),
        }
    }

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
            progress_track: Rgb::new(0x2f, 0x36, 0x45),
            progress_fill: Rgb::new(0xff, 0x70, 0x51),
            diagnostics_field: Rgb::new(0x0a, 0x14, 0x29),
            diagnostics_text: Rgb::new(0xf3, 0xea, 0xd7),
            diagnostics_border: Rgb::new(0xff, 0x70, 0x51),
        }
    }

    pub fn from_artwork(path: &Path) -> Result<Self, PaletteError> {
        // Decoder-side thumbnail scaling can select different colors from
        // sampling the full artwork already used by native preparation.
        let pixbuf = Pixbuf::from_file(path).map_err(PaletteError::Load)?;
        Self::from_pixbuf(&pixbuf)
    }

    /// Derive the palette from already decoded pixels, using the same sample
    /// size and resampling as file-backed artwork.
    pub fn from_pixbuf(pixbuf: &Pixbuf) -> Result<Self, PaletteError> {
        let sample = sample_artwork(pixbuf).ok_or(PaletteError::NoVisiblePixels)?;
        Self::from_sample(&sample)
    }

    fn from_sample(pixbuf: &Pixbuf) -> Result<Self, PaletteError> {
        let swatches = swatches(pixbuf);
        let families = color_families(&swatches);
        let total = families.iter().map(|family| family.count).sum::<u32>();
        if total == 0 {
            return Err(PaletteError::NoVisiblePixels);
        }
        // Bound the search by substantial families, not by individual pixels.
        // Both tones compete for every field source; mean luminance never
        // preselects a presentation or excludes a light/dark alternative.
        let mut sources: Vec<_> = families.iter().collect();
        sources.sort_by_key(|family| std::cmp::Reverse(family.count));
        let candidate = sources
            .iter()
            .take(12)
            .filter(|source| {
                source.count == sources[0].count
                    || (f64::from(source.count) / f64::from(total) >= MINIMUM_ACCENT_DETAIL_SHARE
                        && field_support(source, &families, total) >= MINIMUM_FIELD_FAMILY_SHARE)
            })
            .flat_map(|source| {
                [PaletteTone::Dark, PaletteTone::Light]
                    .into_iter()
                    .filter_map(|tone| composition_candidate(source, tone, &families, total))
            })
            .max_by(|first, second| first.score.total_cmp(&second.score))
            .expect("sampled artwork supports a restrained dark composition");
        let [background, artwork_field, metadata_field] = candidate.fields;
        let semantic = candidate.semantic;

        Ok(Self {
            background,
            artwork_field,
            metadata_field,
            primary_text: semantic.primary_text,
            secondary_text: semantic.secondary_text,
            muted_text: semantic.muted_text,
            accent: semantic.accent,
            status_muted_accent: semantic.status_muted_accent,
            progress_track: semantic.progress_track,
            progress_fill: semantic.accent,
            diagnostics_field: metadata_field,
            diagnostics_text: semantic.primary_text,
            diagnostics_border: semantic.accent,
        })
    }

    pub fn for_artwork(path: Option<&Path>) -> Self {
        match path {
            Some(path) => Self::from_artwork(path).unwrap_or_else(|_| Self::fallback()),
            None => Self::fallback(),
        }
    }
}

struct CompositionCandidate {
    fields: [Rgb; 3],
    semantic: SemanticRoles,
    score: f64,
}

// Weak casts carry unreliable hue. Fade their chroma continuously instead of
// amplifying an almost-neutral sample with a saturation floor.
fn supported_color(color: Rgb) -> Oklch {
    let mut color = color.oklch();
    let confidence = ((color.chroma - 0.012) / 0.033).clamp(0.0, 1.0);
    color.chroma *= confidence * confidence * (3.0 - 2.0 * confidence);
    color
}

fn composition_candidate(
    source: &ColorFamily,
    tone: PaletteTone,
    families: &[ColorFamily],
    total: u32,
) -> Option<CompositionCandidate> {
    let color = supported_color(source.color);
    let field = |lightness, retention: f64, ceiling: f64| {
        Oklch {
            lightness,
            chroma: (color.chroma * retention).min(ceiling),
            hue: color.hue,
        }
        .rgb()
    };
    // One family grounds the large fields. Other families can give the small
    // accent roles emphasis without spreading a detail across the room.
    let fields = match tone {
        PaletteTone::Dark => [
            field(0.16, 0.5, 0.04),
            field(0.34, 0.78, 0.16),
            field(0.22, 0.4, 0.055),
        ],
        PaletteTone::Light => compress_bright_palette([
            field(0.825, 0.6, 0.035),
            // Leave headroom above the primary-text contrast floor, so a
            // tiny chroma change cannot disqualify an otherwise sound tone.
            field(0.765, 0.8, 0.065),
            field(0.91, 0.45, 0.025),
        ]),
    };
    let (artwork, metadata) = separate_presentation_fields(tone, fields[0], fields[1], fields[2]);
    let fields = [fields[0], artwork, metadata];
    let accent_source = families
        .iter()
        .filter(|family| f64::from(family.count) / f64::from(total) >= MINIMUM_ACCENT_DETAIL_SHARE)
        .max_by(|first, second| {
            accent_score(first, color, total).total_cmp(&accent_score(second, color, total))
        })
        .unwrap_or(source);
    let accent = supported_color(accent_source.color);
    let text_source = Oklch {
        chroma: (color.chroma * 0.15).min(0.015),
        ..color
    }
    .rgb()
    .hsl();
    let semantic = semantic_roles(tone.profile(), text_source, accent, fields)?;
    let share = field_support(source, families, total);
    let generated = artwork.oklch();
    // Prefer a well-supported family with modest lightness changes and useful
    // retained color. Chroma loss matters: washing a vivid cover into a light
    // field is not automatically preferable to a dark, chromatic surround.
    let color_support = (color.chroma / 0.08).min(1.0) * (share / 0.3).min(1.0);
    let shade_lightness = color.lightness.min(chromatic_lightness(color.hue));
    let chromatic_weight = (color.chroma / 0.08).min(1.0) * 0.85;
    let reference_lightness =
        color.lightness * (1.0 - chromatic_weight) + shade_lightness * chromatic_weight;
    let score = 0.55 * share.sqrt()
        + 0.45 * f64::from(source.count) / f64::from(total)
        + 0.4 * color_support
        + 0.25 * edge_support(source, families)
        - 0.4 * fields[0].relative_luminance()
        - 0.65 * (reference_lightness - generated.lightness).abs()
        - 3.0 * (color.chroma - generated.chroma).abs()
        + 0.4 * accent_score(accent_source, color, total);
    Some(CompositionCandidate {
        fields,
        semantic,
        score,
    })
}

// The saturated sRGB corners give a hue-relative lightness reference: a
// pale blue can retain its family in a deeper field, whereas darkening yellow
// into brown changes its character. This guides tone ranking only; every
// generated hue still comes from artwork, and both tones remain candidates.
fn chromatic_lightness(hue: f64) -> f64 {
    let corners = [
        Rgb::new(255, 0, 0),
        Rgb::new(255, 255, 0),
        Rgb::new(0, 255, 0),
        Rgb::new(0, 255, 255),
        Rgb::new(0, 0, 255),
        Rgb::new(255, 0, 255),
        Rgb::new(255, 0, 0),
    ]
    .map(Rgb::oklch);
    let start = corners[0].hue;
    let hue = (hue - start).rem_euclid(std::f64::consts::TAU);
    for pair in corners.windows(2) {
        let first = (pair[0].hue - start).rem_euclid(std::f64::consts::TAU);
        let mut second = (pair[1].hue - start).rem_euclid(std::f64::consts::TAU);
        if second == 0.0 {
            second = std::f64::consts::TAU;
        }
        if hue <= second {
            let amount = (hue - first) / (second - first);
            return pair[0].lightness * (1.0 - amount) + pair[1].lightness * amount;
        }
    }
    unreachable!("the sRGB corners span the hue circle")
}

fn field_support(source: &ColorFamily, families: &[ColorFamily], total: u32) -> f64 {
    let source = supported_color(source.color);
    families
        .iter()
        .map(|family| {
            let color = supported_color(family.color);
            let similarity = if source.chroma >= 0.02 && color.chroma >= 0.02 {
                let angle = (source.hue - color.hue).abs();
                let angle = angle.min(std::f64::consts::TAU - angle);
                (1.0 - angle / 0.65).max(0.0)
            } else {
                (1.0 - (source.lightness - color.lightness).abs() / 0.25).max(0.0)
                    * (1.0 - (source.chroma - color.chroma).abs() / 0.025).max(0.0)
            };
            f64::from(family.count) * similarity
        })
        .sum::<f64>()
        / f64::from(total)
}

fn edge_support(source: &ColorFamily, families: &[ColorFamily]) -> f64 {
    let total = families.iter().map(|family| family.edge_count).sum::<u32>();
    families
        .iter()
        .map(|family| {
            let distance = ((source.lab.lightness - family.lab.lightness) / 0.3).powi(2)
                + ((source.lab.a - family.lab.a) / 0.08).powi(2)
                + ((source.lab.b - family.lab.b) / 0.08).powi(2);
            f64::from(family.edge_count) * (-distance).exp()
        })
        .sum::<f64>()
        / f64::from(total.max(1))
}

fn accent_score(family: &ColorFamily, field: Oklch, total: u32) -> f64 {
    let color = supported_color(family.color);
    let share = f64::from(family.count) / f64::from(total);
    let distinction = ((color.chroma * color.hue.cos() - field.chroma * field.hue.cos()).powi(2)
        + (color.chroma * color.hue.sin() - field.chroma * field.hue.sin()).powi(2))
    .sqrt();
    // Small authored details remain useful, but their influence tapers with
    // area. Neutral accents can distinguish strongly colored fields too.
    let detail_confidence = (share / 0.03).sqrt().min(1.0);
    let chromatic = (color.chroma / 0.025).min(1.0);
    let neutral_distinction = chromatic + (1.0 - chromatic) * (field.chroma / 0.12).min(1.0);
    0.04 * share.sqrt()
        + detail_confidence
            * (0.6 * distinction * neutral_distinction + 0.5 * color.chroma.min(0.16))
}

#[derive(Clone, Copy)]
struct SemanticRoles {
    primary_text: Rgb,
    secondary_text: Rgb,
    muted_text: Rgb,
    accent: Rgb,
    status_muted_accent: Rgb,
    progress_track: Rgb,
}

fn semantic_roles(
    profile: ToneProfile,
    text: Hsl,
    accent: Oklch,
    fields: [Rgb; 3],
) -> Option<SemanticRoles> {
    let supporting_fields = [fields[0], fields[2]];
    let primary_text = readable_tint(
        text.with_saturation_and_lightness(text.saturation.min(0.12), profile.primary_lightness),
        &fields,
        7.0,
        profile.contrast_step,
    );
    let secondary_text = readable_tint(
        readable_tint(
            text.with_saturation_and_lightness(
                text.saturation.min(0.12),
                profile.secondary_lightness,
            ),
            &supporting_fields,
            7.0,
            profile.contrast_step,
        )
        .hsl(),
        &fields,
        4.5,
        profile.contrast_step,
    );
    let muted_text = readable_tint(
        readable_tint(
            text.with_saturation_and_lightness(
                text.saturation.min(0.08),
                profile.secondary_lightness,
            ),
            &supporting_fields,
            7.0,
            profile.contrast_step,
        )
        .hsl(),
        &fields,
        4.5,
        profile.contrast_step,
    );
    let accent = readable_perceptual_tint(
        Oklch {
            lightness: profile.accent_lightness,
            ..accent
        },
        &fields,
        4.5,
        profile.contrast_step,
    );
    let status_muted_accent = readable_tint(
        accent.mix(muted_text, 0.55).hsl(),
        &fields,
        4.5,
        profile.contrast_step,
    );
    if [
        (primary_text, 7.0),
        (secondary_text, 4.5),
        (muted_text, 4.5),
        (accent, 4.5),
        (status_muted_accent, 4.5),
    ]
    .into_iter()
    .any(|(color, minimum)| {
        fields
            .iter()
            .any(|field| color.contrast_ratio(*field) < minimum)
    }) || [secondary_text, muted_text].into_iter().any(|color| {
        supporting_fields
            .iter()
            .any(|field| color.contrast_ratio(*field) < 7.0)
    }) {
        return None;
    }
    Some(SemanticRoles {
        primary_text,
        secondary_text,
        muted_text,
        accent,
        status_muted_accent,
        progress_track: progress_track_candidate(fields[2], primary_text, accent)?,
    })
}

fn separate_presentation_fields(
    tone: PaletteTone,
    background: Rgb,
    artwork_field: Rgb,
    metadata_field: Rgb,
) -> (Rgb, Rgb) {
    let minimum_adjacent_separation = match tone {
        PaletteTone::Dark => MINIMUM_DARK_ADJACENT_FIELD_SEPARATION,
        PaletteTone::Light => MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION,
    };

    let mut metadata_hsl = metadata_field.hsl();
    let mut metadata_oklch = metadata_field.oklch();
    let mut separated_metadata = metadata_field;
    while background.perceptual_distance(separated_metadata) < minimum_adjacent_separation {
        match tone {
            PaletteTone::Dark => {
                let next_lightness = (metadata_oklch.lightness + 0.01).min(0.34);
                if next_lightness == metadata_oklch.lightness {
                    break;
                }
                metadata_oklch.lightness = next_lightness;
                separated_metadata = metadata_oklch.rgb();
            }
            PaletteTone::Light => {
                let maximum_lightness = if metadata_oklch.chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA
                {
                    0.78
                } else {
                    0.8
                };
                let next_lightness = (metadata_hsl.lightness + 0.01).min(maximum_lightness);
                if next_lightness == metadata_hsl.lightness {
                    break;
                }
                metadata_hsl.lightness = next_lightness;
                separated_metadata = metadata_hsl.rgb();
            }
        }
    }

    let mut artwork_hsl = artwork_field.hsl();
    let mut artwork_oklch = artwork_field.oklch();
    let mut separated_artwork = artwork_field;

    while separated_artwork.perceptual_distance(background) < minimum_adjacent_separation
        || separated_artwork.perceptual_distance(separated_metadata)
            < MINIMUM_ENDPOINT_FIELD_SEPARATION
    {
        match tone {
            PaletteTone::Dark => {
                let next_lightness = (artwork_oklch.lightness + 0.01).min(0.46);
                if next_lightness == artwork_oklch.lightness {
                    break;
                }
                artwork_oklch.lightness = next_lightness;
                separated_artwork = artwork_oklch.rgb();
            }
            PaletteTone::Light => {
                let next_lightness = (artwork_hsl.lightness - 0.01).max(0.4);
                if next_lightness == artwork_hsl.lightness {
                    break;
                }
                artwork_hsl.lightness = next_lightness;
                separated_artwork = artwork_hsl.rgb();
            }
        }
    }

    (separated_artwork, separated_metadata)
}

fn progress_track_candidate(
    metadata_field: Rgb,
    primary_text: Rgb,
    progress_fill: Rgb,
) -> Option<Rgb> {
    (1..=100)
        .map(|step| metadata_field.mix(primary_text, f64::from(step) / 100.0))
        .find(|track| {
            (1.5..=2.0).contains(&track.contrast_ratio(metadata_field))
                && track.contrast_ratio(progress_fill) >= 3.0
        })
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
    edge_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct ColorFamily {
    color: Rgb,
    lab: Oklab,
    count: u32,
    edge_count: u32,
}

#[derive(Clone, Copy, Default)]
struct Bucket {
    red: u64,
    green: u64,
    blue: u64,
    count: u32,
    edge_count: u32,
}

fn swatches(pixbuf: &Pixbuf) -> Vec<Swatch> {
    let pixels = pixbuf.read_pixel_bytes();
    let pixels = pixels.as_ref();
    let channels = pixbuf.n_channels() as usize;
    let row_stride = pixbuf.rowstride() as usize;
    let mut buckets = [Bucket::default(); 4096];
    let edge = (pixbuf.width().min(pixbuf.height()) as usize / 16).max(1);

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
            if row < edge
                || column < edge
                || row + edge >= pixbuf.height() as usize
                || column + edge >= pixbuf.width() as usize
            {
                bucket.edge_count += 1;
            }
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
            edge_count: bucket.edge_count,
        })
        .collect()
}

fn color_families(swatches: &[Swatch]) -> Vec<ColorFamily> {
    let mut ordered = swatches.to_vec();
    ordered.sort_by(|first, second| {
        second.count.cmp(&first.count).then_with(|| {
            (first.color.red, first.color.green, first.color.blue).cmp(&(
                second.color.red,
                second.color.green,
                second.color.blue,
            ))
        })
    });
    let mut families: Vec<ColorFamily> = Vec::new();

    for swatch in ordered {
        let lab = swatch.color.oklab();
        let nearest = families
            .iter()
            .enumerate()
            .map(|(index, family)| (index, color_family_distance(lab, family.lab)))
            .filter(|(_, distance)| *distance <= FAMILY_CLUSTER_DISTANCE)
            .min_by(|first, second| first.1.total_cmp(&second.1));

        if let Some((index, _)) = nearest {
            let family = &mut families[index];
            let combined_count = family.count + swatch.count;
            family.lab = family.lab.weighted_average(family.count, lab, swatch.count);
            family.count = combined_count;
            family.edge_count += swatch.edge_count;
            family.color = family.lab.oklch().rgb();
        } else {
            families.push(ColorFamily {
                color: swatch.color,
                lab,
                count: swatch.count,
                edge_count: swatch.edge_count,
            });
        }
    }

    families
}

fn color_family_distance(first: Oklab, second: Oklab) -> f64 {
    // Shades of one authored hue often span much more lightness than chroma.
    // Down-weighting lightness lets those shades contribute to one family's
    // salience without merging perceptually distinct hue families.
    let first_chromatic = first.oklch().chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA;
    let second_chromatic = second.oklch().chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA;
    if first_chromatic != second_chromatic {
        return f64::INFINITY;
    }
    (((first.lightness - second.lightness) * 0.25).powi(2)
        + (first.a - second.a).powi(2)
        + (first.b - second.b).powi(2))
    .sqrt()
}

fn compress_bright_palette(mut fields: [Rgb; 3]) -> [Rgb; 3] {
    const BRIGHT_FIELD_CEILING: f64 = 0.8;
    const COMPRESSION_RATIO: f64 = 0.9;

    if fields
        .iter()
        .all(|field| field.hsl().lightness <= BRIGHT_FIELD_CEILING)
    {
        return fields;
    }
    for field in &mut fields {
        let mut hsl = field.hsl();
        hsl.lightness = (hsl.lightness * COMPRESSION_RATIO).min(BRIGHT_FIELD_CEILING);
        *field = hsl.rgb();
    }
    fields
}

fn readable_perceptual_tint(
    mut tint: Oklch,
    fields: &[Rgb],
    minimum_contrast: f64,
    contrast_step: f64,
) -> Rgb {
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

#[cfg(test)]
mod sampling_tests {
    use super::*;

    #[test]
    fn integer_reductions_preserve_filter_rounding_and_edges() {
        for factor in [9, 10, 11, 13, 15, 16, 17, 20, 25, 29, 30, 31] {
            for (width, height) in [(64 * factor, 64 * factor), (47 * factor, 64 * factor)] {
                let stride = (width * 3 + 3) & !3;
                let mut seed = 0xa183_7384_u32;
                let pixels: Vec<u8> = (0..stride * height)
                    .map(|_| {
                        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        (seed >> 24) as u8
                    })
                    .collect();
                let source = Pixbuf::from_mut_slice(
                    pixels,
                    gdk_pixbuf::Colorspace::Rgb,
                    false,
                    8,
                    width,
                    height,
                    stride,
                );
                let actual = sample_artwork(&source).unwrap();
                let expected = source
                    .scale_simple(
                        actual.width(),
                        actual.height(),
                        gdk_pixbuf::InterpType::Bilinear,
                    )
                    .unwrap();
                let actual_bytes = actual.read_pixel_bytes();
                let expected_bytes = expected.read_pixel_bytes();
                for row in 0..actual.height() {
                    let a = (row * actual.rowstride()) as usize;
                    let e = (row * expected.rowstride()) as usize;
                    let count = (actual.width() * 3) as usize;
                    assert_eq!(
                        &actual_bytes[a..a + count],
                        &expected_bytes[e..e + count],
                        "factor {factor}, {width}x{height}, row {row}"
                    );
                }
            }
        }
    }

    #[test]
    fn parallel_sampling_preserves_every_opaque_and_alpha_sample() {
        for channels in [3, 4] {
            for (width, height) in [(997, 643), (1600, 1600)] {
                let stride = (width * channels + 3) & !3;
                let pixels: Vec<u8> = (0..stride * height)
                    .map(|index| ((index * 17 + index / 97) % 256) as u8)
                    .collect();
                let source = Pixbuf::from_mut_slice(
                    pixels,
                    gdk_pixbuf::Colorspace::Rgb,
                    channels == 4,
                    8,
                    width,
                    height,
                    stride,
                );
                let actual = sample_artwork(&source).unwrap();
                let expected = source
                    .scale_simple(
                        actual.width(),
                        actual.height(),
                        gdk_pixbuf::InterpType::Bilinear,
                    )
                    .unwrap();
                let a = actual.read_pixel_bytes();
                let e = expected.read_pixel_bytes();
                for row in 0..actual.height() {
                    let start = (row * actual.rowstride()) as usize;
                    let count = (actual.width() * channels) as usize;
                    assert_eq!(
                        &a[start..start + count],
                        &e[start..start + count],
                        "{channels} channels at row {row}"
                    );
                }
            }
        }
    }
}
