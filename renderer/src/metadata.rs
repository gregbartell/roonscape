use crate::presentation::NowPlayingPresentation;
use crate::{GallerySplitLayout, MetadataFontSizes, Viewport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataTypography {
    EditorialSerif,
    UtilitySans,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLineLayout {
    pub text: String,
    pub typography: MetadataTypography,
    pub preferred_font_size_px: u32,
    pub reduced_font_size_px: u32,
    pub minimum_font_size_px: u32,
    pub maximum_lines: u32,
}

impl MetadataLineLayout {
    pub fn fitting_font_size(&self, fits: impl FnMut(u32) -> bool) -> u32 {
        MetadataFontSizes {
            preferred_px: self.preferred_font_size_px,
            reduced_px: self.reduced_font_size_px,
            minimum_px: self.minimum_font_size_px,
        }
        .fitting_font_size(fits)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MetadataLayout {
    pub title: Option<MetadataLineLayout>,
    pub artist: Option<MetadataLineLayout>,
    pub album: Option<MetadataLineLayout>,
}

pub fn metadata_layout(presentation: &NowPlayingPresentation) -> MetadataLayout {
    metadata_layout_for_viewport(presentation, Viewport::REFERENCE)
}

pub fn metadata_layout_for_viewport(
    presentation: &NowPlayingPresentation,
    viewport: Viewport,
) -> MetadataLayout {
    let typography = GallerySplitLayout::for_viewport(viewport).typography;
    MetadataLayout {
        title: presentation.title.as_deref().map(|text| {
            line_layout(
                text,
                MetadataTypography::EditorialSerif,
                typography.title,
                3,
            )
        }),
        artist: presentation
            .artist
            .as_deref()
            .map(|text| line_layout(text, MetadataTypography::UtilitySans, typography.artist, 2)),
        album: presentation.album.as_deref().map(|text| {
            line_layout(
                text,
                MetadataTypography::EditorialSerif,
                typography.album,
                2,
            )
        }),
    }
}

fn line_layout(
    text: &str,
    typography: MetadataTypography,
    sizes: MetadataFontSizes,
    maximum_lines: u32,
) -> MetadataLineLayout {
    MetadataLineLayout {
        text: text.to_owned(),
        typography,
        preferred_font_size_px: sizes.preferred_px,
        reduced_font_size_px: sizes.reduced_px,
        minimum_font_size_px: sizes.minimum_px,
        maximum_lines,
    }
}
