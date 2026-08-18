use std::collections::HashSet;
use std::fs;
use std::path::Path;

use roonscape_renderer::{
    FALLBACK_FONT_FILES, FALLBACK_FONT_LICENSES, NowPlayingTitleFace, TypographyStyles,
    register_packaged_fallback_fonts, select_capture_typography, select_typography,
};

const NOW_PLAYING_SUPPORTING_RULE: &str = ".now-playing .artist, .now-playing .album, .now-playing .status-label, .now-playing .time, .now-playing .activity-heading, .now-playing .activity-detail, .now-playing .identity-label, .now-playing .identity-name { font-family: \"IBM Plex Sans\", sans-serif; }";

#[test]
fn selects_now_playing_title_and_supporting_faces_independently() {
    for (families, expected_title, expected_full_field) in [
        (
            ["Sitka Display", "Palatino Linotype", "Segoe UI"].as_slice(),
            NowPlayingTitleFace::Preferred,
            ("Palatino Linotype", "Segoe UI"),
        ),
        (
            ["Sitka Display"].as_slice(),
            NowPlayingTitleFace::Preferred,
            ("Libre Baskerville", "IBM Plex Sans"),
        ),
        (
            ["Palatino Linotype", "Segoe UI"].as_slice(),
            NowPlayingTitleFace::Fallback,
            ("Palatino Linotype", "Segoe UI"),
        ),
        (
            [].as_slice(),
            NowPlayingTitleFace::Fallback,
            ("Libre Baskerville", "IBM Plex Sans"),
        ),
    ] {
        let selection = select_typography(&available_families(families));

        assert_eq!(selection.now_playing_title_face(), expected_title);
        assert_eq!(
            selection.now_playing_supporting_family(),
            "IBM Plex Sans",
            "supporting typography must not depend on host font availability"
        );
        assert_eq!(
            (
                selection.full_field_editorial_family(),
                selection.full_field_utility_family(),
            ),
            expected_full_field,
            "Full-field typography should preserve its existing pair selection"
        );
    }
}

#[test]
fn generated_styles_assign_now_playing_roles_without_changing_full_field_roles() {
    let selection = select_typography(&available_families(&[
        "Sitka Display",
        "Palatino Linotype",
        "Segoe UI",
    ]));
    let styles = TypographyStyles::new(selection).to_css();

    assert!(styles.contains(
        ".now-playing .title { font-family: \"Sitka Display\", \"Libre Baskerville\", serif; font-style: normal; font-weight: 700; }"
    ));
    assert!(styles.contains(NOW_PLAYING_SUPPORTING_RULE));
    assert!(styles.contains(
        ".now-playing .status-label, .now-playing .time, .now-playing .identity-label, .now-playing .identity-name { font-variation-settings: \"wdth\" 88; }"
    ));
    assert!(styles.contains(".time { font-variant-numeric: tabular-nums; }"));
    assert!(styles.contains(
        ".full-field .editorial-text, .full-field-heading { font-family: \"Palatino Linotype\", serif; }"
    ));
    assert!(styles.contains(
        ".full-field .utility-text, .full-field .status-label, .full-field .identity-label, .full-field .identity-name, .full-field .full-field-explanation, .diagnostics { font-family: \"Segoe UI\", sans-serif; }"
    ));
}

#[test]
fn generated_fallback_title_style_preserves_ordinary_glyph_fallback() {
    let styles = TypographyStyles::new(select_typography(&available_families(&[]))).to_css();

    assert!(styles.contains(
        ".now-playing .title { font-family: \"Libre Baskerville\", serif; font-style: normal; font-weight: 700; }"
    ));
    assert!(styles.contains(NOW_PLAYING_SUPPORTING_RULE));
}

#[test]
fn capture_workflow_forces_now_playing_title_paths() {
    let installed = available_families(&["Sitka Display", "Palatino Linotype", "Segoe UI"]);
    let absent = available_families(&[]);

    let preferred = select_capture_typography(&installed, NowPlayingTitleFace::Preferred)
        .expect("an installed preferred Title face should be forceable");
    assert_eq!(
        preferred.now_playing_title_face(),
        NowPlayingTitleFace::Preferred
    );
    assert_eq!(preferred.now_playing_supporting_family(), "IBM Plex Sans");

    let fallback = select_capture_typography(&installed, NowPlayingTitleFace::Fallback)
        .expect("the packaged fallback Title should always be forceable");
    assert_eq!(
        fallback.now_playing_title_face(),
        NowPlayingTitleFace::Fallback
    );
    assert_eq!(fallback.now_playing_supporting_family(), "IBM Plex Sans");

    assert_eq!(
        select_capture_typography(&absent, NowPlayingTitleFace::Preferred)
            .expect_err("preferred capture should not use a host substitution")
            .to_string(),
        "capture requested Sitka Display for Now Playing Title, but the host font is unavailable"
    );
}

#[test]
fn ships_and_registers_the_open_fallback_fonts_with_their_license_notices() {
    let renderer_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in FALLBACK_FONT_FILES {
        let font = fs::read(renderer_root.join(relative_path))
            .expect("packaged fallback font should be readable");
        assert!(font.len() > 100_000, "font asset should contain font data");
    }
    for relative_path in FALLBACK_FONT_LICENSES {
        let notice = fs::read_to_string(renderer_root.join(relative_path))
            .expect("packaged font license should be readable");
        assert!(notice.contains("SIL OPEN FONT LICENSE Version 1.1"));
    }

    register_packaged_fallback_fonts(renderer_root)
        .expect("packaged fallback fonts should register without a network or global install");
}

fn available_families(families: &[&str]) -> HashSet<String> {
    families.iter().map(|family| (*family).to_owned()).collect()
}
