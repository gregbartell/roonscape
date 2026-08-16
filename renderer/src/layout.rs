use crate::presentation::NowPlayingPresentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub width_px: u32,
    pub height_px: u32,
}

impl Viewport {
    pub const REFERENCE: Self = Self::new(3840, 2160);
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
pub enum GalleryField {
    Cohesive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkFit {
    Contain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityPlacement {
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GallerySplitRole {
    PlaybackStatus,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GallerySplitTypography {
    pub title: MetadataFontSizes,
    pub artist: MetadataFontSizes,
    pub album: MetadataFontSizes,
    pub status_px: u32,
    pub time_px: u32,
    pub identity_px: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GallerySplitLayout {
    pub field: GalleryField,
    pub outer_gutter_px: u32,
    pub column_gap_px: u32,
    pub artwork_column_width_px: u32,
    pub metadata_column_width_px: u32,
    pub artwork_field_width_px: u32,
    pub artwork_field_height_px: u32,
    pub artwork_fit: ArtworkFit,
    pub identity_placement: IdentityPlacement,
    pub metadata_roles: Vec<GallerySplitRole>,
    pub metadata_right_inset_px: u32,
    pub status_to_title_spacing_px: u32,
    pub artist_spacing_px: u32,
    pub album_spacing_px: u32,
    pub progress_spacing_px: u32,
    pub time_spacing_px: u32,
    pub identity_gap_px: u32,
    pub state_dot_size_px: u32,
    pub progress_height_px: u32,
    pub artwork_shadow_offset_px: u32,
    pub artwork_shadow_blur_px: u32,
    pub typography: GallerySplitTypography,
}

impl GallerySplitLayout {
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
        let artwork_field_size_px =
            artwork_column_width_px.min(((viewport.height_px as f64) * 0.81).round() as u32);
        let typography = GallerySplitTypography {
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
            status_px: scaled(viewport.width_px, 0.008, 12, 30),
            time_px: scaled(viewport.width_px, 0.0072, 11, 26),
            identity_px: scaled(viewport.width_px, 0.0072, 11, 27),
        };

        Self {
            field: GalleryField::Cohesive,
            outer_gutter_px,
            column_gap_px,
            artwork_column_width_px,
            metadata_column_width_px,
            artwork_field_width_px: artwork_field_size_px,
            artwork_field_height_px: artwork_field_size_px,
            artwork_fit: ArtworkFit::Contain,
            identity_placement: IdentityPlacement::BottomRight,
            metadata_roles: Vec::new(),
            metadata_right_inset_px: scaled(viewport.width_px, 0.02, 24, 77),
            status_to_title_spacing_px: scaled(viewport.height_px, 0.046, 32, 96),
            artist_spacing_px: scaled(viewport.height_px, 0.032, 26, 72),
            album_spacing_px: ((typography.album.preferred_px as f64) * 0.48).round() as u32,
            progress_spacing_px: scaled(viewport.height_px, 0.065, 45, 128),
            time_spacing_px: scaled(viewport.width_px, 0.0055, 7, 22),
            identity_gap_px: scaled(viewport.width_px, 0.018, 19, 64),
            state_dot_size_px: ((typography.status_px as f64) * 0.58).round() as u32,
            progress_height_px: scaled(viewport.width_px, 0.0016, 3, 6),
            artwork_shadow_offset_px: scaled(viewport.height_px, 0.04, 36, 86),
            artwork_shadow_blur_px: scaled(viewport.height_px, 0.09, 81, 194),
            typography,
        }
    }
}

fn metadata_roles(presentation: &NowPlayingPresentation) -> Vec<GallerySplitRole> {
    let mut roles = vec![GallerySplitRole::PlaybackStatus];
    if presentation.title.is_some() {
        roles.push(GallerySplitRole::Title);
    }
    if presentation.artist.is_some() {
        roles.push(GallerySplitRole::Artist);
    }
    if presentation.album.is_some() {
        roles.push(GallerySplitRole::Album);
    }
    if presentation.progress.is_some() {
        roles.push(GallerySplitRole::Progress);
    }
    roles
}

fn scaled(axis_px: u32, ratio: f64, minimum_px: u32, maximum_px: u32) -> u32 {
    ((axis_px as f64) * ratio)
        .round()
        .clamp(minimum_px as f64, maximum_px as f64) as u32
}
