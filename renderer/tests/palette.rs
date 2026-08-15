use std::path::Path;

use roonscape_renderer::PresentationPalette;

#[test]
fn derives_a_readable_full_palette_from_current_artwork() {
    let artwork_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/artwork/playing.svg");

    let palette = PresentationPalette::from_artwork(&artwork_path)
        .expect("the shared artwork should produce a palette");

    assert_ne!(palette, PresentationPalette::neutral());
    assert!(palette.primary_text.contrast_ratio(palette.metadata_field) >= 7.0);
    assert!(
        palette
            .secondary_text
            .contrast_ratio(palette.metadata_field)
            >= 4.5
    );
    assert!(
        palette.accent.contrast_ratio(palette.metadata_field) >= 4.5,
        "the state, progress, and Zone accent must stay readable"
    );
    assert_ne!(palette.artwork_field, palette.metadata_field);
}

#[test]
fn uses_a_deliberate_neutral_palette_without_artwork() {
    let palette = PresentationPalette::for_artwork(None)
        .expect("missing artwork should deliberately select the neutral palette");

    assert_eq!(palette.background.to_hex(), "#101217");
    assert_eq!(palette.artwork_field.to_hex(), "#1B1F27");
    assert_eq!(palette.metadata_field.to_hex(), "#151820");
    assert_eq!(palette.primary_text.to_hex(), "#E8E5DE");
    assert_eq!(palette.secondary_text.to_hex(), "#AEB4BE");
    assert_eq!(palette.accent.to_hex(), "#B6A77F");
    assert!(palette.primary_text.contrast_ratio(palette.metadata_field) >= 7.0);
    assert!(
        palette
            .secondary_text
            .contrast_ratio(palette.metadata_field)
            >= 4.5
    );
    assert!(palette.accent.contrast_ratio(palette.metadata_field) >= 4.5);
}
