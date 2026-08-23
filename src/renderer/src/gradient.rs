use std::sync::OnceLock;

use crate::{PresentationPalette, Rgb, Viewport};

const GRADIENT_ANGLE_DEGREES: f64 = 112.0;
const ARTWORK_HOLD: f64 = 0.21;
const BACKGROUND_STOP: f64 = 0.57;
const BLUE_NOISE_SIDE: u32 = 128;
const BLUE_NOISE_AREA: usize = (BLUE_NOISE_SIDE * BLUE_NOISE_SIDE) as usize;
const BLUE_NOISE_MASK_COUNT: usize = 2;
const BLUE_NOISE_BYTES: &[u8; BLUE_NOISE_AREA * BLUE_NOISE_MASK_COUNT * 2] =
    include_bytes!("blue_noise_128x128.bin");
const TRANSFER_LUT_MAX: usize = u16::MAX as usize;
const COLOR_LUT_MAX: usize = u16::MAX as usize + 1;
const POSITION_ONE: i64 = 1_i64 << 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NowPlayingGradientCacheKey {
    logical_viewport: Viewport,
    physical_viewport: Viewport,
    scale_factor: u32,
}

impl NowPlayingGradientCacheKey {
    pub fn new(logical_viewport: Viewport, scale_factor: u32) -> Self {
        assert!(scale_factor > 0, "display scale factor must be positive");
        let physical_viewport = Viewport::new(
            logical_viewport
                .width_px
                .checked_mul(scale_factor)
                .expect("scaled viewport width must fit u32"),
            logical_viewport
                .height_px
                .checked_mul(scale_factor)
                .expect("scaled viewport height must fit u32"),
        );
        Self {
            logical_viewport,
            physical_viewport,
            scale_factor,
        }
    }

    pub const fn logical_viewport(self) -> Viewport {
        self.logical_viewport
    }

    pub const fn physical_viewport(self) -> Viewport {
        self.physical_viewport
    }

    pub const fn scale_factor(self) -> u32 {
        self.scale_factor
    }
}

pub struct NowPlayingGradient {
    rgba8: Vec<u8>,
}

impl NowPlayingGradient {
    pub fn new(palette: PresentationPalette, viewport: Viewport) -> Self {
        let pixel_count = usize::try_from(viewport.width_px)
            .expect("supported viewport width should fit memory addressing")
            .checked_mul(
                usize::try_from(viewport.height_px)
                    .expect("supported viewport height should fit memory addressing"),
            )
            .expect("supported viewport area should fit memory addressing");
        let byte_count = pixel_count
            .checked_mul(4)
            .expect("supported RGBA8 viewport should fit memory addressing");
        let mut rgba8 = vec![u8::MAX; byte_count];
        let geometry = GradientGeometry::new(viewport);
        let colors = GradientColors::new(palette);
        let color_lut = colors.quantized_lut();
        let noise_tile = blue_noise_tile();
        fill_pixels(&mut rgba8, viewport, geometry, &color_lut, noise_tile);

        Self { rgba8 }
    }

    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }

    pub fn into_rgba8(self) -> Vec<u8> {
        self.rgba8
    }
}

#[derive(Clone, Copy)]
struct GradientGeometry {
    origin: i64,
    step_x: i64,
    step_y: i64,
}

fn fill_pixels(
    rgba8: &mut [u8],
    viewport: Viewport,
    geometry: GradientGeometry,
    color_lut: &[QuantizedColor],
    noise_tile: &[BlueNoise; BLUE_NOISE_AREA],
) {
    // Keep palette and viewport changes responsive without creating an
    // unbounded worker pool on large RoonScape Hosts.
    let worker_count = if rgba8.len() >= 4_000_000 {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(4)
    } else {
        1
    };
    let rows_per_worker = (viewport.height_px as usize).div_ceil(worker_count);
    let row_stride = viewport.width_px as usize * 4;
    let chunk_size = rows_per_worker * row_stride;

    std::thread::scope(|scope| {
        for (chunk_index, rows) in rgba8.chunks_mut(chunk_size).enumerate() {
            scope.spawn(move || {
                fill_row_chunk(
                    rows,
                    chunk_index * rows_per_worker,
                    viewport.width_px,
                    geometry,
                    color_lut,
                    noise_tile,
                );
            });
        }
    });
}

fn fill_row_chunk(
    rows: &mut [u8],
    first_y: usize,
    width_px: u32,
    geometry: GradientGeometry,
    color_lut: &[QuantizedColor],
    noise_tile: &[BlueNoise; BLUE_NOISE_AREA],
) {
    let row_stride = width_px as usize * 4;
    for (local_y, row) in rows.chunks_exact_mut(row_stride).enumerate() {
        let y = first_y + local_y;
        let mut position = geometry.origin + y as i64 * geometry.step_y;
        for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
            let color = color_lut[fixed_position_index(position)];
            let noise_index = ((y & (BLUE_NOISE_SIDE as usize - 1)) * BLUE_NOISE_SIDE as usize)
                + (x & (BLUE_NOISE_SIDE as usize - 1));
            let noise = noise_tile[noise_index];
            for (channel, output) in pixel[..3].iter_mut().enumerate() {
                *output = color.quantize(channel, noise.0[channel]);
            }
            position += geometry.step_x;
        }
    }
}

impl GradientGeometry {
    fn new(viewport: Viewport) -> Self {
        let radians = GRADIENT_ANGLE_DEGREES.to_radians();
        let direction_x = radians.sin();
        let direction_y = -radians.cos();
        let width = f64::from(viewport.width_px);
        let height = f64::from(viewport.height_px);
        let line_length = width * direction_x.abs() + height * direction_y.abs();
        let step_x = direction_x / line_length;
        let step_y = direction_y / line_length;
        let origin = 0.5 + (0.5 - width / 2.0) * step_x + (0.5 - height / 2.0) * step_y;
        Self {
            origin: fixed_position(origin),
            step_x: fixed_position(step_x),
            step_y: fixed_position(step_y),
        }
    }
}

fn fixed_position(position: f64) -> i64 {
    (position * POSITION_ONE as f64).round() as i64
}

fn fixed_position_index(position: i64) -> usize {
    let position = position.clamp(0, POSITION_ONE) as u64;
    ((position * COLOR_LUT_MAX as u64) >> 32) as usize
}

struct GradientColors {
    artwork_srgb8: Rgb,
    artwork: LinearRgb,
    background: LinearRgb,
    metadata: LinearRgb,
}

impl GradientColors {
    fn new(palette: PresentationPalette) -> Self {
        Self {
            artwork_srgb8: palette.artwork_field,
            artwork: LinearRgb::from_srgb8(palette.artwork_field),
            background: LinearRgb::from_srgb8(palette.background),
            metadata: LinearRgb::from_srgb8(palette.metadata_field),
        }
    }

    fn quantized_lut(&self) -> Vec<QuantizedColor> {
        // The one-dimensional gradient color is independent of physical pixel
        // position. Precomputing it keeps the per-pixel path to fixed-point
        // TPDF quantization while retaining 16-bit interpolation precision.
        let transfer_lut = linear_to_srgb_lut();
        (0..=COLOR_LUT_MAX)
            .map(|index| {
                let position = index as f64 / COLOR_LUT_MAX as f64;
                if position <= ARTWORK_HOLD {
                    QuantizedColor::solid(self.artwork_srgb8)
                } else {
                    QuantizedColor::new(self.color_at(position, transfer_lut))
                }
            })
            .collect()
    }

    fn color_at(&self, position: f64, transfer_lut: &[f64]) -> [f64; 3] {
        if position < BACKGROUND_STOP {
            return self
                .artwork
                .mix(
                    self.background,
                    (position - ARTWORK_HOLD) / (BACKGROUND_STOP - ARTWORK_HOLD),
                )
                .to_srgb_steps(transfer_lut);
        }
        if position < 1.0 {
            return self
                .background
                .mix(
                    self.metadata,
                    (position - BACKGROUND_STOP) / (1.0 - BACKGROUND_STOP),
                )
                .to_srgb_steps(transfer_lut);
        }
        self.metadata.to_srgb_steps(transfer_lut)
    }
}

#[derive(Clone, Copy)]
struct QuantizedColor {
    scaled_steps: [i32; 3],
    dither: bool,
}

impl QuantizedColor {
    fn new(steps: [f64; 3]) -> Self {
        Self {
            scaled_steps: steps.map(|step| {
                (step * BLUE_NOISE_AREA as f64)
                    .round()
                    .clamp(0.0, u8::MAX as f64 * BLUE_NOISE_AREA as f64) as i32
            }),
            dither: true,
        }
    }

    fn solid(color: Rgb) -> Self {
        Self {
            scaled_steps: [color.red, color.green, color.blue]
                .map(|step| i32::from(step) * BLUE_NOISE_AREA as i32),
            dither: false,
        }
    }

    fn quantize(self, channel: usize, noise: i16) -> u8 {
        let scaled_step = self.scaled_steps[channel];
        if !self.dither {
            return (scaled_step / BLUE_NOISE_AREA as i32) as u8;
        }

        let noisy_step = scaled_step + i32::from(noise);
        round_scaled_step(noisy_step).clamp(0, i32::from(u8::MAX)) as u8
    }
}

fn round_scaled_step(value: i32) -> i32 {
    let area = BLUE_NOISE_AREA as i32;
    let half = area / 2;
    if value >= 0 {
        (value + half) / area
    } else {
        (value - half) / area
    }
}

#[derive(Clone, Copy)]
struct LinearRgb([f64; 3]);

impl LinearRgb {
    fn from_srgb8(color: Rgb) -> Self {
        Self([
            srgb8_to_linear(color.red),
            srgb8_to_linear(color.green),
            srgb8_to_linear(color.blue),
        ])
    }

    fn mix(self, other: Self, amount: f64) -> Self {
        Self(std::array::from_fn(|channel| {
            self.0[channel] * (1.0 - amount) + other.0[channel] * amount
        }))
    }

    fn to_srgb_steps(self, transfer_lut: &[f64]) -> [f64; 3] {
        self.0
            .map(|channel| encoded_srgb_step(channel, transfer_lut))
    }
}

fn srgb8_to_linear(value: u8) -> f64 {
    let encoded = f64::from(value) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f64) -> f64 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn linear_to_srgb_lut() -> &'static [f64] {
    static LUT: OnceLock<Vec<f64>> = OnceLock::new();
    LUT.get_or_init(|| {
        (0..=TRANSFER_LUT_MAX)
            .map(|index| linear_to_srgb(index as f64 / TRANSFER_LUT_MAX as f64) * 255.0)
            .collect()
    })
}

fn encoded_srgb_step(linear: f64, transfer_lut: &[f64]) -> f64 {
    let position = linear.clamp(0.0, 1.0) * TRANSFER_LUT_MAX as f64;
    let lower_index = position.floor() as usize;
    let upper_index = (lower_index + 1).min(TRANSFER_LUT_MAX);
    let amount = position - lower_index as f64;
    transfer_lut[lower_index] * (1.0 - amount) + transfer_lut[upper_index] * amount
}

#[derive(Clone, Copy)]
struct BlueNoise([i16; 3]);

fn blue_noise_tile() -> &'static [BlueNoise; BLUE_NOISE_AREA] {
    static TILE: OnceLock<[BlueNoise; BLUE_NOISE_AREA]> = OnceLock::new();
    TILE.get_or_init(|| {
        std::array::from_fn(|index| {
            let x = index % BLUE_NOISE_SIDE as usize;
            let y = index / BLUE_NOISE_SIDE as usize;
            let (first, second) = channel_noise_indices(x, y);
            BlueNoise(std::array::from_fn(|channel| {
                blue_noise_rank(0, first[channel]) - blue_noise_rank(1, second[channel])
            }))
        })
    })
}

fn blue_noise_rank(mask: usize, index: usize) -> i16 {
    // Two independently seeded 128x128 void-and-cluster rank masks. The
    // checked-in table keeps the expensive optimizer out of startup while
    // retaining a uniform rank distribution and blue spatial spectrum.
    let offset = (mask * BLUE_NOISE_AREA + index) * 2;
    i16::try_from(u16::from_le_bytes([
        BLUE_NOISE_BYTES[offset],
        BLUE_NOISE_BYTES[offset + 1],
    ]))
    .expect("128x128 mask ranks fit i16")
}

fn channel_noise_indices(x: usize, y: usize) -> ([usize; 3], [usize; 3]) {
    let side = BLUE_NOISE_SIDE as usize;
    let wrap = |value| value & (side - 1);
    let index = |sample_x, sample_y| wrap(sample_y) * side + wrap(sample_x);
    (
        [
            index(x, y),
            index(y + 37, side - 1 - x + 17),
            index(side - 1 - x + 71, y + 29),
        ],
        [
            index(x, y),
            index(side - 1 - y + 11, x + 53),
            index(y + 47, x + 83),
        ],
    )
}
