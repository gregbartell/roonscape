use std::collections::HashSet;
use std::error::Error;
use std::ffi::CString;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const FALLBACK_FONT_FILES: [&str; 4] = [
    "assets/fonts/LibreBaskerville-Variable.ttf",
    "assets/fonts/LibreBaskerville-Italic-Variable.ttf",
    "assets/fonts/IBMPlexSans-Variable.ttf",
    "assets/fonts/IBMPlexSans-Italic-Variable.ttf",
];

pub const FALLBACK_FONT_LICENSES: [&str; 2] = [
    "assets/fonts/Libre-Baskerville-OFL.txt",
    "assets/fonts/IBM-Plex-Sans-OFL.txt",
];

const PREFERRED_NOW_PLAYING_TITLE_FAMILY: &str = "Sitka Display";
const PREFERRED_FULL_FIELD_EDITORIAL_FAMILY: &str = "Palatino Linotype";
const PREFERRED_FULL_FIELD_UTILITY_FAMILY: &str = "Segoe UI";
const FALLBACK_EDITORIAL_FAMILY: &str = "Libre Baskerville";
const PACKAGED_SUPPORTING_FAMILY: &str = "IBM Plex Sans";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NowPlayingTitleFace {
    Preferred,
    Fallback,
}

impl NowPlayingTitleFace {
    pub const fn family(self) -> &'static str {
        match self {
            Self::Preferred => PREFERRED_NOW_PLAYING_TITLE_FAMILY,
            Self::Fallback => FALLBACK_EDITORIAL_FAMILY,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FullFieldTypographyPair {
    Preferred,
    Fallback,
}

impl FullFieldTypographyPair {
    const fn families(self) -> (&'static str, &'static str) {
        match self {
            Self::Preferred => (
                PREFERRED_FULL_FIELD_EDITORIAL_FAMILY,
                PREFERRED_FULL_FIELD_UTILITY_FAMILY,
            ),
            Self::Fallback => (FALLBACK_EDITORIAL_FAMILY, PACKAGED_SUPPORTING_FAMILY),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypographySelection {
    now_playing_title: NowPlayingTitleFace,
    full_field: FullFieldTypographyPair,
}

impl TypographySelection {
    pub const fn now_playing_title_face(self) -> NowPlayingTitleFace {
        self.now_playing_title
    }

    pub const fn now_playing_title_family(self) -> &'static str {
        self.now_playing_title.family()
    }

    pub const fn now_playing_supporting_family(self) -> &'static str {
        PACKAGED_SUPPORTING_FAMILY
    }

    pub const fn full_field_editorial_family(self) -> &'static str {
        self.full_field.families().0
    }

    pub const fn full_field_utility_family(self) -> &'static str {
        self.full_field.families().1
    }
}

pub fn select_typography(available_families: &HashSet<String>) -> TypographySelection {
    TypographySelection {
        now_playing_title: if available_families.contains(PREFERRED_NOW_PLAYING_TITLE_FAMILY) {
            NowPlayingTitleFace::Preferred
        } else {
            NowPlayingTitleFace::Fallback
        },
        full_field: select_full_field_typography(available_families),
    }
}

pub fn select_capture_typography(
    available_families: &HashSet<String>,
    requested: NowPlayingTitleFace,
) -> Result<TypographySelection, TypographyError> {
    if requested == NowPlayingTitleFace::Preferred
        && !available_families.contains(PREFERRED_NOW_PLAYING_TITLE_FAMILY)
    {
        return Err(TypographyError::PreferredTitleUnavailable);
    }

    Ok(TypographySelection {
        now_playing_title: requested,
        full_field: select_full_field_typography(available_families),
    })
}

fn select_full_field_typography(available_families: &HashSet<String>) -> FullFieldTypographyPair {
    if available_families.contains(PREFERRED_FULL_FIELD_EDITORIAL_FAMILY)
        && available_families.contains(PREFERRED_FULL_FIELD_UTILITY_FAMILY)
    {
        FullFieldTypographyPair::Preferred
    } else {
        FullFieldTypographyPair::Fallback
    }
}

pub fn register_packaged_fallback_fonts(renderer_root: &Path) -> Result<(), TypographyError> {
    let configuration = unsafe { FcConfigGetCurrent() };
    if configuration.is_null() {
        return Err(TypographyError::FontConfigurationUnavailable);
    }

    for relative_path in FALLBACK_FONT_FILES {
        let path = renderer_root.join(relative_path);
        let encoded_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| TypographyError::InvalidFontPath(path.clone()))?;
        let registered =
            unsafe { FcConfigAppFontAddFile(configuration, encoded_path.as_ptr().cast::<u8>()) };
        if registered == 0 {
            return Err(TypographyError::FontRegistrationFailed(path));
        }
    }

    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub enum TypographyError {
    FontConfigurationUnavailable,
    InvalidFontPath(PathBuf),
    FontRegistrationFailed(PathBuf),
    PreferredTitleUnavailable,
}

impl fmt::Display for TypographyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FontConfigurationUnavailable => {
                formatter.write_str("Fontconfig did not provide a current configuration")
            }
            Self::InvalidFontPath(path) => {
                write!(
                    formatter,
                    "fallback font path contains a null byte: {}",
                    path.display()
                )
            }
            Self::FontRegistrationFailed(path) => {
                write!(
                    formatter,
                    "could not register fallback font: {}",
                    path.display()
                )
            }
            Self::PreferredTitleUnavailable => formatter.write_str(
                "capture requested Sitka Display for Now Playing Title, but the host font is unavailable",
            ),
        }
    }
}

impl Error for TypographyError {}

#[repr(C)]
struct FcConfig {
    _private: [u8; 0],
}

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigGetCurrent() -> *mut FcConfig;
    fn FcConfigAppFontAddFile(configuration: *mut FcConfig, file: *const u8) -> i32;
}
