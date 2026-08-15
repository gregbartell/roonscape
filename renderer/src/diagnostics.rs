use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::Duration;

use gdk_pixbuf::Pixbuf;

use crate::contract::PresentationSnapshot;
use crate::ipc::ConnectionState;

const DIAGNOSTICS_ENVIRONMENT_VARIABLE: &str = "ROONSCAPE_DIAGNOSTICS";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticsConfiguration {
    enabled: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DiagnosticsConfigurationError;

#[derive(Debug)]
pub struct Diagnostics {
    connection: ConnectionState,
    revision: Option<u64>,
    artwork_dimensions: Option<(i32, i32)>,
    previous_frame_at: Option<Duration>,
    frame_time: Option<Duration>,
}

impl DiagnosticsConfiguration {
    pub fn from_environment() -> Result<Self, DiagnosticsConfigurationError> {
        match env::var(DIAGNOSTICS_ENVIRONMENT_VARIABLE) {
            Ok(value) => Self::from_value(Some(&value)),
            Err(env::VarError::NotPresent) => Self::from_value(None),
            Err(env::VarError::NotUnicode(_)) => Err(DiagnosticsConfigurationError),
        }
    }

    pub fn from_value(value: Option<&str>) -> Result<Self, DiagnosticsConfigurationError> {
        let enabled = match value {
            None | Some("0" | "false") => false,
            Some("1" | "true") => true,
            Some(_) => return Err(DiagnosticsConfigurationError),
        };
        Ok(Self { enabled })
    }

    pub fn enabled(self) -> bool {
        self.enabled
    }
}

impl fmt::Display for DiagnosticsConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ROONSCAPE_DIAGNOSTICS must be 1, true, 0, or false")
    }
}

impl Error for DiagnosticsConfigurationError {}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            connection: ConnectionState::Disconnected,
            revision: None,
            artwork_dimensions: None,
            previous_frame_at: None,
            frame_time: None,
        }
    }
}

impl Diagnostics {
    pub fn observe_connection(&mut self, connection: ConnectionState) {
        self.connection = connection;
        if connection == ConnectionState::Disconnected {
            self.artwork_dimensions = None;
        }
    }

    pub fn observe_snapshot(&mut self, snapshot: &PresentationSnapshot, repository_root: &Path) {
        self.revision = Some(snapshot.revision);
        self.artwork_dimensions = snapshot.artwork.as_ref().and_then(|artwork| {
            let path = repository_root.join(&artwork.path);
            Pixbuf::file_info(path).map(|(_, width, height)| (width, height))
        });
    }

    pub fn observe_frame(&mut self, frame_at: Duration) {
        if let Some(previous_frame_at) = self.previous_frame_at {
            self.frame_time = frame_at.checked_sub(previous_frame_at);
        }
        self.previous_frame_at = Some(frame_at);
    }

    pub fn overlay_text(&self, memory_bytes: Option<u64>) -> String {
        let memory = memory_bytes.map_or_else(
            || "—".to_owned(),
            |bytes| format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0)),
        );
        let frame = self.frame_time.map_or_else(
            || "—".to_owned(),
            |duration| format!("{:.1} ms", duration.as_secs_f64() * 1_000.0),
        );
        let artwork = self.artwork_dimensions.map_or_else(
            || "—".to_owned(),
            |(width, height)| format!("{width} × {height}"),
        );
        let connection = match self.connection {
            ConnectionState::Connected => "connected",
            ConnectionState::Disconnected => "disconnected",
        };
        let revision = self
            .revision
            .map_or_else(|| "—".to_owned(), |revision| revision.to_string());

        format!(
            "Memory  {memory}\nFrame   {frame}\nArtwork {artwork}\nConnection {connection}\nRevision {revision}"
        )
    }
}

pub fn current_process_memory_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let resident_kibibytes = status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })?;
    resident_kibibytes.checked_mul(1024)
}
