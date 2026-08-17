use crate::presentation::{
    INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityTransform, NowPlayingPresentation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InactivityLayout {
    pub content_viewport: Viewport,
    pub margin_start_px: u32,
    pub margin_end_px: u32,
    pub margin_top_px: u32,
    pub margin_bottom_px: u32,
}

impl InactivityLayout {
    pub fn for_viewport(viewport: Viewport, transform: InactivityTransform) -> Self {
        if transform == InactivityTransform::default() {
            return Self {
                content_viewport: viewport,
                margin_start_px: 0,
                margin_end_px: 0,
                margin_top_px: 0,
                margin_bottom_px: 0,
            };
        }

        let horizontal_bound = INACTIVE_HORIZONTAL_BOUND as u32;
        let vertical_bound = INACTIVE_VERTICAL_BOUND as u32;
        assert!(
            viewport.width_px > horizontal_bound * 2,
            "viewport width must contain the OLED movement envelope"
        );
        assert!(
            viewport.height_px > vertical_bound * 2,
            "viewport height must contain the OLED movement envelope"
        );
        assert!(
            (-INACTIVE_HORIZONTAL_BOUND..=INACTIVE_HORIZONTAL_BOUND).contains(&transform.offset.x),
            "horizontal OLED offset must stay inside the configured bound"
        );
        assert!(
            (-INACTIVE_VERTICAL_BOUND..=INACTIVE_VERTICAL_BOUND).contains(&transform.offset.y),
            "vertical OLED offset must stay inside the configured bound"
        );

        Self {
            content_viewport: Viewport::new(
                viewport.width_px - horizontal_bound * 2,
                viewport.height_px - vertical_bound * 2,
            ),
            margin_start_px: (INACTIVE_HORIZONTAL_BOUND + transform.offset.x) as u32,
            margin_end_px: (INACTIVE_HORIZONTAL_BOUND - transform.offset.x) as u32,
            margin_top_px: (INACTIVE_VERTICAL_BOUND + transform.offset.y) as u32,
            margin_bottom_px: (INACTIVE_VERTICAL_BOUND - transform.offset.y) as u32,
        }
    }
}

impl Viewport {
    pub const WINDOWED_FIXTURE: Self = Self::new(1600, 900);

    pub const fn new(width_px: u32, height_px: u32) -> Self {
        assert!(width_px > 0, "viewport width must be positive");
        assert!(height_px > 0, "viewport height must be positive");
        Self {
            width_px,
            height_px,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NowPlayingField {
    Cohesive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkFit {
    Contain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkAlignment {
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkContent {
    Supplied,
    QuietField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtworkLayout {
    pub content: ArtworkContent,
    pub fit: ArtworkFit,
    pub alignment: ArtworkAlignment,
}

impl ArtworkLayout {
    pub fn for_presentation(presentation: &NowPlayingPresentation) -> Self {
        Self {
            content: if presentation.artwork_path.is_some() {
                ArtworkContent::Supplied
            } else {
                ArtworkContent::QuietField
            },
            fit: ArtworkFit::Contain,
            alignment: ArtworkAlignment::Center,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityPlacement {
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOverflow {
    EllipsizeEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityLineLayout {
    pub maximum_lines: u32,
    pub overflow: TextOverflow,
}

impl IdentityLineLayout {
    const DEFENSIVE: Self = Self {
        maximum_lines: 1,
        overflow: TextOverflow::EllipsizeEnd,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NowPlayingRole {
    PresentationStatus,
    Title,
    Artist,
    Album,
    Progress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetadataFontSizes {
    pub preferred_px: u32,
    pub reduced_px: u32,
    pub minimum_px: u32,
}

impl MetadataFontSizes {
    pub fn fitting_font_size(&self, mut fits: impl FnMut(u32) -> bool) -> u32 {
        for font_size_px in [self.preferred_px, self.reduced_px, self.minimum_px] {
            if fits(font_size_px) || font_size_px == self.minimum_px {
                return font_size_px;
            }
        }

        unreachable!("the minimum metadata font size is always selected")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NowPlayingTypography {
    pub title: MetadataFontSizes,
    pub artist: MetadataFontSizes,
    pub album: MetadataFontSizes,
    pub time_px: u32,
    pub identity_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationStatusLayout {
    pub symbol_size_px: u32,
    pub symbol_gap_px: u32,
    pub font_px: u32,
    pub letter_spacing_px: u32,
}

impl PresentationStatusLayout {
    fn for_viewport(viewport: Viewport) -> Self {
        let font_px = scaled(viewport.width_px, 0.022, 29, 42);
        Self {
            symbol_size_px: scaled(viewport.width_px, 0.0385, 56, 96),
            symbol_gap_px: scaled(viewport.width_px, 0.0115, 16, 44),
            font_px,
            letter_spacing_px: ((font_px as f64) * 0.055).round() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullFieldLineLayout {
    pub maximum_lines: u32,
    pub wrap: bool,
    pub overflow: TextOverflow,
}

impl FullFieldLineLayout {
    const COMPLETE: Self = Self {
        maximum_lines: 1,
        wrap: false,
        overflow: TextOverflow::EllipsizeEnd,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullFieldLayout {
    pub outer_gutter_px: u32,
    pub copy_width_px: u32,
    pub accent_width_px: u32,
    pub accent_padding_px: u32,
    pub status_spacing_px: u32,
    pub presentation_status: PresentationStatusLayout,
    pub heading_px: u32,
    pub heading_line: FullFieldLineLayout,
    pub explanation_spacing_px: u32,
    pub explanation_px: u32,
    pub explanation_line: FullFieldLineLayout,
    pub identity_width_px: u32,
    pub identity_right_inset_px: u32,
    pub identity_gap_px: u32,
    pub identity_px: u32,
    pub identity_placement: IdentityPlacement,
    pub identity_line: IdentityLineLayout,
}

impl FullFieldLayout {
    pub fn for_viewport(viewport: Viewport) -> Self {
        let now_playing_layout = NowPlayingLayout::for_viewport(viewport);
        let outer_gutter_px = scaled(viewport.width_px, 0.042, 32, 160);
        let explanation_px = scaled(viewport.width_px, 0.0135, 16, 46);
        Self {
            outer_gutter_px,
            copy_width_px: viewport.width_px.saturating_sub(outer_gutter_px * 2),
            accent_width_px: scaled(viewport.width_px, 0.0038, 5, 15),
            accent_padding_px: scaled(viewport.width_px, 0.04, 32, 144),
            status_spacing_px: scaled(viewport.height_px, 0.036, 29, 80),
            presentation_status: PresentationStatusLayout::for_viewport(viewport),
            heading_px: scaled(viewport.width_px, 0.05, 51, 160),
            heading_line: FullFieldLineLayout::COMPLETE,
            explanation_spacing_px: ((explanation_px as f64) * 0.9).round() as u32,
            explanation_px,
            explanation_line: FullFieldLineLayout::COMPLETE,
            identity_width_px: now_playing_layout
                .metadata_column_width_px
                .saturating_sub(now_playing_layout.metadata_right_inset_px),
            identity_right_inset_px: now_playing_layout.metadata_right_inset_px,
            identity_gap_px: scaled(viewport.width_px, 0.018, 19, 64),
            identity_px: scaled(viewport.width_px, 0.0072, 11, 27),
            identity_placement: IdentityPlacement::BottomRight,
            identity_line: IdentityLineLayout::DEFENSIVE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowPlayingLayout {
    pub field: NowPlayingField,
    pub outer_gutter_px: u32,
    pub column_gap_px: u32,
    pub artwork_column_width_px: u32,
    pub metadata_column_width_px: u32,
    pub artwork_field_width_px: u32,
    pub artwork_field_height_px: u32,
    pub identity_placement: IdentityPlacement,
    pub identity_line: IdentityLineLayout,
    pub metadata_roles: Vec<NowPlayingRole>,
    pub metadata_right_inset_px: u32,
    pub status_top_inset_px: u32,
    pub artist_spacing_px: u32,
    pub album_spacing_px: u32,
    pub progress_spacing_px: u32,
    pub time_spacing_px: u32,
    pub identity_gap_px: u32,
    pub presentation_status: PresentationStatusLayout,
    pub progress_height_px: u32,
    pub artwork_shadow_offset_px: u32,
    pub artwork_shadow_blur_px: u32,
    pub typography: NowPlayingTypography,
}

impl NowPlayingLayout {
    pub fn for_presentation(presentation: &NowPlayingPresentation, viewport: Viewport) -> Self {
        let mut layout = Self::for_viewport(viewport);
        layout.metadata_roles = metadata_roles(presentation);
        layout
    }

    pub fn for_viewport(viewport: Viewport) -> Self {
        let outer_gutter_px = scaled(viewport.width_px, 0.042, 32, 160);
        let column_gap_px = scaled(viewport.width_px, 0.05, 40, 192);
        let content_width_px = viewport
            .width_px
            .saturating_sub(outer_gutter_px.saturating_mul(2))
            .saturating_sub(column_gap_px);
        let artwork_column_width_px = ((content_width_px as f64) * 0.59).round() as u32;
        let metadata_column_width_px = content_width_px - artwork_column_width_px;
        let artwork_shadow_offset_px = scaled(viewport.height_px, 0.04, 36, 86);
        let artwork_shadow_blur_px = scaled(viewport.height_px, 0.09, 81, 194);
        let artwork_shadow_extent_px =
            artwork_shadow_offset_px + artwork_shadow_blur_px.div_ceil(2);
        let artwork_height_limit_px = viewport
            .height_px
            .saturating_sub(artwork_shadow_extent_px.saturating_mul(2));
        let artwork_field_size_px = artwork_column_width_px
            .min(((viewport.height_px as f64) * 0.81).round() as u32)
            .min(artwork_height_limit_px);
        let typography = NowPlayingTypography {
            title: MetadataFontSizes {
                preferred_px: scaled(viewport.width_px, 0.046, 53, 168),
                reduced_px: scaled(viewport.width_px, 0.0365, 40, 128),
                minimum_px: scaled(viewport.width_px, 0.028, 36, 96),
            },
            artist: MetadataFontSizes {
                preferred_px: scaled(viewport.width_px, 0.0175, 22, 64),
                reduced_px: scaled(viewport.width_px, 0.0146, 20, 56),
                minimum_px: scaled(viewport.width_px, 0.0125, 18, 48),
            },
            album: MetadataFontSizes {
                preferred_px: scaled(viewport.width_px, 0.0122, 17, 45),
                reduced_px: scaled(viewport.width_px, 0.0106, 16, 40),
                minimum_px: scaled(viewport.width_px, 0.0094, 15, 35),
            },
            time_px: scaled(viewport.width_px, 0.0072, 11, 26),
            identity_px: scaled(viewport.width_px, 0.0072, 11, 27),
        };

        Self {
            field: NowPlayingField::Cohesive,
            outer_gutter_px,
            column_gap_px,
            artwork_column_width_px,
            metadata_column_width_px,
            artwork_field_width_px: artwork_field_size_px,
            artwork_field_height_px: artwork_field_size_px,
            identity_placement: IdentityPlacement::BottomRight,
            identity_line: IdentityLineLayout::DEFENSIVE,
            metadata_roles: Vec::new(),
            metadata_right_inset_px: scaled(viewport.width_px, 0.02, 24, 77),
            status_top_inset_px: scaled(viewport.height_px, 0.018, 16, 44),
            artist_spacing_px: scaled(viewport.height_px, 0.032, 26, 72),
            album_spacing_px: ((typography.album.preferred_px as f64) * 0.48).round() as u32,
            progress_spacing_px: scaled(viewport.height_px, 0.065, 45, 128),
            time_spacing_px: scaled(viewport.width_px, 0.0055, 7, 22),
            identity_gap_px: scaled(viewport.width_px, 0.018, 19, 64),
            presentation_status: PresentationStatusLayout::for_viewport(viewport),
            progress_height_px: scaled(viewport.width_px, 0.0016, 3, 6),
            artwork_shadow_offset_px,
            artwork_shadow_blur_px,
            typography,
        }
    }
}

fn metadata_roles(presentation: &NowPlayingPresentation) -> Vec<NowPlayingRole> {
    let mut roles = vec![NowPlayingRole::PresentationStatus];
    if presentation.title.is_some() {
        roles.push(NowPlayingRole::Title);
    }
    if presentation.artist.is_some() {
        roles.push(NowPlayingRole::Artist);
    }
    if presentation.album.is_some() {
        roles.push(NowPlayingRole::Album);
    }
    if presentation.progress.is_some() {
        roles.push(NowPlayingRole::Progress);
    }
    roles
}

fn scaled(axis_px: u32, ratio: f64, minimum_px: u32, maximum_px: u32) -> u32 {
    ((axis_px as f64) * ratio)
        .round()
        .clamp(minimum_px as f64, maximum_px as f64) as u32
}
