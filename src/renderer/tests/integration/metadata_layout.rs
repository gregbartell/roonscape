use crate::representative_viewports;
use crate::support;

use std::sync::OnceLock;

use gtk::pango::prelude::{FontExt, FontFamilyExt};
use gtk::pango::{self, FontDescription, Layout};
use gtk::prelude::FontMapExt;
use roonscape_renderer::{
    MetadataDensity, MetadataFontSizes, MetadataTypography, NowPlayingLayout, Presentation,
    Viewport, metadata_layout, parse_snapshot, presentation_from_snapshot,
    register_packaged_fallback_fonts,
};

const VIEWPORT: Viewport = Viewport::new(1920, 1200);

#[derive(Clone, Copy)]
struct ExpectedFontTiers {
    title: MetadataFontSizes,
    artist: MetadataFontSizes,
    album: MetadataFontSizes,
}

const fn font_tiers(preferred_px: u32, reduced_px: u32, minimum_px: u32) -> MetadataFontSizes {
    MetadataFontSizes {
        preferred_px,
        reduced_px,
        minimum_px,
    }
}

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
fn uses_the_selected_responsive_metadata_typography() {
    let presentation = now_playing("long-metadata.json");
    let expected_title = presentation
        .title
        .clone()
        .expect("long fixture should have a Title");
    let expected_tiers = [
        ExpectedFontTiers {
            title: font_tiers(88, 68, 54),
            artist: font_tiers(26, 22, 20),
            album: font_tiers(22, 19, 18),
        },
        ExpectedFontTiers {
            title: font_tiers(88, 68, 54),
            artist: font_tiers(29, 23, 20),
            album: font_tiers(23, 20, 18),
        },
        ExpectedFontTiers {
            title: font_tiers(98, 68, 54),
            artist: font_tiers(38, 31, 26),
            album: font_tiers(31, 26, 23),
        },
        ExpectedFontTiers {
            title: font_tiers(98, 77, 65),
            artist: font_tiers(38, 31, 26),
            album: font_tiers(31, 26, 23),
        },
        ExpectedFontTiers {
            title: font_tiers(88, 76, 63),
            artist: font_tiers(35, 28, 24),
            album: font_tiers(28, 24, 21),
        },
        ExpectedFontTiers {
            title: font_tiers(176, 151, 125),
            artist: font_tiers(68, 56, 48),
            album: font_tiers(56, 48, 41),
        },
        ExpectedFontTiers {
            title: font_tiers(180, 154, 128),
            artist: font_tiers(68, 56, 48),
            album: font_tiers(56, 48, 42),
        },
    ];

    for (viewport, expected_tiers) in representative_viewports::REPRESENTATIVE_VIEWPORTS
        .into_iter()
        .zip(expected_tiers)
    {
        let layout = metadata_layout(&presentation, viewport);
        let title = layout.title.expect("long fixture should have a Title");
        let artist = layout.artist.expect("long fixture should have an Artist");
        let album = layout.album.expect("long fixture should have an Album");

        assert_eq!(title.text, expected_title);
        assert_eq!(title.font_sizes, expected_tiers.title);
        assert_eq!(artist.font_sizes, expected_tiers.artist);
        assert_eq!(album.font_sizes, expected_tiers.album);
        assert_eq!(
            (
                title.maximum_lines,
                artist.maximum_lines,
                album.maximum_lines
            ),
            (5, 3, 3)
        );
    }
}

#[test]
fn extreme_metadata_uses_expanded_bounds_and_ellipsizes_at_readable_minimum_sizes() {
    let presentation = now_playing("extreme-metadata.json");
    let expected_title = presentation
        .title
        .clone()
        .expect("extreme fixture should have a Title");
    let expected_artist = presentation
        .artist
        .clone()
        .expect("extreme fixture should have an Artist");
    let expected_album = presentation
        .album
        .clone()
        .expect("extreme fixture should have an Album");

    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = metadata_layout(&presentation, viewport);
        let title = layout.title.expect("extreme fixture should have a Title");
        let artist = layout
            .artist
            .expect("extreme fixture should have an Artist");
        let album = layout.album.expect("extreme fixture should have an Album");

        assert_eq!(title.text, expected_title);
        assert_eq!(artist.text, expected_artist);
        assert_eq!(album.text, expected_album);
        assert_eq!(
            (
                title.maximum_lines,
                artist.maximum_lines,
                album.maximum_lines
            ),
            (5, 3, 3)
        );
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
        Some(MetadataTypography::ArtistSans)
    );
    assert_eq!(
        layout.album.map(|line| line.typography),
        Some(MetadataTypography::AlbumSans)
    );
}

#[test]
fn scales_metadata_typography_primarily_from_viewport_height() {
    let presentation = now_playing("playing.json");
    let preferred_title_sizes =
        representative_viewports::REPRESENTATIVE_VIEWPORTS.map(|viewport| {
            metadata_layout(&presentation, viewport)
                .title
                .expect("Playing should have a Title")
                .font_sizes
                .preferred_px
        });

    assert_eq!(preferred_title_sizes, [88, 88, 98, 98, 88, 176, 180]);
}

#[test]
fn gives_a_short_single_line_title_the_calm_preferred_tier() {
    let font_map = metadata_font_map();
    let context = font_map.create_context();
    let viewport = Viewport::new(1600, 900);
    let mut presentation = now_playing("playing.json");
    presentation.title = Some("Cellout".to_owned());
    let layout = metadata_layout(&presentation, viewport);
    let title = layout.title.expect("Playing should have a Title");
    let available_width_px = NowPlayingLayout::for_viewport(viewport)
        .information
        .musical_metadata_width_px;
    let plan = title.fitting_line_plan(available_width_px, |text, font_size_px| {
        title_width(&context, text, font_size_px)
    });

    assert_eq!(title.single_line_font_size_px(), 88);
    assert_eq!(
        title.single_line_font_size_px(),
        title.font_sizes.preferred_px
    );
    assert_eq!(plan.lines, ["Cellout"]);
    assert_eq!(plan.font_size_px, title.single_line_font_size_px());
    assert_eq!(plan.line_height_percent, 94);
}

#[test]
fn selects_the_largest_title_tier_that_fits_the_complete_metadata_budget() {
    let viewport = Viewport::new(1600, 900);
    let mut presentation = now_playing("playing.json");
    presentation.title = Some("Alpha beta gamma delta epsilon zeta eta theta".to_owned());
    presentation.artist = Some("An artist with several credited names".to_owned());
    presentation.album = Some("An album with an unusually descriptive edition".to_owned());
    let responsive = NowPlayingLayout::for_viewport(viewport);
    let metadata = metadata_layout(&presentation, viewport);

    let generous = metadata.fitting_group_plan(
        540,
        700,
        responsive.metadata_fitting,
        |_, text, font_size_px| (text.chars().count() as u32 * font_size_px / 2, font_size_px),
    );
    let constrained = metadata.fitting_group_plan(
        540,
        280,
        responsive.metadata_fitting,
        |_, text, font_size_px| (text.chars().count() as u32 * font_size_px / 2, font_size_px),
    );

    assert_eq!(generous.density, MetadataDensity::Normal);
    assert_eq!(
        generous.title.as_ref().map(|line| line.font_size_px),
        Some(responsive.typography.title.preferred_px),
    );
    assert!(constrained.height_px <= 280);
    assert!(
        constrained
            .title
            .as_ref()
            .is_some_and(|line| line.font_size_px < responsive.typography.title.preferred_px),
    );
}

#[test]
fn compact_credit_density_preserves_readable_floors_and_bounded_ellipsis() {
    let viewport = Viewport::new(1280, 720);
    let responsive = NowPlayingLayout::for_viewport(viewport);
    let metadata = metadata_layout(&now_playing("extreme-metadata.json"), viewport);

    let plan = metadata.fitting_group_plan(
        responsive.information.musical_metadata_width_px,
        330,
        responsive.metadata_fitting,
        |_, text, font_size_px| (text.chars().count() as u32 * font_size_px / 2, font_size_px),
    );
    assert_eq!(plan.density, MetadataDensity::CompactCredits);
    assert!(plan.height_px <= 330);
    assert_eq!(
        plan.title.as_ref().map(|line| line.font_size_px),
        Some(responsive.typography.title.minimum_px),
    );
    assert_eq!(
        plan.artist.as_ref().map(|line| line.font_size_px),
        Some(responsive.typography.artist.minimum_px),
    );
    assert_eq!(
        plan.album.as_ref().map(|line| line.font_size_px),
        Some(responsive.typography.album.minimum_px),
    );
    for line in [&plan.title, &plan.artist, &plan.album]
        .into_iter()
        .flatten()
    {
        assert!(line.ellipsized);
    }
}

#[test]
fn pango_group_fitting_covers_the_metadata_matrix_with_each_available_title_face() {
    let font_map = metadata_font_map();
    let context = font_map.create_context();
    let viewport = Viewport::new(1280, 720);
    let mut short = now_playing("playing.json");
    short.title = Some("Cellout".to_owned());
    let cases = [
        ("ordinary", now_playing("playing.json")),
        ("short", short),
        ("missing", now_playing("missing-metadata.json")),
        ("long", now_playing("long-metadata.json")),
        ("extreme", now_playing("extreme-metadata.json")),
    ];
    let mut title_families = vec!["Libre Baskerville"];
    if font_map
        .list_families()
        .iter()
        .any(|family| family.name() == "Sitka Display")
    {
        title_families.insert(0, "Sitka Display");
    }

    for title_family in title_families {
        assert_requested_family_resolved(&context, title_family);
        for (case, presentation) in &cases {
            let responsive = NowPlayingLayout::for_presentation(presentation, viewport);
            let metadata = metadata_layout(presentation, viewport);
            let plan = metadata.fitting_group_plan(
                responsive.information.musical_metadata_width_px,
                responsive.metadata_height_budget_px,
                responsive.metadata_fitting,
                |typography, text, font_size_px| {
                    metadata_measurement(&context, title_family, typography, text, font_size_px)
                },
            );

            assert!(
                plan.height_px <= responsive.metadata_height_budget_px,
                "{case} metadata with {title_family} exceeded the rail budget"
            );
            for (line, maximum_lines) in [(&plan.title, 5), (&plan.artist, 3), (&plan.album, 3)] {
                assert!(
                    line.as_ref()
                        .is_none_or(|line| line.lines.len() <= maximum_lines),
                    "{case} metadata with {title_family} exceeded its line bound"
                );
            }
            let title = plan
                .title
                .as_ref()
                .expect("every matrix fixture should retain its Title");
            match *case {
                "ordinary" | "short" | "missing" => {
                    assert_eq!(title.font_size_px, responsive.typography.title.preferred_px);
                    assert!(!title.ellipsized);
                    assert_eq!(plan.density, MetadataDensity::Normal);
                }
                "long" | "extreme" => {
                    assert_eq!(title.font_size_px, responsive.typography.title.minimum_px);
                    assert!(title.ellipsized);
                    assert_eq!(plan.density, MetadataDensity::CompactCredits);
                }
                _ => unreachable!("the matrix should contain only named metadata cases"),
            }
            if *case == "long" {
                let expected_lines: &[&str] = if title_family == "Sitka Display" {
                    &[
                        "An Imaginary",
                        "Catalogue of",
                        "Constellations",
                        "Observed Through the",
                        "Longest Night of the…",
                    ]
                } else {
                    &[
                        "An Imaginary",
                        "Catalogue of",
                        "Constellations",
                        "Observed…",
                    ]
                };
                assert_eq!(title.lines, expected_lines);
            }
            if *case == "extreme" {
                assert!(
                    plan.album.as_ref().is_some_and(|album| album.ellipsized),
                    "extreme Album copy should end-ellipsize with {title_family}"
                );
            }
        }
    }
}

#[test]
fn balances_long_titles_at_word_boundaries_with_fallback_typography() {
    let font_map = metadata_font_map();
    let context = font_map.create_context();
    let presentation = now_playing("long-metadata.json");
    let viewport = Viewport::new(2560, 1080);
    let available_width_px = NowPlayingLayout::for_viewport(viewport)
        .information
        .musical_metadata_width_px;
    let title = metadata_layout(&presentation, viewport)
        .title
        .expect("long fixture should have a Title");

    let plan = title.fitting_line_plan(available_width_px, |text, font_size_px| {
        title_width(&context, text, font_size_px)
    });

    assert_eq!(
        plan.lines,
        [
            "An Imaginary",
            "Catalogue of",
            "Constellations",
            "Observed Through the",
            "Longest Night of the…",
        ]
    );
    assert_eq!(plan.font_size_px, title.font_sizes.minimum_px);
    assert_eq!(plan.line_height_percent, 98);
    assert!(plan.ellipsized);
}

#[test]
fn preserves_the_readable_floor_and_clean_line_bound_at_four_by_three() {
    let font_map = metadata_font_map();
    let context = font_map.create_context();
    let presentation = now_playing("long-metadata.json");
    let viewport = Viewport::new(1600, 1200);
    let available_width_px = NowPlayingLayout::for_viewport(viewport)
        .information
        .musical_metadata_width_px;
    let title = metadata_layout(&presentation, viewport)
        .title
        .expect("long fixture should have a Title");

    let plan = title.fitting_line_plan(available_width_px, |text, font_size_px| {
        title_width(&context, text, font_size_px)
    });

    assert_eq!(plan.lines.len(), 5);
    assert_eq!(plan.font_size_px, 54);
    assert_eq!(plan.line_height_percent, 98);
    assert!(plan.ellipsized);
    assert!(plan.lines.last().is_some_and(|line| line.ends_with('…')));
    assert!(
        title_width(
            &context,
            plan.lines
                .last()
                .expect("the plan should have a final line"),
            plan.font_size_px,
        ) <= available_width_px
    );
}

#[test]
fn rebalances_a_very_short_final_title_line_when_a_better_plan_fits() {
    let mut presentation = now_playing("playing.json");
    presentation.title = Some("An imaginary catalogue of constellations above us".to_owned());
    let title = metadata_layout(&presentation, Viewport::new(1600, 900))
        .title
        .expect("the custom presentation should have a Title");

    let plan = title.fitting_line_plan(200, |text, _| text.chars().count() as u32 * 10);

    assert_eq!(
        plan.lines,
        ["An imaginary", "catalogue of", "constellations", "above us",]
    );
    assert_ne!(plan.lines.last().map(String::as_str), Some("us"));
}

#[test]
fn uses_tighter_leading_for_two_line_titles() {
    let mut presentation = now_playing("playing.json");
    presentation.title = Some("Punctuation! Heavy? Title: Still Balanced.".to_owned());
    let title = metadata_layout(&presentation, Viewport::new(1600, 900))
        .title
        .expect("the punctuation-heavy presentation should have a Title");

    let plan = title.fitting_line_plan(260, |text, _| text.chars().count() as u32 * 10);

    assert_eq!(
        plan.lines,
        ["Punctuation! Heavy?", "Title: Still Balanced."]
    );
    assert_eq!(plan.line_height_percent, 94);
}

#[test]
fn balances_three_line_titles_below_presentation_status() {
    let mut presentation = now_playing("playing.json");
    presentation.title = Some("Alpha beta gamma delta epsilon zeta".to_owned());
    let title = metadata_layout(&presentation, Viewport::new(1600, 900))
        .title
        .expect("the custom presentation should have a Title");

    let plan = title.fitting_line_plan(120, |text, _| text.chars().count() as u32 * 10);

    assert_eq!(plan.lines, ["Alpha beta", "gamma delta", "epsilon zeta"]);
    assert_eq!(plan.line_height_percent, 94);
}

#[test]
fn ellipsizes_extreme_titles_after_five_lines_at_the_readable_floor() {
    let font_map = metadata_font_map();
    let context = font_map.create_context();
    let presentation = now_playing("extreme-metadata.json");
    let viewport = Viewport::new(1280, 720);
    let available_width_px = NowPlayingLayout::for_viewport(viewport)
        .information
        .musical_metadata_width_px;
    let title = metadata_layout(&presentation, viewport)
        .title
        .expect("extreme fixture should have a Title");

    let plan = title.fitting_line_plan(available_width_px, |text, font_size_px| {
        title_width(&context, text, font_size_px)
    });

    assert_eq!(plan.lines.len(), 5);
    assert_eq!(plan.font_size_px, title.font_sizes.minimum_px);
    assert_eq!(plan.line_height_percent, 98);
    assert!(plan.ellipsized);
    assert!(plan.lines.last().is_some_and(|line| line.ends_with('…')));
    assert!(
        title_width(
            &context,
            plan.lines
                .last()
                .expect("the plan should have a final line"),
            plan.font_size_px,
        ) <= available_width_px
    );
}

fn metadata_font_map() -> pango::FontMap {
    let renderer_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    static REGISTERED_FONTS: OnceLock<()> = OnceLock::new();
    REGISTERED_FONTS.get_or_init(|| {
        register_packaged_fallback_fonts(renderer_root)
            .expect("packaged metadata fonts should register");
    });
    let font_map = pangocairo::FontMap::new();
    font_map.changed();
    let available_families = font_map
        .list_families()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert!(available_families.contains("Libre Baskerville"));
    font_map
}

fn title_width(context: &pango::Context, text: &str, font_size_px: u32) -> u32 {
    let line = Layout::new(context);
    let mut font = FontDescription::from_string("Libre Baskerville Bold");
    font.set_absolute_size(f64::from(font_size_px * pango::SCALE as u32));
    line.set_font_description(Some(&font));
    line.set_text(text);
    line.pixel_size()
        .0
        .try_into()
        .expect("line width should be nonnegative")
}

fn assert_requested_family_resolved(context: &pango::Context, title_family: &str) {
    let font = context
        .load_font(&FontDescription::from_string(title_family))
        .expect("the requested Title face should load");
    assert_eq!(
        font.describe().family().as_deref(),
        Some(title_family),
        "Pango silently substituted the requested Title face"
    );
}

fn metadata_measurement(
    context: &pango::Context,
    title_family: &str,
    typography: MetadataTypography,
    text: &str,
    font_size_px: u32,
) -> (u32, u32) {
    let line = Layout::new(context);
    let mut font = FontDescription::from_string(match typography {
        MetadataTypography::EditorialSerif => title_family,
        MetadataTypography::ArtistSans | MetadataTypography::AlbumSans => "IBM Plex Sans",
    });
    font.set_weight(match typography {
        MetadataTypography::EditorialSerif => pango::Weight::Bold,
        MetadataTypography::ArtistSans => pango::Weight::Semibold,
        MetadataTypography::AlbumSans => pango::Weight::Normal,
    });
    font.set_absolute_size(f64::from(font_size_px * pango::SCALE as u32));
    line.set_font_description(Some(&font));
    line.set_text(text);
    let (width_px, height_px) = line.pixel_size();
    (
        width_px.try_into().expect("width should be nonnegative"),
        height_px.try_into().expect("height should be nonnegative"),
    )
}
