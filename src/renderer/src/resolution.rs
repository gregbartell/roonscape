use std::path::Path;

use crate::presentation::trackless_full_field;
use crate::{Presentation, PresentationPalette, PresentationStatus};

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPresentation {
    pub presentation: Presentation,
    pub palette: PresentationPalette,
}

impl ResolvedPresentation {
    pub fn status(&self) -> &PresentationStatus {
        match &self.presentation {
            Presentation::NowPlaying(now_playing) => &now_playing.status,
            Presentation::FullField(full_field) => &full_field.status,
        }
    }
}

pub fn resolve_presentation(
    presentation: &Presentation,
    repository_root: &Path,
) -> ResolvedPresentation {
    let Presentation::NowPlaying(now_playing) = presentation else {
        return resolved(presentation.clone(), PresentationPalette::fallback());
    };
    let artwork_path = now_playing
        .artwork_path
        .as_deref()
        .map(|path| repository_root.join(path));
    let palette = artwork_path
        .as_deref()
        .and_then(|path| PresentationPalette::from_artwork(path).ok());
    if let Some(palette) = palette {
        return resolved(presentation.clone(), palette);
    }
    if !now_playing.has_usable_metadata() {
        return resolved(
            Presentation::FullField(trackless_full_field(now_playing)),
            PresentationPalette::fallback(),
        );
    }

    let mut now_playing = now_playing.clone();
    now_playing.artwork_revision = None;
    now_playing.artwork_path = None;

    resolved(
        Presentation::NowPlaying(now_playing),
        PresentationPalette::fallback(),
    )
}

fn resolved(presentation: Presentation, palette: PresentationPalette) -> ResolvedPresentation {
    ResolvedPresentation {
        presentation,
        palette,
    }
}
