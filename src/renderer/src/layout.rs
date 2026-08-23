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
pub struct ArtworkDimensions {
    pub width_px: u32,
    pub height_px: u32,
}

impl ArtworkDimensions {
    pub const fn new(width_px: u32, height_px: u32) -> Self {
        assert!(width_px > 0, "artwork width must be positive");
        assert!(height_px > 0, "artwork height must be positive");
        Self {
            width_px,
            height_px,
        }
    }

    fn contained_within(self, reservation: Self) -> Self {
        let width_constrained = u64::from(self.width_px) * u64::from(reservation.height_px)
            >= u64::from(self.height_px) * u64::from(reservation.width_px);
        if width_constrained {
            Self::new(
                reservation.width_px,
                rounded_scale(reservation.width_px, self.height_px, self.width_px)
                    .max(1)
                    .min(reservation.height_px),
            )
        } else {
            Self::new(
                rounded_scale(reservation.height_px, self.width_px, self.height_px)
                    .max(1)
                    .min(reservation.width_px),
                reservation.height_px,
            )
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkDecoration {
    ContainedImage(ArtworkDimensions),
    QuietSquareField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtworkPrintPlateLayout {
    pub footprint: ArtworkDimensions,
    pub offset_px: u32,
}

pub(crate) const ARTWORK_DECORATION_BORDER_WIDTH_PX: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtworkLayout {
    pub content: ArtworkContent,
    pub fit: ArtworkFit,
    pub alignment: ArtworkAlignment,
    pub decoration: ArtworkDecoration,
}

impl ArtworkLayout {
    pub fn for_presentation(
        presentation: &NowPlayingPresentation,
        intrinsic_dimensions: Option<ArtworkDimensions>,
    ) -> Self {
        let (content, decoration) = match (presentation.artwork_path.as_ref(), intrinsic_dimensions)
        {
            (Some(_), Some(dimensions)) => (
                ArtworkContent::Supplied,
                ArtworkDecoration::ContainedImage(dimensions),
            ),
            _ => (
                ArtworkContent::QuietField,
                ArtworkDecoration::QuietSquareField,
            ),
        };
        Self {
            content,
            fit: ArtworkFit::Contain,
            alignment: ArtworkAlignment::Center,
            decoration,
        }
    }

    pub fn fitted_image(self, reservation: ArtworkDimensions) -> Option<ArtworkDimensions> {
        let border_extent_px = self.border_extent_px();
        assert!(
            reservation.width_px > border_extent_px,
            "artwork reservation must contain its border"
        );
        assert!(
            reservation.height_px > border_extent_px,
            "artwork reservation must contain its border"
        );
        match (self.fit, self.decoration) {
            (ArtworkFit::Contain, ArtworkDecoration::ContainedImage(dimensions)) => {
                Some(dimensions.contained_within(ArtworkDimensions::new(
                    reservation.width_px - border_extent_px,
                    reservation.height_px - border_extent_px,
                )))
            }
            (ArtworkFit::Contain, ArtworkDecoration::QuietSquareField) => None,
        }
    }

    pub fn visible_decoration(self, reservation: ArtworkDimensions) -> ArtworkDimensions {
        match self.fitted_image(reservation) {
            Some(image) => {
                let border_extent_px = self.border_extent_px();
                ArtworkDimensions::new(
                    image.width_px + border_extent_px,
                    image.height_px + border_extent_px,
                )
            }
            None => reservation,
        }
    }

    fn border_extent_px(self) -> u32 {
        ARTWORK_DECORATION_BORDER_WIDTH_PX * 2
    }
}

fn rounded_scale(axis_px: u32, numerator: u32, denominator: u32) -> u32 {
    let scaled = u64::from(axis_px) * u64::from(numerator);
    ((scaled + u64::from(denominator) / 2) / u64::from(denominator)) as u32
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
    Activity,
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
    pub activity_heading_px: u32,
    pub activity_detail_px: u32,
    pub identity_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetadataFitting {
    pub normal_title_to_credit_gap_px: u32,
    pub compact_title_to_credit_gap_px: u32,
    pub normal_album_gap_px: u32,
    pub compact_album_gap_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NowPlayingFooterContent {
    DeterminateProgress,
    IndeterminateActivity,
    IdentityOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityRowLayout {
    pub output_phrase_max_width_px: u32,
    pub zone_phrase_max_width_px: u32,
    pub phrase_gap_px: u32,
    pub label_px: u32,
    pub label_letter_spacing_px: u32,
    pub label_gap_px: u32,
    pub separator_size_px: u32,
    pub phrase_alignment: IdentityPhraseAlignment,
}

impl IdentityRowLayout {
    pub fn tracked_label_letter_spacing_px(label_px: u32) -> u32 {
        ((label_px as f64) * 0.04).round() as u32
    }

    pub fn separator_diameter_px(name_px: u32) -> u32 {
        ((name_px * 5).div_ceil(26)).clamp(4, 10)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityPhraseAlignment {
    Baseline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationStatusLayout {
    pub symbol_size_px: u32,
    pub symbol_gap_px: u32,
    pub font_px: u32,
    pub letter_spacing_px: u32,
    pub decoration: PresentationStatusDecoration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationStatusDecoration {
    Circle,
    CircleFree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtworkFieldAnchors {
    pub artwork_top_viewport_y_px: u32,
    pub artwork_bottom_viewport_y_px: u32,
    pub information_rail_inset_px: u32,
    pub presentation_status_top_viewport_y_px: u32,
    pub information_rail_bottom_viewport_y_px: u32,
}

impl ArtworkFieldAnchors {
    fn for_artwork_field(
        viewport: Viewport,
        artwork_top_viewport_y_px: u32,
        artwork_field_height_px: u32,
    ) -> Self {
        let artwork_bottom_viewport_y_px = artwork_top_viewport_y_px + artwork_field_height_px;
        let information_rail_inset_px = rounded_fraction(viewport.height_px, 13, 1_000);

        Self {
            artwork_top_viewport_y_px,
            artwork_bottom_viewport_y_px,
            information_rail_inset_px,
            presentation_status_top_viewport_y_px: artwork_top_viewport_y_px
                + information_rail_inset_px,
            information_rail_bottom_viewport_y_px: artwork_bottom_viewport_y_px
                - information_rail_inset_px,
        }
    }

    pub fn presentation_status_margin_top_px(self, container_top_viewport_y_px: u32) -> u32 {
        self.presentation_status_top_viewport_y_px
            .saturating_sub(container_top_viewport_y_px)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BottomAnchor {
    pub bottom_viewport_y_px: u32,
    viewport_height_px: u32,
}

impl BottomAnchor {
    fn for_now_playing(
        viewport: Viewport,
        anchors: ArtworkFieldAnchors,
        footer_optical_raise_px: u32,
    ) -> Self {
        Self {
            bottom_viewport_y_px: anchors
                .information_rail_bottom_viewport_y_px
                .saturating_sub(footer_optical_raise_px),
            viewport_height_px: viewport.height_px,
        }
    }

    fn for_full_field(viewport: Viewport, anchors: ArtworkFieldAnchors) -> Self {
        let established_inset_px = scaled(viewport.height_px, 0.018, 16, 44);
        Self {
            bottom_viewport_y_px: anchors.artwork_bottom_viewport_y_px - established_inset_px,
            viewport_height_px: viewport.height_px,
        }
    }

    pub fn margin_bottom_px(self, container_bottom_viewport_inset_px: u32) -> u32 {
        self.viewport_height_px
            .saturating_sub(container_bottom_viewport_inset_px)
            .saturating_sub(self.bottom_viewport_y_px)
    }
}

impl PresentationStatusLayout {
    fn for_now_playing(viewport: Viewport) -> Self {
        let font_px = scaled(viewport.height_px, 0.02685, 22, 58);
        Self {
            symbol_size_px: ((font_px as f64) * 1.42).round() as u32,
            symbol_gap_px: ((font_px as f64) * 0.42).round() as u32,
            font_px,
            letter_spacing_px: ((font_px as f64) * 0.105).round() as u32,
            decoration: PresentationStatusDecoration::CircleFree,
        }
    }

    fn for_full_field(viewport: Viewport) -> Self {
        let font_px = scaled(viewport.width_px, 0.022, 29, 52);
        Self {
            symbol_size_px: scaled(viewport.width_px, 0.0385, 56, 96),
            symbol_gap_px: scaled(viewport.width_px, 0.0115, 16, 44),
            font_px,
            letter_spacing_px: ((font_px as f64) * 0.055).round() as u32,
            decoration: PresentationStatusDecoration::Circle,
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
pub struct FullFieldFontSize {
    pub preferred_px: u32,
}

impl FullFieldFontSize {
    pub fn fitting_font_size(self, mut fits: impl FnMut(u32) -> bool) -> u32 {
        (1..=self.preferred_px)
            .rev()
            .find(|font_size_px| fits(*font_size_px))
            .unwrap_or(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullFieldSlot {
    pub top_viewport_y_px: u32,
    pub height_px: u32,
}

impl FullFieldSlot {
    pub const fn bottom_viewport_y_px(self) -> u32 {
        self.top_viewport_y_px + self.height_px
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullFieldLayout {
    pub outer_gutter_px: u32,
    pub composition_width_px: u32,
    pub composition_left_viewport_x_px: u32,
    pub accent_width_px: u32,
    pub accent_padding_px: u32,
    pub text_left_viewport_x_px: u32,
    pub status_spacing_px: u32,
    pub presentation_status: PresentationStatusLayout,
    pub presentation_status_slot: FullFieldSlot,
    pub heading_slot: FullFieldSlot,
    pub heading_font: FullFieldFontSize,
    pub heading_line: FullFieldLineLayout,
    pub explanation_spacing_px: u32,
    pub explanation_slot: FullFieldSlot,
    pub explanation_font: FullFieldFontSize,
    pub explanation_line: FullFieldLineLayout,
    pub identity_anchor: BottomAnchor,
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
        let presentation_status = PresentationStatusLayout::for_full_field(viewport);
        let outer_gutter_px = scaled(viewport.width_px, 0.042, 32, 160);
        let composition_width_px = rounded_fraction(viewport.width_px, 3, 5);
        let composition_left_viewport_x_px =
            viewport.width_px.saturating_sub(composition_width_px) / 2;
        let accent_width_px = scaled(viewport.width_px, 0.0038, 5, 15);
        let accent_padding_px = scaled(viewport.width_px, 0.04, 32, 144);
        let heading_font = FullFieldFontSize {
            preferred_px: scaled(viewport.width_px, 0.05, 51, 160),
        };
        let explanation_font = FullFieldFontSize {
            preferred_px: scaled(viewport.width_px, 0.0135, 16, 48),
        };
        let heading_slot_height_px = rounded_fraction(heading_font.preferred_px, 5, 4);
        let heading_slot_top_viewport_y_px =
            viewport.height_px.saturating_sub(heading_slot_height_px) / 2;
        let presentation_status_slot_height_px = presentation_status.symbol_size_px;
        let status_spacing_px = scaled(viewport.height_px, 0.036, 29, 80);
        let presentation_status_top_viewport_y_px = heading_slot_top_viewport_y_px
            .saturating_sub(status_spacing_px)
            .saturating_sub(presentation_status_slot_height_px);
        let explanation_spacing_px = ((explanation_font.preferred_px as f64) * 0.9).round() as u32;
        let explanation_slot_top_viewport_y_px =
            heading_slot_top_viewport_y_px + heading_slot_height_px + explanation_spacing_px;
        let explanation_slot_height_px =
            ((explanation_font.preferred_px as f64) * 1.45).round() as u32;
        let (identity_width_px, identity_right_inset_px) =
            full_field_identity_geometry(viewport, outer_gutter_px);
        Self {
            outer_gutter_px,
            composition_width_px,
            composition_left_viewport_x_px,
            accent_width_px,
            accent_padding_px,
            text_left_viewport_x_px: composition_left_viewport_x_px
                + accent_width_px
                + accent_padding_px,
            status_spacing_px,
            presentation_status,
            presentation_status_slot: FullFieldSlot {
                top_viewport_y_px: presentation_status_top_viewport_y_px,
                height_px: presentation_status_slot_height_px,
            },
            heading_slot: FullFieldSlot {
                top_viewport_y_px: heading_slot_top_viewport_y_px,
                height_px: heading_slot_height_px,
            },
            heading_font,
            heading_line: FullFieldLineLayout::COMPLETE,
            explanation_spacing_px,
            explanation_slot: FullFieldSlot {
                top_viewport_y_px: explanation_slot_top_viewport_y_px,
                height_px: explanation_slot_height_px,
            },
            explanation_font,
            explanation_line: FullFieldLineLayout::COMPLETE,
            identity_anchor: BottomAnchor::for_full_field(
                viewport,
                now_playing_layout.artwork_field_anchors,
            ),
            identity_width_px,
            identity_right_inset_px,
            identity_gap_px: scaled(viewport.width_px, 0.018, 19, 64),
            identity_px: scaled(viewport.width_px, 0.0125, 20, 48),
            identity_placement: IdentityPlacement::BottomRight,
            identity_line: IdentityLineLayout::DEFENSIVE,
        }
    }

    pub fn accent_bottom_viewport_y_px(self, has_explanation: bool) -> u32 {
        if has_explanation {
            self.explanation_slot.bottom_viewport_y_px()
        } else {
            self.heading_slot.bottom_viewport_y_px()
        }
    }

    pub fn text_width_px(self) -> u32 {
        self.composition_width_px
            .saturating_sub(self.accent_width_px)
            .saturating_sub(self.accent_padding_px)
    }
}

fn full_field_identity_geometry(viewport: Viewport, outer_gutter_px: u32) -> (u32, u32) {
    // Full-field Presentations share the responsive vertical anchor only; their
    // established horizontal identity grid is independent of Now Playing.
    let column_gap_px = scaled(viewport.width_px, 0.05, 40, 192);
    let content_width_px = viewport
        .width_px
        .saturating_sub(outer_gutter_px.saturating_mul(2))
        .saturating_sub(column_gap_px);
    let identity_column_width_px = content_width_px - rounded_fraction(content_width_px, 59, 100);
    let identity_right_inset_px = scaled(viewport.width_px, 0.02, 24, 77);
    (
        identity_column_width_px.saturating_sub(identity_right_inset_px),
        identity_right_inset_px,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NowPlayingInformationLayout {
    pub left_viewport_x_px: u32,
    pub utility_width_px: u32,
    pub musical_metadata_width_px: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowPlayingLayout {
    pub field: NowPlayingField,
    pub outer_gutter_px: u32,
    pub column_gap_px: u32,
    pub artwork_column_width_px: u32,
    pub artwork_field_width_px: u32,
    pub artwork_field_height_px: u32,
    pub artwork_print_plate: ArtworkPrintPlateLayout,
    pub identity_placement: IdentityPlacement,
    pub identity_line: IdentityLineLayout,
    pub metadata_roles: Vec<NowPlayingRole>,
    pub information: NowPlayingInformationLayout,
    pub artwork_field_anchors: ArtworkFieldAnchors,
    pub footer_anchor: BottomAnchor,
    pub artist_spacing_px: u32,
    pub album_spacing_px: u32,
    pub time_spacing_px: u32,
    pub footer_content: NowPlayingFooterContent,
    pub footer_gap_px: u32,
    pub footer_optical_raise_px: u32,
    pub footer_height_px: u32,
    pub identity_row: IdentityRowLayout,
    pub presentation_status: PresentationStatusLayout,
    pub progress_height_px: u32,
    pub activity_waveform_width_px: u32,
    pub activity_waveform_height_px: u32,
    pub activity_copy_gap_px: u32,
    pub artwork_shadow_offset_px: u32,
    pub artwork_shadow_blur_px: u32,
    pub typography: NowPlayingTypography,
    pub metadata_fitting: MetadataFitting,
    pub metadata_optical_correction_px: u32,
    pub metadata_height_budget_px: u32,
    pub metadata_region_top_viewport_y_px: u32,
    pub metadata_region_bottom_viewport_y_px: u32,
}

impl NowPlayingLayout {
    pub fn for_presentation(presentation: &NowPlayingPresentation, viewport: Viewport) -> Self {
        let mut layout = Self::for_viewport(viewport);
        layout.metadata_roles = metadata_roles(presentation);
        layout.footer_content = if presentation.progress.is_some() {
            NowPlayingFooterContent::DeterminateProgress
        } else if presentation.activity.is_some() {
            NowPlayingFooterContent::IndeterminateActivity
        } else {
            NowPlayingFooterContent::IdentityOnly
        };
        layout.refresh_metadata_height_budget();
        layout
    }

    pub fn spacing_before_album_px(&self, artist_is_present: bool) -> u32 {
        if artist_is_present {
            self.album_spacing_px
        } else {
            self.artist_spacing_px
        }
    }

    pub fn for_viewport(viewport: Viewport) -> Self {
        let outer_gutter_px = scaled(viewport.width_px, 0.0425, 32, 160);
        let column_gap_px = scaled(viewport.width_px, 0.042, 40, 192);
        let content_width_px = viewport
            .width_px
            .saturating_sub(outer_gutter_px.saturating_mul(2))
            .saturating_sub(column_gap_px);
        let artwork_field_size_px = rounded_fraction(viewport.height_px, 84, 100)
            .min(rounded_fraction(viewport.width_px, 56, 100));
        let artwork_column_width_px = artwork_field_size_px;
        let utility_width_px = content_width_px - artwork_column_width_px;
        let information = NowPlayingInformationLayout {
            left_viewport_x_px: outer_gutter_px + artwork_column_width_px + column_gap_px,
            utility_width_px,
            musical_metadata_width_px: utility_width_px.min(rounded_fraction(
                viewport.height_px,
                72,
                100,
            )),
        };
        let artwork_shadow_offset_px = scaled(viewport.height_px, 0.0085, 6, 20);
        let artwork_shadow_blur_px = scaled(viewport.height_px, 0.017, 12, 41);
        let artwork_top_viewport_y_px =
            viewport.height_px.saturating_sub(artwork_field_size_px) / 2;
        let artwork_field_anchors = ArtworkFieldAnchors::for_artwork_field(
            viewport,
            artwork_top_viewport_y_px,
            artwork_field_size_px,
        );
        let footer_optical_raise_px = rounded_fraction(viewport.height_px, 48, 1_000);
        let footer_anchor =
            BottomAnchor::for_now_playing(viewport, artwork_field_anchors, footer_optical_raise_px);
        let typography = NowPlayingTypography {
            title: MetadataFontSizes {
                preferred_px: scaled(viewport.height_px, 0.0815, 88, 180),
                reduced_px: scaled(viewport.height_px, 0.07, 68, 154).min(scaled(
                    viewport.width_px,
                    0.04,
                    68,
                    154,
                )),
                minimum_px: scaled(viewport.height_px, 0.058, 54, 128).min(scaled(
                    viewport.width_px,
                    0.034,
                    54,
                    128,
                )),
            },
            artist: MetadataFontSizes {
                preferred_px: scaled(viewport.height_px, 0.032, 26, 68),
                reduced_px: scaled(viewport.height_px, 0.026, 22, 56),
                minimum_px: scaled(viewport.height_px, 0.022, 20, 48),
            },
            album: MetadataFontSizes {
                preferred_px: scaled(viewport.height_px, 0.026, 22, 56),
                reduced_px: scaled(viewport.height_px, 0.022, 19, 48),
                minimum_px: scaled(viewport.height_px, 0.019, 18, 42),
            },
            time_px: scaled(viewport.height_px, 0.0259, 22, 56),
            activity_heading_px: scaled(viewport.height_px, 0.0259, 22, 56),
            activity_detail_px: scaled(viewport.height_px, 0.0259, 21, 56),
            identity_px: scaled(viewport.height_px, 0.0241, 22, 52).min(scaled(
                viewport.width_px,
                0.014,
                22,
                52,
            )),
        };
        let identity_phrase_gap_px = ((typography.identity_px as f64) * 1.1).round() as u32;
        let identity_label_px = ((typography.identity_px as f64) * 0.885).round() as u32;
        let identity_separator_size_px =
            IdentityRowLayout::separator_diameter_px(typography.identity_px);
        let identity_fixed_width_px = identity_phrase_gap_px * 2 + identity_separator_size_px;
        let identity_phrase_width_px = information
            .utility_width_px
            .saturating_sub(identity_fixed_width_px);
        let output_phrase_max_width_px = rounded_fraction(identity_phrase_width_px, 57, 100);
        let identity_row = IdentityRowLayout {
            output_phrase_max_width_px,
            zone_phrase_max_width_px: identity_phrase_width_px
                .saturating_sub(output_phrase_max_width_px),
            phrase_gap_px: identity_phrase_gap_px,
            label_px: identity_label_px,
            label_letter_spacing_px: IdentityRowLayout::tracked_label_letter_spacing_px(
                identity_label_px,
            ),
            label_gap_px: ((typography.identity_px as f64) * 0.42).round() as u32,
            separator_size_px: identity_separator_size_px,
            phrase_alignment: IdentityPhraseAlignment::Baseline,
        };
        let metadata_fitting = MetadataFitting {
            normal_title_to_credit_gap_px: scaled(viewport.height_px, 0.0185, 22, 40),
            compact_title_to_credit_gap_px: scaled(viewport.height_px, 0.014, 14, 28),
            normal_album_gap_px: ((typography.album.preferred_px as f64) * 0.38).round() as u32,
            compact_album_gap_px: ((typography.album.minimum_px as f64) * 0.38).round() as u32,
        };

        let mut layout = Self {
            field: NowPlayingField::Cohesive,
            outer_gutter_px,
            column_gap_px,
            artwork_column_width_px,
            artwork_field_width_px: artwork_field_size_px,
            artwork_field_height_px: artwork_field_size_px,
            artwork_print_plate: ArtworkPrintPlateLayout {
                footprint: ArtworkDimensions::new(artwork_field_size_px, artwork_field_size_px),
                offset_px: scaled(viewport.height_px, 0.0045, 6, 12),
            },
            identity_placement: IdentityPlacement::BottomRight,
            identity_line: IdentityLineLayout::DEFENSIVE,
            metadata_roles: Vec::new(),
            information,
            artwork_field_anchors,
            footer_anchor,
            artist_spacing_px: metadata_fitting.normal_title_to_credit_gap_px,
            album_spacing_px: metadata_fitting.normal_album_gap_px,
            time_spacing_px: ((typography.time_px as f64) * 0.58).round() as u32,
            footer_content: NowPlayingFooterContent::IdentityOnly,
            footer_gap_px: scaled(viewport.height_px, 0.0185, 18, 40),
            footer_optical_raise_px,
            footer_height_px: 0,
            identity_row,
            presentation_status: PresentationStatusLayout::for_now_playing(viewport),
            progress_height_px: scaled(viewport.width_px, 0.002, 3, 5),
            activity_waveform_width_px: scaled(viewport.width_px, 0.058, 74, 180),
            activity_waveform_height_px: scaled(viewport.height_px, 0.065, 46, 96),
            activity_copy_gap_px: scaled(viewport.width_px, 0.012, 15, 42),
            artwork_shadow_offset_px,
            artwork_shadow_blur_px,
            typography,
            metadata_fitting,
            metadata_optical_correction_px: scaled(viewport.height_px, 0.004, 3, 10),
            metadata_height_budget_px: 0,
            metadata_region_top_viewport_y_px: 0,
            metadata_region_bottom_viewport_y_px: 0,
        };
        layout.refresh_metadata_height_budget();
        layout
    }

    fn refresh_metadata_height_budget(&mut self) {
        let identity_height_px = rounded_fraction(self.typography.identity_px, 5, 4);
        let footer_content_height_px = match self.footer_content {
            NowPlayingFooterContent::DeterminateProgress => {
                self.progress_height_px
                    + self.time_spacing_px
                    + rounded_fraction(self.typography.time_px, 5, 4)
            }
            NowPlayingFooterContent::IndeterminateActivity => self
                .activity_waveform_height_px
                .max(self.activity_copy_height_px()),
            NowPlayingFooterContent::IdentityOnly => 0,
        };
        let footer_gap_px = if self.footer_content == NowPlayingFooterContent::IdentityOnly {
            0
        } else {
            self.footer_gap_px
        };
        self.footer_height_px = footer_content_height_px + footer_gap_px + identity_height_px;
        let status_bottom_viewport_y_px = self
            .artwork_field_anchors
            .presentation_status_top_viewport_y_px
            + self.presentation_status.symbol_size_px;
        let footer_top_viewport_y_px = self
            .footer_anchor
            .bottom_viewport_y_px
            .saturating_sub(self.footer_height_px);
        self.metadata_region_top_viewport_y_px = status_bottom_viewport_y_px;
        self.metadata_region_bottom_viewport_y_px = footer_top_viewport_y_px;
        self.metadata_height_budget_px = footer_top_viewport_y_px
            .saturating_sub(status_bottom_viewport_y_px)
            .saturating_sub(self.presentation_status.font_px.saturating_mul(2))
            .saturating_sub(self.metadata_optical_correction_px.saturating_mul(2));
    }

    fn activity_copy_height_px(&self) -> u32 {
        rounded_fraction(self.typography.activity_heading_px, 5, 4)
            + rounded_fraction(self.typography.activity_detail_px, 5, 4)
    }

    pub fn metadata_group_offset_px(&self, group_height_px: u32) -> u32 {
        (self
            .metadata_region_bottom_viewport_y_px
            .saturating_sub(self.metadata_region_top_viewport_y_px)
            .saturating_sub(group_height_px)
            / 2)
        .saturating_sub(self.metadata_optical_correction_px)
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
    if presentation.activity.is_some() {
        roles.push(NowPlayingRole::Activity);
    }
    roles
}

fn scaled(axis_px: u32, ratio: f64, minimum_px: u32, maximum_px: u32) -> u32 {
    ((axis_px as f64) * ratio)
        .round()
        .clamp(minimum_px as f64, maximum_px as f64) as u32
}

fn rounded_fraction(value: u32, numerator: u32, denominator: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(numerator);
    ((scaled + u64::from(denominator) / 2) / u64::from(denominator)) as u32
}
