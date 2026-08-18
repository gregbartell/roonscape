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

fn assert_color_near(role: &str, actual: Rgb, expected: Rgb, maximum_channel_delta: u8) {
    for (channel, actual, expected) in [
        ("red", actual.red, expected.red),
        ("green", actual.green, expected.green),
        ("blue", actual.blue, expected.blue),
    ] {
        assert!(
            actual.abs_diff(expected) <= maximum_channel_delta,
            "{role} {channel} channel should stay within {maximum_channel_delta} of the visual direction; expected {expected}, got {actual}"
        );
    }
}

fn hsl_lightness(color: Rgb) -> f64 {
    let maximum = color.red.max(color.green).max(color.blue);
    let minimum = color.red.min(color.green).min(color.blue);
    (f64::from(maximum) + f64::from(minimum)) / (2.0 * 255.0)
}

fn assert_brightness_compression(role: &str, original: Rgb, compressed: Rgb) {
    let original_lightness = hsl_lightness(original);
    let compression = 1.0 - hsl_lightness(compressed) / original_lightness;
    assert!(
        (0.08..=0.12).contains(&compression),
        "{role} should be compressed by 8–12%; got {:.1}%",
        compression * 100.0,
    );
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
    assert_eq!(palette.progress_track.to_hex(), "#9299A8");
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
fn cellout_direction_produces_a_restrained_teal_gray_and_mauve_light_matte() {
    let artwork_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork/cellout-direction.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the Cellout-direction artwork should produce a palette");

    assert_color_near(
        "teal artwork field",
        palette.artwork_field,
        Rgb {
            red: 0x59,
            green: 0x9e,
            blue: 0xab,
        },
        24,
    );
    assert_color_near(
        "cool-gray center transition",
        palette.background,
        Rgb {
            red: 0xb3,
            green: 0xc4,
            blue: 0xc6,
        },
        24,
    );
    assert_color_near(
        "pale-mauve metadata field",
        palette.metadata_field,
        Rgb {
            red: 0xc5,
            green: 0xbe,
            blue: 0xc6,
        },
        24,
    );
    assert!(
        palette.background.contrast_ratio(WHITE) >= 1.4,
        "the bright end should be restrained rather than approaching near-white",
    );
    assert_brightness_compression(
        "cool-gray center transition",
        Rgb {
            red: 0xc9,
            green: 0xd4,
            blue: 0xd5,
        },
        palette.background,
    );
    assert_brightness_compression(
        "pale-mauve metadata field",
        Rgb {
            red: 0xdd,
            green: 0xd3,
            blue: 0xdc,
        },
        palette.metadata_field,
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
fn forever_direction_retains_a_dark_teal_chromatic_matte() {
    let artwork_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork/forever-direction.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the Forever-direction artwork should produce a palette");

    assert!(
        palette.background.contrast_ratio(WHITE) >= 7.0,
        "the Forever direction should retain a dark center transition",
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
    let cellout_direction_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork/cellout-direction.svg");
    let forever_direction_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures/artwork/forever-direction.svg");
    let palettes = [
        ("fallback", PresentationPalette::fallback()),
        (
            "dark artwork",
            PresentationPalette::from_artwork(&dark_artwork_path)
                .expect("dark artwork should produce a palette"),
        ),
        (
            "light artwork",
            PresentationPalette::from_artwork(&light_artwork_path)
                .expect("light artwork should produce a palette"),
        ),
        (
            "representative artwork",
            PresentationPalette::from_artwork(&representative_artwork_path)
                .expect("representative artwork should produce a palette"),
        ),
        (
            "Cellout-direction artwork",
            PresentationPalette::from_artwork(&cellout_direction_path)
                .expect("Cellout-direction artwork should produce a palette"),
        ),
        (
            "Forever-direction artwork",
            PresentationPalette::from_artwork(&forever_direction_path)
                .expect("Forever-direction artwork should produce a palette"),
        ),
    ];

    for (source, palette) in palettes {
        for (field_name, field) in [
            ("background", palette.background),
            ("artwork field", palette.artwork_field),
            ("metadata field", palette.metadata_field),
        ] {
            for (role, color, minimum) in [
                ("primary text", palette.primary_text, 7.0),
                ("secondary text", palette.secondary_text, 4.5),
                ("muted text", palette.muted_text, 4.5),
                ("accent", palette.accent, 4.5),
                ("muted status accent", palette.status_muted_accent, 4.5),
                ("progress track", palette.progress_track, 4.5),
                ("progress fill", palette.progress_fill, 4.5),
            ] {
                assert!(
                    color.contrast_ratio(field) >= minimum,
                    "{source} {role} must have at least {minimum}:1 contrast against the {field_name}"
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
