// Share compilation, linking, and schema initialization across integration tests.
// Font registration retains its own process to exercise first-time initialization.
mod capture_control;
mod diagnostics;
mod display_configuration;
mod feature_integration;
mod fixture_navigation;
mod full_field_layout;
mod inactivity_layout;
mod keyboard;
mod metadata_layout;
mod now_playing_gradient;
mod now_playing_layout;
mod palette;
mod palette_styles;
mod presentation;
mod resolved_presentation;
mod snapshot_contract;
mod socket_snapshot;
mod transitions;
mod typography;

#[path = "../support/representative_viewports.rs"]
mod representative_viewports;
#[path = "../support/mod.rs"]
mod support;
