#[path = "support/representative_viewports.rs"]
mod representative_viewports;
mod support;

use roonscape_renderer::{
    MetadataFontSizes, MetadataTypography, Presentation, TextOverflow, Viewport, metadata_layout,
    parse_snapshot, presentation_from_snapshot,
};

const VIEWPORT: Viewport = Viewport::new(1920, 1200);

fn now_playing(fixture_name: &str) -> roonscape_renderer::NowPlayingPresentation {
    let snapshot = parse_snapshot(&support::fixture(fixture_name))
        .expect("metadata fixture should be a valid shared snapshot");
    let Presentation::NowPlaying(presentation) = presentation_from_snapshot(&snapshot)
        .expect("available metadata fixture should produce Now Playing")
    else {
        panic!("available metadata fixture should produce Now Playing");
    };
    presentation
}

#[test]
fn omits_missing_artist_and_album_from_the_metadata_layout() {
    let presentation = now_playing("missing-metadata.json");
    let layout = metadata_layout(&presentation, VIEWPORT);

    assert_eq!(
        layout.title.as_ref().map(|line| line.text.as_str()),
        Some("Last Light on Phobos")
    );
    assert_eq!(layout.artist, None);
    assert_eq!(layout.album, None);
}

#[test]
fn independently_collapses_each_missing_optional_metadata_line() {
    let missing_artist = metadata_layout(&now_playing("missing-artist.json"), VIEWPORT);
    let missing_album = metadata_layout(&now_playing("missing-album.json"), VIEWPORT);

    assert!(missing_artist.artist.is_none());
    assert_eq!(
        missing_artist.album.as_ref().map(|line| line.text.as_str()),
        Some("Signals from the Quiet Sea")
    );
    assert_eq!(
        missing_album.artist.as_ref().map(|line| line.text.as_str()),
        Some("Evelyn Lark & The Orbital Choir")
    );
    assert!(missing_album.album.is_none());
}

#[test]
fn collapses_blank_optional_metadata_without_dead_spacing() {
    let layout = metadata_layout(&now_playing("blank-optional-metadata.json"), VIEWPORT);

    assert!(layout.title.is_some());
    assert_eq!(layout.artist, None);
    assert_eq!(layout.album, None);
}

#[test]
fn reduces_long_metadata_within_firm_readability_and_line_bounds() {
    let presentation = now_playing("long-metadata.json");
    let layout = metadata_layout(&presentation, VIEWPORT);
    let title = layout.title.expect("long fixture should have a Title");
    let artist = layout.artist.expect("long fixture should have an Artist");
    let album = layout.album.expect("long fixture should have an Album");

    assert_eq!(title.font_sizes, font_sizes(88, 70, 54));
    assert_eq!(artist.font_sizes, font_sizes(34, 28, 24));
    assert_eq!(album.font_sizes, font_sizes(23, 20, 18));
    assert_eq!(
        (
            title.maximum_lines,
            artist.maximum_lines,
            album.maximum_lines
        ),
        (3, 2, 2)
    );
    assert_eq!(title.overflow, TextOverflow::EllipsizeEnd);
    assert_eq!(artist.overflow, TextOverflow::EllipsizeEnd);
    assert_eq!(album.overflow, TextOverflow::EllipsizeEnd);
}

#[test]
fn extreme_metadata_stops_reducing_at_readable_minimum_sizes() {
    let presentation = now_playing("extreme-metadata.json");
    let layout = metadata_layout(&presentation, VIEWPORT);
    let title = layout.title.expect("extreme fixture should have a Title");
    let artist = layout
        .artist
        .expect("extreme fixture should have an Artist");
    let album = layout.album.expect("extreme fixture should have an Album");

    assert_eq!(
        title.fitting_font_size(|_| false),
        title.font_sizes.minimum_px
    );
    assert_eq!(
        artist.fitting_font_size(|_| false),
        artist.font_sizes.minimum_px
    );
    assert_eq!(
        album.fitting_font_size(|_| false),
        album.font_sizes.minimum_px
    );
}

#[test]
fn selects_the_first_font_size_that_fits_the_allocated_pango_layout() {
    let layout = metadata_layout(&now_playing("long-metadata.json"), VIEWPORT);
    let title = layout.title.expect("long fixture should have a Title");
    let mut attempted_sizes = Vec::new();

    let selected = title.fitting_font_size(|font_size_px| {
        attempted_sizes.push(font_size_px);
        font_size_px <= title.font_sizes.reduced_px
    });

    assert_eq!(selected, title.font_sizes.reduced_px);
    assert_eq!(
        attempted_sizes,
        vec![title.font_sizes.preferred_px, title.font_sizes.reduced_px]
    );
}

#[test]
fn assigns_editorial_and_utility_typography_roles() {
    let layout = metadata_layout(&now_playing("playing.json"), VIEWPORT);

    assert_eq!(
        layout.title.map(|line| line.typography),
        Some(MetadataTypography::EditorialSerif)
    );
    assert_eq!(
        layout.artist.map(|line| line.typography),
        Some(MetadataTypography::UtilitySans)
    );
    assert_eq!(
        layout.album.map(|line| line.typography),
        Some(MetadataTypography::EditorialSerif)
    );
}

#[test]
fn scales_metadata_typography_across_representative_landscape_viewports() {
    let presentation = now_playing("playing.json");
    let preferred_title_sizes =
        representative_viewports::REPRESENTATIVE_VIEWPORTS.map(|viewport| {
            metadata_layout(&presentation, viewport)
                .title
                .expect("Playing should have a Title")
                .font_sizes
                .preferred_px
        });

    assert_eq!(preferred_title_sizes, [59, 74, 88, 118, 168]);
}

fn font_sizes(preferred_px: u32, reduced_px: u32, minimum_px: u32) -> MetadataFontSizes {
    MetadataFontSizes {
        preferred_px,
        reduced_px,
        minimum_px,
    }
}
