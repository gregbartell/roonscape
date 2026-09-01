use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFit,
    ArtworkLayout, FullFieldFontSize, FullFieldLayout, FullFieldLineLayout, FullFieldPresentation,
    IdentityLineLayout, IdentityPhraseAlignment, IdentityPlacement, IdentityRowLayout,
    InactivityLayout, InactivityTransform, MetadataGroupPlan, MetadataLayout, MetadataLineLayout,
    MetadataTypography, NowPlayingField, NowPlayingFooterContent, NowPlayingLayout,
    NowPlayingPresentation, NowPlayingRole, Presentation, PresentationActivity,
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

    fn now_playing(&self, viewport: Viewport) -> Option<NowPlayingLayout> {
        let Self::NowPlaying(presentation) = self else {
            return None;
        };
        Some(NowPlayingLayout::for_presentation(presentation, viewport))
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
    presentation_status: RenderedPresentationStatus,
    musical_metadata_slot: gtk::ScrolledWindow,
    title: Option<RenderedMetadataLine>,
    artist: Option<RenderedMetadataLine>,
    album: Option<RenderedMetadataLine>,
    progress: Option<RenderedProgress>,
    activity: Option<RenderedActivity>,
    footer: gtk::Box,
    identity: RenderedIdentity,
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

    pub(crate) fn update_progress(&self, progress: &PresentationProgress) {
        if let Some(rendered_progress) = self.transition.current().value().progress.as_ref() {
            rendered_progress.update(progress);
        }
    }

    pub(crate) fn update_in_place(&mut self, revision: u64, presentation: &Presentation) {
        self.transition.update_current(revision, |current| {
            current.update_in_place(presentation);
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

    fn update_in_place(&mut self, presentation: &Presentation) {
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
        if let (Some(now_playing), Some(layout)) = (
            self.now_playing.as_ref(),
            self.layout_source.now_playing(viewport),
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

    let metadata = metadata(presentation, &layout, rendering);
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

    let musical_metadata = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    footer.add_css_class("utility-footer");
    footer.set_hexpand(true);

    for role in &now_playing_layout.metadata_roles {
        match role {
            NowPlayingRole::PresentationStatus => {}
            NowPlayingRole::Title => {
                musical_metadata.append(&title.as_ref().expect("Title role requires a label").label)
            }
            NowPlayingRole::Artist => musical_metadata
                .append(&artist.as_ref().expect("Artist role requires a label").label),
            NowPlayingRole::Album => {
                musical_metadata.append(&album.as_ref().expect("Album role requires a label").label)
            }
            NowPlayingRole::Progress | NowPlayingRole::Activity => {}
        }
    }

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
    RenderedMetadata {
        root,
        copy,
        musical_metadata_alignment,
        presentation_status: rendered_status,
        musical_metadata_slot,
        title,
        artist,
        album,
        progress,
        activity,
        footer,
        identity,
    }
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
        self.apply_group_fitting(layout);
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
        self.musical_metadata_alignment
            .set_margin_top(dimension(layout.metadata_group_offset_px(plan.height_px)));
        self.musical_metadata_slot
            .set_height_request(dimension(plan.height_px));
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
    set_label_font_size(label, plan.font_size_px);
    label
        .layout()
        .set_line_spacing(plan.line_height_percent as f32 / 100.0);
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
        "{STYLES}\n{}",
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::Arc;

    use gtk::glib::object::ObjectType;

    use super::{
        PRESENTATION_CACHE_CAPACITY, PresentationCaches, PresentationLayoutSource,
        RenderingConfiguration, STYLES,
    };
    use roonscape_renderer::{
        NowPlayingFooterContent, NowPlayingGradientCacheKey, PresentationBehavior,
        PresentationPalette, Viewport, parse_snapshot, presentation_from_snapshot,
    };

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
            .now_playing(Viewport::new(3_840, 2_160))
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
