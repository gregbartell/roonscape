mod contract;
mod display_configuration;
mod ipc;
mod palette;
mod presentation;

pub use contract::{
    ArtworkReference, Availability, DisplayZone, NowPlaying, Playback, PresentationSnapshot,
    Progress, SnapshotError, parse_snapshot,
};
pub use display_configuration::{
    DisplayConfigurationError, InactivityConfiguration, display_configuration_file_path,
    inactivity_configuration_from_display_configuration, load_inactivity_configuration,
};
pub use ipc::{SnapshotReader, SnapshotSocketError, read_snapshot_from_socket};
pub use palette::{PaletteError, PresentationPalette, Rgb};
pub use presentation::{
    INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityTransform, LayoutOffset,
    NowPlayingPresentation, Presentation, PresentationError, PresentationFrame,
    PresentationProgress, PresentationState, PresentationTime, UnavailablePresentation,
    presentation_from_snapshot,
};
