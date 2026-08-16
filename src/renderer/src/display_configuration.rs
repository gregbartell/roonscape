use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

const DISPLAY_CONFIGURATION_SCHEMA: &str =
    include_str!("../../shared/schema/display-configuration.schema.json");

const DEFAULT_GRACE_PERIOD: Duration = Duration::from_secs(300);
const DEFAULT_DIMMED_OPACITY: f64 = 0.35;
const DEFAULT_REPOSITION_CADENCE: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InactivityConfiguration {
    grace_period: Duration,
    dimmed_opacity: f64,
    reposition_cadence: Duration,
}

#[derive(Debug)]
pub enum DisplayConfigurationError {
    Io(io::Error),
    Json(serde_json::Error),
    Schema(String),
    Invalid(&'static str),
    MissingHome,
    RemovedEnvironmentOverride,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DisplayConfigurationFile {
    #[serde(rename = "trackedOutputId")]
    _tracked_output_id: String,
    inactivity: Option<InactivityConfigurationFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InactivityConfigurationFile {
    grace_period_seconds: u64,
    dimmed_opacity: f64,
    reposition_cadence_seconds: u64,
}

impl Default for InactivityConfiguration {
    fn default() -> Self {
        Self {
            grace_period: DEFAULT_GRACE_PERIOD,
            dimmed_opacity: DEFAULT_DIMMED_OPACITY,
            reposition_cadence: DEFAULT_REPOSITION_CADENCE,
        }
    }
}

impl InactivityConfiguration {
    pub fn new(
        grace_period: Duration,
        dimmed_opacity: f64,
        reposition_cadence: Duration,
    ) -> Result<Self, DisplayConfigurationError> {
        if grace_period.is_zero() {
            return Err(DisplayConfigurationError::Invalid(
                "inactivity grace period must be greater than zero",
            ));
        }
        if !dimmed_opacity.is_finite() || !(0.0 < dimmed_opacity && dimmed_opacity < 1.0) {
            return Err(DisplayConfigurationError::Invalid(
                "dimmed opacity must be greater than zero and less than one",
            ));
        }
        if reposition_cadence.is_zero() {
            return Err(DisplayConfigurationError::Invalid(
                "reposition cadence must be greater than zero",
            ));
        }

        Ok(Self {
            grace_period,
            dimmed_opacity,
            reposition_cadence,
        })
    }

    pub fn grace_period(self) -> Duration {
        self.grace_period
    }

    pub fn dimmed_opacity(self) -> f64 {
        self.dimmed_opacity
    }

    pub fn reposition_cadence(self) -> Duration {
        self.reposition_cadence
    }
}

impl fmt::Display for DisplayConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not read Display Configuration: {error}"),
            Self::Json(error) => write!(formatter, "Display Configuration is not valid: {error}"),
            Self::Schema(error) => write!(
                formatter,
                "Display Configuration violates the schema: {error}"
            ),
            Self::Invalid(message) => formatter.write_str(message),
            Self::MissingHome => {
                formatter.write_str("HOME must be set when XDG_CONFIG_HOME is absent")
            }
            Self::RemovedEnvironmentOverride => formatter.write_str(
                "ROONSCAPE_DISPLAY_CONFIG is no longer supported; use roonscape --config PATH",
            ),
        }
    }
}

impl Error for DisplayConfigurationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Schema(_)
            | Self::Invalid(_)
            | Self::MissingHome
            | Self::RemovedEnvironmentOverride => None,
        }
    }
}

pub fn inactivity_configuration_from_display_configuration(
    contents: &str,
) -> Result<InactivityConfiguration, DisplayConfigurationError> {
    let candidate: Value =
        serde_json::from_str(contents).map_err(DisplayConfigurationError::Json)?;
    let schema: Value = serde_json::from_str(DISPLAY_CONFIGURATION_SCHEMA)
        .map_err(DisplayConfigurationError::Json)?;
    let validator = jsonschema::options()
        .build(&schema)
        .map_err(|error| DisplayConfigurationError::Schema(error.to_string()))?;
    validator
        .validate(&candidate)
        .map_err(|error| DisplayConfigurationError::Schema(error.to_string()))?;
    let configuration: DisplayConfigurationFile =
        serde_json::from_value(candidate).map_err(DisplayConfigurationError::Json)?;
    let Some(inactivity) = configuration.inactivity else {
        return Ok(InactivityConfiguration::default());
    };

    InactivityConfiguration::new(
        Duration::from_secs(inactivity.grace_period_seconds),
        inactivity.dimmed_opacity,
        Duration::from_secs(inactivity.reposition_cadence_seconds),
    )
}

pub fn load_inactivity_configuration(
    configuration_file: &Path,
) -> Result<InactivityConfiguration, DisplayConfigurationError> {
    match fs::read_to_string(configuration_file) {
        Ok(contents) => inactivity_configuration_from_display_configuration(&contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(InactivityConfiguration::default())
        }
        Err(error) => Err(DisplayConfigurationError::Io(error)),
    }
}

pub fn display_configuration_file_path() -> Result<PathBuf, DisplayConfigurationError> {
    reject_removed_display_configuration_override()?;
    if let Some(config_root) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_root).join("roonscape/display.json"));
    }

    let home = env::var_os("HOME").ok_or(DisplayConfigurationError::MissingHome)?;
    Ok(PathBuf::from(home).join(".config/roonscape/display.json"))
}

pub fn reject_removed_display_configuration_override() -> Result<(), DisplayConfigurationError> {
    if env::var_os("ROONSCAPE_DISPLAY_CONFIG").is_some() {
        return Err(DisplayConfigurationError::RemovedEnvironmentOverride);
    }
    Ok(())
}
