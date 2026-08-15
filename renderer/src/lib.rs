mod contract;
mod ipc;
mod presentation;

pub use contract::{
    ArtworkReference, Availability, DisplayZone, NowPlaying, Playback, PresentationSnapshot,
    Progress, SnapshotError, parse_snapshot,
};
pub use ipc::{SnapshotReader, SnapshotSocketError, read_snapshot_from_socket};
pub use presentation::{
    NowPlayingPresentation, Presentation, PresentationError, PresentationProgress,
    UnavailablePresentation, presentation_from_snapshot,
};
