mod contract;
mod ipc;
mod presentation;

pub use contract::{
    ArtworkReference, Availability, DisplayZone, NowPlaying, Playback, PresentationSnapshot,
    Progress, SnapshotError, parse_snapshot,
};
pub use ipc::{SnapshotSocketError, read_snapshot_from_socket};
pub use presentation::{
    Presentation, PresentationError, PresentationProgress, presentation_from_snapshot,
};
