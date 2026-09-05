use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFit,
    ArtworkLayout, FullFieldFontSize, FullFieldLayout, FullFieldLineLayout, FullFieldPresentation,
    IdentityLineLayout, IdentityPhraseAlignment, IdentityPlacement, IdentityRowLayout,
    InactivityLayout, InactivityTransform, LyricPresentation, MetadataGroupPlan, MetadataLayout,
    MetadataLineLayout, MetadataTypography, NowPlayingField, NowPlayingFooterContent,
    NowPlayingLayout, NowPlayingPresentation, NowPlayingRole, Presentation, PresentationActivity,
    PresentationBehavior, PresentationPalette, PresentationProgress, PresentationRevision,
    PresentationStatus, PresentationStatusEmphasis, PresentationStatusLayout,
    PresentationStyleLayer, PresentationTransition, PresentationTransitionStyles,
    ResolvedPresentation, TextOverflow, TypographySelection, TypographyStyles, Viewport,
    metadata_layout, resolve_capture_presentation, resolve_presentation,
};

use crate::activity_waveform::activity_waveform;
use crate::artwork_cache::{ArtworkCache, ArtworkCacheKey};
use crate::gradient_cache::{
    CachedNowPlayingGradient, NowPlayingGradientCache, PreparedNowPlayingGradient,
    RenderedNowPlayingGradient,
};
use crate::lyric_motion::{LyricColorRole, LyricCueFrame, LyricCueSlot, LyricFrame, LyricMotion};
use crate::status_symbol::presentation_status_symbol;

const STYLES: &str = include_str!("style.css");
const PRESENTATION_CACHE_CAPACITY: usize = 2;

pub(crate) struct PresentationView {
    root: gtk::Overlay,
    stack: gtk::Stack,
    transition: PresentationTransition<RenderedPresentation>,
    palette_provider: gtk::CssProvider,
    rendering: RenderingConfiguration,
    display_viewport: Viewport,
    layout_viewport: Option<Viewport>,
    inactivity: InactivityTransform,
    transition_clock: Instant,
    caches: PresentationCaches,
}

#[derive(Clone, Copy)]
pub(crate) struct RenderingConfiguration {
    typography: TypographySelection,
    behavior: PresentationBehavior,
    artwork_failure: ArtworkFailure,
    cache_scope: CacheScope,
}

#[derive(Clone, Copy)]
enum ArtworkFailure {
    UseFallback,
    FailCapture,
}

#[derive(Clone, Copy)]
enum CacheScope {
    RendererSession,
    FixtureScenario,
}

impl RenderingConfiguration {
    pub(crate) fn live(typography: TypographySelection, behavior: PresentationBehavior) -> Self {
        Self {
            typography,
            behavior,
            artwork_failure: ArtworkFailure::UseFallback,
            cache_scope: CacheScope::RendererSession,
        }
    }

    pub(crate) fn fixture(typography: TypographySelection, behavior: PresentationBehavior) -> Self {
        Self {
            typography,
            behavior,
            artwork_failure: ArtworkFailure::UseFallback,
            cache_scope: CacheScope::FixtureScenario,
        }
    }

    pub(crate) fn capture(typography: TypographySelection, behavior: PresentationBehavior) -> Self {
        Self {
            typography,
            behavior,
            artwork_failure: ArtworkFailure::FailCapture,
            cache_scope: CacheScope::FixtureScenario,
        }
    }
}

struct RenderedPresentation {
    root: gtk::Widget,
    progress: Option<RenderedProgress>,
    palette: PresentationPalette,
    layout_source: PresentationLayoutSource,
    now_playing: Option<RenderedNowPlaying>,
    full_field: Option<RenderedFullField>,
    diagnostics: Option<gtk::Label>,
    capture_error: Option<String>,
}

enum PresentationLayoutSource {
    NowPlaying(Box<NowPlayingPresentation>),
    FullField,
}

impl PresentationLayoutSource {
    fn for_presentation(presentation: &Presentation) -> Self {
        match presentation {
            Presentation::NowPlaying(presentation) => {
                Self::NowPlaying(Box::new(presentation.clone()))
            }
            Presentation::FullField(_) => Self::FullField,
        }
    }

    fn now_playing(
        &self,
        viewport: Viewport,
        composition_progress: f64,
    ) -> Option<NowPlayingLayout> {
        let Self::NowPlaying(presentation) = self else {
            return None;
        };
        Some(NowPlayingLayout::for_composition_progress(
            presentation,
            viewport,
            composition_progress,
        ))
    }
}

#[derive(Clone)]
struct RenderedProgress {
    root: gtk::Box,
    rail: gtk::Overlay,
    track: gtk::Box,
    fill: gtk::ProgressBar,
    times: gtk::Box,
    elapsed: gtk::Label,
    remaining: gtk::Label,
}

impl RenderedProgress {
    fn update(&self, progress: &PresentationProgress) {
        self.fill.set_fraction(progress.fraction);
        self.elapsed.set_text(&progress.elapsed);
        self.remaining.set_text(&progress.remaining);
    }
}

struct RenderedMetadata {
    root: gtk::Overlay,
    copy: gtk::Box,
    musical_metadata_alignment: gtk::CenterBox,
    ordinary_metadata_stage: gtk::Fixed,
    ordinary_metadata: gtk::Box,
    presentation_status: RenderedPresentationStatus,
    musical_metadata_slot: gtk::ScrolledWindow,
    title: Option<RenderedMetadataLine>,
    artist: Option<RenderedMetadataLine>,
    album: Option<RenderedMetadataLine>,
    lyrics: RenderedLyrics,
    progress: Option<RenderedProgress>,
    activity: Option<RenderedActivity>,
    footer: gtk::Box,
    identity: RenderedIdentity,
}

struct RenderedLyrics {
    root: gtk::Box,
    masthead: gtk::Box,
    masthead_title: Option<gtk::Label>,
    masthead_artist: Option<gtk::Label>,
    reel_region: gtk::ScrolledWindow,
    reel: gtk::Fixed,
    previous: gtk::Label,
    current: gtk::Label,
    next: gtk::Label,
    scale_percentages: Cell<[u8; 3]>,
    line_width_px: Cell<i32>,
    typography: Cell<roonscape_renderer::NowPlayingTypography>,
    palette: PresentationPalette,
    motion: RefCell<LyricMotion>,
    rendered_composition_progress: Cell<f64>,
    behavior: PresentationBehavior,
}

struct RenderedActivity {
    root: gtk::Box,
    waveform: gtk::DrawingArea,
    heading: gtk::Label,
    detail: gtk::Label,
}

struct RenderedNowPlaying {
    background: RenderedNowPlayingBackground,
    content: gtk::Box,
    artwork_column: gtk::CenterBox,
    artwork: RenderedArtwork,
    metadata_slot: gtk::Box,
    metadata: RenderedMetadata,
}

struct RenderedNowPlayingBackground {
    picture: gtk::Picture,
    gradient: Rc<CachedNowPlayingGradient>,
}

#[derive(Clone)]
struct PresentationCaches {
    gradients: Rc<NowPlayingGradientCache>,
    artwork: Rc<ArtworkCache>,
}

impl PresentationCaches {
    fn new(capacity: usize) -> Self {
        Self {
            gradients: Rc::new(NowPlayingGradientCache::new(capacity)),
            artwork: Rc::new(ArtworkCache::new(capacity)),
        }
    }
}

impl CacheScope {
    fn render_replacement<T>(
        self,
        current: &mut PresentationCaches,
        render: impl FnOnce(&PresentationCaches) -> T,
    ) -> T {
        if let Self::FixtureScenario = self {
            *current = PresentationCaches::new(PRESENTATION_CACHE_CAPACITY);
        }
        render(current)
    }
}

struct RenderedArtwork {
    reservation: gtk::AspectFrame,
    print_plate: gtk::Box,
    decoration: gtk::AspectFrame,
    surface: gtk::Picture,
    source_key: Option<ArtworkCacheKey>,
    artwork_cache: Rc<ArtworkCache>,
    layout: ArtworkLayout,
    readiness: ArtworkReadiness,
}

struct ArtworkReadiness {
    scaled: Cell<bool>,
    decode_error: Option<String>,
}

struct RenderedFullField {
    copy: gtk::Box,
    message: gtk::Box,
    presentation_status: RenderedPresentationStatus,
    heading_slot: gtk::Box,
    heading: gtk::Label,
    explanation_slot: Option<gtk::Box>,
    explanation: Option<gtk::Label>,
    identity: Option<RenderedIdentity>,
    fit_readiness: FullFieldFitReadiness,
}

#[derive(Clone)]
struct FullFieldFitReadiness {
    state: Rc<Cell<FullFieldFitState>>,
}

#[derive(Clone, Copy)]
struct FullFieldFitState {
    generation: u64,
    pending: u32,
}

#[derive(Clone)]
struct FullFieldFitGeneration {
    generation: u64,
    readiness: FullFieldFitReadiness,
}

struct RenderedPresentationStatus {
    root: gtk::Box,
    symbol: gtk::Box,
    label: gtk::Label,
    decoration: roonscape_renderer::PresentationStatusDecoration,
    status: PresentationStatus,
    behavior: PresentationBehavior,
}

struct RenderedMetadataLine {
    label: gtk::Label,
    layout: MetadataLineLayout,
    font_family: &'static str,
}

struct RenderedIdentity {
    root: gtk::Grid,
    output: gtk::Box,
    output_label: gtk::Label,
    output_name: gtk::Label,
    zone: Option<RenderedZoneIdentity>,
}

struct RenderedZoneIdentity {
    root: gtk::Box,
    label: gtk::Label,
    name: gtk::Label,
    separator: gtk::Box,
}

impl PresentationView {
    pub(crate) fn new(
        revision: u64,
        presentation: &Presentation,
        initial_viewport: Viewport,
        repository_root: &Path,
        palette_provider: gtk::CssProvider,
        diagnostics_text: Option<&str>,
        rendering: RenderingConfiguration,
    ) -> Self {
        let caches = PresentationCaches::new(PRESENTATION_CACHE_CAPACITY);
        let rendered = render_presentation(
            presentation,
            repository_root,
            diagnostics_text,
            caches.clone(),
            rendering,
        );
        rendered
            .root
            .add_css_class(PresentationStyleLayer::Current.class_name());
        let transition = PresentationTransition::new(revision, rendered);
        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(transition.duration().as_millis() as u32);
        stack.add_child(&transition.current().value().root);
        let root = gtk::Overlay::new();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_child(Some(&stack));

        let mut view = Self {
            root,
            stack,
            transition,
            palette_provider,
            rendering,
            display_viewport: initial_viewport,
            layout_viewport: None,
            inactivity: InactivityTransform::default(),
            transition_clock: Instant::now(),
            caches,
        };
        view.apply_layout();
        view
    }

    pub(crate) fn root(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub(crate) fn apply_inactivity(&mut self, transform: InactivityTransform) {
        if self.inactivity == transform {
            return;
        }
        self.inactivity = transform;
        self.apply_layout();
    }

    fn apply_layout(&mut self) {
        let layout = InactivityLayout::for_viewport(self.display_viewport, self.inactivity);
        self.root.set_opacity(self.inactivity.opacity);
        self.root
            .set_margin_start(dimension(layout.margin_start_px));
        self.root.set_margin_end(dimension(layout.margin_end_px));
        self.root.set_margin_top(dimension(layout.margin_top_px));
        self.root
            .set_margin_bottom(dimension(layout.margin_bottom_px));

        if self.layout_viewport == Some(layout.content_viewport) {
            return;
        }

        self.transition
            .current()
            .value()
            .apply_viewport(layout.content_viewport);
        if let Some(outgoing) = self.transition.outgoing() {
            outgoing.value().apply_viewport(layout.content_viewport);
        }
        self.layout_viewport = Some(layout.content_viewport);
        self.install_palette_styles();
    }

    pub(crate) fn apply_viewport(&mut self, viewport: Viewport) {
        if self.display_viewport == viewport {
            return;
        }
        self.display_viewport = viewport;
        self.apply_layout();
    }

    pub(crate) fn replace(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
    ) {
        if self.rendering.behavior == PresentationBehavior::StaticFixture {
            let rendered = self.render_replacement_at_viewport(presentation, repository_root);
            let released = self.transition.replace_immediately(revision, rendered);
            for layer in released {
                self.remove_layer(layer);
            }
            self.reveal_current();
            return;
        }
        if let Some(discarded) = self.transition.discard_outgoing() {
            self.remove_layer(discarded);
        }
        let rendered = self.render_replacement_at_viewport(presentation, repository_root);
        let started_at = self.transition_clock.elapsed();
        let discarded = self.transition.begin(revision, rendered, started_at);
        debug_assert!(discarded.is_none());

        let outgoing = self
            .transition
            .outgoing()
            .expect("a started presentation transition has an outgoing layer");
        outgoing
            .value()
            .root
            .remove_css_class(PresentationStyleLayer::Current.class_name());
        outgoing
            .value()
            .root
            .add_css_class(PresentationStyleLayer::Outgoing.class_name());
        self.reveal_current();
    }

    pub(crate) fn finish_transition(&mut self) {
        let now = self.transition_clock.elapsed();
        let Some(outgoing) = self.transition.finish(now) else {
            return;
        };

        self.stack.remove(&outgoing.value().root);
        self.install_palette_styles();
    }

    pub(crate) fn update_in_place(&mut self, revision: u64, presentation: &Presentation) {
        let now = self.transition_clock.elapsed();
        let viewport = self.layout_viewport;
        self.transition.update_current(revision, |current| {
            current.update_in_place(revision, presentation, now, viewport);
        });
    }

    pub(crate) fn update_diagnostics(&self, text: &str) {
        self.transition.current().value().update_diagnostics(text);
        if let Some(outgoing) = self.transition.outgoing() {
            outgoing.value().update_diagnostics(text);
        }
    }

    pub(crate) fn capture_ready(&self, revision: u64, viewport: Viewport) -> Result<bool, String> {
        if self.rendering.behavior != PresentationBehavior::StaticFixture
            || self.inactivity != InactivityTransform::default()
            || self.display_viewport != viewport
            || self.layout_viewport != Some(viewport)
            || self.transition.current().revision() != revision
            || self.transition.is_active()
        {
            return Ok(false);
        }

        self.transition.current().value().capture_ready()
    }

    pub(crate) fn layout_ready(&self) -> bool {
        self.transition.current().value().layout_ready()
    }

    fn remove_layer(&self, layer: PresentationRevision<RenderedPresentation>) {
        self.stack.remove(&layer.value().root);
    }

    fn render_replacement_at_viewport(
        &mut self,
        presentation: &Presentation,
        repository_root: &Path,
    ) -> RenderedPresentation {
        let diagnostics_text = self.transition.current().value().diagnostics_text();
        let (resolved, capture_error) =
            resolve_for_rendering(presentation, repository_root, self.rendering);
        let rendering = self.rendering;
        let layout_viewport = self.layout_viewport;
        let root = &self.root;
        rendering
            .cache_scope
            .render_replacement(&mut self.caches, |caches| {
                let render = || {
                    let mut rendered = render_current_from_resolved(
                        &resolved,
                        repository_root,
                        diagnostics_text.as_deref(),
                        caches.clone(),
                        rendering,
                    );
                    rendered.capture_error.clone_from(&capture_error);
                    rendered
                };
                match (&resolved.presentation, layout_viewport) {
                    (Presentation::NowPlaying(_), Some(viewport)) => {
                        let scale_factor =
                            u32::try_from(gtk::prelude::WidgetExt::scale_factor(root))
                                .expect("GTK display scale factor must be positive");
                        // A fresh gradient is independent of foreground construction
                        // and layout. Install the prepared raster only after foreground
                        // construction so the new background is never partially ready.
                        let (rendered, prepared_gradient) = caches.gradients.prepare_while(
                            resolved.palette,
                            viewport,
                            scale_factor,
                            || {
                                let rendered = render();
                                rendered.apply_viewport_foreground(viewport);
                                rendered
                            },
                        );
                        rendered.apply_prepared_now_playing_background(prepared_gradient);
                        rendered
                    }
                    (_, Some(viewport)) => {
                        let rendered = render();
                        rendered.apply_viewport(viewport);
                        rendered
                    }
                    (_, None) => render(),
                }
            })
    }

    fn reveal_current(&self) {
        let current = self.transition.current();
        self.stack.add_child(&current.value().root);
        self.install_palette_styles();
        self.stack.set_visible_child(&current.value().root);
    }

    fn install_palette_styles(&self) {
        let viewport = self.layout_viewport.unwrap_or(Viewport::WINDOWED_FIXTURE);
        let layout = NowPlayingLayout::for_viewport(viewport);
        let full_field_layout = FullFieldLayout::for_viewport(viewport);
        let styles = PresentationTransitionStyles::new(
            self.transition.current().value().palette,
            self.transition
                .outgoing()
                .map(|outgoing| outgoing.value().palette),
        );
        self.palette_provider
            .load_from_data(&styles.to_css(&layout, &full_field_layout));
    }
}

impl FullFieldFitReadiness {
    fn new() -> Self {
        Self {
            state: Rc::new(Cell::new(FullFieldFitState {
                generation: 0,
                pending: 0,
            })),
        }
    }

    fn begin_generation(&self) -> FullFieldFitGeneration {
        let current = self.state.get();
        let generation = current
            .generation
            .checked_add(1)
            .expect("Full-field fit generation must remain representable");
        self.state.set(FullFieldFitState {
            generation,
            ..current
        });
        FullFieldFitGeneration {
            generation,
            readiness: self.clone(),
        }
    }

    fn is_ready(&self) -> bool {
        self.state.get().pending == 0
    }
}

impl FullFieldFitGeneration {
    fn register_fit(&self) {
        let current = self.readiness.state.get();
        self.readiness.state.set(FullFieldFitState {
            pending: current
                .pending
                .checked_add(1)
                .expect("pending Full-field fits must remain representable"),
            ..current
        });
    }

    fn is_current(&self) -> bool {
        self.readiness.state.get().generation == self.generation
    }

    fn complete_fit(&self) {
        let current = self.readiness.state.get();
        self.readiness.state.set(FullFieldFitState {
            pending: current
                .pending
                .checked_sub(1)
                .expect("a completed Full-field fit must be registered"),
            ..current
        });
    }
}

impl RenderedPresentation {
    fn layout_ready(&self) -> bool {
        self.full_field
            .as_ref()
            .is_none_or(|full_field| full_field.fit_readiness.is_ready())
    }

    fn capture_ready(&self) -> Result<bool, String> {
        if let Some(error) = self.capture_error.as_ref() {
            return Err(error.clone());
        }
        if let Some(now_playing) = self.now_playing.as_ref() {
            return now_playing.artwork.capture_ready();
        }
        Ok(self.layout_ready())
    }

    fn update_in_place(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        now: Duration,
        viewport: Option<Viewport>,
    ) {
        match (
            self.now_playing.as_mut(),
            self.full_field.as_mut(),
            presentation,
        ) {
            (Some(rendered), None, Presentation::NowPlaying(presentation)) => {
                rendered
                    .metadata
                    .presentation_status
                    .update(&presentation.status);
                if let (Some(rendered), Some(progress)) =
                    (self.progress.as_ref(), presentation.progress.as_ref())
                {
                    rendered.update(progress);
                }
                rendered
                    .metadata
                    .update_lyrics(revision, presentation, now, viewport);
                self.layout_source =
                    PresentationLayoutSource::NowPlaying(Box::new(presentation.clone()));
                if let Some(viewport) = viewport {
                    let progress = rendered.metadata.lyric_composition_progress(now);
                    let layout = NowPlayingLayout::for_composition_progress(
                        presentation,
                        viewport,
                        progress,
                    );
                    rendered.apply_foreground_layout(&layout);
                    rendered.metadata.apply_lyric_frame(now, &layout);
                }
            }
            (None, Some(rendered), Presentation::FullField(presentation)) => {
                rendered.presentation_status.update(&presentation.status);
            }
            _ => debug_assert!(
                false,
                "in-place updates must preserve presentation composition"
            ),
        }
    }

    fn apply_viewport(&self, viewport: Viewport) {
        self.apply_viewport_foreground(viewport);
        self.apply_now_playing_background(viewport);
    }

    fn apply_viewport_foreground(&self, viewport: Viewport) {
        let composition_progress = self.now_playing.as_ref().map_or(0.0, |now_playing| {
            now_playing.metadata.rendered_composition_progress()
        });
        if let (Some(now_playing), Some(layout)) = (
            self.now_playing.as_ref(),
            self.layout_source
                .now_playing(viewport, composition_progress),
        ) {
            now_playing.apply_foreground_layout(&layout);
        }
        if let Some(full_field) = self.full_field.as_ref() {
            full_field.apply_layout(&FullFieldLayout::for_viewport(viewport));
        }
    }

    fn apply_now_playing_background(&self, viewport: Viewport) {
        if let Some(now_playing) = self.now_playing.as_ref() {
            now_playing.background.apply_viewport(viewport);
        }
    }

    fn apply_prepared_now_playing_background(&self, gradient: PreparedNowPlayingGradient) {
        if let Some(now_playing) = self.now_playing.as_ref() {
            now_playing.background.apply_prepared(gradient);
        }
    }

    fn update_diagnostics(&self, text: &str) {
        if let Some(diagnostics) = self.diagnostics.as_ref() {
            diagnostics.set_text(text);
        }
    }

    fn diagnostics_text(&self) -> Option<String> {
        self.diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.text().to_string())
    }
}

fn render_current_from_resolved(
    resolved: &ResolvedPresentation,
    repository_root: &Path,
    diagnostics_text: Option<&str>,
    caches: PresentationCaches,
    rendering: RenderingConfiguration,
) -> RenderedPresentation {
    let rendered = render_resolved_presentation(
        resolved,
        repository_root,
        diagnostics_text,
        caches,
        rendering,
    );
    rendered
        .root
        .add_css_class(PresentationStyleLayer::Current.class_name());
    rendered
}

fn render_presentation(
    presentation: &Presentation,
    repository_root: &Path,
    diagnostics_text: Option<&str>,
    caches: PresentationCaches,
    rendering: RenderingConfiguration,
) -> RenderedPresentation {
    let (resolved, capture_error) = resolve_for_rendering(presentation, repository_root, rendering);
    let mut rendered = render_resolved_presentation(
        &resolved,
        repository_root,
        diagnostics_text,
        caches,
        rendering,
    );
    rendered.capture_error = capture_error;
    rendered
}

fn resolve_for_rendering(
    presentation: &Presentation,
    repository_root: &Path,
    rendering: RenderingConfiguration,
) -> (ResolvedPresentation, Option<String>) {
    match rendering.artwork_failure {
        ArtworkFailure::UseFallback => (resolve_presentation(presentation, repository_root), None),
        ArtworkFailure::FailCapture => {
            match resolve_capture_presentation(presentation, repository_root) {
                Ok(resolved) => (resolved, None),
                Err(error) => (
                    resolve_presentation(presentation, repository_root),
                    Some(error.to_string()),
                ),
            }
        }
    }
}

fn render_resolved_presentation(
    resolved: &ResolvedPresentation,
    repository_root: &Path,
    diagnostics_text: Option<&str>,
    caches: PresentationCaches,
    rendering: RenderingConfiguration,
) -> RenderedPresentation {
    let layout_source = PresentationLayoutSource::for_presentation(&resolved.presentation);
    match &resolved.presentation {
        Presentation::NowPlaying(presentation) => now_playing(
            presentation,
            repository_root,
            resolved.palette,
            layout_source,
            rendering,
            diagnostics_text,
            caches,
        ),
        Presentation::FullField(presentation) => full_field(
            presentation,
            resolved.palette,
            layout_source,
            diagnostics_text,
            rendering,
        ),
    }
}

fn full_field(
    presentation: &FullFieldPresentation,
    palette: PresentationPalette,
    layout_source: PresentationLayoutSource,
    diagnostics_text: Option<&str>,
    rendering: RenderingConfiguration,
) -> RenderedPresentation {
    let fit_readiness = FullFieldFitReadiness::new();
    let layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let content = gtk::Overlay::new();
    content.set_hexpand(true);
    content.set_vexpand(true);

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.add_css_class("full-copy");
    copy.set_halign(gtk::Align::Center);
    copy.set_valign(gtk::Align::Start);

    let message = gtk::Box::new(gtk::Orientation::Vertical, 0);
    message.set_hexpand(true);
    let rendered_status = presentation_status(
        &presentation.status,
        layout.presentation_status.decoration,
        rendering.behavior,
    );
    message.append(&rendered_status.root);

    let (heading_slot, heading) = full_field_line(presentation.heading, "full-field-heading");
    heading.add_css_class("editorial-text");
    message.append(&heading_slot);

    let (explanation_slot, explanation) = match presentation.explanation {
        Some(text) => {
            let (slot, explanation) = full_field_line(text, "full-field-explanation");
            explanation.add_css_class("utility-text");
            message.append(&slot);
            (Some(slot), Some(explanation))
        }
        None => (None, None),
    };
    copy.append(&message);
    content.set_child(Some(&copy));

    let identity = if let Some(presentation_identity) = presentation.identity.as_ref() {
        let identity = match presentation_identity {
            roonscape_renderer::PresentationIdentity::OutputAndZone {
                tracked_output,
                tracked_zone,
            } => tracked_identity(
                tracked_output,
                Some(tracked_zone),
                layout.identity_placement,
                layout.identity_line,
            ),
            roonscape_renderer::PresentationIdentity::OutputOnly { tracked_output } => {
                tracked_identity(
                    tracked_output,
                    None,
                    layout.identity_placement,
                    layout.identity_line,
                )
            }
        };
        content.add_overlay(&identity.root);
        Some(identity)
    } else {
        None
    };

    let (root, diagnostics) = presentation_layer(&content, "full-field", diagnostics_text);
    RenderedPresentation {
        root: root.upcast(),
        progress: None,
        palette,
        layout_source,
        now_playing: None,
        full_field: Some(RenderedFullField {
            copy,
            message,
            presentation_status: rendered_status,
            heading_slot,
            heading,
            explanation_slot,
            explanation,
            identity,
            fit_readiness,
        }),
        diagnostics,
        capture_error: None,
    }
}

fn diagnostics_view(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("diagnostics");
    label.set_halign(gtk::Align::End);
    label.set_valign(gtk::Align::Start);
    label.set_xalign(0.0);
    label.set_selectable(false);
    label
}

fn now_playing(
    presentation: &NowPlayingPresentation,
    repository_root: &Path,
    palette: PresentationPalette,
    layout_source: PresentationLayoutSource,
    rendering: RenderingConfiguration,
    diagnostics_text: Option<&str>,
    caches: PresentationCaches,
) -> RenderedPresentation {
    let layout = NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE);
    let surface = gtk::Overlay::new();
    surface.set_hexpand(true);
    surface.set_vexpand(true);

    let background = RenderedNowPlayingBackground::new(palette, Rc::clone(&caches.gradients));
    surface.set_child(Some(&background.picture));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.add_css_class("now-playing-content");
    content.set_hexpand(true);
    content.set_vexpand(true);
    surface.add_overlay(&content);

    let artwork_column = gtk::CenterBox::new();
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_orientation(gtk::Orientation::Vertical);
    artwork_column.set_hexpand(false);
    artwork_column.set_vexpand(true);
    artwork_column.set_overflow(gtk::Overflow::Visible);
    let artwork = artwork(presentation, repository_root, Rc::clone(&caches.artwork));
    artwork_column.set_center_widget(Some(&artwork.reservation));

    let metadata = metadata(presentation, &layout, palette, rendering);
    let metadata_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    metadata_slot.add_css_class("metadata-slot");
    metadata_slot.set_hexpand(false);
    metadata_slot.set_vexpand(true);
    metadata_slot.append(&metadata.root);

    content.append(&artwork_column);
    content.append(&metadata_slot);
    let progress = metadata.progress.clone();
    let now_playing = RenderedNowPlaying {
        background,
        content,
        artwork_column,
        artwork,
        metadata_slot,
        metadata,
    };
    let class_name = match layout.field {
        NowPlayingField::Cohesive => "now-playing",
    };
    let (root, diagnostics) = presentation_layer(&surface, class_name, diagnostics_text);
    RenderedPresentation {
        root: root.upcast(),
        progress,
        palette,
        layout_source,
        now_playing: Some(now_playing),
        full_field: None,
        diagnostics,
        capture_error: None,
    }
}

fn presentation_layer(
    content: &impl IsA<gtk::Widget>,
    class_name: &str,
    diagnostics_text: Option<&str>,
) -> (gtk::Overlay, Option<gtk::Label>) {
    let root = gtk::Overlay::new();
    root.add_css_class(class_name);
    root.set_hexpand(true);
    root.set_vexpand(true);
    root.set_child(Some(content));
    let diagnostics = diagnostics_text.map(|text| {
        let diagnostics = diagnostics_view(text);
        root.add_overlay(&diagnostics);
        diagnostics
    });
    (root, diagnostics)
}

fn artwork(
    presentation: &NowPlayingPresentation,
    repository_root: &Path,
    artwork_cache: Rc<ArtworkCache>,
) -> RenderedArtwork {
    let source_key = presentation.artwork_path.as_deref().map(|path| {
        ArtworkCacheKey::new(repository_root.join(path), presentation.artwork_revision)
    });
    let source = source_key.as_ref().and_then(|key| {
        artwork_cache
            .source(key)
            .map(|source| (key.clone(), source))
    });
    let decode_error = source_key
        .as_ref()
        .filter(|_| source.is_none())
        .map(|key| format!("could not decode artwork at {}", key.path().display()));
    let intrinsic_dimensions = source.as_ref().map(|(_, artwork)| {
        ArtworkDimensions::new(
            artwork
                .width()
                .try_into()
                .expect("decoded artwork width should be positive"),
            artwork
                .height()
                .try_into()
                .expect("decoded artwork height should be positive"),
        )
    });
    let layout = ArtworkLayout::for_presentation(presentation, intrinsic_dimensions);
    let picture = match layout.content {
        ArtworkContent::Supplied => {
            let picture = gtk::Picture::new();
            picture.set_alternative_text(Some("Current album artwork"));
            picture
        }
        ArtworkContent::QuietField => {
            let picture = gtk::Picture::new();
            picture.add_css_class("artwork-missing");
            picture
        }
    };
    picture.add_css_class("artwork");
    picture.set_can_shrink(true);
    match layout.fit {
        ArtworkFit::Contain => picture.set_keep_aspect_ratio(true),
    }
    picture.set_hexpand(true);
    picture.set_vexpand(true);

    let (horizontal_alignment, vertical_alignment) = match layout.alignment {
        ArtworkAlignment::Center => (0.5, 0.5),
    };
    let decoration_ratio = match layout.decoration {
        ArtworkDecoration::ContainedImage(dimensions) => {
            dimensions.width_px as f32 / dimensions.height_px as f32
        }
        ArtworkDecoration::QuietSquareField => 1.0,
    };
    let decoration = gtk::AspectFrame::new(
        horizontal_alignment,
        vertical_alignment,
        decoration_ratio,
        false,
    );
    decoration.set_hexpand(true);
    decoration.set_vexpand(true);
    decoration.set_child(Some(&picture));

    let stage = gtk::Overlay::new();
    stage.set_hexpand(true);
    stage.set_vexpand(true);
    stage.set_overflow(gtk::Overflow::Visible);
    let stage_field = gtk::Box::new(gtk::Orientation::Vertical, 0);
    stage_field.set_hexpand(true);
    stage_field.set_vexpand(true);
    stage.set_child(Some(&stage_field));

    let print_plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
    print_plate.add_css_class("artwork-print-plate");
    print_plate.set_halign(gtk::Align::Start);
    print_plate.set_valign(gtk::Align::Start);
    stage.add_overlay(&print_plate);
    stage.set_clip_overlay(&print_plate, false);
    stage.set_measure_overlay(&print_plate, false);
    stage.add_overlay(&decoration);
    stage.set_clip_overlay(&decoration, false);
    stage.set_measure_overlay(&decoration, false);

    let reservation = gtk::AspectFrame::new(horizontal_alignment, vertical_alignment, 1.0, false);
    reservation.add_css_class("artwork-reservation");
    reservation.set_halign(gtk::Align::Start);
    reservation.set_valign(gtk::Align::Center);
    reservation.set_hexpand(false);
    reservation.set_vexpand(false);
    reservation.set_overflow(gtk::Overflow::Visible);
    reservation.set_child(Some(&stage));

    RenderedArtwork {
        reservation,
        print_plate,
        decoration,
        surface: picture,
        source_key: source.map(|(key, _)| key),
        artwork_cache,
        layout,
        readiness: ArtworkReadiness {
            scaled: Cell::new(source_key.is_none()),
            decode_error,
        },
    }
}

fn metadata(
    presentation: &NowPlayingPresentation,
    now_playing_layout: &NowPlayingLayout,
    palette: PresentationPalette,
    rendering: RenderingConfiguration,
) -> RenderedMetadata {
    let root = gtk::Overlay::new();
    root.add_css_class("metadata-column");
    root.set_hexpand(true);
    root.set_vexpand(true);
    let field = gtk::Box::new(gtk::Orientation::Vertical, 0);
    field.set_hexpand(true);
    field.set_vexpand(true);
    root.set_child(Some(&field));

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.add_css_class("metadata-copy");
    copy.set_hexpand(true);

    let musical_metadata = gtk::Overlay::new();
    musical_metadata.add_css_class("musical-metadata");
    musical_metadata.set_halign(gtk::Align::Start);
    musical_metadata.set_hexpand(false);
    // This supplies a pixel maximum without exposing scrolling in the presentation.
    let musical_metadata_slot = gtk::ScrolledWindow::new();
    musical_metadata_slot.add_css_class("musical-metadata-slot");
    musical_metadata_slot.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
    musical_metadata_slot.set_min_content_height(0);
    musical_metadata_slot.set_propagate_natural_width(true);
    musical_metadata_slot.set_propagate_natural_height(false);
    musical_metadata_slot.set_halign(gtk::Align::Start);
    musical_metadata_slot.set_hexpand(false);
    musical_metadata_slot.set_child(Some(&musical_metadata));
    let musical_metadata_alignment = gtk::CenterBox::new();
    musical_metadata_alignment.set_hexpand(true);
    musical_metadata_alignment.set_start_widget(Some(&musical_metadata_slot));
    copy.append(&musical_metadata_alignment);

    let rendered_status = presentation_status(
        &presentation.status,
        now_playing_layout.presentation_status.decoration,
        rendering.behavior,
    );
    rendered_status.root.set_halign(gtk::Align::Start);
    rendered_status.root.set_valign(gtk::Align::Start);
    root.add_overlay(&rendered_status.root);
    root.set_measure_overlay(&rendered_status.root, false);

    let layout = metadata_layout(presentation, Viewport::WINDOWED_FIXTURE);
    let title = layout.title.as_ref().map(|layout| {
        metadata_line(
            layout,
            "title",
            rendering.typography.now_playing_title_family(),
        )
    });
    let artist = layout.artist.as_ref().map(|layout| {
        metadata_line(
            layout,
            "artist",
            rendering.typography.now_playing_supporting_family(),
        )
    });
    let album = layout.album.as_ref().map(|layout| {
        metadata_line(
            layout,
            "album",
            rendering.typography.now_playing_supporting_family(),
        )
    });
    let progress = presentation.progress.as_ref().map(progress_view);
    let activity = presentation
        .activity
        .as_deref()
        .map(|activity| activity_view(activity, rendering.behavior));
    let lyrics = lyric_view(
        presentation,
        presentation.lyrics.as_deref(),
        palette,
        rendering.behavior,
    );
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    footer.add_css_class("utility-footer");
    footer.set_hexpand(true);

    let ordinary_metadata = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ordinary_metadata.add_css_class("ordinary-metadata");
    for role in &now_playing_layout.metadata_roles {
        match role {
            NowPlayingRole::PresentationStatus => {}
            NowPlayingRole::Title => ordinary_metadata
                .append(&title.as_ref().expect("Title role requires a label").label),
            NowPlayingRole::Artist => ordinary_metadata
                .append(&artist.as_ref().expect("Artist role requires a label").label),
            NowPlayingRole::Album => ordinary_metadata
                .append(&album.as_ref().expect("Album role requires a label").label),
            NowPlayingRole::Progress | NowPlayingRole::Activity => {}
        }
    }
    let ordinary_metadata_stage = gtk::Fixed::new();
    ordinary_metadata_stage.set_hexpand(true);
    ordinary_metadata_stage.set_vexpand(true);
    ordinary_metadata_stage.put(&ordinary_metadata, 0.0, 0.0);
    musical_metadata.set_child(Some(&ordinary_metadata_stage));
    musical_metadata.add_overlay(&lyrics.root);
    musical_metadata.set_clip_overlay(&lyrics.root, true);
    musical_metadata.set_measure_overlay(&lyrics.root, false);

    match now_playing_layout.footer_content {
        NowPlayingFooterContent::DeterminateProgress => footer.append(
            &progress
                .as_ref()
                .expect("determinate footer requires a timeline")
                .root,
        ),
        NowPlayingFooterContent::IndeterminateActivity => footer.append(
            &activity
                .as_ref()
                .expect("indeterminate footer requires activity")
                .root,
        ),
        NowPlayingFooterContent::IdentityOnly => {}
    }

    copy.set_halign(gtk::Align::Fill);
    copy.set_valign(gtk::Align::Start);
    root.add_overlay(&copy);
    root.set_measure_overlay(&copy, false);
    let identity = tracked_identity(
        &presentation.tracked_output,
        Some(&presentation.tracked_zone),
        now_playing_layout.identity_placement,
        now_playing_layout.identity_line,
    );
    identity.root.set_halign(gtk::Align::Fill);
    footer.append(&identity.root);
    footer.set_halign(gtk::Align::Fill);
    footer.set_valign(gtk::Align::End);
    root.add_overlay(&footer);
    root.set_measure_overlay(&footer, false);
    let rendered = RenderedMetadata {
        root,
        copy,
        musical_metadata_alignment,
        ordinary_metadata_stage,
        ordinary_metadata,
        presentation_status: rendered_status,
        musical_metadata_slot,
        title,
        artist,
        album,
        lyrics,
        progress,
        activity,
        footer,
        identity,
    };
    rendered.apply_composition_ownership(f64::from(presentation.lyrics.is_some()));
    rendered
}

fn lyric_view(
    presentation: &NowPlayingPresentation,
    lyrics: Option<&LyricPresentation>,
    palette: PresentationPalette,
    behavior: PresentationBehavior,
) -> RenderedLyrics {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("lyric-composition");
    root.set_hexpand(true);
    let masthead = gtk::Box::new(gtk::Orientation::Vertical, 0);
    masthead.add_css_class("lyric-masthead");
    let masthead_title = presentation.title.as_ref().map(|text| {
        let label = metadata_label(text, "lyric-masthead-title");
        label.add_css_class("editorial-text");
        masthead.append(&label);
        label
    });
    let masthead_artist = presentation.artist.as_ref().map(|text| {
        let label = metadata_label(text, "lyric-masthead-artist");
        label.add_css_class("utility-text");
        masthead.append(&label);
        label
    });
    root.append(&masthead);

    let previous = lyric_label("", "lyric-previous");
    let current = lyric_label(" ", "lyric-current");
    let next = lyric_label("", "lyric-next");
    for label in [&previous, &current, &next] {
        label.set_lines(4);
        label.set_ellipsize(pango::EllipsizeMode::End);
    }
    let reel = gtk::Fixed::new();
    reel.add_css_class("lyric-reel");
    reel.set_hexpand(true);
    reel.set_vexpand(true);
    reel.set_overflow(gtk::Overflow::Hidden);
    for label in [&previous, &current, &next] {
        reel.put(label, 0.0, 0.0);
    }
    // Traveling children may leave the reel without changing its allocation.
    let reel_clip = gtk::Overlay::new();
    let reel_field = gtk::Box::new(gtk::Orientation::Vertical, 0);
    reel_field.set_hexpand(true);
    reel_field.set_vexpand(true);
    reel_clip.set_child(Some(&reel_field));
    reel_clip.add_overlay(&reel);
    reel_clip.set_measure_overlay(&reel, false);
    reel_clip.set_clip_overlay(&reel, true);
    let reel_region = gtk::ScrolledWindow::new();
    reel_region.add_css_class("lyric-reel-region");
    reel_region.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
    reel_region.set_propagate_natural_height(false);
    reel_region.set_hexpand(true);
    reel_region.set_vexpand(true);
    reel_region.set_overflow(gtk::Overflow::Hidden);
    reel_region.set_child(Some(&reel_clip));
    root.append(&reel_region);

    let rendered = RenderedLyrics {
        root,
        masthead,
        masthead_title,
        masthead_artist,
        reel_region,
        reel,
        previous,
        current,
        next,
        scale_percentages: Cell::new([100; 3]),
        line_width_px: Cell::new(1),
        typography: Cell::new(
            NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE).typography,
        ),
        palette,
        motion: RefCell::new(LyricMotion::new(0, lyrics, 1)),
        rendered_composition_progress: Cell::new(f64::from(lyrics.is_some())),
        behavior,
    };
    rendered.apply_frame(
        Duration::ZERO,
        &NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE),
    );
    rendered
}

fn lyric_label(text: &str, class_name: &str) -> gtk::Label {
    let label = metadata_label(text, class_name);
    label.add_css_class("utility-text");
    label.set_halign(gtk::Align::Start);
    label.set_valign(gtk::Align::Start);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::Word);
    label.set_max_width_chars(1);
    label
}

fn metadata_line(
    layout: &MetadataLineLayout,
    class_name: &str,
    font_family: &'static str,
) -> RenderedMetadataLine {
    let label = metadata_label(&layout.text, class_name);
    label.add_css_class(match layout.typography {
        MetadataTypography::EditorialSerif => "editorial-text",
        MetadataTypography::ArtistSans | MetadataTypography::AlbumSans => "utility-text",
    });
    label.set_lines(layout.maximum_lines as i32);
    apply_text_overflow(&label, layout.overflow);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::Word);
    // Keep a long label's natural width from overriding the explicit group measure.
    label.set_max_width_chars(1);
    set_label_font_size(&label, layout.font_sizes.preferred_px);

    RenderedMetadataLine {
        label,
        layout: layout.clone(),
        font_family,
    }
}

fn apply_text_overflow(label: &gtk::Label, overflow: TextOverflow) {
    match overflow {
        TextOverflow::EllipsizeEnd => label.set_ellipsize(pango::EllipsizeMode::End),
    }
}

fn apply_full_field_line_layout(label: &gtk::Label, layout: FullFieldLineLayout) {
    apply_text_overflow(label, layout.overflow);
    label.set_lines(layout.maximum_lines as i32);
    label.set_single_line_mode(layout.maximum_lines == 1);
    if layout.maximum_lines == 1 {
        label.set_max_width_chars(1);
    }
    label.set_wrap(layout.wrap);
}

fn full_field_line(text: &str, class_name: &str) -> (gtk::Box, gtk::Label) {
    let label = metadata_label(text, class_name);
    label.set_hexpand(true);
    label.set_valign(gtk::Align::Center);
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    slot.set_hexpand(true);
    slot.append(&label);
    (slot, label)
}

fn set_label_font_size(label: &gtk::Label, font_size_px: u32) {
    label.set_attributes(Some(&font_size_attributes(font_size_px)));
}

fn font_size_attributes(font_size_px: u32) -> pango::AttrList {
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrSize::new_size_absolute(
        font_size_px as i32 * pango::SCALE,
    ));
    attributes
}

fn set_tracked_label_typography(label: &gtk::Label, font_size_px: u32, letter_spacing_px: u32) {
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrSize::new_size_absolute(
        font_size_px as i32 * pango::SCALE,
    ));
    attributes.insert(pango::AttrInt::new_letter_spacing(
        letter_spacing_px as i32 * pango::SCALE,
    ));
    label.set_attributes(Some(&attributes));
}

impl RenderedNowPlaying {
    fn apply_foreground_layout(&self, layout: &NowPlayingLayout) {
        let gutter = dimension(layout.outer_gutter_px);
        self.content.set_margin_start(gutter);
        self.content.set_margin_end(gutter);
        self.content.set_margin_top(0);
        self.content.set_margin_bottom(0);
        self.content.set_spacing(dimension(layout.column_gap_px));

        self.artwork_column
            .set_width_request(dimension(layout.artwork_column_width_px));
        self.artwork.apply_layout(layout);
        self.metadata_slot
            .set_width_request(dimension(layout.information.utility_width_px));
        self.metadata.apply_layout(layout);
    }
}

impl RenderedNowPlayingBackground {
    fn new(palette: PresentationPalette, gradient_cache: Rc<NowPlayingGradientCache>) -> Self {
        let picture = gtk::Picture::new();
        picture.set_can_shrink(false);
        picture.set_keep_aspect_ratio(false);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        let gradient = Rc::new(gradient_cache.gradient(palette));
        picture.connect_scale_factor_notify({
            let gradient = Rc::clone(&gradient);
            move |picture| {
                if let Some(rendered) = gradient.refresh(display_scale_factor(picture)) {
                    install_now_playing_gradient(picture, rendered);
                }
            }
        });
        Self { picture, gradient }
    }

    fn apply_viewport(&self, viewport: Viewport) {
        if let Some(rendered) = self
            .gradient
            .render(viewport, display_scale_factor(&self.picture))
        {
            install_now_playing_gradient(&self.picture, rendered);
        }
    }

    fn apply_prepared(&self, prepared: PreparedNowPlayingGradient) {
        if let Some(rendered) = self.gradient.render_prepared(prepared) {
            install_now_playing_gradient(&self.picture, rendered);
        }
    }
}

fn display_scale_factor(widget: &impl IsA<gtk::Widget>) -> u32 {
    u32::try_from(gtk::prelude::WidgetExt::scale_factor(widget))
        .expect("GTK display scale factor must be positive")
}

fn install_now_playing_gradient(picture: &gtk::Picture, gradient: RenderedNowPlayingGradient) {
    // GTK lays widgets out in logical pixels, then rasterizes them at the
    // widget scale factor. Giving the Picture one texture pixel per physical
    // output pixel avoids resampling the spatial dither during rasterization.
    let bytes = gtk::glib::Bytes::from_owned(gradient.rgba8);
    let texture = gdk::MemoryTexture::new(
        dimension(gradient.physical_viewport.width_px),
        dimension(gradient.physical_viewport.height_px),
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        gradient.stride_bytes,
    );
    picture.set_size_request(
        dimension(gradient.logical_viewport.width_px),
        dimension(gradient.logical_viewport.height_px),
    );
    picture.set_paintable(Some(&texture));
}

impl RenderedArtwork {
    fn capture_ready(&self) -> Result<bool, String> {
        match self.readiness.decode_error.as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(self.readiness.scaled.get()),
        }
    }

    fn apply_layout(&self, now_playing: &NowPlayingLayout) {
        let reservation = ArtworkDimensions::new(
            now_playing.artwork_field_width_px,
            now_playing.artwork_field_height_px,
        );
        self.reservation.set_size_request(
            dimension(reservation.width_px),
            dimension(reservation.height_px),
        );
        let plate = self.layout.print_plate_geometry(
            reservation,
            now_playing.artwork_border_width_px,
            now_playing.artwork_print_plate,
        );
        self.print_plate.set_size_request(
            dimension(plate.footprint.width_px),
            dimension(plate.footprint.height_px),
        );
        self.print_plate.set_margin_start(dimension(plate.left_px));
        self.print_plate.set_margin_top(dimension(plate.top_px));
        let visible = self
            .layout
            .visible_decoration_with_border(reservation, now_playing.artwork_border_width_px);
        self.decoration
            .set_ratio(visible.width_px as f32 / visible.height_px as f32);
        self.surface
            .set_size_request(dimension(visible.width_px), dimension(visible.height_px));
        if let Some(source_key) = self.source_key.as_ref() {
            let image = self
                .layout
                .fitted_image_with_border(reservation, now_playing.artwork_border_width_px)
                .expect("supplied artwork should have fitted image dimensions");
            let scaled = self
                .artwork_cache
                .scaled(source_key, image)
                .expect("positive artwork dimensions should produce a scaled image");
            self.surface.set_pixbuf(Some(&scaled));
            self.readiness.scaled.set(true);
        }
    }
}

impl RenderedFullField {
    fn apply_layout(&self, layout: &FullFieldLayout) {
        // GTK can allocate an earlier viewport after a newer layout is queued.
        // Only the latest generation may commit its deferred font fitting.
        let fit_generation = self.fit_readiness.begin_generation();
        self.copy
            .set_width_request(dimension(layout.composition_width_px));
        self.copy
            .set_margin_top(dimension(layout.presentation_status_slot.top_viewport_y_px));
        self.message
            .set_margin_start(dimension(layout.accent_padding_px));
        self.presentation_status
            .apply_layout(layout.presentation_status);
        self.presentation_status
            .root
            .set_height_request(dimension(layout.presentation_status_slot.height_px));
        self.presentation_status
            .root
            .set_margin_bottom(dimension(layout.status_spacing_px));
        self.heading_slot
            .set_height_request(dimension(layout.heading_slot.height_px));
        apply_full_field_font_size(&self.heading, layout.heading_font, fit_generation.clone());
        apply_full_field_line_layout(&self.heading, layout.heading_line);
        if let (Some(slot), Some(explanation)) =
            (self.explanation_slot.as_ref(), self.explanation.as_ref())
        {
            slot.set_margin_top(dimension(layout.explanation_spacing_px));
            slot.set_height_request(dimension(layout.explanation_slot.height_px));
            apply_full_field_font_size(explanation, layout.explanation_font, fit_generation);
            apply_full_field_line_layout(explanation, layout.explanation_line);
        }
        if let Some(identity) = self.identity.as_ref() {
            let gutter = dimension(layout.outer_gutter_px);
            identity
                .root
                .set_margin_end(gutter + dimension(layout.identity_right_inset_px));
            identity
                .root
                .set_margin_bottom(dimension(layout.identity_anchor.margin_bottom_px(0)));
            identity
                .root
                .set_width_request(dimension(layout.identity_width_px));
            identity.apply_layout(layout.identity_gap_px, layout.identity_px);
        }
    }
}

impl RenderedMetadata {
    fn update_lyrics(
        &self,
        revision: u64,
        presentation: &NowPlayingPresentation,
        now: Duration,
        viewport: Option<Viewport>,
    ) {
        let system_animations_enabled =
            gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
        let measurement_layout = viewport.map(|viewport| {
            NowPlayingLayout::for_composition_progress(presentation, viewport, 1.0)
        });
        let rendered_lines = presentation.lyrics.as_deref().map_or(1, |lyrics| {
            measurement_layout.as_ref().map_or_else(
                || self.lyrics.rendered_line_count(&lyrics.current),
                |layout| self.lyrics.rendered_line_count_at(&lyrics.current, layout),
            )
        });
        let mut motion = self.lyrics.motion.borrow_mut();
        motion.observe_playback(
            revision,
            presentation.playback_position_seconds,
            presentation.status.symbol == roonscape_renderer::PresentationStatusSymbol::Playing,
            now,
        );
        motion.update(
            revision,
            presentation.lyrics.as_deref(),
            rendered_lines,
            now,
            self.lyrics
                .behavior
                .animations_enabled(system_animations_enabled),
        );
        drop(motion);
        self.apply_composition_ownership(self.lyrics.composition_timeline_progress(now));
    }

    fn apply_composition_ownership(&self, progress: f64) {
        let (ordinary_opacity, reel_opacity, masthead_opacity) = composition_ownership(progress);
        self.ordinary_metadata.set_opacity(ordinary_opacity);
        let ordinary_retirement = 1.0 - ordinary_opacity;
        let ordinary_travel_px = f64::from(self.lyrics.typography.get().lyric_current_px) * 1.75;
        self.ordinary_metadata_stage.move_(
            &self.ordinary_metadata,
            0.0,
            -ordinary_retirement * ordinary_travel_px,
        );
        self.lyrics.root.set_opacity(1.0);
        let lyric_travel_px = f64::from(self.lyrics.typography.get().lyric_current_px) * 1.8;
        self.lyrics
            .root
            .set_margin_top(((1.0 - reel_opacity) * lyric_travel_px).round() as i32);
        self.lyrics.reel_region.set_opacity(reel_opacity);
        self.lyrics.masthead.set_opacity(masthead_opacity);
    }

    fn lyric_composition_progress(&self, now: Duration) -> f64 {
        self.lyrics.composition_layout_progress(now)
    }

    fn rendered_composition_progress(&self) -> f64 {
        self.lyrics.rendered_composition_progress.get()
    }

    fn apply_lyric_frame(&self, now: Duration, layout: &NowPlayingLayout) {
        self.lyrics.apply_frame(now, layout);
    }

    fn apply_layout(&self, layout: &NowPlayingLayout) {
        let musical_metadata_width = dimension(layout.information.musical_metadata_width_px);
        self.musical_metadata_slot.set_min_content_width(-1);
        self.musical_metadata_slot.set_max_content_width(-1);
        self.musical_metadata_slot
            .set_min_content_width(musical_metadata_width);
        self.musical_metadata_slot
            .set_max_content_width(musical_metadata_width);
        self.presentation_status
            .apply_layout(layout.presentation_status);
        self.presentation_status.root.set_margin_top(dimension(
            layout
                .artwork_field_anchors
                .presentation_status_margin_top_px(0),
        ));
        self.copy
            .set_margin_top(dimension(layout.metadata_region_top_viewport_y_px));
        self.copy.set_height_request(dimension(
            layout
                .metadata_region_bottom_viewport_y_px
                .saturating_sub(layout.metadata_region_top_viewport_y_px),
        ));
        for line in [&self.title, &self.artist, &self.album]
            .into_iter()
            .flatten()
        {
            line.label.set_width_request(musical_metadata_width);
        }
        self.lyrics.apply_layout(layout);
        self.apply_group_fitting(layout);
        self.musical_metadata_alignment.set_margin_top(0);
        self.musical_metadata_slot.set_height_request(dimension(
            layout
                .metadata_region_bottom_viewport_y_px
                .saturating_sub(layout.metadata_region_top_viewport_y_px),
        ));
        if let Some(progress) = self.progress.as_ref() {
            progress.root.set_margin_top(0);
            progress
                .rail
                .set_height_request(dimension(layout.progress_fill_height_px));
            progress
                .track
                .set_height_request(dimension(layout.progress_track_height_px));
            progress
                .fill
                .set_height_request(dimension(layout.progress_fill_height_px));
            progress
                .times
                .set_margin_top(dimension(layout.time_spacing_px));
            set_label_font_size(&progress.elapsed, layout.typography.time_px);
            set_label_font_size(&progress.remaining, layout.typography.time_px);
        }
        if let Some(activity) = self.activity.as_ref() {
            activity.root.set_margin_top(0);
            activity
                .root
                .set_spacing(dimension(layout.activity_copy_gap_px));
            activity.waveform.set_size_request(
                dimension(layout.activity_waveform_width_px),
                dimension(layout.activity_waveform_height_px),
            );
            set_label_font_size(&activity.heading, layout.typography.activity_heading_px);
            set_label_font_size(&activity.detail, layout.typography.activity_detail_px);
        }

        self.footer.set_spacing(dimension(layout.footer_gap_px));
        self.footer
            .set_margin_bottom(dimension(layout.footer_anchor.margin_bottom_px(0)));
        self.identity
            .apply_now_playing_layout(layout.identity_row, layout.typography.identity_px);
    }

    fn apply_group_fitting(&self, layout: &NowPlayingLayout) {
        let metadata = MetadataLayout {
            title: self
                .title
                .as_ref()
                .map(|line| line.layout_with_font_sizes(layout.typography.title)),
            artist: self
                .artist
                .as_ref()
                .map(|line| line.layout_with_font_sizes(layout.typography.artist)),
            album: self
                .album
                .as_ref()
                .map(|line| line.layout_with_font_sizes(layout.typography.album)),
        };
        let plan = metadata.fitting_group_plan(
            layout.information.musical_metadata_width_px,
            layout.metadata_height_budget_px,
            layout.metadata_fitting,
            |typography, text, font_size_px| {
                self.line(typography).measure_text_px(text, font_size_px)
            },
        );
        self.apply_group_plan(&plan);
        self.ordinary_metadata
            .set_margin_top(dimension(layout.metadata_group_offset_px(plan.height_px)));
    }

    fn line(&self, typography: MetadataTypography) -> &RenderedMetadataLine {
        match typography {
            MetadataTypography::EditorialSerif => self.title.as_ref(),
            MetadataTypography::ArtistSans => self.artist.as_ref(),
            MetadataTypography::AlbumSans => self.album.as_ref(),
        }
        .expect("a measured metadata role has a rendered line")
    }

    fn apply_group_plan(&self, plan: &MetadataGroupPlan) {
        if let (Some(line), Some(line_plan)) = (self.title.as_ref(), plan.title.as_ref()) {
            apply_metadata_line_plan(&line.label, line_plan, 0);
        }
        if let (Some(line), Some(line_plan)) = (self.artist.as_ref(), plan.artist.as_ref()) {
            apply_metadata_line_plan(
                &line.label,
                line_plan,
                u32::from(plan.title.is_some()) * plan.title_to_credit_gap_px,
            );
        }
        if let (Some(line), Some(line_plan)) = (self.album.as_ref(), plan.album.as_ref()) {
            let spacing_px = if plan.artist.is_some() {
                plan.album_gap_px
            } else {
                u32::from(plan.title.is_some()) * plan.title_to_credit_gap_px
            };
            apply_metadata_line_plan(&line.label, line_plan, spacing_px);
        }
    }
}

impl RenderedLyrics {
    fn apply_layout(&self, layout: &NowPlayingLayout) {
        let width = dimension(layout.information.musical_metadata_width_px);
        self.line_width_px.set(width);
        self.typography.set(layout.typography);
        self.root.set_width_request(width);
        self.root
            .set_height_request(dimension(layout.metadata_height_budget_px));
        self.masthead.set_spacing(dimension(
            (layout.typography.lyric_masthead_artist_px as f64 * 0.25).round() as u32,
        ));
        if let Some(title) = self.masthead_title.as_ref() {
            title.set_width_request(width);
            set_label_font_size(title, layout.typography.lyric_masthead_title_px);
        }
        if let Some(artist) = self.masthead_artist.as_ref() {
            artist.set_width_request(width);
            set_label_font_size(artist, layout.typography.lyric_masthead_artist_px);
        }
        let reel_margin_top =
            dimension((layout.typography.lyric_current_px as f64 * 0.52).round() as u32);
        self.reel_region.set_margin_top(reel_margin_top);
        let (_, masthead_height, _, _) = self.masthead.measure(gtk::Orientation::Vertical, width);
        let reel_height = dimension(layout.metadata_height_budget_px)
            .saturating_sub(masthead_height)
            .saturating_sub(reel_margin_top);
        self.reel_region.set_height_request(reel_height);
        for label in [&self.previous, &self.current, &self.next] {
            label.set_width_request(width);
        }
        let current_text = self
            .motion
            .borrow()
            .frame_at(Duration::ZERO)
            .cues
            .into_iter()
            .find(|cue| cue.slot == LyricCueSlot::Current)
            .map(|cue| cue.text);
        if let Some(current_text) = current_text {
            let rendered_lines = self.rendered_line_count(&current_text);
            self.motion
                .borrow_mut()
                .reconcile_rendered_lines(rendered_lines, Duration::ZERO);
        }
        self.apply_frame(Duration::ZERO, layout);
    }

    fn composition_timeline_progress(&self, now: Duration) -> f64 {
        self.motion.borrow().frame_at(now).composition_progress
    }

    fn composition_layout_progress(&self, now: Duration) -> f64 {
        self.update_rendered_composition_progress(self.composition_timeline_progress(now))
    }

    fn update_rendered_composition_progress(&self, timeline_progress: f64) -> f64 {
        let progress = composition_geometry(timeline_progress);
        self.rendered_composition_progress.set(progress);
        progress
    }

    fn rendered_line_count(&self, text: &str) -> i32 {
        self.rendered_line_count_with(
            text,
            self.line_width_px.get(),
            self.typography.get().lyric_current_px,
        )
    }

    fn rendered_line_count_at(&self, text: &str, layout: &NowPlayingLayout) -> i32 {
        self.rendered_line_count_with(
            text,
            dimension(layout.information.musical_metadata_width_px),
            layout.typography.lyric_current_px,
        )
    }

    fn rendered_line_count_with(&self, text: &str, width: i32, font_size_px: u32) -> i32 {
        if text.trim().is_empty() {
            return 1;
        }
        let previous_text = self.current.text();
        let previous_attributes = self.current.attributes();
        self.current.set_text(text);
        set_lyric_label_style(&self.current, font_size_px, self.palette.primary_text);
        let layout = self.current.layout();
        layout.set_width(width.saturating_mul(pango::SCALE));
        let lines = layout.line_count();
        self.current.set_text(&previous_text);
        self.current.set_attributes(previous_attributes.as_ref());
        lines
    }

    fn apply_frame(&self, now: Duration, layout: &NowPlayingLayout) {
        let frame = self.motion.borrow().frame_at(now);
        self.apply_frame_state(&frame, layout);
    }

    fn apply_frame_state(&self, frame: &LyricFrame, layout: &NowPlayingLayout) {
        self.update_rendered_composition_progress(frame.composition_progress);
        for label in [&self.previous, &self.current, &self.next] {
            label.set_visible(false);
        }

        for cue in &frame.cues {
            let label = self.label(cue.slot);
            label.set_text(&cue.text);
            label.set_visible(true);
            label.set_opacity(cue.opacity);
            let scale = lyric_cue_scale(
                layout.typography.lyric_neighbor_px,
                layout.typography.lyric_current_px,
                cue.emphasis,
            );
            let color = lyric_color(self.palette, cue);
            set_lyric_label_style(label, layout.typography.lyric_current_px, color);
            let attributes = label
                .attributes()
                .expect("lyric styling installs attributes");
            attributes.insert(pango::AttrInt::new_weight(if cue.emphasis >= 0.5 {
                pango::Weight::Semibold
            } else {
                pango::Weight::Normal
            }));
            label.set_attributes(Some(&attributes));
            self.apply_scale(cue.slot, scale);
            let label_layout = label.layout();
            label_layout.set_width(self.line_width_px.get().saturating_mul(pango::SCALE));
        }

        let allocated_reel_height = self.reel_region.height();
        let reel_height = if allocated_reel_height > 0 {
            allocated_reel_height
        } else if self.reel.height() > 0 {
            self.reel.height()
        } else {
            dimension(layout.metadata_height_budget_px)
        };
        let focal_center_y = f64::from(reel_height) / 2.0;
        let gap = f64::from(layout.typography.lyric_neighbor_px) * 0.72;
        let current_geometry = frame
            .cues
            .iter()
            .find(|cue| cue.slot == LyricCueSlot::Current)
            .map(|cue| {
                let label = self.label(cue.slot);
                let label_height = f64::from(label.layout().pixel_size().1)
                    * lyric_cue_scale(
                        layout.typography.lyric_neighbor_px,
                        layout.typography.lyric_current_px,
                        cue.emphasis,
                    );
                let y = lyric_cue_y(
                    cue.position,
                    label_height,
                    reel_height,
                    layout.typography.lyric_neighbor_px,
                    focal_center_y,
                );
                (y, label_height)
            });
        for cue in &frame.cues {
            let label = self.label(cue.slot);
            let (_, label_height) = label.layout().pixel_size();
            let label_height = f64::from(label_height)
                * lyric_cue_scale(
                    layout.typography.lyric_neighbor_px,
                    layout.typography.lyric_current_px,
                    cue.emphasis,
                );
            let mut y = lyric_cue_y(
                cue.position,
                label_height,
                reel_height,
                layout.typography.lyric_neighbor_px,
                focal_center_y,
            );
            if cue.departing {
                let focal_y = focal_center_y - label_height / 2.0;
                y = focal_y + cue.position * (focal_center_y + label_height / 2.0);
            }
            if frame.cue_motion_active
                && !cue.departing
                && let Some((current_y, current_height)) = current_geometry
            {
                y = match cue.slot {
                    LyricCueSlot::Previous => y.min(current_y - gap - label_height),
                    LyricCueSlot::Next => y.max(current_y + current_height + gap),
                    LyricCueSlot::Current => y,
                };
            }
            let maximum_y = (f64::from(reel_height) - label_height).max(0.0);
            if !cue.departing {
                y = y.clamp(0.0, maximum_y);
            }
            self.reel.move_(label, 0.0, y.round());
        }
    }

    fn label(&self, slot: LyricCueSlot) -> &gtk::Label {
        match slot {
            LyricCueSlot::Previous => &self.previous,
            LyricCueSlot::Current => &self.current,
            LyricCueSlot::Next => &self.next,
        }
    }

    fn apply_scale(&self, slot: LyricCueSlot, scale: f64) {
        let index = match slot {
            LyricCueSlot::Previous => 0,
            LyricCueSlot::Current => 1,
            LyricCueSlot::Next => 2,
        };
        let percentage = (scale * 100.0).round().clamp(30.0, 100.0) as u8;
        let mut percentages = self.scale_percentages.get();
        if percentages[index] == percentage {
            return;
        }
        let label = self.label(slot);
        label.remove_css_class(&format!("lyric-scale-{:03}", percentages[index]));
        label.add_css_class(&format!("lyric-scale-{percentage:03}"));
        percentages[index] = percentage;
        self.scale_percentages.set(percentages);
    }
}

fn set_lyric_label_style(label: &gtk::Label, font_size_px: u32, color: roonscape_renderer::Rgb) {
    let attributes = font_size_attributes(font_size_px);
    attributes.insert(pango::AttrInt::new_weight(pango::Weight::Semibold));
    attributes.insert(pango::AttrColor::new_foreground(
        u16::from(color.red) * 257,
        u16::from(color.green) * 257,
        u16::from(color.blue) * 257,
    ));
    label.set_attributes(Some(&attributes));
}

fn lyric_color(palette: PresentationPalette, cue: &LyricCueFrame) -> roonscape_renderer::Rgb {
    let from = lyric_role_color(palette, cue.color_from);
    let to = lyric_role_color(palette, cue.color_to);
    let mix = |left: u8, right: u8| {
        (f64::from(left) + (f64::from(right) - f64::from(left)) * cue.color_progress).round() as u8
    };
    roonscape_renderer::Rgb {
        red: mix(from.red, to.red),
        green: mix(from.green, to.green),
        blue: mix(from.blue, to.blue),
    }
}

fn lyric_role_color(palette: PresentationPalette, role: LyricColorRole) -> roonscape_renderer::Rgb {
    match role {
        LyricColorRole::Previous => palette.muted_text,
        LyricColorRole::Focal => palette.primary_text,
        LyricColorRole::Next => palette.secondary_text,
    }
}

fn lyric_cue_scale(neighbor_px: u32, focal_px: u32, emphasis: f64) -> f64 {
    let neighbor_scale = f64::from(neighbor_px) / f64::from(focal_px);
    neighbor_scale + (1.0 - neighbor_scale) * emphasis
}

fn lyric_cue_y(
    position: f64,
    label_height: f64,
    reel_height: i32,
    neighbor_font_size_px: u32,
    focal_center_y: f64,
) -> f64 {
    let focal_y = focal_center_y - label_height / 2.0;
    let edge_inset = f64::from(neighbor_font_size_px) * 0.35;
    let interpolate = |from: f64, to: f64, progress: f64| from + (to - from) * progress;
    if position < 0.0 {
        interpolate(focal_y, edge_inset, -position.clamp(-1.0, 0.0))
    } else {
        let next_y = (f64::from(reel_height) - edge_inset - label_height).max(0.0);
        interpolate(focal_y, next_y, position.clamp(0.0, 1.0))
    }
}

fn composition_ownership(progress: f64) -> (f64, f64, f64) {
    let ordinary = 1.0 - motion_phase(progress, 0.0, 0.62);
    let reel = motion_phase(progress, 0.12, 0.46);
    let masthead = reel * motion_phase(1.0 - ordinary, 0.7, 0.3);
    (ordinary, reel, masthead)
}

fn composition_geometry(progress: f64) -> f64 {
    motion_phase(progress, 0.12, 0.72)
}

fn motion_phase(value: f64, start: f64, duration: f64) -> f64 {
    let progress = ((value - start) / duration).clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

impl RenderedPresentationStatus {
    fn update(&mut self, status: &PresentationStatus) {
        if self.status == *status {
            return;
        }

        self.root.remove_css_class("status-full");
        self.root.remove_css_class("status-muted");
        self.root.add_css_class(match status.emphasis {
            PresentationStatusEmphasis::FullAccent => "status-full",
            PresentationStatusEmphasis::MutedAccent => "status-muted",
        });
        self.label.set_text(status.label);

        let width = self.symbol.width_request();
        let height = self.symbol.height_request();
        self.root.remove(&self.symbol);
        let symbol = presentation_status_symbol(status, self.decoration, self.behavior);
        symbol.set_size_request(width, height);
        self.root.prepend(&symbol);
        self.symbol = symbol;
        self.status = *status;
    }

    fn apply_layout(&self, layout: PresentationStatusLayout) {
        self.root.set_spacing(dimension(layout.symbol_gap_px));
        let symbol_size = dimension(layout.symbol_size_px);
        self.symbol.set_size_request(symbol_size, symbol_size);
        set_tracked_label_typography(&self.label, layout.font_px, layout.letter_spacing_px);
    }
}

impl RenderedIdentity {
    fn apply_layout(&self, gap_px: u32, name_px: u32) {
        self.root.set_column_spacing(gap_px.div_ceil(2));
        let label_px = ((name_px as f64) * 0.84).round() as u32;
        let separator_px = IdentityRowLayout::separator_diameter_px(name_px);
        let label_letter_spacing_px = IdentityRowLayout::tracked_label_letter_spacing_px(label_px);
        self.apply_typography(
            label_px,
            name_px,
            label_px / 2,
            separator_px,
            label_letter_spacing_px,
        );
    }

    fn apply_now_playing_layout(&self, layout: IdentityRowLayout, name_px: u32) {
        self.root.set_column_spacing(layout.phrase_gap_px);
        self.apply_typography(
            layout.label_px,
            name_px,
            layout.label_gap_px,
            layout.separator_size_px,
            layout.label_letter_spacing_px,
        );
        match layout.phrase_alignment {
            IdentityPhraseAlignment::Baseline => {
                for label in [&self.output_label, &self.output_name] {
                    label.set_valign(gtk::Align::Baseline);
                }
                if let Some(zone) = self.zone.as_ref() {
                    zone.label.set_valign(gtk::Align::Baseline);
                    zone.name.set_valign(gtk::Align::Baseline);
                }
            }
        }
        self.output.set_hexpand(false);
        self.output.set_size_request(-1, -1);
        let (_, output_width, _, _) = self.output.measure(gtk::Orientation::Horizontal, -1);
        self.output.set_size_request(
            output_width.min(dimension(layout.output_phrase_max_width_px)),
            -1,
        );
        if let Some(zone) = self.zone.as_ref() {
            zone.root.set_hexpand(false);
            zone.root.set_size_request(-1, -1);
            let (_, natural_width, _, _) = zone.root.measure(gtk::Orientation::Horizontal, -1);
            zone.root.set_size_request(
                natural_width.min(dimension(layout.zone_phrase_max_width_px)),
                -1,
            );
            zone.root.set_halign(gtk::Align::Start);
        }
        self.output_name.set_hexpand(true);
        if let Some(zone) = self.zone.as_ref() {
            zone.name.set_hexpand(true);
            zone.name.set_xalign(0.0);
        }
    }

    fn apply_typography(
        &self,
        label_px: u32,
        name_px: u32,
        label_gap_px: u32,
        separator_px: u32,
        label_letter_spacing_px: u32,
    ) {
        set_tracked_label_typography(&self.output_label, label_px, label_letter_spacing_px);
        set_label_font_size(&self.output_name, name_px);
        self.output_label.set_margin_end(dimension(label_gap_px));
        if let Some(zone) = self.zone.as_ref() {
            set_tracked_label_typography(&zone.label, label_px, label_letter_spacing_px);
            set_label_font_size(&zone.name, name_px);
            zone.separator
                .set_size_request(dimension(separator_px), dimension(separator_px));
            zone.label.set_margin_end(dimension(label_gap_px));
        }
    }
}

impl RenderedMetadataLine {
    fn layout_with_font_sizes(
        &self,
        font_sizes: roonscape_renderer::MetadataFontSizes,
    ) -> MetadataLineLayout {
        MetadataLineLayout {
            font_sizes,
            ..self.layout.clone()
        }
    }

    fn measure_text_px(&self, text: &str, font_size_px: u32) -> (u32, u32) {
        let measurement = pango::Layout::new(&self.label.pango_context());
        let mut font = pango::FontDescription::new();
        font.set_family(self.font_family);
        font.set_style(pango::Style::Normal);
        font.set_weight(match self.layout.typography {
            MetadataTypography::EditorialSerif => pango::Weight::Bold,
            MetadataTypography::ArtistSans => pango::Weight::Semibold,
            MetadataTypography::AlbumSans => pango::Weight::Normal,
        });
        font.set_absolute_size(f64::from(font_size_px * pango::SCALE as u32));
        measurement.set_font_description(Some(&font));
        measurement.set_text(text);
        let (width_px, height_px) = measurement.pixel_size();
        (width_px.max(0) as u32, height_px.max(0) as u32)
    }
}

fn apply_metadata_line_plan(
    label: &gtk::Label,
    plan: &roonscape_renderer::MetadataLinePlan,
    margin_top_px: u32,
) {
    label.set_text(&plan.lines.join("\n"));
    label.set_lines(plan.lines.len() as i32);
    label.set_margin_top(dimension(margin_top_px));
    // Keep the selected font's native leading, as in the settled ordinary
    // composition. Mutating GtkLabel's transient Pango layout here made
    // playback refreshes change leading after GTK had allocated the label.
    set_label_font_size(label, plan.font_size_px);
}

fn apply_full_field_font_size(
    label: &gtk::Label,
    sizes: FullFieldFontSize,
    fit_generation: FullFieldFitGeneration,
) {
    set_label_font_size(label, sizes.preferred_px);
    fit_generation.register_fit();
    let fitted_label = label.clone();
    label.add_tick_callback(move |_, _| {
        if !fit_generation.is_current() {
            fit_generation.complete_fit();
            return gtk::glib::ControlFlow::Break;
        }
        if fitted_label.width() <= 0 {
            return gtk::glib::ControlFlow::Continue;
        }
        fit_full_field_line(&fitted_label, sizes);
        fit_generation.complete_fit();
        gtk::glib::ControlFlow::Break
    });
}

fn fit_full_field_line(label: &gtk::Label, sizes: FullFieldFontSize) {
    let _ = sizes.fitting_font_size(|font_size_px| {
        set_label_font_size(label, font_size_px);
        let measurement = pango::Layout::new(&label.pango_context());
        measurement.set_text(&label.text());
        measurement.set_attributes(label.attributes().as_ref());
        let (width_px, _) = measurement.pixel_size();
        width_px <= label.width()
    });
}

fn dimension(value: u32) -> i32 {
    i32::try_from(value).expect("supported viewport dimensions fit GTK's signed sizes")
}

fn presentation_status(
    status: &PresentationStatus,
    decoration: roonscape_renderer::PresentationStatusDecoration,
    behavior: PresentationBehavior,
) -> RenderedPresentationStatus {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("presentation-status");
    row.add_css_class(match status.emphasis {
        PresentationStatusEmphasis::FullAccent => "status-full",
        PresentationStatusEmphasis::MutedAccent => "status-muted",
    });
    row.set_halign(gtk::Align::Start);
    row.set_valign(gtk::Align::Start);

    let symbol = presentation_status_symbol(status, decoration, behavior);
    row.append(&symbol);
    let label = metadata_label(status.label, "status-label");
    row.append(&label);
    RenderedPresentationStatus {
        root: row,
        symbol,
        label,
        decoration,
        status: *status,
        behavior,
    }
}

fn progress_view(progress: &PresentationProgress) -> RenderedProgress {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
    group.add_css_class("progress-group");

    let rail = gtk::Overlay::new();
    rail.add_css_class("progress-rail");
    rail.set_hexpand(true);
    let track = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    track.add_css_class("progress-track");
    track.set_halign(gtk::Align::Fill);
    track.set_valign(gtk::Align::Center);
    rail.set_child(Some(&track));
    let fill = gtk::ProgressBar::new();
    fill.add_css_class("progress-fill");
    fill.set_fraction(progress.fraction);
    fill.set_show_text(false);
    fill.set_hexpand(true);
    fill.set_halign(gtk::Align::Fill);
    fill.set_valign(gtk::Align::Center);
    rail.add_overlay(&fill);
    rail.set_measure_overlay(&fill, true);
    group.append(&rail);

    let times = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    times.add_css_class("times");
    let elapsed = metadata_label(&progress.elapsed, "time");
    let remaining = metadata_label(&progress.remaining, "time");
    remaining.set_halign(gtk::Align::End);
    remaining.set_hexpand(true);
    times.append(&elapsed);
    times.append(&remaining);
    group.append(&times);
    RenderedProgress {
        root: group,
        rail,
        track,
        fill,
        times,
        elapsed,
        remaining,
    }
}

fn activity_view(
    activity: &PresentationActivity,
    behavior: PresentationBehavior,
) -> RenderedActivity {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("activity-group");
    root.set_halign(gtk::Align::Start);
    root.set_valign(gtk::Align::Center);

    let waveform = activity_waveform(activity.waveform, behavior);
    root.append(&waveform);

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.set_valign(gtk::Align::Center);
    let heading = metadata_label(activity.heading, "activity-heading");
    heading.add_css_class("utility-text");
    let detail = metadata_label(activity.detail, "activity-detail");
    detail.add_css_class("utility-text");
    copy.append(&heading);
    copy.append(&detail);
    root.append(&copy);

    RenderedActivity {
        root,
        waveform,
        heading,
        detail,
    }
}

fn tracked_identity(
    tracked_output: &str,
    tracked_zone: Option<&str>,
    placement: IdentityPlacement,
    line_layout: IdentityLineLayout,
) -> RenderedIdentity {
    let row = gtk::Grid::new();
    row.add_css_class("tracked-identity");
    row.set_column_homogeneous(false);
    row.set_hexpand(true);
    match placement {
        IdentityPlacement::BottomRight => {
            row.set_halign(gtk::Align::End);
            row.set_valign(gtk::Align::End);
        }
    }

    let output = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    output.set_hexpand(true);
    output.set_halign(gtk::Align::Fill);
    let output_label = metadata_label("OUTPUT", "identity-label");
    let output_name = identity_name(tracked_output, line_layout);
    output_label.set_valign(gtk::Align::Baseline);
    output_name.set_valign(gtk::Align::Baseline);
    output.append(&output_label);
    output.append(&output_name);

    row.attach(&output, 0, 0, 1, 1);
    let zone = if let Some(tracked_zone) = tracked_zone {
        let zone = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        zone.set_hexpand(true);
        zone.set_halign(gtk::Align::End);
        let zone_label = metadata_label("ZONE", "identity-label");
        let zone_name = identity_name(tracked_zone, line_layout);
        zone_label.set_valign(gtk::Align::Baseline);
        zone_name.set_valign(gtk::Align::Baseline);
        zone_name.set_xalign(1.0);
        zone.append(&zone_label);
        zone.append(&zone_name);

        let separator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        separator.add_css_class("identity-separator");
        separator.set_halign(gtk::Align::Center);
        separator.set_valign(gtk::Align::Center);

        row.attach(&separator, 1, 0, 1, 1);
        row.attach(&zone, 2, 0, 1, 1);
        Some(RenderedZoneIdentity {
            root: zone,
            label: zone_label,
            name: zone_name,
            separator,
        })
    } else {
        output.set_hexpand(false);
        output.set_halign(gtk::Align::End);
        None
    };
    RenderedIdentity {
        root: row,
        output,
        output_label,
        output_name,
        zone,
    }
}

fn identity_name(text: &str, layout: IdentityLineLayout) -> gtk::Label {
    let label = metadata_label(text, "identity-name");
    apply_text_overflow(&label, layout.overflow);
    label.set_lines(layout.maximum_lines as i32);
    label.set_single_line_mode(layout.maximum_lines == 1);
    label
}

fn metadata_label(text: &str, class_name: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class(class_name);
    label.set_xalign(0.0);
    label
}

pub(crate) fn install_style_providers(typography: TypographySelection) -> gtk::CssProvider {
    let static_provider = gtk::CssProvider::new();
    static_provider.load_from_data(&format!(
        "{STYLES}\n{}\n{}",
        lyric_scale_styles(),
        TypographyStyles::new(typography).to_css()
    ));
    let palette_provider = gtk::CssProvider::new();
    let display = gdk::Display::default().expect("GTK should have a display");
    gtk::style_context_add_provider_for_display(
        &display,
        &static_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &palette_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    palette_provider
}

fn lyric_scale_styles() -> String {
    (30..=100)
        .map(|percentage| {
            format!(
                ".lyric-scale-{percentage:03} {{ transform: scale({:.2}); transform-origin: left top; }}",
                f64::from(percentage) / 100.0
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::Arc;

    use gtk::glib::object::ObjectType;
    use gtk::prelude::*;

    use super::{
        PRESENTATION_CACHE_CAPACITY, PresentationCaches, PresentationLayoutSource,
        RenderingConfiguration, STYLES, composition_ownership, lyric_view,
    };
    use crate::lyric_motion::LyricMotionCause;
    use roonscape_renderer::{
        LyricNeighborVisibility, NowPlayingFooterContent, NowPlayingGradientCacheKey,
        NowPlayingLayout, Presentation, PresentationBehavior, PresentationPalette, Viewport,
        parse_snapshot, presentation_from_snapshot,
    };

    fn lyric_presentation(fixture: &str) -> roonscape_renderer::NowPlayingPresentation {
        let snapshot = parse_snapshot(match fixture {
            "playing.json" => include_str!("../../shared/fixtures/playing.json"),
            "long-metadata.json" => include_str!("../../shared/fixtures/long-metadata.json"),
            "lyrics-one-line.json" => include_str!("../../shared/fixtures/lyrics-one-line.json"),
            "lyrics-two-line.json" => include_str!("../../shared/fixtures/lyrics-two-line.json"),
            "lyrics-blank-cue.json" => include_str!("../../shared/fixtures/lyrics-blank-cue.json"),
            "lyrics-revision-after.json" => {
                include_str!("../../shared/fixtures/lyrics-revision-after.json")
            }
            _ => panic!("unsupported lyric fixture"),
        })
        .expect("lyric fixture should satisfy the shared contract");
        let Presentation::NowPlaying(presentation) =
            presentation_from_snapshot(&snapshot).expect("lyric fixture should be presentable")
        else {
            panic!("lyric fixture should use Now Playing");
        };
        presentation
    }

    fn allocate_lyrics(lyrics: &super::RenderedLyrics, layout: &NowPlayingLayout) {
        lyrics.apply_layout(layout);
        lyrics.root.allocate(
            layout.information.musical_metadata_width_px as i32,
            layout.metadata_height_budget_px as i32,
            -1,
            None,
        );
        while gtk::glib::MainContext::default().iteration(false) {}
        lyrics.apply_frame(std::time::Duration::ZERO, layout);
        lyrics.root.allocate(
            layout.information.musical_metadata_width_px as i32,
            layout.metadata_height_budget_px as i32,
            -1,
            None,
        );
        while gtk::glib::MainContext::default().iteration(false) {}
    }

    fn rendered_now_playing(
        presentation: &roonscape_renderer::NowPlayingPresentation,
        behavior: PresentationBehavior,
    ) -> super::RenderedPresentation {
        super::now_playing(
            presentation,
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
            PresentationPalette::fallback(),
            PresentationLayoutSource::for_presentation(&Presentation::NowPlaying(
                presentation.clone(),
            )),
            RenderingConfiguration::live(
                roonscape_renderer::select_typography(&HashSet::new()),
                behavior,
            ),
            None,
            PresentationCaches::new(PRESENTATION_CACHE_CAPACITY),
        )
    }

    fn lyric_motion_frame(
        rendered: &super::RenderedPresentation,
        now: std::time::Duration,
    ) -> crate::lyric_motion::LyricFrame {
        rendered
            .now_playing
            .as_ref()
            .expect("lyric motion requires Now Playing")
            .metadata
            .lyrics
            .motion
            .borrow()
            .frame_at(now)
    }

    fn assert_rendered_lyric_roles(
        rendered: &super::RenderedPresentation,
        previous: Option<&str>,
        current: Option<&str>,
        next: Option<&str>,
    ) {
        let lyrics = &rendered
            .now_playing
            .as_ref()
            .expect("rendered lyric roles require Now Playing")
            .metadata
            .lyrics;
        for (label, expected) in [
            (&lyrics.previous, previous),
            (&lyrics.current, current),
            (&lyrics.next, next),
        ] {
            assert_eq!(label.is_visible(), expected.is_some());
            if let Some(expected) = expected {
                assert_eq!(label.text(), expected);
                assert_eq!(label.opacity(), 1.0);
            }
        }
    }

    fn assert_rendered_composition_ownership(
        rendered: &super::RenderedPresentation,
        ordinary: f64,
        reel: f64,
        masthead: f64,
    ) {
        let metadata = &rendered
            .now_playing
            .as_ref()
            .expect("composition ownership requires Now Playing")
            .metadata;
        assert_eq!(metadata.ordinary_metadata.opacity(), ordinary);
        assert_eq!(metadata.lyrics.reel_region.opacity(), reel);
        assert_eq!(metadata.lyrics.masthead.opacity(), masthead);
    }

    fn visual_label_bounds(
        label: &gtk::Label,
        ancestor: &impl IsA<gtk::Widget>,
    ) -> gtk::graphene::Rect {
        label
            .compute_bounds(ancestor)
            .expect("visible lyric label should have ancestor-relative bounds")
    }

    fn assert_height_aware_cues_remain_separate(
        rendered: &super::RenderedLyrics,
        layout: &NowPlayingLayout,
        started_at: std::time::Duration,
    ) {
        assert_eq!(
            rendered.motion.borrow().frame_at(started_at).cause,
            LyricMotionCause::NaturalCueHandoff { height_aware: true }
        );
        let mut rendered_line_counts = [None; 3];
        let mut previous_centers = [None; 3];
        for offset in (0..620).step_by(20).chain(std::iter::once(619)) {
            let now = started_at + std::time::Duration::from_millis(offset);
            rendered.apply_frame(now, layout);
            rendered.root.allocate(
                layout.information.musical_metadata_width_px as i32,
                layout.metadata_height_budget_px as i32,
                -1,
                None,
            );
            while gtk::glib::MainContext::default().iteration(false) {}
            let frame = rendered.motion.borrow().frame_at(now);
            let visible = frame
                .cues
                .iter()
                .filter(|cue| cue.opacity >= 0.05)
                .map(|cue| {
                    let slot_index = match cue.slot {
                        crate::lyric_motion::LyricCueSlot::Previous => 0,
                        crate::lyric_motion::LyricCueSlot::Current => 1,
                        crate::lyric_motion::LyricCueSlot::Next => 2,
                    };
                    let line_count = rendered.label(cue.slot).layout().line_count();
                    if let Some(expected) = rendered_line_counts[slot_index] {
                        assert_eq!(
                            line_count, expected,
                            "a traveling cue must keep its authored Pango wrapping: cue={cue:?}, offset={offset}"
                        );
                    } else {
                        rendered_line_counts[slot_index] = Some(line_count);
                    }
                    let bounds = visual_label_bounds(rendered.label(cue.slot), &rendered.reel);
                    if !cue.departing {
                        assert!(
                            bounds.y() >= 0.0
                                && bounds.y() + bounds.height() <= rendered.reel.height() as f32,
                            "context and focal cues must remain inside the reel: cue={cue:?}, bounds={bounds:?}, offset={offset}"
                        );
                    } else {
                        assert_eq!(rendered.reel.overflow(), gtk::Overflow::Hidden);
                    }
                    let center = bounds.y() + bounds.height() / 2.0;
                    if let Some(previous_center) = previous_centers[slot_index]
                        && (cue.departing || cue.slot != crate::lyric_motion::LyricCueSlot::Previous)
                    {
                        assert!(
                            center <= previous_center + 1.0,
                            "Reel Lift cues must travel monotonically upward: cue={cue:?}, previous_center={previous_center}, center={center}, offset={offset}"
                        );
                        assert!(
                            previous_center - center <= rendered.reel.height() as f32 * 0.16,
                            "Reel Lift must not catch up with a frame-to-frame jump: cue={cue:?}, previous_center={previous_center}, center={center}, offset={offset}"
                        );
                    }
                    previous_centers[slot_index] = Some(center);
                    (cue.slot, bounds)
                })
                .collect::<Vec<_>>();
            let outgoing_bounds = visible.iter().find_map(|(slot, bounds)| {
                (*slot == crate::lyric_motion::LyricCueSlot::Previous).then_some(bounds)
            });
            let incoming_bounds = visible.iter().find_map(|(slot, bounds)| {
                (*slot == crate::lyric_motion::LyricCueSlot::Current).then_some(bounds)
            });
            if let (Some(outgoing_bounds), Some(incoming_bounds)) =
                (outgoing_bounds, incoming_bounds)
            {
                let overlaps = outgoing_bounds.y() + outgoing_bounds.height() > incoming_bounds.y();
                if overlaps {
                    let outgoing = frame
                        .cues
                        .iter()
                        .find(|cue| cue.slot == crate::lyric_motion::LyricCueSlot::Previous)
                        .expect("outgoing cue should exist");
                    let incoming = frame
                        .cues
                        .iter()
                        .find(|cue| cue.slot == crate::lyric_motion::LyricCueSlot::Current)
                        .expect("incoming cue should exist");
                    assert!(
                        outgoing.opacity.min(incoming.opacity) <= 0.6,
                        "overlapping height-aware cues must not form a fully opaque text block: outgoing={outgoing:?}, incoming={incoming:?}, outgoing_bounds={outgoing_bounds:?}, incoming_bounds={incoming_bounds:?}, offset={offset}"
                    );
                }
            }
            assert!(
                frame
                    .cues
                    .iter()
                    .map(|cue| cue.emphasis * cue.opacity)
                    .fold(0.0_f64, f64::max)
                    >= 0.45,
                "height-aware handoff must retain a dominant cue: frame={frame:?}, offset={offset}"
            );
        }
    }

    fn ordinary_metadata_remains_stable_on_playback_updates() {
        let window = gtk::Window::new();
        window.set_default_size(1_280, 720);
        let families = window
            .pango_context()
            .list_families()
            .into_iter()
            .map(|family| family.name().to_string())
            .collect();
        let typography = roonscape_renderer::select_typography(&families);
        super::install_style_providers(typography);
        let ordinary = lyric_presentation("long-metadata.json");
        let viewport = Viewport::new(1_280, 720);
        let mut rendered = super::now_playing(
            &ordinary,
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
            PresentationPalette::fallback(),
            PresentationLayoutSource::for_presentation(&Presentation::NowPlaying(ordinary.clone())),
            RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
            None,
            PresentationCaches::new(PRESENTATION_CACHE_CAPACITY),
        );
        rendered.apply_viewport(viewport);
        window.set_child(Some(&rendered.root));
        window.present();
        for _ in 0..40 {
            while gtk::glib::MainContext::default().iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let heights = || {
            let metadata = &rendered.now_playing.as_ref().unwrap().metadata;
            [&metadata.title, &metadata.artist, &metadata.album]
                .map(|line| line.as_ref().unwrap().label.layout().pixel_size().1)
        };
        let before = heights();
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(ordinary),
            std::time::Duration::from_secs(1),
            Some(viewport),
        );
        for _ in 0..40 {
            while gtk::glib::MainContext::default().iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let metadata = &rendered.now_playing.as_ref().unwrap().metadata;
        let after = [&metadata.title, &metadata.artist, &metadata.album]
            .map(|line| line.as_ref().unwrap().label.layout().pixel_size().1);
        window.destroy();
        super::install_style_providers(roonscape_renderer::select_typography(&HashSet::new()));
        assert_eq!(
            before, after,
            "playback updates must preserve fitted metadata line heights"
        );
    }

    #[test]
    fn allocated_lyric_reel_recomputes_neighbors_and_blank_visibility() {
        roonscape_renderer::register_packaged_fallback_fonts(Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
        gtk::init().expect("GTK should initialize for native lyric layout coverage");
        super::install_style_providers(roonscape_renderer::select_typography(&HashSet::new()));
        ordinary_metadata_remains_stable_on_playback_updates();
        short_blanks_promote_without_a_skipped_cue_cut();
        a_seek_within_the_incoming_cue_settles_its_handoff();
        tall_departure_keeps_focal_size_and_reveals_separate_memory();
        blank_promotion_preserves_context_and_an_interrupted_departure();

        let one_line = lyric_presentation("lyrics-one-line.json");
        let one_line_lyrics = one_line
            .lyrics
            .as_deref()
            .expect("fixture should have lyrics");
        let rendered = lyric_view(
            &one_line,
            Some(one_line_lyrics),
            PresentationPalette::fallback(),
            PresentationBehavior::StaticFixture,
        );
        let compact_layout =
            NowPlayingLayout::for_presentation(&one_line, Viewport::new(1_280, 720));
        allocate_lyrics(&rendered, &compact_layout);

        assert_eq!(rendered.current.layout().line_count(), 1);
        assert!(rendered.previous.is_visible());
        assert!(rendered.next.is_visible());
        let root_height = rendered.root.height() as f32;
        for label in [&rendered.previous, &rendered.current, &rendered.next] {
            let bounds = visual_label_bounds(label, &rendered.root);
            assert!(bounds.y() >= 0.0, "lyric bounds={bounds:?}");
            assert!(
                bounds.y() + bounds.height() <= root_height,
                "lyric bounds={bounds:?}, root_height={root_height}, reel_height={}, region_height={}, region_bounds={:?}",
                rendered.reel.height(),
                rendered.reel_region.height(),
                rendered.reel_region.compute_bounds(&rendered.root)
            );
        }
        let region_bounds = rendered
            .reel_region
            .compute_bounds(&rendered.root)
            .expect("lyric region should have root-relative bounds");
        let current_bounds = rendered
            .current
            .compute_bounds(&rendered.root)
            .expect("focal lyric should have root-relative bounds");
        assert!(
            ((current_bounds.y() + current_bounds.height() / 2.0)
                - (region_bounds.y() + region_bounds.height() / 2.0))
                .abs()
                <= 1.0,
            "the focal cue should preserve the lyric region's center anchor: current={current_bounds:?}, region={region_bounds:?}"
        );
        assert_eq!(rendered.reel_region.overflow(), gtk::Overflow::Hidden);
        assert!(rendered.current.has_css_class("utility-text"));
        assert!(!rendered.current.has_css_class("editorial-text"));
        for label in [&rendered.previous, &rendered.current, &rendered.next] {
            assert_eq!(label.lines(), 4);
            assert_eq!(label.ellipsize(), gtk::pango::EllipsizeMode::End);
        }

        let two_line = lyric_presentation("lyrics-two-line.json");
        let two_line_lyrics = two_line
            .lyrics
            .as_deref()
            .expect("fixture should have lyrics");
        let resized = lyric_view(
            &two_line,
            Some(two_line_lyrics),
            PresentationPalette::fallback(),
            PresentationBehavior::StaticFixture,
        );
        for viewport in [
            Viewport::new(1_280, 720),
            Viewport::new(1_600, 900),
            Viewport::new(1_600, 1_200),
            Viewport::new(1_920, 1_200),
            Viewport::new(2_560, 1_080),
            Viewport::new(3_840, 2_160),
            Viewport::new(3_840, 2_400),
        ] {
            let layout = NowPlayingLayout::for_presentation(&two_line, viewport);
            allocate_lyrics(&resized, &layout);
            let expected =
                LyricNeighborVisibility::for_rendered_lines(resized.current.layout().line_count());
            assert_eq!(
                resized.previous.is_visible(),
                expected.previous,
                "viewport={viewport:?} measured={} current={}",
                resized.rendered_line_count(&two_line_lyrics.current),
                resized.current.layout().line_count()
            );
            assert_eq!(resized.next.is_visible(), expected.next);
            let root_height = resized.root.height() as f32;
            for label in [&resized.previous, &resized.current, &resized.next] {
                if label.is_visible() {
                    let bounds = visual_label_bounds(label, &resized.root);
                    assert!(bounds.y() >= 0.0);
                    assert!(bounds.y() + bounds.height() <= root_height);
                }
            }
            let region_bounds = resized
                .reel_region
                .compute_bounds(&resized.root)
                .expect("peer viewport reel should have root-relative bounds");
            let current_bounds = resized
                .current
                .compute_bounds(&resized.root)
                .expect("peer viewport focal cue should have root-relative bounds");
            assert!(
                ((current_bounds.y() + current_bounds.height() / 2.0)
                    - (region_bounds.y() + region_bounds.height() / 2.0))
                    .abs()
                    <= 1.0,
                "peer viewport focal anchor should remain centered: viewport={viewport:?}, current={current_bounds:?}, region={region_bounds:?}"
            );
            if resized.next.is_visible() {
                let next_bounds = visual_label_bounds(&resized.next, &resized.root);
                let tier_distance = next_bounds.y() + next_bounds.height() / 2.0
                    - (current_bounds.y() + current_bounds.height() / 2.0);
                assert!(
                    tier_distance >= region_bounds.height() * 0.28,
                    "peer viewport tiers should remain broadly separated: viewport={viewport:?}, current={current_bounds:?}, next={next_bounds:?}, region={region_bounds:?}"
                );
            }
        }

        let blank = lyric_presentation("lyrics-blank-cue.json");
        let blank_lyrics = blank.lyrics.as_deref().expect("fixture should have lyrics");
        let blank_rendered = lyric_view(
            &blank,
            Some(blank_lyrics),
            PresentationPalette::fallback(),
            PresentationBehavior::StaticFixture,
        );
        let blank_layout = NowPlayingLayout::for_presentation(&blank, Viewport::new(1_280, 720));
        allocate_lyrics(&blank_rendered, &blank_layout);

        assert!(blank_rendered.previous.is_visible());
        assert!(!blank_rendered.current.is_visible());
        assert!(blank_rendered.next.is_visible());
        gtk::Settings::default()
            .expect("GTK settings should be available")
            .set_gtk_enable_animations(true);

        let presentation = lyric_presentation("lyrics-one-line.json");
        let initial = presentation
            .lyrics
            .as_deref()
            .expect("fixture should have lyrics");
        let rendered = lyric_view(
            &presentation,
            Some(initial),
            PresentationPalette::fallback(),
            PresentationBehavior::Dynamic,
        );
        let layout = NowPlayingLayout::for_presentation(&presentation, Viewport::new(1_280, 720));
        allocate_lyrics(&rendered, &layout);

        let settled_focal = rendered
            .current
            .compute_bounds(&rendered.reel)
            .expect("settled focal cue should have reel-relative bounds");
        let settled_next = rendered
            .next
            .compute_bounds(&rendered.reel)
            .expect("settled Next Cue should have reel-relative bounds");
        let settled_focal_center = settled_focal.y() + settled_focal.height() / 2.0;
        let settled_next_center = settled_next.y() + settled_next.height() / 2.0;
        assert!(
            settled_next_center - settled_focal_center >= rendered.reel.height() as f32 * 0.32,
            "the focal and Next Cue tiers should remain broadly separated: focal={settled_focal:?}, next={settled_next:?}, reel_height={}",
            rendered.reel.height()
        );

        let mut adjacent = initial.clone();
        adjacent.current_index += 1;
        adjacent.previous = Some(initial.current.clone());
        adjacent.current = initial
            .next
            .clone()
            .expect("fixture should have a next cue");
        rendered.motion.borrow_mut().update(
            0,
            Some(&adjacent),
            rendered.rendered_line_count(&adjacent.current),
            std::time::Duration::ZERO,
            true,
        );
        let midpoint = std::time::Duration::from_millis(310);
        rendered.apply_frame(midpoint, &layout);
        rendered.root.allocate(
            layout.information.musical_metadata_width_px as i32,
            layout.metadata_height_budget_px as i32,
            -1,
            None,
        );
        while gtk::glib::MainContext::default().iteration(false) {}
        let frame = rendered.motion.borrow().frame_at(midpoint);
        assert_eq!(
            frame.cause,
            LyricMotionCause::NaturalCueHandoff {
                height_aware: false
            }
        );
        assert_eq!(rendered.previous.text(), initial.current);
        assert_eq!(rendered.current.text(), adjacent.current);
        assert!(rendered.previous.opacity() > 0.0);
        assert!(rendered.current.opacity() > 0.0);
        let outgoing_bounds = visual_label_bounds(&rendered.previous, &rendered.reel);
        let incoming_bounds = visual_label_bounds(&rendered.current, &rendered.reel);
        assert!(
            incoming_bounds.height() >= outgoing_bounds.height() * 1.35,
            "the incoming cue should be decisively larger by midpoint: outgoing={outgoing_bounds:?}, incoming={incoming_bounds:?}"
        );
        let incoming_center = incoming_bounds.y() + incoming_bounds.height() / 2.0;
        assert!(
            (incoming_center - settled_focal_center).abs() <= rendered.reel.height() as f32 * 0.16,
            "the incoming cue should be nearing the stable focal anchor by midpoint: focal={settled_focal:?}, incoming={incoming_bounds:?}, reel_height={}",
            rendered.reel.height()
        );

        assert!(!STYLES.contains("lyric-promoting-out"));
        assert!(!STYLES.contains("lyric-promoting-in"));

        let mut wrapped = adjacent.clone();
        wrapped.current_index += 1;
        wrapped.previous = Some(adjacent.current.clone());
        wrapped.current =
            "We find the signal where the last blue horizon meets the dark beyond us".to_owned();
        let wrapped_lines = rendered.rendered_line_count(&wrapped.current);
        assert!(
            wrapped_lines >= 3,
            "the test cue must wrap through Pango rather than source newlines"
        );
        rendered.motion.borrow_mut().update(
            0,
            Some(&wrapped),
            wrapped_lines,
            std::time::Duration::from_secs(1),
            true,
        );
        assert_height_aware_cues_remain_separate(
            &rendered,
            &layout,
            std::time::Duration::from_secs(1),
        );

        let settled_at = std::time::Duration::from_millis(1_620);
        rendered
            .motion
            .borrow_mut()
            .reconcile_rendered_lines(wrapped_lines, settled_at);
        let mut short = wrapped.clone();
        short.current_index += 1;
        short.previous = Some(wrapped.current.clone());
        short.current = "Short destination".to_owned();
        short.next = Some("After".to_owned());
        rendered.motion.borrow_mut().update(
            0,
            Some(&short),
            1,
            std::time::Duration::from_secs(2),
            true,
        );
        assert_height_aware_cues_remain_separate(
            &rendered,
            &layout,
            std::time::Duration::from_secs(2),
        );

        let mut tall_source = initial.clone();
        tall_source.current_index = 20;
        tall_source.current = "The quiet horizon turns until the room remembers".to_owned();
        tall_source.next = Some(
            "We find the signal where the last blue horizon meets the dark beyond us".to_owned(),
        );
        let tall_rendered = lyric_view(
            &presentation,
            Some(&tall_source),
            PresentationPalette::fallback(),
            PresentationBehavior::Dynamic,
        );
        let tall_layout =
            NowPlayingLayout::for_presentation(&presentation, Viewport::new(1_600, 1_200));
        allocate_lyrics(&tall_rendered, &tall_layout);
        let tall_source_lines = tall_rendered.rendered_line_count(&tall_source.current);
        assert_eq!(tall_source_lines, 3);
        tall_rendered
            .motion
            .borrow_mut()
            .reconcile_rendered_lines(tall_source_lines, std::time::Duration::ZERO);
        let mut tall_target = tall_source.clone();
        tall_target.current_index += 1;
        tall_target.previous = Some(tall_source.current.clone());
        tall_target.current = tall_source.next.clone().expect("tall target should exist");
        tall_target.next = Some("The signal returns".to_owned());
        let tall_target_lines = tall_rendered.rendered_line_count(&tall_target.current);
        assert_eq!(tall_target_lines, 4);
        let three_line_layout =
            NowPlayingLayout::for_presentation(&presentation, Viewport::new(1_600, 900));
        assert_eq!(
            tall_rendered.rendered_line_count_at(&tall_source.current, &three_line_layout),
            2
        );
        assert_eq!(
            tall_rendered.rendered_line_count_at(&tall_target.current, &three_line_layout),
            3
        );
        tall_rendered.motion.borrow_mut().update(
            0,
            Some(&tall_target),
            tall_target_lines,
            std::time::Duration::from_secs(3),
            true,
        );
        assert_height_aware_cues_remain_separate(
            &tall_rendered,
            &tall_layout,
            std::time::Duration::from_secs(3),
        );

        let ordinary = lyric_presentation("playing.json");
        let mut entering = ordinary.clone();
        entering.lyrics = lyric_presentation("lyrics-one-line.json").lyrics;
        let viewport = Viewport::new(1_280, 720);
        let caches = PresentationCaches::new(PRESENTATION_CACHE_CAPACITY);
        let mut rendered = super::now_playing(
            &ordinary,
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
            PresentationPalette::fallback(),
            PresentationLayoutSource::for_presentation(&Presentation::NowPlaying(ordinary.clone())),
            RenderingConfiguration::live(
                roonscape_renderer::select_typography(&std::collections::HashSet::new()),
                PresentationBehavior::Dynamic,
            ),
            None,
            caches,
        );
        rendered.apply_viewport(viewport);
        rendered.root.allocate(
            viewport.width_px as i32,
            viewport.height_px as i32,
            -1,
            None,
        );
        while gtk::glib::MainContext::default().iteration(false) {}
        let now_playing = rendered
            .now_playing
            .as_ref()
            .expect("ordinary state should render Now Playing");
        let artwork = now_playing.artwork.surface.as_ptr();
        let status = now_playing.metadata.presentation_status.root.as_ptr();
        let footer = now_playing.metadata.footer.as_ptr();
        let identity = now_playing.metadata.identity.root.as_ptr();
        let ordinary_before = now_playing
            .metadata
            .ordinary_metadata
            .compute_bounds(&rendered.root)
            .expect("ordinary metadata should have presentation-relative bounds");

        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(entering.clone()),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(entering.clone()),
            std::time::Duration::from_millis(290),
            Some(viewport),
        );
        rendered.root.allocate(
            viewport.width_px as i32,
            viewport.height_px as i32,
            -1,
            None,
        );
        while gtk::glib::MainContext::default().iteration(false) {}

        let now_playing = rendered
            .now_playing
            .as_ref()
            .expect("lyric state should retain Now Playing");
        assert_eq!(now_playing.artwork.surface.as_ptr(), artwork);
        assert_eq!(
            now_playing.metadata.presentation_status.root.as_ptr(),
            status
        );
        assert_eq!(now_playing.metadata.footer.as_ptr(), footer);
        assert_eq!(now_playing.metadata.identity.root.as_ptr(), identity);
        assert!(now_playing.metadata.lyrics.reel_region.opacity() > 0.8);
        assert!(now_playing.metadata.lyrics.masthead.opacity() > 0.5);
        assert!(now_playing.metadata.ordinary_metadata.opacity() > 0.0);
        assert!(
            now_playing.metadata.ordinary_metadata.opacity() <= 0.12,
            "the oversized ordinary Title must substantially relinquish ownership by midpoint"
        );
        assert!(
            now_playing.metadata.ordinary_metadata.opacity()
                * now_playing.metadata.lyrics.masthead.opacity()
                <= 0.1,
            "ordinary and compact Title/Artist groups must not compete perceptually"
        );
        assert_eq!(now_playing.artwork.surface.opacity(), 1.0);
        let ordinary_midpoint = now_playing
            .metadata
            .ordinary_metadata
            .compute_bounds(&rendered.root)
            .expect("retiring ordinary metadata should retain bounds");
        let lyrics_midpoint = now_playing
            .metadata
            .lyrics
            .root
            .compute_bounds(&rendered.root)
            .expect("entering lyric composition should have bounds");
        let focal_font_size = now_playing
            .metadata
            .lyrics
            .typography
            .get()
            .lyric_current_px as f32;
        assert!(
            ordinary_before.y() - ordinary_midpoint.y() >= focal_font_size * 1.4,
            "ordinary copy should clear the focal lyric region by midpoint: before={ordinary_before:?}, midpoint={ordinary_midpoint:?}, focal_font_size={focal_font_size}"
        );

        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(entering),
            std::time::Duration::from_millis(580),
            Some(viewport),
        );
        rendered.root.allocate(
            viewport.width_px as i32,
            viewport.height_px as i32,
            -1,
            None,
        );
        while gtk::glib::MainContext::default().iteration(false) {}
        let lyrics_settled = rendered
            .now_playing
            .as_ref()
            .expect("settled lyric state should retain Now Playing")
            .metadata
            .lyrics
            .root
            .compute_bounds(&rendered.root)
            .expect("settled lyric composition should have bounds");
        assert!(
            lyrics_midpoint.y() - lyrics_settled.y() <= focal_font_size * 0.2,
            "the lyric composition should be near its destination by midpoint: midpoint={lyrics_midpoint:?}, settled={lyrics_settled:?}, focal_font_size={focal_font_size}"
        );

        // Drive semantic causes through the complete presentation-update boundary. The
        // lower-level motion checks above remain focused on allocated native geometry.
        let initial_presentation = lyric_presentation("lyrics-one-line.json");
        let initial_lyrics = initial_presentation
            .lyrics
            .as_deref()
            .expect("fixture should begin inside the lyric composition")
            .clone();
        let mut complete =
            rendered_now_playing(&initial_presentation, PresentationBehavior::Dynamic);
        complete.apply_viewport(viewport);
        complete.update_in_place(
            40,
            &Presentation::NowPlaying(initial_presentation.clone()),
            std::time::Duration::ZERO,
            Some(viewport),
        );

        let mut natural_lyrics = initial_lyrics.clone();
        natural_lyrics.current_index += 1;
        natural_lyrics.previous = Some(initial_lyrics.current.clone());
        natural_lyrics.current = initial_lyrics
            .next
            .clone()
            .expect("fixture should have an adjacent destination");
        natural_lyrics.next = Some("A third complete-state cue".to_owned());
        let mut natural = initial_presentation.clone();
        natural.lyrics = Some(Box::new(natural_lyrics.clone()));
        complete.update_in_place(
            40,
            &Presentation::NowPlaying(natural.clone()),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        assert_eq!(
            lyric_motion_frame(&complete, std::time::Duration::ZERO).cause,
            LyricMotionCause::NaturalCueHandoff {
                height_aware: false
            }
        );

        let mut paused = natural.clone();
        paused.status = lyric_presentation("lyrics-blank-cue.json").status;
        complete.update_in_place(
            41,
            &Presentation::NowPlaying(paused.clone()),
            std::time::Duration::from_millis(100),
            Some(viewport),
        );
        assert!(
            lyric_motion_frame(&complete, std::time::Duration::from_millis(100)).cue_motion_active,
            "a pause update should let the selected Natural Cue Handoff settle"
        );
        complete.update_in_place(
            41,
            &Presentation::NowPlaying(paused),
            std::time::Duration::from_millis(620),
            Some(viewport),
        );
        assert!(
            !lyric_motion_frame(&complete, std::time::Duration::from_millis(620)).cue_motion_active
        );
        assert_rendered_lyric_roles(
            &complete,
            natural_lyrics.previous.as_deref(),
            Some(&natural_lyrics.current),
            natural_lyrics.next.as_deref(),
        );
        assert_rendered_composition_ownership(&complete, 0.0, 1.0, 1.0);

        let mut seek_lyrics = natural_lyrics.clone();
        seek_lyrics.current_index += 1;
        seek_lyrics.previous = Some(natural_lyrics.current.clone());
        seek_lyrics.current = "External seek destination".to_owned();
        seek_lyrics.next = Some("Timeline revision source".to_owned());
        let mut seek = natural.clone();
        seek.lyrics = Some(Box::new(seek_lyrics.clone()));
        complete.update_in_place(
            42,
            &Presentation::NowPlaying(seek.clone()),
            std::time::Duration::from_secs(1),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&complete, std::time::Duration::from_secs(1));
        assert_eq!(frame.cause, LyricMotionCause::ExternalSeek);
        assert!(!frame.cue_motion_active);
        assert_rendered_lyric_roles(
            &complete,
            seek_lyrics.previous.as_deref(),
            Some(&seek_lyrics.current),
            seek_lyrics.next.as_deref(),
        );

        let mut revised_lyrics = seek_lyrics.clone();
        revised_lyrics.timeline_signature = lyric_presentation("lyrics-revision-after.json")
            .lyrics
            .expect("revision fixture should have lyrics")
            .timeline_signature;
        revised_lyrics.current_index += 1;
        revised_lyrics.previous = Some("Corrected previous cue".to_owned());
        revised_lyrics.current = "Corrected selected cue".to_owned();
        revised_lyrics.next = Some("After correction".to_owned());
        let mut revised = seek.clone();
        revised.lyrics = Some(Box::new(revised_lyrics.clone()));
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(revised.clone()),
            std::time::Duration::from_millis(1_200),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&complete, std::time::Duration::from_millis(1_200));
        assert_eq!(frame.cause, LyricMotionCause::TimelineRevision);
        assert!(!frame.cue_motion_active);
        assert_rendered_lyric_roles(
            &complete,
            revised_lyrics.previous.as_deref(),
            Some(&revised_lyrics.current),
            revised_lyrics.next.as_deref(),
        );

        let mut handoff_lyrics = revised_lyrics.clone();
        handoff_lyrics.current_index += 1;
        handoff_lyrics.previous = Some(revised_lyrics.current.clone());
        handoff_lyrics.current = "Handoff destination".to_owned();
        handoff_lyrics.next = Some("Interruption destination".to_owned());
        let mut handoff = revised.clone();
        handoff.lyrics = Some(Box::new(handoff_lyrics.clone()));
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(handoff.clone()),
            std::time::Duration::from_secs(2),
            Some(viewport),
        );
        assert!(lyric_motion_frame(&complete, std::time::Duration::from_secs(2)).cue_motion_active);

        let mut interrupted_lyrics = handoff_lyrics.clone();
        interrupted_lyrics.current_index += 1;
        interrupted_lyrics.previous = Some(handoff_lyrics.current.clone());
        interrupted_lyrics.current = "Interruption destination".to_owned();
        interrupted_lyrics.next = Some("Before Intentional Blank".to_owned());
        let mut interrupted = handoff.clone();
        interrupted.lyrics = Some(Box::new(interrupted_lyrics.clone()));
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(interrupted.clone()),
            std::time::Duration::from_millis(2_100),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&complete, std::time::Duration::from_millis(2_100));
        assert_eq!(frame.cause, LyricMotionCause::InterruptedHandoffDestination);
        assert!(!frame.cue_motion_active);
        assert_rendered_lyric_roles(
            &complete,
            interrupted_lyrics.previous.as_deref(),
            Some(&interrupted_lyrics.current),
            interrupted_lyrics.next.as_deref(),
        );

        let mut blank_lyrics = interrupted_lyrics.clone();
        blank_lyrics.current_index += 1;
        blank_lyrics.previous = None;
        blank_lyrics.current.clear();
        blank_lyrics.next = None;
        let mut blank = interrupted.clone();
        blank.lyrics = Some(Box::new(blank_lyrics.clone()));
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(blank.clone()),
            std::time::Duration::from_secs(3),
            Some(viewport),
        );
        assert_eq!(
            lyric_motion_frame(&complete, std::time::Duration::from_secs(3)).cause,
            LyricMotionCause::IntentionalBlankEntry
        );

        let mut next_blank_lyrics = blank_lyrics.clone();
        next_blank_lyrics.current_index += 1;
        next_blank_lyrics.current = " ".to_owned();
        let mut next_blank = blank.clone();
        next_blank.lyrics = Some(Box::new(next_blank_lyrics));
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(next_blank.clone()),
            std::time::Duration::from_millis(3_100),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&complete, std::time::Duration::from_millis(3_100));
        assert_eq!(frame.cause, LyricMotionCause::IntentionalBlankContinuation);
        assert!(frame.cue_motion_active);
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(next_blank.clone()),
            std::time::Duration::from_millis(3_440),
            Some(viewport),
        );
        assert_rendered_lyric_roles(&complete, None, None, None);
        assert_rendered_composition_ownership(&complete, 0.0, 1.0, 1.0);

        let mut no_lyrics = next_blank.clone();
        no_lyrics.lyrics = None;
        complete.update_in_place(
            44,
            &Presentation::NowPlaying(no_lyrics.clone()),
            std::time::Duration::from_secs(4),
            Some(viewport),
        );
        assert_eq!(
            lyric_motion_frame(&complete, std::time::Duration::from_secs(4)).cause,
            LyricMotionCause::CompositionExit
        );
        complete.update_in_place(
            44,
            &Presentation::NowPlaying(no_lyrics),
            std::time::Duration::from_millis(4_580),
            Some(viewport),
        );
        assert_rendered_composition_ownership(&complete, 1.0, 0.0, 0.0);
        complete.update_in_place(
            45,
            &Presentation::NowPlaying(next_blank.clone()),
            std::time::Duration::from_millis(4_700),
            Some(viewport),
        );
        assert_eq!(
            lyric_motion_frame(&complete, std::time::Duration::from_millis(4_700)).cause,
            LyricMotionCause::CompositionEntry
        );
        complete.update_in_place(
            45,
            &Presentation::NowPlaying(next_blank),
            std::time::Duration::from_millis(5_280),
            Some(viewport),
        );
        assert_rendered_lyric_roles(&complete, None, None, None);
        assert_rendered_composition_ownership(&complete, 0.0, 1.0, 1.0);

        let mut reduced =
            rendered_now_playing(&initial_presentation, PresentationBehavior::StaticFixture);
        reduced.apply_viewport(viewport);
        reduced.update_in_place(
            50,
            &Presentation::NowPlaying(initial_presentation),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        reduced.update_in_place(
            50,
            &Presentation::NowPlaying(natural),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        let frame = lyric_motion_frame(&reduced, std::time::Duration::ZERO);
        assert_eq!(
            frame.cause,
            LyricMotionCause::NaturalCueHandoff {
                height_aware: false
            }
        );
        assert!(!frame.cue_motion_active);
        assert_rendered_lyric_roles(
            &reduced,
            natural_lyrics.previous.as_deref(),
            Some(&natural_lyrics.current),
            natural_lyrics.next.as_deref(),
        );
        assert_rendered_composition_ownership(&reduced, 0.0, 1.0, 1.0);
    }

    fn blank_promotion_preserves_context_and_an_interrupted_departure() {
        let before = lyric_presentation("lyrics-one-line.json");
        let mut blank = before.clone();
        let cue = blank.lyrics.as_mut().unwrap();
        cue.current_index += 1;
        cue.previous = Some(cue.current.clone());
        cue.current.clear();
        let viewport = Viewport::new(1280, 720);
        let mut rendered = rendered_now_playing(&before, PresentationBehavior::Dynamic);
        rendered.apply_viewport(viewport);
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(before),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(blank.clone()),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        let at = std::time::Duration::from_millis(100);
        let departing = lyric_motion_frame(&rendered, at);
        assert!(
            departing.cues.iter().any(|cue| cue.color_to
                == crate::lyric_motion::LyricColorRole::Next
                && cue.opacity == 1.0),
            "blank entry should retain anticipation"
        );
        let mut after = blank;
        let cue = after.lyrics.as_mut().unwrap();
        cue.current_index += 1;
        cue.current = cue.next.take().unwrap();
        rendered.update_in_place(1, &Presentation::NowPlaying(after), at, Some(viewport));
        let promoting = lyric_motion_frame(&rendered, at);
        assert!(
            promoting.cue_motion_active,
            "a short blank must not force a cut"
        );
        assert_eq!(promoting.cause, LyricMotionCause::IntentionalBlankExit);
        let outgoing = |frame: &crate::lyric_motion::LyricFrame| {
            frame
                .cues
                .iter()
                .find(|cue| cue.slot == crate::lyric_motion::LyricCueSlot::Previous)
                .unwrap()
                .clone()
        };
        assert_eq!(outgoing(&departing).position, outgoing(&promoting).position);
        assert_eq!(outgoing(&departing).emphasis, outgoing(&promoting).emphasis);
    }

    fn tall_departure_keeps_focal_size_and_reveals_separate_memory() {
        let mut before = lyric_presentation("lyrics-one-line.json");
        let cue = before.lyrics.as_mut().unwrap();
        cue.current = "First line\nSecond line\nThird line".into();
        cue.next = Some("After".into());
        let viewport = Viewport::new(1280, 720);
        let mut rendered = rendered_now_playing(&before, PresentationBehavior::Dynamic);
        rendered.apply_viewport(viewport);
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(before.clone()),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        let mut after = before.clone();
        let cue = after.lyrics.as_mut().unwrap();
        cue.current_index += 1;
        cue.previous = Some(cue.current.clone());
        cue.current = "After".into();
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(after),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        let departure = lyric_motion_frame(&rendered, std::time::Duration::from_millis(100));
        let outgoing = departure
            .cues
            .iter()
            .find(|cue| cue.text == before.lyrics.as_ref().unwrap().current)
            .unwrap();
        assert_eq!(
            outgoing.emphasis, 1.0,
            "tall departure must not compress toward memory"
        );
        let arrival = lyric_motion_frame(&rendered, std::time::Duration::from_millis(550));
        let memory = arrival
            .cues
            .iter()
            .find(|cue| cue.text == before.lyrics.as_ref().unwrap().current)
            .unwrap();
        assert_eq!(memory.emphasis, 0.0);
        assert!(
            memory.opacity > 0.5,
            "context should return gently before settlement"
        );
    }

    fn a_seek_within_the_incoming_cue_settles_its_handoff() {
        let mut before = lyric_presentation("lyrics-one-line.json");
        before.playback_position_seconds = Some(171.0);
        let viewport = Viewport::new(1280, 720);
        let mut rendered = rendered_now_playing(&before, PresentationBehavior::Dynamic);
        rendered.apply_viewport(viewport);
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(before.clone()),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        let mut incoming = before;
        incoming.playback_position_seconds = Some(171.1);
        let cue = incoming.lyrics.as_mut().unwrap();
        cue.current_index += 1;
        cue.previous = Some(cue.current.clone());
        cue.current = cue.next.take().unwrap();
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(incoming.clone()),
            std::time::Duration::from_millis(100),
            Some(viewport),
        );
        assert!(
            lyric_motion_frame(&rendered, std::time::Duration::from_millis(200)).cue_motion_active
        );
        // A continuous source refresh must not interrupt the lift.
        incoming.playback_position_seconds = Some(171.18);
        rendered.update_in_place(
            2,
            &Presentation::NowPlaying(incoming.clone()),
            std::time::Duration::from_millis(200),
            Some(viewport),
        );
        assert!(
            lyric_motion_frame(&rendered, std::time::Duration::from_millis(200)).cue_motion_active
        );
        // This seek retains exactly the same cue and destination neighbors.
        incoming.playback_position_seconds = Some(176.0);
        rendered.update_in_place(
            3,
            &Presentation::NowPlaying(incoming),
            std::time::Duration::from_millis(250),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&rendered, std::time::Duration::from_millis(250));
        assert!(
            !frame.cue_motion_active,
            "same-cue seeks install a complete endpoint"
        );
        assert_eq!(frame.cause, LyricMotionCause::ExternalSeek);
    }

    fn short_blanks_promote_without_a_skipped_cue_cut() {
        let mut snapshot =
            parse_snapshot(include_str!("../../shared/fixtures/lyrics-one-line.json")).unwrap();
        snapshot.lyrics = Some(roonscape_renderer::SynchronizedLyrics {
            cues: vec![
                roonscape_renderer::LyricCue {
                    at_seconds: 0.0,
                    text: "Before".into(),
                },
                roonscape_renderer::LyricCue {
                    at_seconds: 10.0,
                    text: "".into(),
                },
                roonscape_renderer::LyricCue {
                    at_seconds: 10.3,
                    text: "After".into(),
                },
            ],
        });
        snapshot
            .timing
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .seconds = 9.4;
        let Presentation::NowPlaying(before) = presentation_from_snapshot(&snapshot).unwrap()
        else {
            panic!("Now Playing");
        };
        let viewport = Viewport::new(1280, 720);
        let mut rendered = rendered_now_playing(&before, PresentationBehavior::Dynamic);
        rendered.apply_viewport(viewport);
        rendered.update_in_place(
            1,
            &Presentation::NowPlaying(before),
            std::time::Duration::ZERO,
            Some(viewport),
        );
        snapshot
            .timing
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .seconds = 9.7;
        let after = presentation_from_snapshot(&snapshot).unwrap();
        rendered.update_in_place(
            1,
            &after,
            std::time::Duration::from_millis(300),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&rendered, std::time::Duration::from_millis(300));
        assert!(
            frame.cue_motion_active,
            "short blanks preserve advance promotion: {frame:?}"
        );
        assert_eq!(
            frame.cause,
            LyricMotionCause::NaturalCueHandoff {
                height_aware: false
            }
        );
    }

    #[test]
    fn composition_ownership_crosses_continuously_without_doubling_the_masthead() {
        let mut saw_copy_overlap = false;
        let mut previous = composition_ownership(0.0);
        for step in 0..=100 {
            let progress = f64::from(step) / 100.0;
            let (ordinary, reel, masthead) = composition_ownership(progress);
            assert!(
                ordinary.max(reel) >= 0.4,
                "one content group must retain clear ownership at progress {progress}: ordinary={ordinary}, reel={reel}"
            );
            assert!(masthead <= reel);
            if ordinary > 0.0 && reel > 0.0 {
                saw_copy_overlap = true;
            }
            if step > 0 {
                let current = (ordinary, reel, masthead);
                assert!(
                    (ordinary - previous.0).abs() <= 0.1
                        && (reel - previous.1).abs() <= 0.1
                        && (masthead - previous.2).abs() <= 0.1,
                    "composition ownership must not cut between adjacent frames: previous={previous:?}, current={current:?}"
                );
            }
            previous = (ordinary, reel, masthead);
        }
        assert!(
            saw_copy_overlap,
            "copy groups should cross through a short overlap"
        );

        assert_eq!(composition_ownership(0.0), (1.0, 0.0, 0.0));
        assert_eq!(composition_ownership(1.0), (0.0, 1.0, 1.0));
        let (ordinary, reel, masthead) = composition_ownership(0.5);
        assert!(reel > ordinary, "the lyric reel should own the midpoint");
        assert!(
            masthead < reel,
            "the masthead should wait until ordinary Title/Artist copy recedes"
        );
    }

    fn populate_presentation_caches(
        caches: &PresentationCaches,
    ) -> (gdk_pixbuf::Pixbuf, Arc<[u8]>) {
        let artwork = caches
            .artwork
            .source(&crate::artwork_cache::ArtworkCacheKey::new(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../shared/fixtures/artwork/playing.svg"),
                Some(3),
            ))
            .expect("Playing artwork should decode");
        let gradient = caches.gradients.raster(
            PresentationPalette::fallback(),
            NowPlayingGradientCacheKey::new(Viewport::new(16, 9), 1),
        );
        (artwork, gradient)
    }

    #[test]
    fn live_mode_reuses_artwork_and_gradient_caches_across_replacements() {
        let mut current = PresentationCaches::new(PRESENTATION_CACHE_CAPACITY);
        let rendering = RenderingConfiguration::live(
            roonscape_renderer::select_typography(&HashSet::new()),
            PresentationBehavior::Dynamic,
        );

        let (first_artwork, first_gradient) = rendering
            .cache_scope
            .render_replacement(&mut current, populate_presentation_caches);
        let (reused_artwork, reused_gradient) = rendering
            .cache_scope
            .render_replacement(&mut current, populate_presentation_caches);

        assert_eq!(first_artwork.as_ptr(), reused_artwork.as_ptr());
        assert!(Arc::ptr_eq(&first_gradient, &reused_gradient));
    }

    #[test]
    fn fixture_mode_and_presentation_capture_render_replacements_with_fresh_caches() {
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        for rendering in [
            RenderingConfiguration::fixture(typography, PresentationBehavior::Dynamic),
            RenderingConfiguration::capture(typography, PresentationBehavior::StaticFixture),
        ] {
            let mut current = PresentationCaches::new(PRESENTATION_CACHE_CAPACITY);

            let (first_artwork, first_gradient) = rendering
                .cache_scope
                .render_replacement(&mut current, populate_presentation_caches);
            let (fresh_artwork, fresh_gradient) = rendering
                .cache_scope
                .render_replacement(&mut current, populate_presentation_caches);

            assert_ne!(first_artwork.as_ptr(), fresh_artwork.as_ptr());
            assert!(!Arc::ptr_eq(&first_gradient, &fresh_gradient));
        }
    }

    #[test]
    fn keeps_the_current_footer_geometry_when_the_viewport_changes() {
        let snapshot = parse_snapshot(include_str!("../../shared/fixtures/playing.json"))
            .expect("Playing fixture should satisfy the shared contract");
        let presentation = presentation_from_snapshot(&snapshot)
            .expect("Playing fixture should produce a presentation");
        let layout = PresentationLayoutSource::for_presentation(&presentation)
            .now_playing(Viewport::new(3_840, 2_160), 1.0)
            .expect("Playing should retain a Now Playing layout");

        assert_eq!(
            layout.footer_content,
            NowPlayingFooterContent::DeterminateProgress,
        );
        assert_eq!(
            layout.metadata_region_bottom_viewport_y_px,
            layout.footer_anchor.bottom_viewport_y_px - layout.footer_height_px,
        );
    }

    #[test]
    fn removes_status_decoration_only_from_the_now_playing_circle_free_cell() {
        assert!(STYLES.contains(
            ".now-playing .status-symbol-circle-free {\n  border: 0;\n  border-radius: 0;\n  background-color: transparent;\n}"
        ));
        assert!(STYLES.contains(".status-symbol-container"));
        assert!(STYLES.contains("border-radius: 999px;"));
    }

    #[test]
    fn gives_the_determinate_rail_square_noninteractive_layers() {
        assert!(STYLES.contains(".progress-track"));
        assert!(STYLES.contains("progressbar.progress-fill trough"));
        assert!(STYLES.contains("background-color: transparent;"));
        assert!(STYLES.contains("border-radius: 0;"));
        assert!(!STYLES.contains("min-width: 4px;"));
    }
}
