mod contract;
mod diagnostics;
mod display_configuration;
mod ipc;
mod keyboard;
mod layout;
mod metadata;
mod palette;
mod presentation;
mod resolution;
mod style;
mod transition;
mod typography;

pub use contract::{
    ArtworkReference, Availability, NowPlaying, Playback, PresentationSnapshot, Progress,
    SnapshotError, TrackedOutput, TrackedZone, parse_snapshot,
};
pub use diagnostics::{
    Diagnostics, DiagnosticsConfiguration, DiagnosticsConfigurationError,
    current_process_memory_bytes,
};
pub use display_configuration::{
    DisplayConfigurationError, InactivityConfiguration, display_configuration_file_path,
    inactivity_configuration_from_display_configuration, load_inactivity_configuration,
};
pub use ipc::{
    ConnectionState, SnapshotEvent, SnapshotReader, SnapshotSocketError, SnapshotSubscription,
    read_snapshot_from_socket,
};
pub use keyboard::{RendererKey, should_close_renderer};
pub use layout::{
    ArtworkAlignment, ArtworkContent, ArtworkFit, ArtworkLayout, FullFieldLayout, GalleryField,
    GallerySplitLayout, GallerySplitRole, GallerySplitTypography, IdentityLineLayout,
    IdentityPlacement, InactivityLayout, MetadataFontSizes, TextOverflow, Viewport,
};
pub use metadata::{
    MetadataLayout, MetadataLineLayout, MetadataTypography, metadata_layout,
    metadata_layout_for_viewport,
};
pub use palette::{PaletteError, PresentationPalette, Rgb};
pub use presentation::{
    FullFieldPresentation, INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityTransform,
    LayoutOffset, NowPlayingPresentation, Presentation, PresentationError, PresentationFrame,
    PresentationIdentity, PresentationProgress, PresentationState, PresentationTime,
    PresentationUpdate, StatusEmphasis, presentation_from_snapshot,
};
pub use resolution::{ResolvedPresentation, resolve_presentation};
pub use style::{
    DiagnosticsStyle, PresentationStyleLayer, PresentationTransitionStyles, TypographyStyles,
};
pub use transition::{PresentationRevision, PresentationTransition};
pub use typography::{
    FALLBACK_FONT_FILES, FALLBACK_FONT_LICENSES, TypographyError, TypographyPair,
    register_packaged_fallback_fonts, select_capture_typography, select_typography,
};
