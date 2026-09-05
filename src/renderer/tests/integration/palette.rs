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

const fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}
const MINIMUM_ADJACENT_FIELD_SEPARATION: f64 = 0.05;
const MINIMUM_LIGHT_ADJACENT_FIELD_SEPARATION: f64 = 0.045;
const MINIMUM_ENDPOINT_FIELD_SEPARATION: f64 = 0.12;

fn assert_color_near(role: &str, actual: Rgb, expected: Rgb, maximum_channel_delta: u8) {
    let actual_color = actual;
    for (channel, actual, expected) in [
        ("red", actual.red, expected.red),
        ("green", actual.green, expected.green),
        ("blue", actual.blue, expected.blue),
    ] {
        assert!(
            actual.abs_diff(expected) <= maximum_channel_delta,
            "{role} {channel} channel should stay within {maximum_channel_delta} of the visual direction; expected {expected}, got {actual} in {}",
            actual_color.to_hex(),
        );
    }
}

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
    lightness: f64,
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
        lightness: lab.lightness,
        chroma: (lab.a * lab.a + lab.b * lab.b).sqrt(),
        hue: lab.b.atan2(lab.a).to_degrees().rem_euclid(360.0),
    }
}

fn hue_distance(first: f64, second: f64) -> f64 {
    let distance = (first - second).abs();
    distance.min(360.0 - distance)
}

fn nearest_field(palette: PresentationPalette, family: Rgb) -> (usize, Rgb, TestOklch) {
    let family_hue = oklch(family).hue;
    [palette.artwork_field, palette.metadata_field]
        .into_iter()
        .enumerate()
        .map(|(index, color)| (index, color, oklch(color)))
        .min_by(|first, second| {
            hue_distance(first.2.hue, family_hue).total_cmp(&hue_distance(second.2.hue, family_hue))
        })
        .expect("a presentation palette always has two endpoint fields")
}

fn assert_dark_family_relationships(
    palette: PresentationPalette,
    primary_family: Rgb,
    secondary_family: Rgb,
    accent_family: Rgb,
    incidental_highlight: Rgb,
    name: &str,
) {
    let (primary_index, primary_field, primary) = nearest_field(palette, primary_family);
    let (secondary_index, secondary_field, secondary) = nearest_field(palette, secondary_family);
    let primary_source = oklch(primary_family);
    let secondary_source = oklch(secondary_family);

    assert_ne!(
        primary_index,
        secondary_index,
        "{name} should retain distinct primary and secondary authored families; fields were {} and {}",
        palette.artwork_field.to_hex(),
        palette.metadata_field.to_hex(),
    );
    for (role, field, output, source) in [
        ("primary", primary_field, primary, primary_source),
        ("secondary", secondary_field, secondary, secondary_source),
    ] {
        assert!(
            hue_distance(output.hue, source.hue) <= 18.0,
            "{name} {role} field should retain its authored hue; got {}",
            field.to_hex(),
        );
        assert!(
            (0.015..=source.chroma * 1.05).contains(&output.chroma),
            "{name} {role} field should retain bounded source chroma; source {:.3}, output {:.3} ({})",
            source.chroma,
            output.chroma,
            field.to_hex(),
        );
    }
    assert!(
        secondary.lightness >= 0.25,
        "{name} dark secondary should remain readable at television distance (OKLab L >= 0.25); got {:.3} ({})",
        secondary.lightness,
        secondary_field.to_hex(),
    );

    let accent = oklch(palette.accent);
    assert!(
        hue_distance(accent.hue, oklch(accent_family).hue) <= 18.0,
        "{name} accent should stay near the intended salient family; got {}",
        palette.accent.to_hex(),
    );
    assert!(
        hue_distance(accent.hue, oklch(incidental_highlight).hue) > 24.0,
        "{name} incidental highlight must not take over semantic accents; got {}",
        palette.accent.to_hex(),
    );
    for field in [palette.artwork_field, palette.metadata_field] {
        assert!(
            hue_distance(oklch(field).hue, oklch(incidental_highlight).hue) > 18.0,
            "{name} incidental highlight must not take over a presentation field; got {}",
            field.to_hex(),
        );
    }
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
fn keeps_the_representative_artworks_navy_coral_and_cream_direction() {
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared/fixtures/artwork/playing.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the shared artwork should produce a palette");

    assert_color_near(
        "navy field",
        palette.background,
        Rgb {
            red: 0x07,
            green: 0x15,
            blue: 0x22,
        },
        18,
    );
    assert_color_near(
        "coral accent",
        palette.accent,
        Rgb {
            red: 0xff,
            green: 0x70,
            blue: 0x51,
        },
        32,
    );
    assert_color_near(
        "cream primary text",
        palette.primary_text,
        Rgb {
            red: 0xf3,
            green: 0xea,
            blue: 0xd7,
        },
        24,
    );
    assert_color_near(
        "muted supporting text",
        palette.muted_text,
        Rgb {
            red: 0x92,
            green: 0x99,
            blue: 0xa8,
        },
        32,
    );
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
fn light_blue_and_blush_reference_keeps_its_approved_palette_direction() {
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

    assert_color_near(
        "approved blue artwork field",
        palette.artwork_field,
        Rgb {
            red: 0x4b,
            green: 0x8b,
            blue: 0xbb,
        },
        8,
    );
    assert_color_near(
        "approved blush metadata field",
        palette.metadata_field,
        Rgb {
            red: 0xd0,
            green: 0xbd,
            blue: 0xb8,
        },
        8,
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
fn keeps_a_below_ceiling_light_palette_at_its_generated_lightness() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = synthetic_artwork(&directory, "moderate-light.svg", "#cacaca", "#75a0a5");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("moderately light artwork should produce a palette");

    assert!(
        (0.62..=0.7).contains(&hsl_lightness(palette.background)),
        "the below-ceiling center field should keep its generated lightness: {:?}",
        palette.background,
    );
    assert!(
        (0.66..=0.74).contains(&hsl_lightness(palette.metadata_field)),
        "the below-ceiling metadata field should keep its generated lightness: {:?}",
        palette.metadata_field,
    );
}

#[test]
fn restrained_light_produces_a_restrained_teal_gray_and_mauve_light_matte() {
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
    assert!(
        palette.artwork_field.green > palette.artwork_field.red + 8
            && palette.artwork_field.blue > palette.artwork_field.red + 8,
        "the artwork field should retain the cyan color family",
    );
    assert!(
        palette.metadata_field.red > palette.metadata_field.green
            && palette.metadata_field.blue > palette.metadata_field.green,
        "the metadata field should retain the plum color family",
    );
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
fn ochre_artwork_with_a_salient_muted_purple_family_retains_a_purple_field() {
    let palette = palette_from_realistic_artwork(
        "ochre-purple.svg",
        ["#9b741d", "#85651c"],
        ["#504c69", "#3f3e5c"],
        ["#292720", "#c1b58f"],
        "#00d9ff",
    );

    assert_dark_family_relationships(
        palette,
        rgb(0x9b, 0x74, 0x1d),
        rgb(0x50, 0x4c, 0x69),
        rgb(0x9b, 0x74, 0x1d),
        rgb(0x00, 0xd9, 0xff),
        "ochre and muted purple artwork",
    );
}

#[test]
fn related_shades_do_not_displace_a_smaller_independently_salient_family() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = directory.path().join("ochre-olive-purple-cyan.svg");
    fs::write(
        &artwork_path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="#2b2924"/>
            <rect width="44" height="100" fill="#ad7d13"/>
            <rect width="30" height="100" x="44" fill="#515417"/>
            <rect width="12" height="100" x="74" fill="#4c496f"/>
            <rect width="10" height="100" x="86" fill="#c1b58f"/>
            <rect width="4" height="100" x="96" fill="#00d9ff"/>
        </svg>"##,
    )
    .expect("the competing-family fixture should be writable");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("competing authored families should produce a palette");
    assert_dark_family_relationships(
        palette,
        rgb(0xad, 0x7d, 0x13),
        rgb(0x4c, 0x49, 0x6f),
        rgb(0xad, 0x7d, 0x13),
        rgb(0x00, 0xd9, 0xff),
        "ochre with a related olive shade and independently salient purple",
    );
}

#[test]
fn icy_blue_artwork_with_a_salient_deep_violet_family_retains_a_violet_field() {
    let palette = palette_from_realistic_artwork(
        "icy-blue-violet.svg",
        ["#8eb3c2", "#769eae"],
        ["#493259", "#3f3157"],
        ["#20272b", "#c5c1bc"],
        "#e12531",
    );

    assert_dark_family_relationships(
        palette,
        rgb(0x8e, 0xb3, 0xc2),
        rgb(0x49, 0x32, 0x59),
        rgb(0x8e, 0xb3, 0xc2),
        rgb(0xe1, 0x25, 0x31),
        "icy blue and deep violet artwork",
    );
}

#[test]
fn cream_and_warm_artwork_with_a_salient_steel_blue_family_retains_a_blue_field() {
    let palette = palette_from_realistic_artwork(
        "cream-warm-steel-blue.svg",
        ["#758fb2", "#5f7092"],
        ["#9a5b43", "#6f3d2d"],
        ["#ead9b8", "#292724"],
        "#e8dc24",
    );

    assert_dark_family_relationships(
        palette,
        rgb(0x75, 0x8f, 0xb2),
        rgb(0x9a, 0x5b, 0x43),
        rgb(0x9a, 0x5b, 0x43),
        rgb(0xe8, 0xdc, 0x24),
        "steel blue and warm umber artwork",
    );
}

#[test]
fn copper_artwork_with_a_salient_navy_family_keeps_warm_roles_and_a_navy_field() {
    let palette = palette_from_realistic_artwork(
        "copper-navy.svg",
        ["#a84f24", "#8e3f1e"],
        ["#353346", "#292d43"],
        ["#24201d", "#b39a82"],
        "#d533c7",
    );

    assert_dark_family_relationships(
        palette,
        rgb(0xa8, 0x4f, 0x24),
        rgb(0x35, 0x33, 0x46),
        rgb(0xa8, 0x4f, 0x24),
        rgb(0xd5, 0x33, 0xc7),
        "copper and midnight navy artwork",
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
