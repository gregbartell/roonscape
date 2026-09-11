use std::fs;
use std::path::{Path, PathBuf};

use roonscape_renderer::{PresentationPalette, Rgb};
use tempfile::{TempDir, tempdir};

const BLACK: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};
const WHITE: Rgb = Rgb {
    red: 255,
    green: 255,
    blue: 255,
};

#[test]
fn grayscale_and_weak_scanning_casts_do_not_acquire_visible_tints() {
    let directory = tempdir().unwrap();
    for (field, detail) in [("#303030", "#dddddd"), ("#313030", "#dddcda")] {
        let path = synthetic_artwork(&directory, "neutral.svg", field, detail);
        let palette = PresentationPalette::from_artwork(&path).unwrap();
        for color in [
            palette.background,
            palette.artwork_field,
            palette.metadata_field,
            palette.primary_text,
            palette.secondary_text,
            palette.muted_text,
            palette.accent,
            palette.status_muted_accent,
        ] {
            assert!(
                oklch(color).chroma < 0.01,
                "neutral artwork must not manufacture a visible tint: {}",
                color.to_hex(),
            );
        }
    }
}

#[test]
fn muted_paper_and_blue_detail_can_support_a_light_composition() {
    let directory = tempdir().unwrap();
    let path = synthetic_artwork(&directory, "paper.svg", "#c7ac87", "#28569a");
    let palette = PresentationPalette::from_artwork(&path).unwrap();
    assert!(palette.background.contrast_ratio(BLACK) >= 7.0);
    assert!(
        palette.accent.blue > palette.accent.red + 20,
        "the small blue detail should supply an accent: {}",
        palette.accent.to_hex(),
    );
}

#[test]
fn a_dark_artwork_edge_can_anchor_a_bright_monochrome_composition() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("monochrome-edge.svg");
    fs::write(
        &path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
        <rect width="64" height="64" fill="#202020"/>
        <rect x="4" y="4" width="56" height="56" fill="#c6c6c6"/>
        <rect x="4" y="4" width="14" height="56" fill="#707070"/>
    </svg>"##,
    )
    .unwrap();
    let palette = PresentationPalette::from_artwork(&path).unwrap();
    assert!(
        palette.background.contrast_ratio(WHITE) >= 7.0,
        "the surrounding field should support the artwork's dark edge",
    );
}

#[test]
fn muted_dark_artwork_does_not_become_a_vivid_accent() {
    let directory = tempdir().unwrap();
    let path = synthetic_artwork(&directory, "dark-cast.svg", "#021621", "#b8b8b8");
    let palette = PresentationPalette::from_artwork(&path).unwrap();
    assert!(
        oklch(palette.accent).chroma <= oklch(rgb(2, 22, 33)).chroma + 0.005,
        "lightening a weak dark cast must not manufacture chroma: {}",
        palette.accent.to_hex(),
    );
}

#[test]
fn incidental_pixels_and_small_channel_changes_preserve_palette_direction() {
    for (field, detail) in [
        (rgb(35, 35, 35), rgb(190, 190, 190)),
        (rgb(199, 172, 135), rgb(40, 86, 154)),
        (rgb(60, 118, 121), rgb(215, 215, 210)),
        (rgb(140, 180, 230), rgb(35, 70, 145)),
        (rgb(239, 198, 206), rgb(49, 44, 47)),
    ] {
        let pixels: Vec<_> = (0..64 * 64)
            .flat_map(|index| {
                let color = if index % 64 > 48 && index / 64 < 16 {
                    detail
                } else {
                    field
                };
                [color.red, color.green, color.blue]
            })
            .collect();
        let palette = |pixels| {
            let image = gdk_pixbuf::Pixbuf::from_mut_slice(
                pixels,
                gdk_pixbuf::Colorspace::Rgb,
                false,
                8,
                64,
                64,
                64 * 3,
            );
            PresentationPalette::from_pixbuf(&image).unwrap()
        };
        let before = palette(pixels.clone());
        for changes in [
            [-1, -1, -1],
            [1, 1, 1],
            [-1, 0, 0],
            [1, 0, 0],
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
        ] {
            let mut changed: Vec<u8> = pixels
                .iter()
                .enumerate()
                .map(|(index, value)| (i16::from(*value) + changes[index % 3]).clamp(0, 255) as u8)
                .collect();
            for index in [37, 319, 880, 1764] {
                changed[index * 3..index * 3 + 3].copy_from_slice(&[255, 0, 180]);
            }
            let after = palette(changed);
            for (first, second) in [
                (before.background, after.background),
                (before.artwork_field, after.artwork_field),
                (before.metadata_field, after.metadata_field),
                (before.accent, after.accent),
            ] {
                assert!(
                    oklab_distance(first, second) < 0.04,
                    "incidental changes must not redirect a palette: {} -> {}",
                    first.to_hex(),
                    second.to_hex()
                );
            }
        }
    }
}

#[test]
fn file_and_decoded_artwork_use_the_same_palette_sample() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork");
    for name in ["playing.jpg", "light.jpg"] {
        let path = root.join(name);
        let decoded = gdk_pixbuf::Pixbuf::from_file(&path).unwrap();
        assert_eq!(
            PresentationPalette::from_artwork(&path).unwrap(),
            PresentationPalette::from_pixbuf(&decoded).unwrap(),
            "{name}: file loading must preserve the decoded artwork's palette sample"
        );
    }
}

#[test]
fn artwork_palette_blends_keep_text_distinct_across_dark_and_light_fields() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork");
    let dark = PresentationPalette::from_artwork(&root.join("playing.jpg")).unwrap();
    let light = PresentationPalette::from_artwork(&root.join("light.jpg")).unwrap();
    assert_eq!(dark.mix(light, 0.0), dark);
    assert_eq!(dark.mix(light, 1.0), light);
    for (from, to) in [
        (dark, light),
        (light, dark),
        (PresentationPalette::fallback(), light),
        (dark.mix(light, 0.5), dark),
    ] {
        for step in 1..100 {
            let palette = from.mix(to, f64::from(step) / 100.0);
            for text in [
                palette.primary_text,
                palette.secondary_text,
                palette.muted_text,
                palette.accent,
                palette.status_muted_accent,
            ] {
                for field in [palette.background, palette.metadata_field] {
                    assert!(
                        text.contrast_ratio(field) >= 3.0,
                        "a crossfade must not converge to unreadable text: step={step}, text={text:?}, field={field:?}"
                    );
                }
            }
            for neighbor in [palette.secondary_text, palette.muted_text] {
                for weight in [0.25, 0.5, 0.75] {
                    let channel = |primary: u8, supporting: u8| {
                        (f64::from(primary) * weight + f64::from(supporting) * (1.0 - weight))
                            .round() as u8
                    };
                    let cue = Rgb {
                        red: channel(palette.primary_text.red, neighbor.red),
                        green: channel(palette.primary_text.green, neighbor.green),
                        blue: channel(palette.primary_text.blue, neighbor.blue),
                    };
                    for field in [palette.background, palette.metadata_field] {
                        assert!(
                            cue.contrast_ratio(field) >= 3.0,
                            "Natural Cue Handoff colors must stay readable during artwork blending: step={step}, cue={cue:?}, field={field:?}"
                        );
                    }
                }
            }
        }
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}
const MINIMUM_ADJACENT_FIELD_SEPARATION: f64 = 0.05;
const MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION: f64 = 0.045;
const MINIMUM_ENDPOINT_FIELD_SEPARATION: f64 = 0.12;

fn hsl_lightness(color: Rgb) -> f64 {
    let maximum = color.red.max(color.green).max(color.blue);
    let minimum = color.red.min(color.green).min(color.blue);
    (f64::from(maximum) + f64::from(minimum)) / (2.0 * 255.0)
}

// Keep the perceptual oracle independent from the production implementation so
// a defect in that conversion cannot make the palette regression pass itself.
#[derive(Clone, Copy)]
struct TestOklab {
    lightness: f64,
    a: f64,
    b: f64,
}

#[derive(Clone, Copy)]
struct TestOklch {
    chroma: f64,
    hue: f64,
}

fn oklab(color: Rgb) -> TestOklab {
    let linear_channel = |channel: u8| {
        let encoded = f64::from(channel) / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    };
    let red = linear_channel(color.red);
    let green = linear_channel(color.green);
    let blue = linear_channel(color.blue);
    let l = (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
    let m = (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
    let s = (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();

    TestOklab {
        lightness: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    }
}

fn oklab_distance(first: Rgb, second: Rgb) -> f64 {
    let first = oklab(first);
    let second = oklab(second);
    ((first.lightness - second.lightness).powi(2)
        + (first.a - second.a).powi(2)
        + (first.b - second.b).powi(2))
    .sqrt()
}

fn synthetic_artwork(directory: &TempDir, file_name: &str, field: &str, accent: &str) -> PathBuf {
    let artwork_path = directory.path().join(file_name);
    fs::write(
        &artwork_path,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
                <rect width="64" height="64" fill="{field}"/>
                <circle cx="48" cy="16" r="10" fill="{accent}"/>
            </svg>"#
        ),
    )
    .expect("the synthetic artwork fixture should be writable");
    artwork_path
}

fn realistic_family_artwork(
    directory: &TempDir,
    file_name: &str,
    primary_family: [&str; 2],
    secondary_family: [&str; 2],
    neutral_family: [&str; 2],
    incidental_highlight: &str,
) -> PathBuf {
    let artwork_path = directory.path().join(file_name);
    fs::write(
        &artwork_path,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
                <rect width="100" height="100" fill="{}"/>
                <rect width="38" height="50" fill="{}"/>
                <rect width="38" height="50" y="50" fill="{}"/>
                <rect width="28" height="50" x="38" fill="{}"/>
                <rect width="28" height="50" x="38" y="50" fill="{}"/>
                <rect width="28" height="50" x="66" fill="{}"/>
                <rect width="28" height="50" x="66" y="50" fill="{}"/>
                <rect width="6" height="100" x="94" fill="{}"/>
            </svg>"#,
            neutral_family[0],
            primary_family[0],
            primary_family[1],
            secondary_family[0],
            secondary_family[1],
            neutral_family[0],
            neutral_family[1],
            incidental_highlight,
        ),
    )
    .expect("the synthetic family artwork fixture should be writable");
    artwork_path
}

fn oklch(color: Rgb) -> TestOklch {
    let lab = oklab(color);
    TestOklch {
        chroma: (lab.a * lab.a + lab.b * lab.b).sqrt(),
        hue: lab.b.atan2(lab.a).to_degrees().rem_euclid(360.0),
    }
}

fn hue_distance(first: f64, second: f64) -> f64 {
    let distance = (first - second).abs();
    distance.min(360.0 - distance)
}

fn assert_artwork_family(role: &str, color: Rgb, sources: &[Rgb]) {
    let output = oklch(color);
    if output.chroma < 0.01 {
        return;
    }
    assert!(
        sources.iter().any(|source| {
            let source = oklch(*source);
            source.chroma >= 0.01
                && hue_distance(output.hue, source.hue) <= 20.0
                && output.chroma <= source.chroma + 0.008
        }),
        "{role} must retain a supplied family without manufacturing chroma: {}",
        color.to_hex()
    );
}

fn hex_rgb(value: &str) -> Rgb {
    rgb(
        u8::from_str_radix(&value[1..3], 16).unwrap(),
        u8::from_str_radix(&value[3..5], 16).unwrap(),
        u8::from_str_radix(&value[5..7], 16).unwrap(),
    )
}

fn palette_from_realistic_artwork(
    file_name: &str,
    primary_family: [&str; 2],
    secondary_family: [&str; 2],
    neutral_family: [&str; 2],
    incidental_highlight: &str,
) -> PresentationPalette {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = realistic_family_artwork(
        &directory,
        file_name,
        primary_family,
        secondary_family,
        neutral_family,
        incidental_highlight,
    );
    PresentationPalette::from_artwork(&artwork_path)
        .expect("synthetic color-family artwork should produce a palette")
}

#[test]
fn representative_artwork_retains_navy_and_coral_with_quiet_text() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/playing.svg");
    let palette = PresentationPalette::from_artwork(&path).unwrap();
    assert!(palette.artwork_field.blue > palette.artwork_field.red + 15);
    assert_artwork_family("accent", palette.accent, &[rgb(255, 112, 81)]);
    assert!(oklch(palette.accent).chroma > 0.08);
    for text in [
        palette.primary_text,
        palette.secondary_text,
        palette.muted_text,
    ] {
        assert!(
            oklch(text).chroma < 0.025,
            "text should support the artwork: {}",
            text.to_hex()
        );
    }
}

#[test]
fn uses_the_fixed_no_art_palette_without_artwork() {
    let palette = PresentationPalette::for_artwork(None);

    assert_eq!(palette.background.to_hex(), "#071522");
    assert_eq!(palette.artwork_field.to_hex(), "#142856");
    assert_eq!(palette.metadata_field.to_hex(), "#0A1429");
    assert_eq!(palette.primary_text.to_hex(), "#F3EAD7");
    assert_eq!(palette.secondary_text.to_hex(), "#C9C5BD");
    assert_eq!(palette.muted_text.to_hex(), "#9299A8");
    assert_eq!(palette.accent.to_hex(), "#FF7051");
    assert_eq!(palette.status_muted_accent.to_hex(), "#C38781");
    assert_eq!(palette.progress_track.to_hex(), "#2F3645");
    assert_eq!(palette.progress_fill.to_hex(), "#FF7051");
    assert_eq!(palette.diagnostics_field.to_hex(), "#0A1429");
    assert_eq!(palette.diagnostics_text.to_hex(), "#F3EAD7");
    assert_eq!(palette.diagnostics_border.to_hex(), "#FF7051");
}

#[test]
fn allows_light_artwork_to_own_a_light_presentation() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = synthetic_artwork(&directory, "light.svg", "#f4e7c5", "#e59a73");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("light artwork should produce a palette");

    assert!(
        palette.background.contrast_ratio(BLACK) >= 7.0,
        "a predominantly light artwork should be allowed to produce a light field"
    );
    assert!(
        oklab_distance(palette.artwork_field, palette.metadata_field)
            >= MINIMUM_ENDPOINT_FIELD_SEPARATION,
        "light artwork should retain visibly differentiated presentation fields",
    );
}

#[test]
fn blue_and_blush_artwork_does_not_require_a_prescribed_field_assignment() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = directory.path().join("light-blue-blush-reference.svg");
    fs::write(
        &artwork_path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
            <rect width="64" height="64" fill="#f7f7f5"/>
            <rect width="16" height="64" x="24" fill="#242323"/>
            <rect width="6" height="16" x="2" y="4" fill="#4b8bbb"/>
            <rect width="6" height="16" x="8" y="4" fill="#6a849a"/>
            <rect width="6" height="16" x="14" y="4" fill="#7f93a6"/>
            <rect width="16" height="16" x="46" y="42" fill="#a87877"/>
        </svg>"##,
    )
    .expect("the light blue-and-blush reference fixture should be writable");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the light blue-and-blush reference should produce a palette");

    let sources = [
        rgb(247, 247, 245),
        rgb(36, 35, 35),
        rgb(75, 139, 187),
        rgb(106, 132, 154),
        rgb(127, 147, 166),
        rgb(168, 120, 119),
    ];
    for color in [
        palette.artwork_field,
        palette.background,
        palette.metadata_field,
        palette.accent,
    ] {
        assert_artwork_family("blue and blush composition", color, &sources);
    }
    assert!(
        oklab_distance(palette.artwork_field, palette.metadata_field)
            >= MINIMUM_ENDPOINT_FIELD_SEPARATION
    );
}

#[test]
fn visual_acceptance_light_artwork_fixture_produces_a_light_presentation() {
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/light.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the shared light artwork fixture should produce a palette");

    assert!(
        palette.background.contrast_ratio(BLACK) >= 7.0,
        "the visual acceptance fixture should exercise a readable light presentation"
    );
}

#[test]
fn moderate_light_artwork_keeps_bounded_light_fields() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = synthetic_artwork(&directory, "moderate-light.svg", "#cacaca", "#75a0a5");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("moderately light artwork should produce a palette");

    for field in [
        palette.background,
        palette.artwork_field,
        palette.metadata_field,
    ] {
        assert!(
            (0.5..=0.8).contains(&hsl_lightness(field)),
            "light fields must remain restrained: {}",
            field.to_hex()
        );
    }
}

#[test]
fn bright_artwork_retains_a_restrained_light_matte_and_authored_accents() {
    let artwork_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork/restrained-light.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the Restrained light palette artwork should produce a palette");

    assert!(
        palette.background.contrast_ratio(WHITE) >= 1.4,
        "the bright end should be restrained rather than approaching near-white",
    );
    assert!(
        hsl_lightness(palette.background) > 0.5 && hsl_lightness(palette.metadata_field) > 0.5,
        "restraining brightness should preserve a light presentation",
    );
    let sources = [
        rgb(245, 244, 241),
        rgb(57, 156, 171),
        rgb(160, 92, 145),
        rgb(212, 217, 216),
    ];
    for color in [
        palette.artwork_field,
        palette.metadata_field,
        palette.accent,
    ] {
        assert_artwork_family("restrained light composition", color, &sources);
    }
}

#[test]
fn allows_dark_artwork_to_own_a_dark_presentation() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = synthetic_artwork(&directory, "dark.svg", "#08172d", "#db674f");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("dark artwork should produce a palette");

    assert!(
        palette.background.contrast_ratio(WHITE) >= 7.0,
        "a predominantly dark artwork should produce a dark field"
    );
}

#[test]
fn weak_dark_artwork_patterns_keep_perceptually_separated_gradient_stops() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let patterns = [
        ("dark-red", "#241516", "#823b36"),
        ("dark-warm", "#2e1712", "#ab5a28"),
        ("dark-teal", "#102329", "#346b70"),
        ("dark-olive", "#261f0d", "#887129"),
        ("low-chroma", "#282725", "#5f5b52"),
    ];
    let mut failures = Vec::new();

    for (name, field, accent) in patterns {
        let artwork_path = synthetic_artwork(&directory, &format!("{name}.svg"), field, accent);
        let palette = PresentationPalette::from_artwork(&artwork_path)
            .expect("synthetic artwork should produce a palette");
        let separation = oklab_distance(palette.artwork_field, palette.metadata_field);

        if separation < MINIMUM_ENDPOINT_FIELD_SEPARATION {
            failures.push(format!(
                "{name} endpoints: {separation:.3} from {} to {}",
                palette.artwork_field.to_hex(),
                palette.metadata_field.to_hex(),
            ));
        }
        for (leg, first, second) in [
            (
                "artwork/background",
                palette.artwork_field,
                palette.background,
            ),
            (
                "background/metadata",
                palette.background,
                palette.metadata_field,
            ),
        ] {
            let separation = oklab_distance(first, second);
            if separation < MINIMUM_ADJACENT_FIELD_SEPARATION {
                failures.push(format!(
                    "{name} {leg}: {separation:.3} from {} to {}",
                    first.to_hex(),
                    second.to_hex(),
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "presentation endpoints should reach ΔE_OK >= 0.12 and adjacent stops ΔE_OK >= 0.05:\n{}",
        failures.join("\n"),
    );
}

#[test]
fn multiple_substantial_families_supply_fields_and_accents_without_invented_hues() {
    for (name, primary, secondary, neutral, detail) in [
        (
            "ochre-purple",
            ["#9b741d", "#85651c"],
            ["#504c69", "#3f3e5c"],
            ["#292720", "#c1b58f"],
            "#00d9ff",
        ),
        (
            "blue-violet",
            ["#8eb3c2", "#769eae"],
            ["#493259", "#3f3157"],
            ["#20272b", "#c5c1bc"],
            "#e12531",
        ),
        (
            "steel-umber",
            ["#758fb2", "#5f7092"],
            ["#9a5b43", "#6f3d2d"],
            ["#ead9b8", "#292724"],
            "#e8dc24",
        ),
        (
            "copper-navy",
            ["#a84f24", "#8e3f1e"],
            ["#353346", "#292d43"],
            ["#24201d", "#b39a82"],
            "#d533c7",
        ),
    ] {
        let palette = palette_from_realistic_artwork(name, primary, secondary, neutral, detail);
        let mut sources: Vec<_> = primary
            .into_iter()
            .chain(secondary)
            .chain(neutral)
            .map(hex_rgb)
            .collect();
        for field in [
            palette.artwork_field,
            palette.background,
            palette.metadata_field,
        ] {
            assert_artwork_family(name, field, &sources);
        }
        assert!(
            oklch(palette.artwork_field).chroma >= 0.015,
            "substantial color should remain recognizable: {name}"
        );
        sources.push(hex_rgb(detail));
        assert_artwork_family("accent", palette.accent, &sources);
    }
}

#[test]
fn related_shades_leave_room_for_a_distinctive_accent() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("related-shades.svg");
    fs::write(
        &path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <rect width="100" height="100" fill="#ad7d13"/>
        <rect width="30" height="100" x="44" fill="#515417"/>
        <rect width="12" height="100" x="74" fill="#4c496f"/>
        <rect width="10" height="100" x="86" fill="#c1b58f"/>
        <rect width="4" height="100" x="96" fill="#00d9ff"/>
    </svg>"##,
    )
    .unwrap();
    let palette = PresentationPalette::from_artwork(&path).unwrap();
    assert!(hue_distance(oklch(palette.artwork_field).hue, oklch(palette.accent).hue) > 40.0);
    assert_artwork_family(
        "accent",
        palette.accent,
        &[hex_rgb("#4c496f"), hex_rgb("#00d9ff")],
    );
}

#[test]
fn near_monochrome_charcoal_with_sparse_warm_detail_stays_restrained() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = directory.path().join("charcoal-sparse-warm.svg");
    fs::write(
        &artwork_path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
            <rect width="64" height="64" fill="#272727"/>
            <rect width="32" height="64" fill="#363535"/>
            <rect width="6" height="6" x="47" y="9" fill="#8b4036"/>
        </svg>"##,
    )
    .expect("the sparse-detail fixture should be writable");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("near-monochrome artwork should produce a palette");

    for (role, field) in [
        ("artwork field", palette.artwork_field),
        ("metadata field", palette.metadata_field),
    ] {
        assert!(
            oklch(field).chroma <= 0.025,
            "{role} should stay restrained rather than amplify sparse warm detail; got {}",
            field.to_hex(),
        );
    }
    assert!(
        hue_distance(oklch(palette.accent).hue, oklch(rgb(0x8b, 0x40, 0x36)).hue) <= 18.0,
        "sparse authored coral detail should own restrained semantic accents; got {}",
        palette.accent.to_hex(),
    );
}

#[test]
fn monochromatic_artwork_uses_lightness_separation_without_inventing_field_hues() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = synthetic_artwork(&directory, "monochrome.svg", "#282828", "#565656");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("monochromatic artwork should produce a palette");

    for (role, color) in [
        ("artwork field", palette.artwork_field),
        ("metadata field", palette.metadata_field),
    ] {
        let minimum = color.red.min(color.green).min(color.blue);
        let maximum = color.red.max(color.green).max(color.blue);
        assert!(
            maximum - minimum <= 2,
            "{role} should remain neutral for monochromatic artwork; got {}",
            color.to_hex(),
        );
    }
    assert!(
        oklab_distance(palette.artwork_field, palette.metadata_field)
            >= MINIMUM_ENDPOINT_FIELD_SEPARATION,
        "monochromatic presentation fields should gain separation through lightness",
    );
}

#[test]
fn light_monochromatic_artwork_keeps_all_three_gradient_stops_distinct() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = synthetic_artwork(&directory, "light-monochrome.svg", "#ededed", "#ffffff");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("light monochromatic artwork should produce a palette");

    for (leg, first, second) in [
        (
            "artwork/background",
            palette.artwork_field,
            palette.background,
        ),
        (
            "background/metadata",
            palette.background,
            palette.metadata_field,
        ),
    ] {
        let separation = oklab_distance(first, second);
        assert!(
            separation >= MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION,
            "light monochromatic {leg} should reach ΔE_OK >= 0.045; got {separation:.3}",
        );
    }
}

#[test]
fn dark_teal_retains_a_dark_teal_chromatic_matte() {
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/dark-teal.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the Dark teal palette artwork should produce a palette");

    assert!(
        palette.background.contrast_ratio(WHITE) >= 7.0,
        "the dark teal palette should retain a dark center transition",
    );
    assert!(
        palette.artwork_field.green > palette.artwork_field.red + 8
            && palette.artwork_field.blue > palette.artwork_field.red + 8,
        "the artwork field should retain its teal direction: {:?}",
        palette.artwork_field,
    );
    assert!(
        palette.accent.green > palette.accent.red + 24
            && palette.accent.blue > palette.accent.red + 24,
        "the salient artwork accent should remain teal: {:?}",
        palette.accent,
    );
}

#[test]
fn uses_the_fixed_fallback_for_unreadable_artwork() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = directory.path().join("corrupt.img");
    fs::write(&artwork_path, b"not an image")
        .expect("the unreadable artwork fixture should be writable");

    let palette = PresentationPalette::for_artwork(Some(&artwork_path));

    assert_eq!(palette, PresentationPalette::fallback());
}

#[test]
fn every_semantic_text_and_accent_role_meets_its_field_contrast() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let dark_artwork_path = synthetic_artwork(&directory, "dark.svg", "#08172d", "#db674f");
    let light_artwork_path = synthetic_artwork(&directory, "light.svg", "#f4e7c5", "#e59a73");
    let representative_artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/playing.svg");
    let restrained_light_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork/restrained-light.svg");
    let dark_teal_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/dark-teal.svg");
    let palettes = [
        ("fallback", PresentationPalette::fallback(), false),
        (
            "dark artwork",
            PresentationPalette::from_artwork(&dark_artwork_path)
                .expect("dark artwork should produce a palette"),
            true,
        ),
        (
            "light artwork",
            PresentationPalette::from_artwork(&light_artwork_path)
                .expect("light artwork should produce a palette"),
            true,
        ),
        (
            "representative artwork",
            PresentationPalette::from_artwork(&representative_artwork_path)
                .expect("representative artwork should produce a palette"),
            true,
        ),
        (
            "Restrained light palette artwork",
            PresentationPalette::from_artwork(&restrained_light_path)
                .expect("Restrained light palette artwork should produce a palette"),
            true,
        ),
        (
            "Dark teal palette artwork",
            PresentationPalette::from_artwork(&dark_teal_path)
                .expect("Dark teal palette artwork should produce a palette"),
            true,
        ),
    ];

    for (source, palette, artwork_derived) in palettes {
        assert_readable_roles(source, palette, artwork_derived);
    }
}

fn assert_readable_roles(source: &str, palette: PresentationPalette, artwork_derived: bool) {
    let supporting_text_minimum = if artwork_derived { 7.0 } else { 4.5 };
    for (field_name, field, supporting_minimum) in [
        ("background", palette.background, supporting_text_minimum),
        ("artwork field", palette.artwork_field, 4.5),
        (
            "metadata field",
            palette.metadata_field,
            supporting_text_minimum,
        ),
    ] {
        for (role, color, minimum) in [
            ("primary text", palette.primary_text, 7.0),
            ("secondary text", palette.secondary_text, supporting_minimum),
            ("muted text", palette.muted_text, supporting_minimum),
            ("accent", palette.accent, 4.5),
            ("muted status accent", palette.status_muted_accent, 4.5),
            ("progress fill", palette.progress_fill, 4.5),
        ] {
            assert!(
                color.contrast_ratio(field) >= minimum,
                "{source} {role} must have at least {minimum}:1 contrast against the {field_name}; got {:.2}:1 from {} on {}",
                color.contrast_ratio(field),
                color.to_hex(),
                field.to_hex(),
            );
        }
    }
    for (role, color, minimum) in [
        ("diagnostics text", palette.diagnostics_text, 7.0),
        ("diagnostics border", palette.diagnostics_border, 4.5),
    ] {
        assert!(
            color.contrast_ratio(palette.diagnostics_field) >= minimum,
            "{source} {role} must have at least {minimum}:1 contrast against the diagnostics field"
        );
    }
}

#[test]
fn progress_roles_preserve_fill_track_and_field_contrast() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let representative_artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/playing.svg");
    let light_artwork_path = synthetic_artwork(&directory, "light.svg", "#f4e7c5", "#e59a73");
    let low_chroma_artwork_path =
        synthetic_artwork(&directory, "low-chroma.svg", "#62656a", "#8b817c");
    let palettes = [
        ("fallback", PresentationPalette::fallback()),
        (
            "representative dark artwork",
            PresentationPalette::from_artwork(&representative_artwork_path)
                .expect("representative artwork should produce a palette"),
        ),
        (
            "light artwork",
            PresentationPalette::from_artwork(&light_artwork_path)
                .expect("light artwork should produce a palette"),
        ),
        (
            "low-chroma artwork",
            PresentationPalette::from_artwork(&low_chroma_artwork_path)
                .expect("low-chroma artwork should produce a palette"),
        ),
    ];

    for (source, palette) in palettes {
        let fill_track_contrast = palette.progress_fill.contrast_ratio(palette.progress_track);
        let track_field_contrast = palette
            .progress_track
            .contrast_ratio(palette.metadata_field);

        assert!(
            fill_track_contrast >= 3.0,
            "{source} progress fill and track must differ by at least 3:1; got {fill_track_contrast:.2}:1",
        );
        assert!(
            (1.5..=2.0).contains(&track_field_contrast),
            "{source} progress track should remain a restrained 1.5–2:1 against the metadata field; got {track_field_contrast:.2}:1",
        );
        assert_eq!(
            palette.progress_fill, palette.accent,
            "{source} progress fill should retain the full artwork-derived accent",
        );
    }
}

#[test]
fn saturated_and_neutral_color_extremes_preserve_all_readability_contracts() {
    for red in [0_u8, 64, 128, 192, 255] {
        for green in [0_u8, 64, 128, 192, 255] {
            for blue in [0_u8, 64, 128, 192, 255] {
                let image =
                    gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, false, 8, 1, 1).unwrap();
                image.fill(u32::from_be_bytes([red, green, blue, 255]));
                let palette = PresentationPalette::from_pixbuf(&image).unwrap();
                let source = rgb(red, green, blue).to_hex();
                assert_readable_roles(&source, palette, true);
                assert!(
                    palette.accent.contrast_ratio(palette.progress_track) >= 3.0,
                    "{source}: fill/track separation"
                );
                assert!(
                    (1.5..=2.0).contains(
                        &palette
                            .progress_track
                            .contrast_ratio(palette.metadata_field)
                    ),
                    "{source}: restrained track"
                );
                assert!(
                    oklab_distance(palette.artwork_field, palette.metadata_field)
                        >= MINIMUM_ENDPOINT_FIELD_SEPARATION,
                    "{source}: distinct endpoints"
                );
                let dark = palette.background.contrast_ratio(WHITE) >= 7.0;
                let minimum = if dark {
                    MINIMUM_ADJACENT_FIELD_SEPARATION
                } else {
                    MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION
                };
                for field in [palette.artwork_field, palette.metadata_field] {
                    assert!(
                        oklab_distance(field, palette.background) >= minimum,
                        "{source}: distinct adjacent fields"
                    );
                    assert!(
                        hsl_lightness(field) <= 0.8,
                        "{source}: bright-field restraint"
                    );
                }
            }
        }
    }
}
