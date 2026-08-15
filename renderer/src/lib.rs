mod contract;
mod ipc;
mod metadata;
mod palette;
mod presentation;
mod transition;

pub use contract::{
    ArtworkReference, Availability, DisplayZone, NowPlaying, Playback, PresentationSnapshot,
    Progress, SnapshotError, parse_snapshot,
};
pub use ipc::{SnapshotReader, SnapshotSocketError, read_snapshot_from_socket};
pub use metadata::{
    MetadataLayout, MetadataLineLayout, MetadataOverflow, MetadataTypography, metadata_layout,
};
pub use palette::{PaletteError, PresentationPalette, Rgb};
pub use presentation::{
    NowPlayingPresentation, Presentation, PresentationError, PresentationProgress,
    PresentationState, PresentationTime, PresentationUpdate, UnavailablePresentation,
    presentation_from_snapshot,
};
pub use transition::{PresentationRevision, PresentationTransition};
