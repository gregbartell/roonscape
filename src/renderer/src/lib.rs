mod capture_control;
mod contract;
mod diagnostics;
mod display_configuration;
mod fixture_navigation;
mod gradient;
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

pub use capture_control::{
    CaptureControl, CaptureControlError, CaptureControlEvent, FixtureSelection,
    FixtureSelectionIdentity, PaintedFixtureSelection,
};
pub use contract::{
    ArtworkReference, Availability, LyricCue, NowPlaying, Playback, PresentationSnapshot, Progress,
    SnapshotError, SynchronizedLyrics, TrackedOutput, TrackedZone, parse_snapshot,
};
pub use diagnostics::{
    Diagnostics, DiagnosticsConfiguration, DiagnosticsConfigurationError,
    current_process_memory_bytes,
};
pub use display_configuration::{
    DisplayConfigurationError, InactivityConfiguration, display_configuration_file_path,
    inactivity_configuration_from_display_configuration, load_inactivity_configuration,
    reject_removed_display_configuration_override,
};
pub use fixture_navigation::FixtureNavigation;
pub use gradient::{NowPlayingGradient, NowPlayingGradientCacheKey};
pub use ipc::{
    ConnectionState, SnapshotEvent, SnapshotReader, SnapshotSocketError, SnapshotSubscription,
    read_snapshot_from_socket,
};
pub use keyboard::{NavigationIntent, RendererAction, RendererKey, RendererKeyboard};
pub use layout::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFieldAnchors,
    ArtworkFit, ArtworkLayout, ArtworkPrintPlateGeometry, ArtworkPrintPlateLayout, BottomAnchor,
    FullFieldFontSize, FullFieldLayout, FullFieldLineLayout, FullFieldSlot, IdentityLineLayout,
    IdentityPhraseAlignment, IdentityPlacement, IdentityRowLayout, InactivityLayout,
    LyricNeighborVisibility, MetadataFitting, MetadataFontSizes, NowPlayingField,
    NowPlayingFooterContent, NowPlayingInformationLayout, NowPlayingLayout, NowPlayingRole,
    NowPlayingTypography, PresentationStatusDecoration, PresentationStatusLayout, TextOverflow,
    Viewport,
};
pub use metadata::{
    MetadataDensity, MetadataGroupPlan, MetadataLayout, MetadataLineLayout, MetadataLinePlan,
    MetadataTypography, metadata_layout,
};
pub use palette::{PaletteError, PresentationPalette, Rgb};
pub use presentation::{
    FullFieldPresentation, INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityTransform,
    LayoutOffset, LyricPresentation, NowPlayingPresentation, Presentation, PresentationActivity,
    PresentationActivityMotion, PresentationActivityWaveform, PresentationBehavior,
    PresentationError, PresentationFrame, PresentationIdentity, PresentationProgress,
    PresentationState, PresentationStatus, PresentationStatusEmphasis, PresentationStatusMotion,
    PresentationStatusSymbol, PresentationTime, PresentationUpdate, classify_presentation_update,
    presentation_from_snapshot,
};
pub use resolution::{
    ArtworkResolutionError, ResolvedPresentation, resolve_capture_presentation,
    resolve_presentation,
};
pub use style::{
    DiagnosticsStyle, PresentationStyleLayer, PresentationTransitionStyles, TypographyStyles,
};
pub use transition::{PresentationRevision, PresentationTransition};
pub use typography::{
    FALLBACK_FONT_FILES, FALLBACK_FONT_LICENSES, NowPlayingTitleFace, TypographyError,
    TypographySelection, register_packaged_fallback_fonts, select_capture_typography,
    select_typography,
};
