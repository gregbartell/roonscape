use crate::presentation::NowPlayingPresentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataTypography {
    EditorialSerif,
    UtilitySans,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataOverflow {
    EllipsizeEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLineLayout {
    pub text: String,
    pub typography: MetadataTypography,
    pub preferred_font_size_px: u32,
    pub reduced_font_size_px: u32,
    pub minimum_font_size_px: u32,
    pub maximum_lines: u32,
    pub overflow: MetadataOverflow,
}

impl MetadataLineLayout {
    pub fn fitting_font_size(&self, mut fits: impl FnMut(u32) -> bool) -> u32 {
        for font_size_px in [
            self.preferred_font_size_px,
            self.reduced_font_size_px,
            self.minimum_font_size_px,
        ] {
            if fits(font_size_px) || font_size_px == self.minimum_font_size_px {
                return font_size_px;
            }
        }

        unreachable!("the minimum metadata font size is always selected")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MetadataLayout {
    pub title: Option<MetadataLineLayout>,
    pub artist: Option<MetadataLineLayout>,
    pub album: Option<MetadataLineLayout>,
}

#[derive(Clone, Copy)]
struct LayoutBounds {
    typography: MetadataTypography,
    preferred_size_px: u32,
    reduced_size_px: u32,
    minimum_size_px: u32,
    maximum_lines: u32,
}

const TITLE_BOUNDS: LayoutBounds = LayoutBounds {
    typography: MetadataTypography::EditorialSerif,
    preferred_size_px: 108,
    reduced_size_px: 84,
    minimum_size_px: 64,
    maximum_lines: 3,
};

const ARTIST_BOUNDS: LayoutBounds = LayoutBounds {
    typography: MetadataTypography::UtilitySans,
    preferred_size_px: 38,
    reduced_size_px: 32,
    minimum_size_px: 28,
    maximum_lines: 2,
};

const ALBUM_BOUNDS: LayoutBounds = LayoutBounds {
    typography: MetadataTypography::EditorialSerif,
    preferred_size_px: 31,
    reduced_size_px: 27,
    minimum_size_px: 24,
    maximum_lines: 2,
};

pub fn metadata_layout(presentation: &NowPlayingPresentation) -> MetadataLayout {
    MetadataLayout {
        title: presentation
            .title
            .as_deref()
            .map(|text| line_layout(text, TITLE_BOUNDS)),
        artist: presentation
            .artist
            .as_deref()
            .map(|text| line_layout(text, ARTIST_BOUNDS)),
        album: presentation
            .album
            .as_deref()
            .map(|text| line_layout(text, ALBUM_BOUNDS)),
    }
}

fn line_layout(text: &str, bounds: LayoutBounds) -> MetadataLineLayout {
    MetadataLineLayout {
        text: text.to_owned(),
        typography: bounds.typography,
        preferred_font_size_px: bounds.preferred_size_px,
        reduced_font_size_px: bounds.reduced_size_px,
        minimum_font_size_px: bounds.minimum_size_px,
        maximum_lines: bounds.maximum_lines,
        overflow: MetadataOverflow::EllipsizeEnd,
    }
}
