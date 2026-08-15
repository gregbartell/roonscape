use std::collections::HashSet;
use std::fs;
use std::path::Path;

use roonscape_renderer::{
    FALLBACK_FONT_FILES, FALLBACK_FONT_LICENSES, TypographyPair, register_packaged_fallback_fonts,
    select_typography,
};

#[test]
fn selects_the_preferred_pair_only_when_both_host_faces_are_available() {
    let complete = available_families(["Palatino Linotype", "Segoe UI"]);
    let serif_only = available_families(["Palatino Linotype"]);
    let sans_only = available_families(["Segoe UI"]);
    let absent = available_families([]);

    assert_eq!(select_typography(&complete), TypographyPair::Preferred);
    assert_eq!(select_typography(&serif_only), TypographyPair::Fallback);
    assert_eq!(select_typography(&sans_only), TypographyPair::Fallback);
    assert_eq!(select_typography(&absent), TypographyPair::Fallback);
}

#[test]
fn exposes_complete_editorial_and_utility_pairs_without_mixing() {
    assert_eq!(
        (
            TypographyPair::Preferred.editorial_family(),
            TypographyPair::Preferred.utility_family(),
        ),
        ("Palatino Linotype", "Segoe UI")
    );
    assert_eq!(
        (
            TypographyPair::Fallback.editorial_family(),
            TypographyPair::Fallback.utility_family(),
        ),
        ("Libre Baskerville", "IBM Plex Sans")
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

fn available_families<const N: usize>(families: [&str; N]) -> HashSet<String> {
    families.into_iter().map(str::to_owned).collect()
}
