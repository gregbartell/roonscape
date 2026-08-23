use roonscape_renderer::{
    NowPlayingGradient, NowPlayingGradientCacheKey, PresentationPalette, Rgb, Viewport,
};

#[test]
fn cache_key_scales_logical_viewports_to_physical_texture_pixels() {
    let logical_viewport = Viewport::new(1_600, 900);

    let unscaled = NowPlayingGradientCacheKey::new(logical_viewport, 1);
    let hidpi = NowPlayingGradientCacheKey::new(logical_viewport, 2);

    assert_eq!(unscaled.logical_viewport(), logical_viewport);
    assert_eq!(unscaled.physical_viewport(), logical_viewport);
    assert_eq!(unscaled.scale_factor(), 1);
    assert_eq!(hidpi.logical_viewport(), logical_viewport);
    assert_eq!(hidpi.physical_viewport(), Viewport::new(3_200, 1_800));
    assert_eq!(hidpi.scale_factor(), 2);
    assert_ne!(unscaled, hidpi, "scale changes must invalidate the texture");
}

#[test]
fn cache_key_invalidates_for_logical_viewport_changes_at_the_same_scale() {
    let first = NowPlayingGradientCacheKey::new(Viewport::new(1_280, 720), 2);
    let second = NowPlayingGradientCacheKey::new(Viewport::new(1_600, 900), 2);

    assert_ne!(
        first, second,
        "viewport changes must invalidate the texture"
    );
    assert_eq!(first.physical_viewport(), Viewport::new(2_560, 1_440));
    assert_eq!(second.physical_viewport(), Viewport::new(3_200, 1_800));
}

#[test]
fn dithered_now_playing_gradient_has_no_broad_transition_plateaus_at_4k() {
    let viewport = Viewport::new(3_840, 2_160);
    let gradient = NowPlayingGradient::new(PresentationPalette::fallback(), viewport);
    let row = (1_000..3_840).map(|x| pixel(&gradient, viewport, x, 100));

    let longest_run = longest_identical_run(row);

    assert!(
        longest_run <= 32,
        "dithering should break up broad transition plateaus; longest run was {longest_run}px",
    );
}

#[test]
fn dark_4k_gradient_keeps_local_noise_energy_and_error_uniform() {
    let viewport = Viewport::new(3_840, 2_160);
    let palette = gradient_palette(rgb(12, 12, 12), rgb(28, 28, 28), rgb(44, 44, 44));
    let gradient = NowPlayingGradient::new(palette, viewport);
    let windows = (1_000..3_816)
        .step_by(64)
        .map(|first_x| {
            let errors = (68..132)
                .flat_map(|y| (first_x..first_x + 64).map(move |x| (x, y)))
                .map(|(x, y)| {
                    f64::from(pixel(&gradient, viewport, x, y)[0])
                        - ideal_encoded_code(palette, viewport, x, y, 0)
                })
                .collect::<Vec<_>>();
            (root_mean_square(&errors), mean(&errors))
        })
        .collect::<Vec<_>>();
    let min_rms = windows
        .iter()
        .map(|(rms, _)| *rms)
        .reduce(f64::min)
        .unwrap();
    let max_rms = windows
        .iter()
        .map(|(rms, _)| *rms)
        .reduce(f64::max)
        .unwrap();
    let worst_mean_error = windows
        .iter()
        .map(|(_, mean)| mean.abs())
        .reduce(f64::max)
        .unwrap();

    assert!(
        max_rms / min_rms <= 1.15,
        "4K local noise energy should not contain recurring troughs; min={min_rms:.4}, max={max_rms:.4}",
    );
    assert!(
        worst_mean_error < 0.02,
        "4K local quantization error should remain locally unbiased; worst={worst_mean_error:.4}",
    );
}

#[test]
fn artwork_field_hold_remains_genuinely_solid() {
    let viewport = Viewport::new(320, 180);
    let palette = PresentationPalette::fallback();
    let gradient = NowPlayingGradient::new(palette, viewport);

    for y in 0..viewport.height_px {
        for x in 0..viewport.width_px * 3 / 100 {
            assert_eq!(
                pixel(&gradient, viewport, x, y),
                rgb_array(palette.artwork_field),
                "the complete leftmost 3% lies inside the specified 0-21% artwork hold",
            );
        }
    }
}

#[test]
fn quantization_noise_energy_and_mean_stay_stable_across_code_phases() {
    let viewport = Viewport::new(768, 512);
    let palette = gradient_palette(rgb(12, 12, 12), rgb(28, 28, 28), rgb(44, 44, 44));
    let gradient = NowPlayingGradient::new(palette, viewport);
    let phases = [0.0, 0.25, 0.5, 0.75];
    let mut errors = std::array::from_fn::<_, 4, _>(|_| Vec::new());

    for y in 0..viewport.height_px {
        for x in 0..viewport.width_px {
            let ideal = ideal_encoded_code(palette, viewport, x, y, 0);
            if gradient_position(viewport, x, y) <= 0.21 {
                continue;
            }
            let fraction = ideal.fract();
            for (index, phase) in phases.into_iter().enumerate() {
                let distance = (fraction - phase).abs().min((fraction + 1.0 - phase).abs());
                if distance <= 0.025 {
                    errors[index].push(f64::from(pixel(&gradient, viewport, x, y)[0]) - ideal);
                }
            }
        }
    }

    let rms = std::array::from_fn::<_, 4, _>(|index| root_mean_square(&errors[index]));
    let means = std::array::from_fn::<_, 4, _>(|index| mean(&errors[index]));
    let min_rms = rms.into_iter().reduce(f64::min).unwrap();
    let max_rms = rms.into_iter().reduce(f64::max).unwrap();

    assert!(
        max_rms / min_rms <= 1.15,
        "1-LSB TPDF noise should keep energy nearly constant across encoded-code phases; RMS={rms:?}",
    );
    assert!(
        means.into_iter().all(|value| value.abs() < 0.02),
        "local quantization error should remain unbiased across encoded-code phases; means={means:?}",
    );
}

#[test]
fn spatial_dither_suppresses_low_frequencies_and_decorrelates_rgb() {
    const SIDE: usize = 128;
    let viewport = Viewport::new(512, 384);
    let palette = gradient_palette(rgb(12, 12, 12), rgb(28, 28, 28), rgb(44, 44, 44));
    let gradient = NowPlayingGradient::new(palette, viewport);
    let mut residuals = std::array::from_fn::<_, 3, _>(|_| Vec::with_capacity(SIDE * SIDE));
    let mut white_residual = Vec::with_capacity(SIDE * SIDE);

    for local_y in 0..SIDE {
        for local_x in 0..SIDE {
            let x = 240 + local_x as u32;
            let y = 96 + local_y as u32;
            for (channel, values) in residuals.iter_mut().enumerate() {
                let ideal = ideal_encoded_code(palette, viewport, x, y, channel);
                values.push(f64::from(pixel(&gradient, viewport, x, y)[channel]) - ideal);
                if channel == 0 {
                    white_residual.push(white_noise_quantization_error(ideal, x, y));
                }
            }
        }
    }

    // A small box filter isolates the low spatial frequencies that form visible
    // structure. Proper blue noise should leave materially less filtered energy
    // than a deterministic white/hash-noise TPDF reference with the same amplitude.
    let blue_ratio = low_frequency_energy_ratio(&residuals[0], SIDE);
    let white_ratio = low_frequency_energy_ratio(&white_residual, SIDE);
    assert!(
        blue_ratio <= white_ratio * 0.75,
        "blue-noise residual should suppress low-frequency energy relative to white noise; blue={blue_ratio:.4}, white={white_ratio:.4}",
    );

    let correlations = [
        correlation(&residuals[0], &residuals[1]),
        correlation(&residuals[0], &residuals[2]),
        correlation(&residuals[1], &residuals[2]),
    ];
    assert!(
        correlations.into_iter().all(|value| value.abs() < 0.15),
        "RGB noise sampling should be decorrelated; correlations={correlations:?}",
    );
}

#[test]
fn generation_is_byte_deterministic_for_the_same_palette_and_viewport() {
    let viewport = Viewport::new(257, 113);
    let first = NowPlayingGradient::new(PresentationPalette::fallback(), viewport);
    let second = NowPlayingGradient::new(PresentationPalette::fallback(), viewport);

    assert_eq!(first.rgba8(), second.rgba8());
}

#[test]
fn light_gradient_stays_within_one_lsb_of_its_palette_field_bounds() {
    let viewport = Viewport::new(320, 180);
    let palette = gradient_palette(rgb(198, 210, 232), rgb(238, 224, 201), rgb(210, 198, 226));
    let gradient = NowPlayingGradient::new(palette, viewport);

    for pixel in gradient.rgba8().chunks_exact(4) {
        assert!((197..=239).contains(&pixel[0]));
        assert!((197..=225).contains(&pixel[1]));
        assert!((200..=233).contains(&pixel[2]));
        assert_eq!(pixel[3], 255);
    }
}

#[test]
fn viewport_changes_produce_exactly_sized_one_to_one_pixels() {
    let palette = PresentationPalette::fallback();
    let first_viewport = Viewport::new(128, 72);
    let second_viewport = Viewport::new(160, 120);
    let first = NowPlayingGradient::new(palette, first_viewport);
    let second = NowPlayingGradient::new(palette, second_viewport);

    assert_eq!(first.rgba8().len(), 128 * 72 * 4);
    assert_eq!(second.rgba8().len(), 160 * 120 * 4);
    assert_ne!(
        pixel(&first, first_viewport, 64, 36),
        pixel(&second, second_viewport, 64, 36),
        "physical pixel coordinates should be regenerated for the new viewport geometry",
    );
}

fn pixel(gradient: &NowPlayingGradient, viewport: Viewport, x: u32, y: u32) -> [u8; 3] {
    let offset = ((y * viewport.width_px + x) * 4) as usize;
    let bytes = &gradient.rgba8()[offset..offset + 3];
    [bytes[0], bytes[1], bytes[2]]
}

fn longest_identical_run(pixels: impl IntoIterator<Item = [u8; 3]>) -> usize {
    let mut previous = None;
    let mut current_run = 0;
    let mut longest_run = 0;
    for pixel in pixels {
        if previous == Some(pixel) {
            current_run += 1;
        } else {
            previous = Some(pixel);
            current_run = 1;
        }
        longest_run = longest_run.max(current_run);
    }
    longest_run
}

fn gradient_palette(
    artwork_field: Rgb,
    background: Rgb,
    metadata_field: Rgb,
) -> PresentationPalette {
    PresentationPalette {
        artwork_field,
        background,
        metadata_field,
        ..PresentationPalette::fallback()
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}

const fn rgb_array(color: Rgb) -> [u8; 3] {
    [color.red, color.green, color.blue]
}

fn root_mean_square(values: &[f64]) -> f64 {
    (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn low_frequency_energy_ratio(values: &[f64], side: usize) -> f64 {
    const RADIUS: usize = 3;
    let mut filtered = Vec::new();
    for y in RADIUS..side - RADIUS {
        for x in RADIUS..side - RADIUS {
            let mut sum = 0.0;
            for sample_y in y - RADIUS..=y + RADIUS {
                for sample_x in x - RADIUS..=x + RADIUS {
                    sum += values[sample_y * side + sample_x];
                }
            }
            filtered.push(sum / ((RADIUS * 2 + 1).pow(2)) as f64);
        }
    }
    root_mean_square(&filtered) / root_mean_square(values)
}

fn correlation(left: &[f64], right: &[f64]) -> f64 {
    let left_mean = mean(left);
    let right_mean = mean(right);
    let mut covariance = 0.0;
    let mut left_variance = 0.0;
    let mut right_variance = 0.0;
    for (&left, &right) in left.iter().zip(right) {
        let left = left - left_mean;
        let right = right - right_mean;
        covariance += left * right;
        left_variance += left * left;
        right_variance += right * right;
    }
    covariance / (left_variance * right_variance).sqrt()
}

fn white_noise_quantization_error(ideal: f64, x: u32, y: u32) -> f64 {
    let first = f64::from(noise_hash(x, y, 0x9e37_79b9)) / f64::from(u32::MAX);
    let second = f64::from(noise_hash(x, y, 0x85eb_ca6b)) / f64::from(u32::MAX);
    (ideal + first - second).round().clamp(0.0, 255.0) - ideal
}

fn noise_hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut hash = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b) ^ seed;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^ (hash >> 16)
}

fn ideal_encoded_code(
    palette: PresentationPalette,
    viewport: Viewport,
    x: u32,
    y: u32,
    channel: usize,
) -> f64 {
    let position = gradient_position(viewport, x, y).clamp(0.0, 1.0);
    let artwork = rgb_array(palette.artwork_field)[channel];
    if position <= 0.21 {
        return f64::from(artwork);
    }
    let background = rgb_array(palette.background)[channel];
    let metadata = rgb_array(palette.metadata_field)[channel];
    let (from, to, amount) = if position < 0.57 {
        (artwork, background, (position - 0.21) / (0.57 - 0.21))
    } else {
        (background, metadata, (position - 0.57) / (1.0 - 0.57))
    };
    let linear = srgb8_to_linear(from) * (1.0 - amount) + srgb8_to_linear(to) * amount;
    linear_to_srgb(linear) * 255.0
}

fn gradient_position(viewport: Viewport, x: u32, y: u32) -> f64 {
    let radians = 112.0_f64.to_radians();
    let direction_x = radians.sin();
    let direction_y = -radians.cos();
    let width = f64::from(viewport.width_px);
    let height = f64::from(viewport.height_px);
    let line_length = width * direction_x.abs() + height * direction_y.abs();
    let step_x = direction_x / line_length;
    let step_y = direction_y / line_length;
    let origin = 0.5 + (0.5 - width / 2.0) * step_x + (0.5 - height / 2.0) * step_y;
    origin + f64::from(x) * step_x + f64::from(y) * step_y
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
