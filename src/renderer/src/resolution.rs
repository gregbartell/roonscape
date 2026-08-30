use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::presentation::trackless_full_field;
use crate::{PaletteError, Presentation, PresentationPalette, PresentationStatus};

#[derive(Debug)]
pub struct ArtworkResolutionError {
    path: PathBuf,
    source: PaletteError,
}

impl fmt::Display for ArtworkResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not decode or derive a palette from artwork at {}: {}",
            self.path.display(),
            self.source
        )
    }
}

impl Error for ArtworkResolutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

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
    resolve_presentation_with_artwork(presentation, repository_root)
        .unwrap_or_else(|_| resolve_without_artwork_palette(presentation))
}

pub fn resolve_capture_presentation(
    presentation: &Presentation,
    repository_root: &Path,
) -> Result<ResolvedPresentation, ArtworkResolutionError> {
    resolve_presentation_with_artwork(presentation, repository_root)
}

fn resolve_presentation_with_artwork(
    presentation: &Presentation,
    repository_root: &Path,
) -> Result<ResolvedPresentation, ArtworkResolutionError> {
    let Presentation::NowPlaying(now_playing) = presentation else {
        return Ok(resolved(
            presentation.clone(),
            PresentationPalette::fallback(),
        ));
    };
    let Some(artwork_path) = now_playing.artwork_path.as_deref() else {
        return Ok(resolve_without_artwork_palette(presentation));
    };
    let artwork_path = repository_root.join(artwork_path);
    match PresentationPalette::from_artwork(&artwork_path) {
        Ok(palette) => Ok(resolved(presentation.clone(), palette)),
        Err(source) => Err(ArtworkResolutionError {
            path: artwork_path,
            source,
        }),
    }
}

fn resolve_without_artwork_palette(presentation: &Presentation) -> ResolvedPresentation {
    let Presentation::NowPlaying(now_playing) = presentation else {
        return resolved(presentation.clone(), PresentationPalette::fallback());
    };
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
