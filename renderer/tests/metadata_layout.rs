mod support;

use roonscape_renderer::{
    MetadataTypography, Presentation, metadata_layout, parse_snapshot, presentation_from_snapshot,
};

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
    let layout = metadata_layout(&presentation);

    assert_eq!(
        layout.title.as_ref().map(|line| line.text.as_str()),
        Some("Last Light on Phobos")
    );
    assert_eq!(layout.artist, None);
    assert_eq!(layout.album, None);
}

#[test]
fn independently_collapses_each_missing_optional_metadata_line() {
    let missing_artist = metadata_layout(&now_playing("missing-artist.json"));
    let missing_album = metadata_layout(&now_playing("missing-album.json"));

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
fn reduces_long_metadata_within_firm_readability_and_line_bounds() {
    let presentation = now_playing("long-metadata.json");
    let layout = metadata_layout(&presentation);
    let title = layout.title.expect("long fixture should have a Title");
    let artist = layout.artist.expect("long fixture should have an Artist");
    let album = layout.album.expect("long fixture should have an Album");

    assert_eq!(
        (
            title.preferred_font_size_px,
            title.reduced_font_size_px,
            title.minimum_font_size_px,
        ),
        (108, 84, 64)
    );
    assert_eq!(
        (
            artist.preferred_font_size_px,
            artist.reduced_font_size_px,
            artist.minimum_font_size_px,
        ),
        (38, 32, 28)
    );
    assert_eq!(
        (
            album.preferred_font_size_px,
            album.reduced_font_size_px,
            album.minimum_font_size_px,
        ),
        (31, 27, 24)
    );
    assert_eq!(
        (
            title.maximum_lines,
            artist.maximum_lines,
            album.maximum_lines
        ),
        (3, 2, 2)
    );
}

#[test]
fn extreme_metadata_stops_reducing_at_readable_minimum_sizes() {
    let presentation = now_playing("extreme-metadata.json");
    let layout = metadata_layout(&presentation);
    let title = layout.title.expect("extreme fixture should have a Title");
    let artist = layout
        .artist
        .expect("extreme fixture should have an Artist");
    let album = layout.album.expect("extreme fixture should have an Album");

    assert_eq!(
        title.fitting_font_size(|_| false),
        title.minimum_font_size_px
    );
    assert_eq!(
        artist.fitting_font_size(|_| false),
        artist.minimum_font_size_px
    );
    assert_eq!(
        album.fitting_font_size(|_| false),
        album.minimum_font_size_px
    );
}

#[test]
fn selects_the_first_font_size_that_fits_the_allocated_pango_layout() {
    let layout = metadata_layout(&now_playing("long-metadata.json"));
    let title = layout.title.expect("long fixture should have a Title");
    let mut attempted_sizes = Vec::new();

    let selected = title.fitting_font_size(|font_size_px| {
        attempted_sizes.push(font_size_px);
        font_size_px <= title.reduced_font_size_px
    });

    assert_eq!(selected, title.reduced_font_size_px);
    assert_eq!(
        attempted_sizes,
        vec![title.preferred_font_size_px, title.reduced_font_size_px]
    );
}

#[test]
fn assigns_editorial_and_utility_typography_roles() {
    let layout = metadata_layout(&now_playing("playing.json"));

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
