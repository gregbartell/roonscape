use crate::presentation::NowPlayingPresentation;
use crate::{GallerySplitLayout, MetadataFontSizes, TextOverflow, Viewport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataTypography {
    EditorialSerif,
    UtilitySans,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLineLayout {
    pub text: String,
    pub typography: MetadataTypography,
    pub font_sizes: MetadataFontSizes,
    pub maximum_lines: u32,
    pub overflow: TextOverflow,
}

impl MetadataLineLayout {
    pub fn fitting_font_size(&self, fits: impl FnMut(u32) -> bool) -> u32 {
        self.font_sizes.fitting_font_size(fits)
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
        font_sizes: sizes,
        maximum_lines,
        overflow: TextOverflow::EllipsizeEnd,
    }
}
