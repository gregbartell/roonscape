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

const PREFERRED_EDITORIAL_FAMILY: &str = "Palatino Linotype";
const PREFERRED_UTILITY_FAMILY: &str = "Segoe UI";
const FALLBACK_EDITORIAL_FAMILY: &str = "Libre Baskerville";
const FALLBACK_UTILITY_FAMILY: &str = "IBM Plex Sans";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypographyPair {
    Preferred,
    Fallback,
}

impl TypographyPair {
    pub const fn editorial_family(self) -> &'static str {
        self.families().0
    }

    pub const fn utility_family(self) -> &'static str {
        self.families().1
    }

    const fn families(self) -> (&'static str, &'static str) {
        match self {
            Self::Preferred => (PREFERRED_EDITORIAL_FAMILY, PREFERRED_UTILITY_FAMILY),
            Self::Fallback => (FALLBACK_EDITORIAL_FAMILY, FALLBACK_UTILITY_FAMILY),
        }
    }
}

pub fn select_typography(available_families: &HashSet<String>) -> TypographyPair {
    if available_families.contains(PREFERRED_EDITORIAL_FAMILY)
        && available_families.contains(PREFERRED_UTILITY_FAMILY)
    {
        TypographyPair::Preferred
    } else {
        TypographyPair::Fallback
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
