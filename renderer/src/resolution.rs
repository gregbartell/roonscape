use std::path::Path;

use crate::presentation::trackless_full_field;
use crate::{Presentation, PresentationPalette};

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPresentation {
    pub presentation: Presentation,
    pub palette: PresentationPalette,
}

pub fn resolve_presentation(
    presentation: &Presentation,
    repository_root: &Path,
) -> ResolvedPresentation {
    let Presentation::NowPlaying(now_playing) = presentation else {
        return ResolvedPresentation {
            presentation: presentation.clone(),
            palette: PresentationPalette::fallback(),
        };
    };
    let artwork_path = now_playing
        .artwork_path
        .as_deref()
        .map(|path| repository_root.join(path));
    let palette = artwork_path
        .as_deref()
        .and_then(|path| PresentationPalette::from_artwork(path).ok());
    if palette.is_none() && !now_playing.has_usable_metadata() {
        return ResolvedPresentation {
            presentation: Presentation::FullField(trackless_full_field(now_playing)),
            palette: PresentationPalette::fallback(),
        };
    }

    ResolvedPresentation {
        presentation: presentation.clone(),
        palette: palette.unwrap_or_else(PresentationPalette::fallback),
    }
}
