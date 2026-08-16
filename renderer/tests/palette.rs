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
fn keeps_the_prototype_artworks_navy_coral_and_cream_direction() {
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/artwork/playing.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the shared artwork should produce a palette");

    assert!(
        palette.background.blue > palette.background.red
            && palette.background.blue > palette.background.green,
        "the prototype artwork should retain its navy field direction"
    );
    assert!(
        palette.accent.red > palette.accent.green && palette.accent.green > palette.accent.blue,
        "the prototype artwork should retain its coral accent direction"
    );
    assert!(
        palette.primary_text.red >= palette.primary_text.green
            && palette.primary_text.green >= palette.primary_text.blue,
        "the prototype artwork should retain its warm cream text direction"
    );
}

#[test]
fn uses_the_fixed_prototype_palette_without_artwork() {
    let palette = PresentationPalette::for_artwork(None)
        .expect("missing artwork should deliberately select the fallback palette");

    assert_eq!(palette.background.to_hex(), "#071522");
    assert_eq!(palette.artwork_field.to_hex(), "#142856");
    assert_eq!(palette.metadata_field.to_hex(), "#0A1429");
    assert_eq!(palette.primary_text.to_hex(), "#F3EAD7");
    assert_eq!(palette.secondary_text.to_hex(), "#C9C5BD");
    assert_eq!(palette.muted_text.to_hex(), "#9299A8");
    assert_eq!(palette.accent.to_hex(), "#FF7051");
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
fn uses_the_fixed_fallback_for_unreadable_artwork() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let artwork_path = directory.path().join("corrupt.img");
    fs::write(&artwork_path, b"not an image")
        .expect("the unreadable artwork fixture should be writable");

    let palette = PresentationPalette::for_artwork(Some(&artwork_path))
        .expect("unreadable artwork should select the fallback palette");

    assert_eq!(palette, PresentationPalette::fallback());
}

#[test]
fn every_semantic_text_and_accent_role_meets_its_field_contrast() {
    let directory = tempdir().expect("a temporary artwork directory should be available");
    let dark_artwork_path = synthetic_artwork(&directory, "dark.svg", "#08172d", "#db674f");
    let light_artwork_path = synthetic_artwork(&directory, "light.svg", "#f4e7c5", "#e59a73");
    let prototype_artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/artwork/playing.svg");
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
            "prototype artwork",
            PresentationPalette::from_artwork(&prototype_artwork_path)
                .expect("prototype artwork should produce a palette"),
        ),
    ];

    for (source, palette) in palettes {
        for (role, color, field, minimum) in [
            (
                "primary text",
                palette.primary_text,
                palette.metadata_field,
                7.0,
            ),
            (
                "secondary text",
                palette.secondary_text,
                palette.metadata_field,
                4.5,
            ),
            (
                "muted text",
                palette.muted_text,
                palette.metadata_field,
                4.5,
            ),
            ("accent", palette.accent, palette.metadata_field, 4.5),
            (
                "progress track",
                palette.progress_track,
                palette.metadata_field,
                4.5,
            ),
            (
                "progress fill",
                palette.progress_fill,
                palette.metadata_field,
                4.5,
            ),
            (
                "diagnostics text",
                palette.diagnostics_text,
                palette.diagnostics_field,
                7.0,
            ),
            (
                "diagnostics border",
                palette.diagnostics_border,
                palette.diagnostics_field,
                4.5,
            ),
        ] {
            assert!(
                color.contrast_ratio(field) >= minimum,
                "{source} {role} must have at least {minimum}:1 contrast"
            );
        }
    }
}
