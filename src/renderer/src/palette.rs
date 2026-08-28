use std::error::Error;
use std::fmt;
use std::path::Path;

use gdk_pixbuf::Pixbuf;

const SAMPLE_SIZE: i32 = 64;
const LIGHT_TONE_LUMINANCE: f64 = 0.55;
const BRIGHT_TONE_LUMINANCE: f64 = 0.65;
const MINIMUM_CHROMATIC_SATURATION: f64 = 0.08;
// Normalized OKLab distance. The endpoint floor rejects the near-solid dark
// captures, while the smaller adjacent floor keeps the middle stop visible
// without requiring every artwork to match unusually broad palettes.
const MINIMUM_ENDPOINT_FIELD_SEPARATION: f64 = 0.12;
const MINIMUM_DARK_ADJACENT_FIELD_SEPARATION: f64 = 0.05;
const MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION: f64 = 0.045;
const FAMILY_CLUSTER_DISTANCE: f64 = 0.045;
// Recovered artwork places authored muted navy/violet families just above
// 0.025 OKLCH chroma and independently salient details near 3% area.
const MINIMUM_CHROMATIC_FAMILY_CHROMA: f64 = 0.025;
const MINIMUM_FIELD_FAMILY_SHARE: f64 = 0.03;
const DOMINANT_NEUTRAL_FIELD_SHARE: f64 = 0.65;
const MINIMUM_DARK_SECONDARY_TO_PRIMARY_SHARE: f64 = 0.1;
const MINIMUM_LIGHT_SECONDARY_TO_PRIMARY_SHARE: f64 = 0.03;
const MINIMUM_SECONDARY_HUE_DISTANCE: f64 = 0.8;
const MINIMUM_ACCENT_DETAIL_SHARE: f64 = 0.008;
const MINIMUM_GLOBAL_ACCENT_SHARE: f64 = 0.07;
const MAXIMUM_ANCHORED_ACCENT_HUE_DISTANCE: f64 = 0.55;
const MAXIMUM_DARK_ARTWORK_CHROMA: f64 = 0.16;
const MAXIMUM_DARK_METADATA_CHROMA: f64 = 0.12;
// These are calibrated in OKLCH against the approved 082afdc dark-field
// luminance, independently from the HSL lightness values used elsewhere.
const DARK_ARTWORK_OKLCH_LIGHTNESS: f64 = 0.34;
const DARK_METADATA_OKLCH_LIGHTNESS: f64 = 0.27;
const DARK_RESTRAINED_METADATA_OKLCH_LIGHTNESS: f64 = 0.22;
const DARK_FIELD_CHROMA_RETENTION: f64 = 0.78;

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
    background_lightness: f64,
    metadata_lightness: f64,
    artwork_lightness: f64,
    primary_lightness: f64,
    secondary_lightness: f64,
    accent_lightness: f64,
    contrast_step: f64,
}

#[derive(Clone, Copy)]
struct FieldCandidate {
    artwork_source: ColorFamily,
    metadata_source: ColorFamily,
    background: Rgb,
    artwork_field: Rgb,
    metadata_field: Rgb,
    accent: Hsl,
}

impl FieldCandidate {
    fn presentation_fields(self) -> [Rgb; 3] {
        [self.background, self.artwork_field, self.metadata_field]
    }
}

impl PaletteTone {
    fn profile(self, artwork_luminance: f64) -> ToneProfile {
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
            Self::Light => {
                let bright_end = ((artwork_luminance - LIGHT_TONE_LUMINANCE)
                    / (BRIGHT_TONE_LUMINANCE - LIGHT_TONE_LUMINANCE))
                    .clamp(0.0, 1.0);
                ToneProfile {
                    background_lightness: 0.64 + 0.18 * bright_end,
                    metadata_lightness: 0.68 + 0.18 * bright_end,
                    artwork_lightness: 0.53 + 0.04 * bright_end,
                    primary_lightness: 0.14,
                    secondary_lightness: 0.28,
                    accent_lightness: 0.32,
                    contrast_step: -0.02,
                }
            }
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
            progress_track: Rgb::new(0x2f, 0x36, 0x45),
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
        let families = color_families(&swatches);
        let dominant_family = families
            .iter()
            .max_by_key(|family| family.count)
            .ok_or(PaletteError::NoVisiblePixels)?;
        let dominant = dominant_family.color;
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
        let artwork_luminance = artwork_luminance(&swatches);
        let tone = palette_tone(artwork_luminance);
        let profile = tone.profile(artwork_luminance);
        let candidate = match tone {
            PaletteTone::Dark => {
                let background = dominant_hsl
                    .with_saturation_and_lightness(
                        field_saturation(dominant_hsl.saturation, 0.12, 0.52),
                        profile.background_lightness,
                    )
                    .rgb();
                select_field_candidate(
                    &families,
                    profile,
                    dominant_hsl,
                    light_hsl,
                    tone,
                    |artwork_source, metadata_source| {
                        let generated_fields = (
                            dark_field(
                                artwork_source.color,
                                DARK_ARTWORK_OKLCH_LIGHTNESS,
                                MAXIMUM_DARK_ARTWORK_CHROMA,
                            ),
                            dark_field(
                                metadata_source.color,
                                if std::ptr::eq(artwork_source, metadata_source) {
                                    DARK_RESTRAINED_METADATA_OKLCH_LIGHTNESS
                                } else {
                                    DARK_METADATA_OKLCH_LIGHTNESS
                                },
                                MAXIMUM_DARK_METADATA_CHROMA,
                            ),
                        );
                        let fields = separate_presentation_fields(
                            tone,
                            background,
                            generated_fields.0,
                            generated_fields.1,
                        );
                        FieldCandidate {
                            artwork_source: *artwork_source,
                            metadata_source: *metadata_source,
                            background,
                            artwork_field: fields.0,
                            metadata_field: fields.1,
                            accent: salient_dark_accent(&families, artwork_source, metadata_source)
                                .hsl(),
                        }
                    },
                )
            }
            PaletteTone::Light => {
                let family_pixel_count = families.iter().map(|family| family.count).sum::<u32>();
                let neutral_dominated = dominant.oklch().chroma < MINIMUM_CHROMATIC_FAMILY_CHROMA
                    && f64::from(dominant_family.count) / f64::from(family_pixel_count)
                        >= DOMINANT_NEUTRAL_FIELD_SHARE;
                let light_artwork_source = if neutral_dominated {
                    swatches
                        .iter()
                        .max_by(|first, second| {
                            light_field_swatch_score(first)
                                .total_cmp(&light_field_swatch_score(second))
                                .then_with(|| first.count.cmp(&second.count))
                        })
                        .map_or(vibrant, |swatch| swatch.color)
                } else {
                    vibrant
                };
                let light_source_distance = {
                    let first = light_artwork_source.oklch();
                    let second = vibrant.oklch();
                    let distance = (first.hue - second.hue).abs();
                    distance.min(std::f64::consts::TAU - distance)
                };
                let neutral_bridge = neutral_dominated && light_source_distance >= 0.8;
                let light_background_source = if neutral_bridge {
                    dominant_hsl
                } else {
                    vibrant_hsl
                };
                let background_lightness =
                    profile.background_lightness - if neutral_bridge { 0.04 } else { 0.0 };
                let generated_background = light_background_source
                    .with_saturation_and_lightness(
                        field_saturation(light_background_source.saturation, 0.08, 0.18),
                        background_lightness,
                    )
                    .rgb();
                select_field_candidate(
                    &families,
                    profile,
                    dominant_hsl,
                    light_hsl,
                    tone,
                    |artwork_source, metadata_source| {
                        let metadata_hsl = metadata_source.color.hsl();
                        let generated_fields = [
                            generated_background,
                            light_artwork_field(light_artwork_source, profile.artwork_lightness),
                            metadata_hsl
                                .with_saturation_and_lightness(
                                    field_saturation(metadata_hsl.saturation, 0.1, 0.22),
                                    profile.metadata_lightness,
                                )
                                .rgb(),
                        ];
                        let [background, artwork_field, metadata_field] =
                            compress_bright_palette(generated_fields);
                        let fields = separate_presentation_fields(
                            tone,
                            background,
                            artwork_field,
                            metadata_field,
                        );
                        FieldCandidate {
                            artwork_source: *artwork_source,
                            metadata_source: *metadata_source,
                            background,
                            artwork_field: fields.0,
                            metadata_field: fields.1,
                            accent: metadata_hsl,
                        }
                    },
                )
            }
        };
        let background = candidate.background;
        let artwork_field = candidate.artwork_field;
        let metadata_field = candidate.metadata_field;
        let accent_hsl = candidate.accent;
        let presentation_fields = [background, artwork_field, metadata_field];
        let semantic = semantic_roles(
            profile,
            dominant_hsl,
            light_hsl,
            accent_hsl,
            presentation_fields,
        )
        .expect("selected palette fields support every semantic role");

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

fn dark_field(source: Rgb, lightness: f64, maximum_chroma: f64) -> Rgb {
    let source = source.oklch();
    Oklch {
        lightness,
        chroma: (source.chroma * DARK_FIELD_CHROMA_RETENTION).min(maximum_chroma),
        hue: source.hue,
    }
    .rgb()
}

fn salient_dark_accent(
    families: &[ColorFamily],
    primary: &ColorFamily,
    secondary: &ColorFamily,
) -> Rgb {
    let total = families.iter().map(|family| family.count).sum::<u32>();
    let selected = families
        .iter()
        .filter(|family| {
            let color = family.color.oklch();
            let share = f64::from(family.count) / f64::from(total);
            color.chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA
                && share >= MINIMUM_ACCENT_DETAIL_SHARE
                && (share >= MINIMUM_GLOBAL_ACCENT_SHARE
                    || family_hue_distance(family, primary) <= MAXIMUM_ANCHORED_ACCENT_HUE_DISTANCE
                    || family_hue_distance(family, secondary)
                        <= MAXIMUM_ANCHORED_ACCENT_HUE_DISTANCE)
        })
        .max_by(|first, second| {
            accent_family_rank(first, primary, secondary)
                .total_cmp(&accent_family_rank(second, primary, secondary))
        })
        .unwrap_or(primary);
    selected.color
}

fn family_hue_distance(first: &ColorFamily, second: &ColorFamily) -> f64 {
    let first = family_field_source(first);
    let second = family_field_source(second);
    let distance = (first.hue - second.hue).abs();
    distance.min(std::f64::consts::TAU - distance)
}

fn family_field_source(family: &ColorFamily) -> Oklch {
    let centroid = family.color.oklch();
    if centroid.chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA {
        centroid
    } else {
        family.exemplar.color.oklch()
    }
}

fn accent_family_score(family: &ColorFamily) -> f64 {
    let color = family.color.oklch();
    color.chroma * f64::from(family.count).sqrt() * (0.35 + color.lightness)
}

fn accent_family_rank(family: &ColorFamily, primary: &ColorFamily, secondary: &ColorFamily) -> f64 {
    let family_chroma = family.color.oklch().chroma;
    let primary_chroma = family_field_source(primary).chroma;
    let secondary_emphasis = if family_hue_distance(family, secondary)
        <= MAXIMUM_ANCHORED_ACCENT_HUE_DISTANCE
        && family_chroma >= primary_chroma * 1.5
    {
        1.3
    } else {
        1.0
    };
    accent_family_score(family) * secondary_emphasis
}

fn field_saturation(source: f64, minimum: f64, maximum: f64) -> f64 {
    // Near-neutral artwork has no meaningful hue to strengthen.
    if source < MINIMUM_CHROMATIC_SATURATION {
        source
    } else {
        source.clamp(minimum, maximum)
    }
}

fn light_artwork_field(source: Rgb, target_lightness: f64) -> Rgb {
    let source_hsl = source.hsl();
    if (0.45..=0.58).contains(&source_hsl.lightness) {
        source
    } else {
        source_hsl
            .with_saturation_and_lightness(
                field_saturation(source_hsl.saturation, 0.24, 0.45),
                target_lightness,
            )
            .rgb()
    }
}

fn select_field_candidate(
    families: &[ColorFamily],
    profile: ToneProfile,
    dominant: Hsl,
    light: Hsl,
    tone: PaletteTone,
    build: impl Fn(&ColorFamily, &ColorFamily) -> FieldCandidate,
) -> FieldCandidate {
    let mut field_families = eligible_field_families(families);
    field_families.sort_by_key(|family| std::cmp::Reverse(family.count));
    let family_pixel_count = families.iter().map(|family| family.count).sum();
    let primary = field_families
        .first()
        .expect("palettes always have sampled color families");
    let restrained = build(primary, primary);
    field_families
        .iter()
        .skip(1)
        .filter(|secondary| meaningful_secondary(primary, secondary, family_pixel_count, tone))
        .map(|secondary| build(primary, secondary))
        .filter(|candidate| field_candidate_is_eligible(*candidate, profile, dominant, light))
        .max_by(|first, second| {
            first
                .metadata_source
                .count
                .cmp(&second.metadata_source.count)
                .then_with(|| {
                    field_candidate_tiebreaker(*first)
                        .total_cmp(&field_candidate_tiebreaker(*second))
                })
        })
        .unwrap_or(restrained)
}

fn meaningful_secondary(
    primary: &ColorFamily,
    secondary: &ColorFamily,
    total: u32,
    tone: PaletteTone,
) -> bool {
    let secondary_share = f64::from(secondary.count) / f64::from(total);
    let relative_share = f64::from(secondary.count) / f64::from(primary.count);
    let primary_color = family_field_source(primary);
    let secondary_color = family_field_source(secondary);
    let hue_distance = (primary_color.hue - secondary_color.hue).abs();
    let hue_distance = hue_distance.min(std::f64::consts::TAU - hue_distance);
    let (minimum_relative_share, minimum_hue_distance) = match tone {
        PaletteTone::Dark => (
            MINIMUM_DARK_SECONDARY_TO_PRIMARY_SHARE,
            MINIMUM_SECONDARY_HUE_DISTANCE,
        ),
        PaletteTone::Light => (
            MINIMUM_LIGHT_SECONDARY_TO_PRIMARY_SHARE,
            MINIMUM_SECONDARY_HUE_DISTANCE,
        ),
    };

    secondary_share >= MINIMUM_FIELD_FAMILY_SHARE
        && relative_share >= minimum_relative_share
        && secondary_color.chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA
        && hue_distance >= minimum_hue_distance
}

fn field_candidate_is_eligible(
    candidate: FieldCandidate,
    profile: ToneProfile,
    dominant: Hsl,
    light: Hsl,
) -> bool {
    let fields = candidate.presentation_fields();
    candidate
        .artwork_field
        .perceptual_distance(candidate.background)
        >= MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION
        && candidate
            .background
            .perceptual_distance(candidate.metadata_field)
            >= MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION
        && candidate
            .artwork_field
            .perceptual_distance(candidate.metadata_field)
            >= MINIMUM_ENDPOINT_FIELD_SEPARATION
        && semantic_roles(profile, dominant, light, candidate.accent, fields).is_some()
}

fn field_candidate_tiebreaker(candidate: FieldCandidate) -> f64 {
    let primary = candidate.artwork_source.color.oklch();
    let secondary = candidate.metadata_source.color.oklch();
    let hue_distance = (primary.hue - secondary.hue).abs();
    let hue_breadth = hue_distance.min(std::f64::consts::TAU - hue_distance);
    candidate.artwork_field.oklch().chroma
        + candidate.metadata_field.oklch().chroma
        + hue_breadth * 0.01
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
    dominant: Hsl,
    light: Hsl,
    accent: Hsl,
    fields: [Rgb; 3],
) -> Option<SemanticRoles> {
    let supporting_fields = [fields[0], fields[2]];
    let primary_text = readable_tint(
        light.with_saturation_and_lightness(
            light.saturation.clamp(0.08, 0.24),
            profile.primary_lightness,
        ),
        &fields,
        7.0,
        profile.contrast_step,
    );
    let secondary_text = readable_tint(
        readable_tint(
            dominant.with_saturation_and_lightness(
                dominant.saturation.clamp(0.12, 0.3),
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
            dominant.with_saturation_and_lightness(
                dominant.saturation.clamp(0.08, 0.2),
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
    let accent = readable_tint(
        accent.with_saturation_and_lightness(
            accent.saturation.clamp(0.48, 1.0),
            profile.accent_lightness,
        ),
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
            track.contrast_ratio(metadata_field) >= 1.5
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
}

#[derive(Clone, Copy, Debug)]
struct ColorFamily {
    color: Rgb,
    lab: Oklab,
    count: u32,
    exemplar: Swatch,
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
            family.color = family.lab.oklch().rgb();
            if swatch_score(&swatch)
                .total_cmp(&swatch_score(&family.exemplar))
                .then_with(|| swatch.count.cmp(&family.exemplar.count))
                .is_gt()
            {
                family.exemplar = swatch;
            }
        } else {
            families.push(ColorFamily {
                color: swatch.color,
                lab,
                count: swatch.count,
                exemplar: swatch,
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

fn eligible_field_families(families: &[ColorFamily]) -> Vec<&ColorFamily> {
    let total = families.iter().map(|family| family.count).sum::<u32>();
    let minimum_count = (f64::from(total) * MINIMUM_FIELD_FAMILY_SHARE).ceil() as u32;
    let dominant = families.iter().max_by_key(|family| family.count);
    let mut eligible = families
        .iter()
        .filter(|family| {
            family.count >= minimum_count
                && family.color.oklch().chroma >= MINIMUM_CHROMATIC_FAMILY_CHROMA
        })
        .collect::<Vec<_>>();
    if let Some(dominant) = dominant
        && dominant.color.oklch().chroma < MINIMUM_CHROMATIC_FAMILY_CHROMA
        && f64::from(dominant.count) / f64::from(total) >= DOMINANT_NEUTRAL_FIELD_SHARE
    {
        eligible.push(dominant);
    }

    if eligible.is_empty() {
        dominant.into_iter().collect()
    } else {
        eligible
    }
}

fn swatch_score(swatch: &Swatch) -> f64 {
    swatch_score_with_population_weight(swatch, 0.75)
}

fn light_field_swatch_score(swatch: &Swatch) -> f64 {
    swatch_score_with_population_weight(swatch, 0.25)
}

fn swatch_score_with_population_weight(swatch: &Swatch, population_weight: f64) -> f64 {
    let hsl = swatch.color.hsl();
    let useful_lightness = 1.0 - (hsl.lightness - 0.55).abs();
    hsl.saturation.powi(2)
        * useful_lightness.max(0.2)
        * (1.0 + f64::from(swatch.count).ln() * population_weight)
}

fn text_score(swatch: &Swatch) -> f64 {
    let hsl = swatch.color.hsl();
    swatch.color.relative_luminance() + hsl.saturation * 0.18 + f64::from(swatch.count).ln() * 0.01
}

fn artwork_luminance(swatches: &[Swatch]) -> f64 {
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
    weighted_luminance / pixel_count as f64
}

fn palette_tone(artwork_luminance: f64) -> PaletteTone {
    if artwork_luminance >= LIGHT_TONE_LUMINANCE {
        PaletteTone::Light
    } else {
        PaletteTone::Dark
    }
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
        hsl.lightness *= COMPRESSION_RATIO;
        *field = hsl.rgb();
    }
    fields
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
