use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkLayout, FullFieldFontSize,
    FullFieldLayout, FullFieldPresentation, IdentityRowLayout, InactivityLayout,
    InactivityTransform, LyricPresentation, MetadataGroupPlan, MetadataLayout, MetadataLineLayout,
    MetadataTypography, NowPlayingFooterContent, NowPlayingLayout, NowPlayingPresentation,
    NowPlayingRole, Presentation, PresentationActivity, PresentationBehavior, PresentationPalette,
    PresentationProgress, PresentationRevision, PresentationStatus, PresentationStatusEmphasis,
    PresentationStatusLayout, PresentationStyleLayer, PresentationTransition,
    PresentationTransitionStyles, ReplacementFade, ResolvedPresentation, TypographySelection,
    TypographyStyles, Viewport, metadata_layout, resolve_capture_presentation,
    resolve_presentation,
};

use crate::activity_waveform::activity_waveform;
use crate::artwork_cache::{ArtworkCache, ArtworkCacheKey};
use crate::gradient_cache::{
    CachedNowPlayingGradient, NowPlayingGradientCache, PreparedNowPlayingGradient,
    RenderedNowPlayingGradient,
};
use crate::lyric_motion::{LyricFrame, LyricMotion};
use crate::lyric_reel::LyricReel;
use crate::status_symbol::presentation_status_symbol;

const STYLES: &str = include_str!("style.css");
const PRESENTATION_CACHE_CAPACITY: usize = 2;

pub(crate) struct PresentationView {
    root: gtk::Overlay,
    layers: gtk::Overlay,
    transition: PresentationTransition<RenderedPresentation>,
    palette_provider: gtk::CssProvider,
    rendering: RenderingConfiguration,
    display_viewport: Viewport,
    layout_viewport: Option<Viewport>,
    inactivity: InactivityTransform,
    transition_clock: Instant,
    outgoing_text_opacity: f64,
    retained_layers: Vec<PresentationRevision<RenderedPresentation>>,
    caches: PresentationCaches,
    artwork_source: Option<(String, Option<u64>)>,
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
    metadata_fade: ReplacementFade<MetadataContent>,
    typography: TypographySelection,
    title: Option<RenderedMetadataLine>,
    artist: Option<RenderedMetadataLine>,
    album: Option<RenderedMetadataLine>,
    lyrics: Rc<RenderedLyrics>,
    progress: Option<RenderedProgress>,
    activity: Option<RenderedActivity>,
    timing_slot: gtk::Overlay,
    timing_fade: ReplacementFade<TimingContent>,
    footer: gtk::Box,
    identity: RenderedIdentity,
}

#[derive(PartialEq)]
struct MetadataContent {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

impl MetadataContent {
    fn for_presentation(presentation: &NowPlayingPresentation) -> Self {
        Self {
            title: presentation.title.clone(),
            artist: presentation.artist.clone(),
            album: presentation.album.clone(),
        }
    }

    fn apply_to(&self, presentation: &mut NowPlayingPresentation) {
        presentation.title.clone_from(&self.title);
        presentation.artist.clone_from(&self.artist);
        presentation.album.clone_from(&self.album);
    }
}

#[derive(PartialEq)]
enum TimingContent {
    Quiet,
    Progress,
    Activity(Box<PresentationActivity>),
}

impl TimingContent {
    fn for_presentation(presentation: &NowPlayingPresentation) -> Self {
        if presentation.progress.is_some() {
            Self::Progress
        } else if let Some(activity) = &presentation.activity {
            Self::Activity(activity.clone())
        } else {
            Self::Quiet
        }
    }
}

struct RenderedLyrics {
    root: gtk::Box,
    masthead: gtk::Box,
    masthead_title: gtk::Label,
    masthead_artist: gtk::Label,
    reel_region: gtk::ScrolledWindow,
    reel: Rc<LyricReel>,
    cue_width_px: Cell<i32>,
    cue_height_px: Cell<i32>,
    typography: Cell<roonscape_renderer::NowPlayingTypography>,
    motion: RefCell<LyricMotion>,
    rendered_at: Cell<Duration>,
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
    visuals: RenderedArtworkLayers,
    content: gtk::Box,
    artwork_column: gtk::CenterBox,
    metadata_slot: gtk::Box,
    metadata: RenderedMetadata,
}

struct RenderedArtworkLayer {
    root: gtk::Overlay,
    background: RenderedNowPlayingBackground,
    content: gtk::Box,
    artwork_column: gtk::CenterBox,
    artwork: RenderedArtwork,
    palette: PresentationPalette,
}

struct RenderedArtworkLayers {
    root: gtk::Overlay,
    transition: PresentationTransition<RenderedArtworkLayer>,
    retained: Vec<PresentationRevision<RenderedArtworkLayer>>,
    from_palette: PresentationPalette,
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
    fade: ReplacementFade<PresentationStatus>,
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
            .add_css_class(&PresentationStyleLayer::Current.class_name());
        let transition = PresentationTransition::new(revision, rendered);
        let layers = gtk::Overlay::new();
        layers.set_hexpand(true);
        layers.set_vexpand(true);
        layers.add_overlay(&transition.current().value().root);
        let root = gtk::Overlay::new();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_child(Some(&layers));

        let mut view = Self {
            root,
            layers,
            transition,
            palette_provider,
            rendering,
            display_viewport: initial_viewport,
            layout_viewport: None,
            inactivity: InactivityTransform::default(),
            transition_clock: Instant::now(),
            outgoing_text_opacity: 1.0,
            retained_layers: Vec::new(),
            caches,
            artwork_source: artwork_source(presentation),
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
        for layer in &self.retained_layers {
            layer.value().apply_viewport(layout.content_viewport);
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
        self.artwork_source = artwork_source(presentation);
        if matches!(presentation, Presentation::NowPlaying(_)) {
            // Retiring layers keep their geometry and artwork/metadata fades,
            // but no previous-song cue may paint in the incoming song's frame.
            for layer in std::iter::once(self.transition.current())
                .chain(self.transition.outgoing())
                .chain(self.retained_layers.iter())
            {
                if let Some(rendered) = &layer.value().now_playing {
                    rendered.metadata.lyrics.reel.widget.set_opacity(0.0);
                }
            }
        }
        if !animations_enabled(self.rendering.behavior) {
            let rendered = self.render_replacement_at_viewport(presentation, repository_root);
            let released = self.transition.replace_immediately(revision, rendered);
            for layer in released {
                self.remove_layer(layer);
            }
            self.release_retained_layers();
            self.reveal_current();
            return;
        }
        let rendered = self.render_replacement_at_viewport(presentation, repository_root);
        rendered.root.set_opacity(0.0);
        rendered.set_text_opacity(if rendered.now_playing.is_some() {
            1.0
        } else {
            0.0
        });
        // Both treatments spend the first 225 ms retiring text. A destination
        // can therefore change kind while invisible without restarting departure.
        let duration = Duration::from_millis(if rendered.now_playing.is_some() {
            450
        } else {
            675
        });
        if self.transition.is_active() && self.transition.current().value().root.opacity() == 0.0 {
            let released = self
                .transition
                .retarget_current(revision, rendered, duration);
            self.remove_layer(released);
            self.reveal_current();
            return;
        }
        self.retain_visible_composite();
        self.outgoing_text_opacity = self.transition.current().value().text_opacity();
        let started_at = self.transition_clock.elapsed();
        let discarded = self
            .transition
            .begin_with_duration(revision, rendered, started_at, duration);
        debug_assert!(discarded.is_none());

        let outgoing = self
            .transition
            .outgoing()
            .expect("a started presentation transition has an outgoing layer");
        outgoing
            .value()
            .root
            .remove_css_class(&PresentationStyleLayer::Current.class_name());
        outgoing
            .value()
            .root
            .add_css_class(&PresentationStyleLayer::Outgoing.class_name());
        self.reveal_current();
    }

    pub(crate) fn finish_transition(&mut self) {
        let now = self.transition_clock.elapsed();
        self.advance_transition(now);
    }

    fn advance_transition(&mut self, now: Duration) {
        let mut palette_changed = false;
        self.transition
            .update_current(self.transition.current().revision(), |current| {
                if let Some(rendered) = &mut current.now_playing
                    && rendered.visuals.transition.is_active()
                {
                    let palette = rendered.visuals.advance(now);
                    current.set_palette(palette);
                    palette_changed = true;
                }
            });
        if palette_changed {
            self.install_palette_styles();
        }
        if let Some(outgoing) = self.transition.outgoing() {
            let progress = self.transition.progress(now);
            let current = self.transition.current().value();
            let departure_end = if current.now_playing.is_some() {
                0.5
            } else {
                1.0 / 3.0
            };
            outgoing.value().set_text_opacity(
                self.outgoing_text_opacity * (1.0 - motion_phase(progress, 0.0, departure_end)),
            );
            // Composite the live incoming widget tree over the retained opaque
            // composition. Fading both would expose the surface underneath.
            if current.now_playing.is_some() {
                current.root.set_opacity(motion_phase(progress, 0.5, 0.5));
            } else {
                current
                    .root
                    .set_opacity(motion_phase(progress, 1.0 / 3.0, 1.0 / 3.0));
                current.set_text_opacity(motion_phase(progress, 2.0 / 3.0, 1.0 / 3.0));
            }
        }
        let Some(outgoing) = self.transition.finish(now) else {
            return;
        };

        self.layers.remove_overlay(&outgoing.value().root);
        self.release_retained_layers();
        self.install_palette_styles();
    }

    // Keep the actual compositing stack and each layer's local transforms and
    // opacity. Promoting only the partially visible target loses the background
    // beneath it; making that target opaque instead exposes an obsolete track.
    fn retain_visible_composite(&mut self) {
        let opaque = self.transition.current().value().root.opacity() == 1.0;
        for layer in self
            .transition
            .retain_visible_composite(opaque, &mut self.retained_layers)
        {
            self.remove_layer(layer);
        }
        for (index, layer) in self.retained_layers.iter().enumerate() {
            set_layer_style(&layer.value().root, PresentationStyleLayer::Retained(index));
        }
    }

    fn release_retained_layers(&mut self) {
        for layer in self.retained_layers.drain(..) {
            self.layers.remove_overlay(&layer.value().root);
        }
    }

    pub(crate) fn update_in_place(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
    ) {
        let now = self.transition_clock.elapsed();
        if self.artwork_source == artwork_source(presentation)
            && let Presentation::NowPlaying(incoming) = presentation
        {
            let current = self.transition.current().value();
            let changes_composition = match &current.now_playing {
                None => incoming.has_usable_metadata(),
                Some(rendered) => {
                    !incoming.has_usable_metadata()
                        && rendered.visuals.current().artwork.source_key.is_none()
                }
            };
            if changes_composition {
                self.replace(revision, presentation, repository_root);
                return;
            }
        }
        if self.artwork_source != artwork_source(presentation) {
            self.update_artwork(revision, presentation, repository_root);
        }
        let viewport = self.layout_viewport;
        self.transition.update_current(revision, |current| {
            current.update_in_place(revision, presentation, now, viewport);
        });
    }

    fn update_artwork(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
    ) {
        let (resolved, capture_error) =
            resolve_for_rendering(presentation, repository_root, self.rendering);
        let Presentation::NowPlaying(incoming) = &resolved.presentation else {
            self.replace(revision, presentation, repository_root);
            return;
        };
        if self.transition.current().value().now_playing.is_none() {
            self.replace(revision, presentation, repository_root);
            return;
        }
        self.artwork_source = artwork_source(presentation);
        let transition_clock = self.transition_clock;
        let animated = animations_enabled(self.rendering.behavior);
        let viewport = self.layout_viewport;
        let layer =
            RenderedArtworkLayer::new(incoming, repository_root, resolved.palette, &self.caches);
        self.transition.update_current(revision, |current| {
            let rendered = current
                .now_playing
                .as_mut()
                .expect("Now Playing artwork update");
            if let Some(viewport) = viewport {
                let layout = NowPlayingLayout::for_composition_progress(
                    incoming,
                    viewport,
                    rendered.metadata.rendered_composition_progress(),
                );
                layer.apply_layout(&layout);
                layer.background.apply_viewport(viewport);
            }
            // An invisible destination can take its latest artwork immediately;
            // its parent still owns the coordinated Now Playing reveal.
            let animate_artwork = animated && current.root.opacity() > 0.0;
            rendered.visuals.replace(
                revision,
                layer,
                transition_clock.elapsed(),
                animate_artwork,
                current.palette,
            );
            if !animate_artwork {
                current.set_palette(resolved.palette);
            }
            current.capture_error = capture_error;
        });
        self.install_palette_styles();
    }

    pub(crate) fn update_diagnostics(&self, text: &str) {
        self.transition.current().value().update_diagnostics(text);
        if let Some(outgoing) = self.transition.outgoing() {
            outgoing.value().update_diagnostics(text);
        }
        for layer in &self.retained_layers {
            layer.value().update_diagnostics(text);
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
        self.layers.remove_overlay(&layer.value().root);
    }

    fn render_replacement_at_viewport(
        &mut self,
        presentation: &Presentation,
        repository_root: &Path,
    ) -> RenderedPresentation {
        let now = self.transition_clock.elapsed();
        let diagnostics_text = self.transition.current().value().diagnostics_text();
        let (resolved, capture_error) =
            resolve_for_rendering(presentation, repository_root, self.rendering);
        let rendering = self.rendering;
        let layout_viewport = self.layout_viewport;
        let mut inherited_composition = if rendering.behavior == PresentationBehavior::StaticFixture
        {
            None
        } else {
            // An unrevealed destination has no visible geometry to hand on.
            std::iter::once(self.transition.current())
                .chain(self.transition.outgoing())
                .chain(self.retained_layers.iter().rev())
                .find(|layer| layer.value().root.opacity() > 0.0)
                .and_then(|layer| layer.value().now_playing.as_ref())
                .map(|current| {
                    let lyrics = &current.metadata.lyrics;
                    lyrics
                        .motion
                        .borrow()
                        .inherit_composition(lyrics.rendered_at.get(), now)
                })
        };
        let root = &self.root;
        rendering
            .cache_scope
            .render_replacement(&mut self.caches, |caches| {
                let mut render = || {
                    let mut rendered = render_current_from_resolved(
                        &resolved,
                        repository_root,
                        diagnostics_text.as_deref(),
                        caches.clone(),
                        rendering,
                    );
                    if let Some(motion) = inherited_composition.take()
                        && let Some(incoming) = &rendered.now_playing
                        && let Presentation::NowPlaying(presentation) = &resolved.presentation
                    {
                        // Only geometry crosses the song boundary. The new song
                        // owns its lyric discovery deadline and every visible cue.
                        *incoming.metadata.lyrics.motion.borrow_mut() = motion;
                        incoming.metadata.update_lyrics(0, presentation, now);
                        incoming.metadata.lyric_composition_progress(now);
                    }
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
        self.layers.add_overlay(&current.value().root);
        self.install_palette_styles();
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
        let mut css = styles.to_css(&layout, &full_field_layout);
        for (index, layer) in self.retained_layers.iter().enumerate() {
            css.push_str(&PresentationStyleLayer::Retained(index).to_css(
                layer.value().palette,
                &layout,
                &full_field_layout,
            ));
        }
        let presentations =
            std::iter::once((PresentationStyleLayer::Current, self.transition.current()))
                .chain(
                    self.transition
                        .outgoing()
                        .map(|layer| (PresentationStyleLayer::Outgoing, layer)),
                )
                .chain(
                    self.retained_layers
                        .iter()
                        .enumerate()
                        .map(|(index, layer)| (PresentationStyleLayer::Retained(index), layer)),
                );
        for (style, layer) in presentations {
            if let Some(rendered) = &layer.value().now_playing {
                css.push_str(&rendered.visuals.styles(style, &layout, &full_field_layout));
            }
        }
        self.palette_provider.load_from_data(&css);
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
    fn set_palette(&mut self, palette: PresentationPalette) {
        self.palette = palette;
        if let Some(rendered) = &self.now_playing {
            rendered.metadata.lyrics.reel.set_palette(palette);
        }
    }

    fn text_opacity(&self) -> f64 {
        if let Some(now_playing) = &self.now_playing {
            now_playing.metadata.root.opacity()
        } else {
            self.full_field
                .as_ref()
                .expect("Full-field presentation")
                .copy
                .opacity()
        }
    }

    fn set_text_opacity(&self, opacity: f64) {
        if let Some(now_playing) = &self.now_playing {
            now_playing.metadata.root.set_opacity(opacity);
        }
        if let Some(full_field) = &self.full_field {
            full_field.copy.set_opacity(opacity);
            if let Some(identity) = &full_field.identity {
                identity.root.set_opacity(opacity);
            }
        }
    }

    fn layout_ready(&self) -> bool {
        self.now_playing
            .as_ref()
            .is_none_or(|now_playing| now_playing.metadata.lyrics.layout_ready())
            && self
                .full_field
                .as_ref()
                .is_none_or(|full_field| full_field.fit_readiness.is_ready())
    }

    fn capture_ready(&self) -> Result<bool, String> {
        if let Some(error) = self.capture_error.as_ref() {
            return Err(error.clone());
        }
        if let Some(now_playing) = self.now_playing.as_ref() {
            return Ok(!now_playing.visuals.transition.is_active()
                && !now_playing.metadata.metadata_fade.is_active()
                && now_playing.visuals.current().artwork.capture_ready()?
                && self.layout_ready());
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
                    .update(&presentation.status, now);
                rendered
                    .metadata
                    .update_metadata(presentation, now, self.root.opacity() > 0.0);
                rendered.metadata.update_timing(presentation, now);
                rendered.metadata.update_lyrics(revision, presentation, now);
                // Keep fitting and composition geometry tied to the visible metadata
                // until its replacement has reached zero opacity.
                let mut displayed = presentation.clone();
                rendered
                    .metadata
                    .metadata_fade
                    .displayed()
                    .apply_to(&mut displayed);
                self.layout_source =
                    PresentationLayoutSource::NowPlaying(Box::new(displayed.clone()));
                if let Some(viewport) = viewport {
                    let progress = rendered.metadata.lyric_composition_progress(now);
                    let layout =
                        NowPlayingLayout::for_composition_progress(&displayed, viewport, progress);
                    rendered.apply_foreground_layout(&layout);
                    rendered.metadata.apply_lyric_frame(now, &layout);
                }
            }
            (None, Some(rendered), Presentation::FullField(presentation)) => {
                rendered
                    .presentation_status
                    .update(&presentation.status, now);
            }
            (None, Some(rendered), Presentation::NowPlaying(presentation)) => {
                // Unusable artwork with no metadata resolves to Full-field.
                rendered
                    .presentation_status
                    .update(&presentation.status, now);
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
            now_playing.visuals.apply_background_viewport(viewport);
        }
    }

    fn apply_prepared_now_playing_background(&self, gradient: PreparedNowPlayingGradient) {
        if let Some(now_playing) = self.now_playing.as_ref() {
            now_playing
                .visuals
                .current()
                .background
                .apply_prepared(gradient);
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
        .add_css_class(&PresentationStyleLayer::Current.class_name());
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
            } => tracked_identity(tracked_output, Some(tracked_zone)),
            roonscape_renderer::PresentationIdentity::OutputOnly { tracked_output } => {
                tracked_identity(tracked_output, None)
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

    let visuals = RenderedArtworkLayers::new(RenderedArtworkLayer::new(
        presentation,
        repository_root,
        palette,
        &caches,
    ));
    surface.set_child(Some(&visuals.root));

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

    let metadata = metadata(presentation, &layout, palette, rendering);
    let metadata_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    metadata_slot.add_css_class("metadata-slot");
    metadata_slot.set_hexpand(false);
    metadata_slot.set_vexpand(true);
    metadata_slot.append(&metadata.root);

    content.append(&artwork_column);
    content.append(&metadata_slot);
    let now_playing = RenderedNowPlaying {
        visuals,
        content,
        artwork_column,
        metadata_slot,
        metadata,
    };
    let (root, diagnostics) = presentation_layer(&surface, "now-playing", diagnostics_text);
    RenderedPresentation {
        root: root.upcast(),
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

fn artwork_source(presentation: &Presentation) -> Option<(String, Option<u64>)> {
    match presentation {
        Presentation::NowPlaying(presentation) => presentation
            .artwork_path
            .as_ref()
            .map(|path| (path.clone(), presentation.artwork_revision)),
        Presentation::FullField(_) => None,
    }
}

fn set_layer_style(root: &impl IsA<gtk::Widget>, style: PresentationStyleLayer) {
    if root.has_css_class(&style.class_name()) {
        return;
    }
    for class in root.css_classes() {
        if class.starts_with("presentation-") {
            root.remove_css_class(&class);
        }
    }
    root.add_css_class(&style.class_name());
}

impl RenderedArtworkLayer {
    fn new(
        presentation: &NowPlayingPresentation,
        repository_root: &Path,
        palette: PresentationPalette,
        caches: &PresentationCaches,
    ) -> Self {
        let root = gtk::Overlay::new();
        root.set_hexpand(true);
        root.set_vexpand(true);
        let background = RenderedNowPlayingBackground::new(palette, Rc::clone(&caches.gradients));
        root.set_child(Some(&background.picture));
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        content.set_hexpand(true);
        content.set_vexpand(true);
        let artwork_column = gtk::CenterBox::new();
        artwork_column.set_orientation(gtk::Orientation::Vertical);
        artwork_column.set_vexpand(true);
        artwork_column.set_overflow(gtk::Overflow::Visible);
        let artwork = artwork(presentation, repository_root, Rc::clone(&caches.artwork));
        artwork_column.set_center_widget(Some(&artwork.reservation));
        content.append(&artwork_column);
        root.add_overlay(&content);
        Self {
            root,
            background,
            content,
            artwork_column,
            artwork,
            palette,
        }
    }

    fn apply_layout(&self, layout: &NowPlayingLayout) {
        let gutter = dimension(layout.outer_gutter_px);
        self.content.set_margin_start(gutter);
        self.content.set_margin_end(gutter);
        self.artwork_column
            .set_width_request(dimension(layout.artwork_column_width_px));
        self.artwork.apply_layout(layout);
    }
}

impl RenderedArtworkLayers {
    fn new(layer: RenderedArtworkLayer) -> Self {
        let root = gtk::Overlay::new();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.add_overlay(&layer.root);
        root.set_measure_overlay(&layer.root, true);
        let from_palette = layer.palette;
        Self {
            root,
            transition: PresentationTransition::new(0, layer),
            retained: Vec::new(),
            from_palette,
        }
    }

    fn current(&self) -> &RenderedArtworkLayer {
        self.transition.current().value()
    }

    fn layers(&self) -> impl Iterator<Item = (PresentationStyleLayer, &RenderedArtworkLayer)> {
        std::iter::once((PresentationStyleLayer::Current, self.current()))
            .chain(
                self.transition
                    .outgoing()
                    .map(|layer| (PresentationStyleLayer::Outgoing, layer.value())),
            )
            .chain(
                self.retained
                    .iter()
                    .enumerate()
                    .map(|(index, layer)| (PresentationStyleLayer::Retained(index), layer.value())),
            )
    }

    fn replace(
        &mut self,
        revision: u64,
        layer: RenderedArtworkLayer,
        now: Duration,
        animated: bool,
        visible_palette: PresentationPalette,
    ) {
        let duration = Duration::from_millis(225);
        if !animated {
            for released in self.transition.replace_immediately(revision, layer) {
                self.root.remove_overlay(&released.value().root);
            }
            self.release_retained();
        } else {
            layer.root.set_opacity(0.0);
            if self.transition.is_active() && self.current().root.opacity() == 0.0 {
                let released = self.transition.retarget_current(revision, layer, duration);
                self.root.remove_overlay(&released.value().root);
            } else {
                let opaque = self.current().root.opacity() == 1.0;
                for released in self
                    .transition
                    .retain_visible_composite(opaque, &mut self.retained)
                {
                    self.root.remove_overlay(&released.value().root);
                }
                self.from_palette = visible_palette;
                let discarded = self
                    .transition
                    .begin_with_duration(revision, layer, now, duration);
                debug_assert!(discarded.is_none());
            }
        }
        self.root.add_overlay(&self.current().root);
        self.root.set_measure_overlay(&self.current().root, true);
    }

    fn advance(&mut self, now: Duration) -> PresentationPalette {
        let progress = self.transition.progress(now);
        let opacity = motion_phase(progress, 0.0, 1.0);
        self.current().root.set_opacity(opacity);
        let palette = self.from_palette.mix(self.current().palette, opacity);
        if let Some(outgoing) = self.transition.finish(now) {
            self.root.remove_overlay(&outgoing.value().root);
            self.release_retained();
        }
        palette
    }

    fn release_retained(&mut self) {
        for layer in self.retained.drain(..) {
            self.root.remove_overlay(&layer.value().root);
        }
    }

    fn apply_layout(&self, layout: &NowPlayingLayout) {
        for (_, layer) in self.layers() {
            layer.apply_layout(layout);
        }
    }

    fn apply_background_viewport(&self, viewport: Viewport) {
        for (_, layer) in self.layers() {
            layer.background.apply_viewport(viewport);
        }
    }

    fn styles(
        &self,
        parent: PresentationStyleLayer,
        layout: &NowPlayingLayout,
        full_field: &FullFieldLayout,
    ) -> String {
        let mut css = String::new();
        for (style, layer) in self.layers() {
            set_layer_style(&layer.root, style);
            css.push_str(&style.to_css_within(parent, layer.palette, layout, full_field));
        }
        css
    }
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
    picture.set_keep_aspect_ratio(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);

    let decoration_ratio = match layout.decoration {
        ArtworkDecoration::ContainedImage(dimensions) => {
            dimensions.width_px as f32 / dimensions.height_px as f32
        }
        ArtworkDecoration::QuietSquareField => 1.0,
    };
    let decoration = gtk::AspectFrame::new(0.5, 0.5, decoration_ratio, false);
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

    let reservation = gtk::AspectFrame::new(0.5, 0.5, 1.0, false);
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

fn metadata_lines(
    presentation: &NowPlayingPresentation,
    typography: TypographySelection,
) -> [Option<RenderedMetadataLine>; 3] {
    let layout = metadata_layout(presentation, Viewport::WINDOWED_FIXTURE);
    let title = layout
        .title
        .as_ref()
        .map(|layout| metadata_line(layout, "title", typography.now_playing_title_family()));
    let artist = layout
        .artist
        .as_ref()
        .map(|layout| metadata_line(layout, "artist", typography.now_playing_supporting_family()));
    let album = layout
        .album
        .as_ref()
        .map(|layout| metadata_line(layout, "album", typography.now_playing_supporting_family()));
    [title, artist, album]
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

    let [title, artist, album] = metadata_lines(presentation, rendering.typography);
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
        rendering.typography.now_playing_supporting_family(),
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

    let timing_slot = gtk::Overlay::new();
    timing_slot.set_hexpand(true);
    footer.append(&timing_slot);
    match now_playing_layout.footer_content {
        NowPlayingFooterContent::DeterminateProgress => timing_slot.add_overlay(
            &progress
                .as_ref()
                .expect("determinate footer requires a timeline")
                .root,
        ),
        NowPlayingFooterContent::IndeterminateActivity => timing_slot.add_overlay(
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
        metadata_fade: ReplacementFade::new(MetadataContent::for_presentation(presentation)),
        typography: rendering.typography,
        title,
        artist,
        album,
        lyrics,
        progress,
        activity,
        timing_slot,
        timing_fade: ReplacementFade::new(TimingContent::for_presentation(presentation)),
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
    supporting_family: &'static str,
) -> Rc<RenderedLyrics> {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("lyric-composition");
    root.set_hexpand(true);
    root.set_valign(gtk::Align::Start);
    let masthead = gtk::Box::new(gtk::Orientation::Vertical, 0);
    masthead.add_css_class("lyric-masthead");
    let masthead_title = metadata_label("", "lyric-masthead-title");
    masthead_title.add_css_class("editorial-text");
    let masthead_artist = metadata_label("", "lyric-masthead-artist");
    masthead_artist.add_css_class("utility-text");
    for (label, text) in [
        (&masthead_title, &presentation.title),
        (&masthead_artist, &presentation.artist),
    ] {
        label.set_text(text.as_deref().unwrap_or_default());
        label.set_visible(text.is_some());
        label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_single_line_mode(true);
        label.set_max_width_chars(1);
        masthead.append(label);
    }
    root.append(&masthead);

    let reel = LyricReel::new(
        NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE).typography,
        palette,
        supporting_family,
    );
    let reel_region = gtk::ScrolledWindow::new();
    reel_region.add_css_class("lyric-reel-region");
    reel_region.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
    reel_region.set_propagate_natural_height(false);
    reel_region.set_hexpand(true);
    reel_region.set_vexpand(true);
    reel_region.set_overflow(gtk::Overflow::Hidden);
    reel_region.set_child(Some(&reel.widget));
    root.append(&reel_region);

    let rendered = Rc::new(RenderedLyrics {
        root,
        masthead,
        masthead_title,
        masthead_artist,
        reel_region,
        reel,
        cue_width_px: Cell::new(1),
        cue_height_px: Cell::new(1),
        typography: Cell::new(
            NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE).typography,
        ),
        motion: RefCell::new(LyricMotion::new(0, lyrics)),
        rendered_at: Cell::new(Duration::ZERO),
        rendered_composition_progress: Cell::new(f64::from(lyrics.is_some())),
        behavior,
    });
    rendered.apply_frame(
        Duration::ZERO,
        &NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE),
    );
    rendered
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
    label.set_ellipsize(pango::EllipsizeMode::End);
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

fn full_field_line(text: &str, class_name: &str) -> (gtk::Box, gtk::Label) {
    let label = metadata_label(text, class_name);
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_lines(1);
    label.set_single_line_mode(true);
    label.set_max_width_chars(1);
    label.set_wrap(false);
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
        self.visuals.apply_layout(layout);
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
        if let (Some(slot), Some(explanation)) =
            (self.explanation_slot.as_ref(), self.explanation.as_ref())
        {
            slot.set_margin_top(dimension(layout.explanation_spacing_px));
            slot.set_height_request(dimension(layout.explanation_slot.height_px));
            apply_full_field_font_size(explanation, layout.explanation_font, fit_generation);
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
    fn update_metadata(
        &mut self,
        presentation: &NowPlayingPresentation,
        now: Duration,
        visible: bool,
    ) {
        if !self.metadata_fade.retarget(
            MetadataContent::for_presentation(presentation),
            now,
            visible && animations_enabled(self.lyrics.behavior),
        ) {
            return;
        }
        // Both ordinary text and the compact masthead swap as one invisible group.
        self.apply_composition_ownership(self.rendered_composition_progress());
        while let Some(child) = self.ordinary_metadata.first_child() {
            self.ordinary_metadata.remove(&child);
        }
        [self.title, self.artist, self.album] = metadata_lines(presentation, self.typography);
        for line in [&self.title, &self.artist, &self.album]
            .into_iter()
            .flatten()
        {
            self.ordinary_metadata.append(&line.label);
        }
        for (label, text) in [
            (&self.lyrics.masthead_title, &presentation.title),
            (&self.lyrics.masthead_artist, &presentation.artist),
        ] {
            label.set_text(text.as_deref().unwrap_or_default());
            label.set_visible(text.is_some());
        }
    }

    fn update_timing(&mut self, presentation: &NowPlayingPresentation, now: Duration) {
        if self.timing_fade.update(
            TimingContent::for_presentation(presentation),
            now,
            animations_enabled(self.lyrics.behavior) && presentation.progress.is_none(),
        ) {
            if let Some(progress) = self.progress.take() {
                self.timing_slot.remove_overlay(&progress.root);
            }
            if let Some(activity) = self.activity.take() {
                self.timing_slot.remove_overlay(&activity.root);
            }
            match self.timing_fade.displayed() {
                TimingContent::Progress => {
                    let progress = progress_view(
                        presentation
                            .progress
                            .as_ref()
                            .expect("numeric timing target"),
                    );
                    self.timing_slot.add_overlay(&progress.root);
                    self.progress = Some(progress);
                }
                TimingContent::Activity(activity) => {
                    let activity = activity_view(activity, self.lyrics.behavior);
                    self.timing_slot.add_overlay(&activity.root);
                    self.activity = Some(activity);
                }
                TimingContent::Quiet => {}
            }
        }
        self.timing_slot.set_opacity(self.timing_fade.opacity());
        if let (Some(rendered), Some(progress)) = (&self.progress, &presentation.progress) {
            rendered.update(progress);
        }
    }

    fn update_lyrics(&self, revision: u64, presentation: &NowPlayingPresentation, now: Duration) {
        let mut motion = self.lyrics.motion.borrow_mut();
        motion.observe_playback(
            revision,
            presentation.playback_position_seconds,
            presentation.status.symbol == roonscape_renderer::PresentationStatusSymbol::Playing,
            now,
        );
        motion.update_with_information(
            revision,
            presentation.lyrics.as_deref(),
            presentation.lyrics_known,
            now,
            animations_enabled(self.lyrics.behavior),
        );
        drop(motion);
        self.apply_composition_ownership(self.lyrics.composition_timeline_progress(now));
    }

    fn apply_composition_ownership(&self, progress: f64) {
        let (ordinary_opacity, reel_opacity, masthead_opacity) = composition_ownership(progress);
        self.ordinary_metadata
            .set_opacity(ordinary_opacity * self.metadata_fade.opacity());
        // Preserve the established travel independently of exclusive text ownership.
        let ordinary_retirement = motion_phase(progress, 0.0, 0.62);
        let ordinary_travel_px = f64::from(self.lyrics.typography.get().lyric_cue_px) * 1.75;
        self.ordinary_metadata_stage.move_(
            &self.ordinary_metadata,
            0.0,
            -ordinary_retirement * ordinary_travel_px,
        );
        self.lyrics.root.set_opacity(1.0);
        let lyric_travel_px = f64::from(self.lyrics.typography.get().lyric_cue_px) * 1.8;
        self.lyrics.root.set_margin_top(
            ((1.0 - motion_phase(progress, 0.12, 0.46)) * lyric_travel_px).round() as i32,
        );
        self.lyrics.reel_region.set_opacity(reel_opacity);
        self.lyrics
            .masthead
            .set_opacity(masthead_opacity * self.metadata_fade.opacity());
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
        // The unmeasured lyric overlay needs its column even when every ordinary
        // metadata label is absent; otherwise the overlay's child collapses.
        self.ordinary_metadata_stage
            .set_width_request(musical_metadata_width);
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
            progress.root.set_valign(gtk::Align::Center);
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

        self.timing_slot
            .set_height_request(dimension(layout.timing_height_px()));
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
        let height = dimension(layout.metadata_height_budget_px);
        self.cue_width_px.set(dimension(layout.lyric_width_px));
        self.typography.set(layout.typography);
        self.root.set_width_request(width);
        self.root.set_height_request(height);
        self.masthead.set_spacing(dimension(
            (layout.typography.lyric_masthead_artist_px as f64 * 0.25).round() as u32,
        ));
        self.masthead_title.set_width_request(width);
        set_label_font_size(
            &self.masthead_title,
            layout.typography.lyric_masthead_title_px,
        );
        self.masthead_artist.set_width_request(width);
        set_label_font_size(
            &self.masthead_artist,
            layout.typography.lyric_masthead_artist_px,
        );
        let reel_margin_top =
            dimension((layout.typography.lyric_cue_px as f64 * 0.52).round() as u32);
        self.reel_region.set_margin_top(reel_margin_top);
        let (_, masthead_height, _, _) = self.masthead.measure(gtk::Orientation::Vertical, width);
        let reel_height = dimension(layout.metadata_height_budget_px)
            .saturating_sub(masthead_height)
            .saturating_sub(reel_margin_top);
        self.cue_height_px.set(reel_height.max(1));
        self.reel_region.set_height_request(reel_height);
        self.apply_frame(self.rendered_at.get(), layout);
    }

    fn composition_timeline_progress(&self, now: Duration) -> f64 {
        self.motion.borrow().frame_at(now).composition_progress
    }

    fn composition_layout_progress(&self, now: Duration) -> f64 {
        self.rendered_at.set(now);
        self.update_rendered_composition_progress(self.composition_timeline_progress(now))
    }

    fn update_rendered_composition_progress(&self, timeline_progress: f64) -> f64 {
        let progress = composition_geometry(timeline_progress);
        self.rendered_composition_progress.set(progress);
        progress
    }

    fn apply_frame(&self, now: Duration, layout: &NowPlayingLayout) {
        self.rendered_at.set(now);
        let frame = self.motion.borrow().frame_at(now);
        self.apply_frame_state(&frame, layout);
    }

    fn apply_frame_state(&self, frame: &LyricFrame, layout: &NowPlayingLayout) {
        self.update_rendered_composition_progress(frame.composition_progress);
        self.reel.update(
            frame,
            self.cue_width_px.get(),
            self.cue_height_px.get(),
            layout.typography,
        );
    }

    fn layout_ready(&self) -> bool {
        self.reel.widget.height() > 0 && self.reel.widget.height() == self.reel_region.height()
    }
}

fn composition_ownership(progress: f64) -> (f64, f64, f64) {
    // The same ownership boundary works in both directions and on reversal.
    // Entry establishes compact metadata before the prepared cue rises;
    // exit retires the reel before ordinary text returns to its space.
    let ordinary = 1.0 - motion_phase(progress, 0.0, 0.35);
    let reel = motion_phase(progress, 0.35, 0.2);
    let masthead = motion_phase(progress, 0.35, 0.25);
    (ordinary, reel, masthead)
}

fn animations_enabled(behavior: PresentationBehavior) -> bool {
    behavior.animations_enabled(
        gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations()),
    )
}

fn composition_geometry(progress: f64) -> f64 {
    motion_phase(progress, 0.12, 0.72)
}

fn motion_phase(value: f64, start: f64, duration: f64) -> f64 {
    let progress = ((value - start) / duration).clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

impl RenderedPresentationStatus {
    fn update(&mut self, status: &PresentationStatus, now: Duration) {
        let changed = self
            .fade
            .update(*status, now, animations_enabled(self.behavior));
        self.root.set_opacity(self.fade.opacity());
        if !changed {
            return;
        }
        let status = self.fade.displayed();

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
        for label in [&self.output_label, &self.output_name] {
            label.set_valign(gtk::Align::Baseline);
        }
        if let Some(zone) = self.zone.as_ref() {
            zone.label.set_valign(gtk::Align::Baseline);
            zone.name.set_valign(gtk::Align::Baseline);
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
        fade: ReplacementFade::new(*status),
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

fn tracked_identity(tracked_output: &str, tracked_zone: Option<&str>) -> RenderedIdentity {
    let row = gtk::Grid::new();
    row.add_css_class("tracked-identity");
    row.set_column_homogeneous(false);
    row.set_hexpand(true);
    row.set_halign(gtk::Align::End);
    row.set_valign(gtk::Align::End);

    let output = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    output.set_hexpand(true);
    output.set_halign(gtk::Align::Fill);
    let output_label = metadata_label("OUTPUT", "identity-label");
    let output_name = identity_name(tracked_output);
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
        let zone_name = identity_name(tracked_zone);
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

fn identity_name(text: &str) -> gtk::Label {
    let label = metadata_label(text, "identity-name");
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_lines(1);
    label.set_single_line_mode(true);
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
    use gtk::prelude::*;

    use super::{
        PRESENTATION_CACHE_CAPACITY, PresentationCaches, PresentationLayoutSource,
        RenderingConfiguration, STYLES, composition_ownership,
    };
    use crate::lyric_motion::LyricMotionCause;
    use roonscape_renderer::{
        NowPlayingFooterContent, NowPlayingGradientCacheKey, NowPlayingLayout, Presentation,
        PresentationBehavior, PresentationPalette, Viewport, parse_snapshot,
        presentation_from_snapshot,
    };

    fn lyric_view(
        presentation: &roonscape_renderer::NowPlayingPresentation,
        lyrics: Option<&roonscape_renderer::LyricPresentation>,
        palette: PresentationPalette,
        behavior: PresentationBehavior,
    ) -> std::rc::Rc<super::RenderedLyrics> {
        super::lyric_view(
            presentation,
            lyrics,
            palette,
            behavior,
            roonscape_renderer::select_typography(&HashSet::new()).now_playing_supporting_family(),
        )
    }

    fn lyric_presentation(fixture: &str) -> roonscape_renderer::NowPlayingPresentation {
        let snapshot = parse_snapshot(match fixture {
            "playing.json" => include_str!("../../shared/fixtures/playing.json"),
            "paused.json" => include_str!("../../shared/fixtures/paused.json"),
            "long-metadata.json" => include_str!("../../shared/fixtures/long-metadata.json"),
            "lyrics-one-line.json" => include_str!("../../shared/fixtures/lyrics-one-line.json"),
            "lyrics-two-line.json" => include_str!("../../shared/fixtures/lyrics-two-line.json"),
            "lyrics-four-lines.json" => {
                include_str!("../../shared/fixtures/lyrics-four-lines.json")
            }
            "lyrics-blank-cue.json" => include_str!("../../shared/fixtures/lyrics-blank-cue.json"),
            "lyrics-long-masthead.json" => {
                include_str!("../../shared/fixtures/lyrics-long-masthead.json")
            }
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
        for _ in 0..2 {
            rendered.root.allocate(1280, 720, -1, None);
            while gtk::glib::MainContext::default().iteration(false) {}
        }
        let cues: Vec<_> = lyrics
            .reel
            .visible_cues()
            .into_iter()
            .map(|cue| cue.cue)
            .collect();
        for (role, expected) in [
            (crate::lyric_motion::LyricColorRole::Earlier, previous),
            (crate::lyric_motion::LyricColorRole::Focal, current),
            (crate::lyric_motion::LyricColorRole::Upcoming, next),
        ] {
            let actual = cues
                .iter()
                .filter(|cue| cue.role == role && cue.opacity > 0.0 && !cue.text.trim().is_empty());
            if let Some(expected) = expected {
                assert!(
                    actual
                        .clone()
                        .any(|cue| cue.text == expected && cue.opacity == 1.0),
                    "expected {expected:?} in {cues:?}"
                );
            } else {
                assert_eq!(actual.count(), 0);
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

    fn assert_reel_motion_remains_ordered(
        rendered: &super::RenderedLyrics,
        layout: &NowPlayingLayout,
        started_at: std::time::Duration,
    ) {
        let mut previous = std::collections::HashMap::new();
        for offset in (0..=620).step_by(20) {
            rendered.apply_frame(
                started_at + std::time::Duration::from_millis(offset),
                layout,
            );
            let cues = rendered.reel.visible_cues();
            for pair in cues.windows(2) {
                assert!(
                    pair[0].index < pair[1].index,
                    "timed identity must remain ordered"
                );
                assert!(
                    pair[0].y + pair[0].height < pair[1].y,
                    "cues must not overlap: {pair:?}"
                );
            }
            for cue in cues {
                let lines: Vec<_> = cue
                    .layout
                    .lines_readonly()
                    .iter()
                    .map(|line| (line.start_index(), line.length()))
                    .collect();
                if let Some((last_lines, last_y, last_scale)) =
                    previous.insert(cue.index, (lines.clone(), cue.y, cue.scale))
                {
                    assert_eq!(lines, last_lines, "wrapping must not change during motion");
                    assert_eq!(
                        cue.scale, last_scale,
                        "glyph size must not change during motion"
                    );
                    if cue.cue.role != crate::lyric_motion::LyricColorRole::Upcoming {
                        assert!(
                            cue.y <= last_y + 0.1,
                            "incoming and earlier cues must travel upward: cue={cue:?}, last_y={last_y}"
                        );
                    }
                    assert!(
                        (cue.y - last_y).abs() < f64::from(rendered.reel_region.height()) * 0.16,
                        "motion must remain continuous"
                    );
                }
            }
        }
    }

    fn timing_variants_keep_allocated_content_stable() {
        use roonscape_renderer::{Playback, PresentationUpdate, classify_presentation_update};
        use std::time::Duration;

        fn settle(milliseconds: u64) {
            let until = std::time::Instant::now() + Duration::from_millis(milliseconds);
            while std::time::Instant::now() < until {
                while gtk::glib::MainContext::default().iteration(false) {}
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn geometry(rendered: &super::RenderedPresentation) -> Vec<(gtk::graphene::Rect, i32)> {
            let content = rendered.now_playing.as_ref().unwrap();
            let metadata = &content.metadata;
            let mut bounds = vec![
                (
                    content
                        .visuals
                        .current()
                        .artwork
                        .reservation
                        .compute_bounds(&rendered.root)
                        .unwrap(),
                    0,
                ),
                (metadata.footer.compute_bounds(&rendered.root).unwrap(), 0),
                (
                    metadata
                        .identity
                        .root
                        .compute_bounds(&rendered.root)
                        .unwrap(),
                    0,
                ),
            ];
            let labels = if metadata.lyrics.masthead.opacity() > 0.0 {
                [
                    &metadata.lyrics.masthead_title,
                    &metadata.lyrics.masthead_artist,
                ]
                .into_iter()
                .filter(|label| label.is_visible())
                .collect::<Vec<_>>()
            } else {
                [&metadata.title, &metadata.artist, &metadata.album]
                    .into_iter()
                    .flatten()
                    .map(|line| &line.label)
                    .collect()
            };
            for label in labels {
                bounds.push((
                    label.compute_bounds(&rendered.root).unwrap(),
                    label.layout().line_count(),
                ));
            }
            bounds
        }

        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        for fixture in [
            "playing.json",
            "long-metadata.json",
            "missing-artist.json",
            "lyrics-one-line.json",
        ] {
            for viewport in [Viewport::new(1280, 720), Viewport::new(1600, 1200)] {
                let mut snapshot = parse_snapshot(
                    &std::fs::read_to_string(repository.join("src/shared/fixtures").join(fixture))
                        .unwrap(),
                )
                .unwrap();
                if fixture == "playing.json" {
                    snapshot.now_playing.as_mut().unwrap().title =
                        Some("Hanging On The Telephone".to_owned());
                }
                let timing = snapshot.timing.clone();
                let mut previous = presentation_from_snapshot(&snapshot).unwrap();
                let window = gtk::Window::new();
                window.set_default_size(viewport.width_px as i32, viewport.height_px as i32);
                let mut view = super::PresentationView::new(
                    0,
                    &previous,
                    viewport,
                    &repository,
                    super::install_style_providers(typography),
                    None,
                    RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
                );
                window.set_child(Some(&view.root()));
                window.present();
                settle(100);
                let baseline = geometry(view.transition.current().value());
                for (index, (playback, has_timing)) in [
                    (Playback::Loading, false),
                    (Playback::Playing, false),
                    (Playback::Playing, true),
                    (Playback::Paused, true),
                    (Playback::Paused, false),
                    (Playback::Playing, true),
                ]
                .into_iter()
                .enumerate()
                {
                    snapshot.playback = Some(playback);
                    snapshot.timing = timing.clone();
                    if !has_timing {
                        // Retain lyric selection while exercising absent numeric timing.
                        snapshot.timing.as_mut().unwrap().duration_seconds = None;
                    }
                    let mut next = presentation_from_snapshot(&snapshot).unwrap();
                    if index == 0 {
                        let Presentation::NowPlaying(presentation) = &mut next else {
                            unreachable!()
                        };
                        presentation.progress = None;
                        presentation.activity = None;
                    }
                    match classify_presentation_update(&previous, &next) {
                        PresentationUpdate::InPlace => {
                            view.update_in_place(index as u64 + 1, &next, &repository)
                        }
                        PresentationUpdate::TransitionRequired => {
                            view.replace(index as u64 + 1, &next, &repository)
                        }
                    }
                    // Inspect early, middle, and settled frames of the live layers.
                    for delay in [50, 175, 250] {
                        settle(delay);
                        assert_eq!(
                            geometry(view.transition.current().value()),
                            baseline,
                            "current geometry: {fixture}, {viewport:?}, {playback:?}, timing={has_timing}"
                        );
                        if let Some(outgoing) = view.transition.outgoing() {
                            assert_eq!(
                                geometry(outgoing.value()),
                                baseline,
                                "outgoing metadata must not form a displaced duplicate"
                            );
                        }
                    }
                    view.finish_transition();
                    previous = next;
                }
                window.destroy();
            }
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

    fn composed_lyrics_remain_above_footer() {
        for fixture in [
            "lyrics-long-masthead.json",
            "lyrics-one-line.json",
            "lyrics-two-line.json",
            "lyrics-blank-cue.json",
        ] {
            let presentation = lyric_presentation(fixture);
            let mut rendered =
                rendered_now_playing(&presentation, PresentationBehavior::StaticFixture);
            let window = gtk::Window::new();
            window.set_child(Some(&rendered.root));
            for (width, height) in [
                (1_600, 900),
                (1_280, 720),
                (1_600, 1_200),
                (1_920, 1_200),
                (2_560, 1_080),
                (3_840, 2_160),
                (3_840, 2_400),
            ] {
                let viewport = Viewport::new(width, height);
                rendered.apply_viewport(viewport);
                window.set_default_size(width as i32, height as i32);
                window.present();
                // Exercise static presentation allocation without playback updates
                // or manually repositioning cues after GTK has measured the text.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    while gtk::glib::MainContext::default().iteration(false) {}
                    if rendered.root.width() == width as i32
                        && rendered.root.height() == height as i32
                        && rendered.layout_ready()
                    {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "composition should settle: fixture={fixture}, viewport={viewport:?}, actual={}x{}",
                        rendered.root.width(),
                        rendered.root.height()
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let metadata = &rendered.now_playing.as_ref().unwrap().metadata;
                let footer = metadata.footer.compute_bounds(&rendered.root).unwrap();
                let lyrics = &metadata.lyrics;
                let region = lyrics.reel.widget.compute_bounds(&rendered.root).unwrap();
                assert!(
                    region.y() + region.height() <= footer.y() + 1.0,
                    "the reel clip must remain above the footer: fixture={fixture}, viewport={viewport:?}, region={region:?}, footer={footer:?}"
                );
                assert_eq!(lyrics.reel.widget.overflow(), gtk::Overflow::Hidden);
                assert!(!lyrics.reel.visible_cues().is_empty());
                // Refreshes can arrive faster than GTK paints, especially at 4K.
                // They must not continually restart allocation readiness.
                rendered.update_in_place(
                    1,
                    &Presentation::NowPlaying(presentation.clone()),
                    std::time::Duration::ZERO,
                    Some(viewport),
                );
                assert!(
                    rendered.layout_ready(),
                    "an unchanged settled presentation must remain ready: fixture={fixture}, viewport={viewport:?}"
                );
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    rendered.update_in_place(
                        1,
                        &Presentation::NowPlaying(presentation.clone()),
                        std::time::Duration::ZERO,
                        Some(viewport),
                    );
                    while gtk::glib::MainContext::default().iteration(false) {}
                    if rendered.layout_ready() {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "refreshes must allow layout to settle: fixture={fixture}, viewport={viewport:?}"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            window.destroy();
        }
    }

    fn lyric_masthead_fits_long_metadata() {
        let presentation = lyric_presentation("long-metadata.json");
        let rendered = lyric_view(
            &presentation,
            None,
            PresentationPalette::fallback(),
            PresentationBehavior::Dynamic,
        );
        for viewport in [Viewport::new(3840, 2160), Viewport::new(1280, 720)] {
            let layout = NowPlayingLayout::for_presentation(&presentation, viewport);
            rendered.apply_layout(&layout);
            let width = layout.information.musical_metadata_width_px as i32;
            let (minimum, _, _, _) = rendered.masthead.measure(gtk::Orientation::Horizontal, -1);
            assert!(
                minimum <= width,
                "masthead needs {minimum}px but has {width}px"
            );
        }
    }

    #[test]
    fn allocated_lyric_reel_preserves_geometry_and_events() {
        roonscape_renderer::register_packaged_fallback_fonts(Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
        gtk::init().expect("GTK should initialize for native lyric layout coverage");
        super::install_style_providers(roonscape_renderer::select_typography(&HashSet::new()));
        reel_capacity_and_primary_position_follow_available_space();
        blanks_retain_the_packed_reel_across_peer_viewports();
        hyphenated_lyric_words_move_intact_to_the_next_line();
        complete_cues_fit_below_the_primary_position();
        reel_handoffs_keep_wrapping_and_outgoing_geometry();
        composition_transitions_preserve_fitted_cues();
        status_and_timing_replacements_preserve_the_existing_metadata();
        compatible_metadata_preserves_live_content_and_refits_invisibly();
        metadata_availability_preserves_full_field_resolution();
        now_playing_artwork_and_text_reveal_together();
        opening_lyrics_enter_from_regular_composition();
        unknown_incoming_lyrics_retain_composition_until_expiry();
        replacement_preserves_rendered_intermediate_composition();
        rapid_replacement_inherits_the_visible_composition();
        song_replacement_removes_every_previous_lyric_reel();
        current_track_artwork_preserves_readable_text();
        artwork_updates_recolor_the_existing_lyric_reel();
        repeated_artwork_updates_preserve_visible_appearance();
        track_replacement_supersedes_artwork_without_restarting_its_reveal();
        interrupted_reveal_preserves_the_visible_layers();
        invisible_destinations_keep_the_original_departure_clock();
        incoming_foreground_updates_during_the_coordinated_reveal();
        full_field_replacement_never_superimposes_messages();
        lyric_masthead_fits_long_metadata();
        composed_lyrics_remain_above_footer();
        ordinary_metadata_remains_stable_on_playback_updates();
        timing_variants_keep_allocated_content_stable();
        short_blanks_return_during_their_departure();
        continuous_snapshot_at_a_cue_boundary_animates();
        a_seek_within_the_incoming_cue_settles_its_handoff();
        authoritative_relocations_render_destination_content_with_timing();

        blank_promotion_preserves_context_and_an_interrupted_departure();

        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);

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
        let artwork = now_playing.visuals.current().artwork.surface.as_ptr();
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
        assert_eq!(
            now_playing.visuals.current().artwork.surface.as_ptr(),
            artwork
        );
        assert_eq!(
            now_playing.metadata.presentation_status.root.as_ptr(),
            status
        );
        assert_eq!(now_playing.metadata.footer.as_ptr(), footer);
        assert_eq!(now_playing.metadata.identity.root.as_ptr(), identity);
        assert!(now_playing.metadata.lyrics.reel_region.opacity() > 0.8);
        assert!(now_playing.metadata.lyrics.masthead.opacity() > 0.5);
        assert_eq!(now_playing.metadata.ordinary_metadata.opacity(), 0.0);
        assert_eq!(now_playing.visuals.current().artwork.surface.opacity(), 1.0);
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
        let focal_font_size = now_playing.metadata.lyrics.typography.get().lyric_cue_px as f32;
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
        natural_lyrics
            .timeline
            .push("A third complete-state cue".to_owned());
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
            LyricMotionCause::NaturalCueHandoff
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
            natural_lyrics.previous(),
            Some(natural_lyrics.current()),
            natural_lyrics.next(),
        );
        assert_rendered_composition_ownership(&complete, 0.0, 1.0, 1.0);

        let mut seek_lyrics = natural_lyrics.clone();
        seek_lyrics.current_index += 1;
        seek_lyrics.timeline[seek_lyrics.current_index] = "External seek destination".to_owned();
        seek_lyrics
            .timeline
            .push("Timeline revision source".to_owned());
        let mut seek = natural.clone();
        seek.playback_position_seconds = natural
            .playback_position_seconds
            .map(|position| position + 10.0);
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
            seek_lyrics.previous(),
            Some(seek_lyrics.current()),
            seek_lyrics.next(),
        );

        let mut revised_lyrics = seek_lyrics.clone();
        revised_lyrics.timeline_signature = lyric_presentation("lyrics-revision-after.json")
            .lyrics
            .expect("revision fixture should have lyrics")
            .timeline_signature;
        revised_lyrics.current_index += 1;
        revised_lyrics.timeline[revised_lyrics.current_index - 1] =
            "Corrected previous cue".to_owned();
        revised_lyrics.timeline[revised_lyrics.current_index] = "Corrected selected cue".to_owned();
        revised_lyrics.timeline.push("After correction".to_owned());
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
            revised_lyrics.previous(),
            Some(revised_lyrics.current()),
            revised_lyrics.next(),
        );

        let mut handoff_lyrics = revised_lyrics.clone();
        handoff_lyrics.current_index += 1;
        handoff_lyrics.timeline[handoff_lyrics.current_index] = "Handoff destination".to_owned();
        handoff_lyrics
            .timeline
            .push("Interruption destination".to_owned());
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
        interrupted_lyrics
            .timeline
            .push("Before Intentional Blank".to_owned());
        let mut interrupted = handoff.clone();
        interrupted.lyrics = Some(Box::new(interrupted_lyrics.clone()));
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(interrupted.clone()),
            std::time::Duration::from_millis(2_100),
            Some(viewport),
        );
        let frame = lyric_motion_frame(&complete, std::time::Duration::from_millis(2_100));
        assert_eq!(frame.cause, LyricMotionCause::NaturalCueHandoff);
        assert!(frame.cue_motion_active);
        complete.update_in_place(
            43,
            &Presentation::NowPlaying(interrupted.clone()),
            std::time::Duration::from_millis(2_720),
            Some(viewport),
        );
        assert_rendered_lyric_roles(
            &complete,
            interrupted_lyrics.previous(),
            Some(interrupted_lyrics.current()),
            interrupted_lyrics.next(),
        );

        let mut blank_lyrics = interrupted_lyrics.clone();
        blank_lyrics.current_index += 1;
        blank_lyrics.timeline[blank_lyrics.current_index].clear();
        blank_lyrics.timeline.push(" ".to_owned());
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
        assert_rendered_lyric_roles(&complete, Some("Interruption destination"), None, None);
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
        assert_rendered_lyric_roles(&complete, Some("Interruption destination"), None, None);
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
        assert_eq!(frame.cause, LyricMotionCause::NaturalCueHandoff);
        assert!(!frame.cue_motion_active);
        assert_rendered_lyric_roles(
            &reduced,
            natural_lyrics.previous(),
            Some(natural_lyrics.current()),
            natural_lyrics.next(),
        );
        assert_rendered_composition_ownership(&reduced, 0.0, 1.0, 1.0);
    }

    fn composition_transitions_preserve_fitted_cues() {
        for (width, height) in [
            (1280, 720),
            (1600, 900),
            (1600, 1200),
            (1920, 1200),
            (2560, 1080),
            (3840, 2160),
            (3840, 2400),
        ] {
            for fixture in ["lyrics-four-lines.json", "lyrics-two-line.json"] {
                let presentation = lyric_presentation(fixture);
                let rendered = lyric_view(
                    &presentation,
                    presentation.lyrics.as_deref(),
                    PresentationPalette::fallback(),
                    PresentationBehavior::StaticFixture,
                );
                let viewport = Viewport::new(width, height);
                let destination = NowPlayingLayout::for_presentation(&presentation, viewport);
                allocate_lyrics(&rendered, &destination);
                let fitted = || {
                    rendered
                        .reel
                        .visible_cues()
                        .into_iter()
                        .map(|cue| {
                            let lines = cue
                                .layout
                                .lines_readonly()
                                .iter()
                                .map(|line| (line.start_index(), line.length()))
                                .collect::<Vec<_>>();
                            (cue.index, lines, cue.scale)
                        })
                        .collect::<Vec<_>>()
                };
                let expected = fitted();
                assert!(!expected.is_empty());
                // Entry, exit, and reversal all use the same fitted destination.
                for progress in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 0.8, 0.4, 0.6, 1.0] {
                    let layout = NowPlayingLayout::for_composition_progress(
                        &presentation,
                        viewport,
                        progress,
                    );
                    allocate_lyrics(&rendered, &layout);
                    assert_eq!(
                        fitted(),
                        expected,
                        "{fixture}, {width}x{height}, {progress}"
                    );
                }
                // Allocation changes during travel must not trigger height fitting.
                rendered.reel.widget.allocate(width as i32, 120, -1, None);
                let allocated = fitted();
                assert!(!allocated.is_empty());
                for cue in allocated {
                    assert_eq!(&cue, expected.iter().find(|item| item.0 == cue.0).unwrap());
                }
            }
        }
    }

    fn reel_capacity_and_primary_position_follow_available_space() {
        let mut presentation = lyric_presentation("lyrics-one-line.json");
        let lyrics = presentation.lyrics.as_mut().unwrap();
        lyrics.timeline = [
            "Earlier", "Again", "Before", "Focal", "Again", "After", "Further", "Last",
        ]
        .map(str::to_owned)
        .to_vec();
        lyrics.current_index = 3;

        let rendered = lyric_view(
            &presentation,
            presentation.lyrics.as_deref(),
            PresentationPalette::fallback(),
            PresentationBehavior::StaticFixture,
        );
        let layout = NowPlayingLayout::for_presentation(&presentation, Viewport::new(1600, 1200));
        allocate_lyrics(&rendered, &layout);
        let cues = rendered.reel.visible_cues();
        assert!(
            cues.len() > 3,
            "available space must expose additional context: {cues:?}"
        );
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for palette in [
            PresentationPalette::fallback(),
            PresentationPalette::from_artwork(
                &repository.join("src/shared/fixtures/artwork/dark-teal.jpg"),
            )
            .unwrap(),
            PresentationPalette::from_artwork(
                &repository.join("src/shared/fixtures/artwork/light.jpg"),
            )
            .unwrap(),
        ] {
            rendered.reel.set_palette(palette);
            for cue in rendered.reel.visible_cues() {
                assert_eq!(cue.scale, 1.0, "short context keeps the preferred cue size");
                assert_eq!(
                    cue.color,
                    if cue.index == 3 {
                        palette.primary_text
                    } else {
                        palette.secondary_text
                    },
                    "active uses primary text; earlier and upcoming share secondary text"
                );
            }
        }
        let focal = cues.iter().find(|cue| cue.index == 3).unwrap();
        assert!((focal.y - f64::from(rendered.reel_region.height()) / 3.0).abs() < 1.0);
        for pair in cues.windows(2) {
            assert!(pair[0].index < pair[1].index);
            assert!(pair[0].y + pair[0].height < pair[1].y);
        }
    }

    fn blanks_retain_the_packed_reel_across_peer_viewports() {
        for (width, height) in [
            (1280, 720),
            (1600, 900),
            (1600, 1200),
            (1920, 1200),
            (2560, 1080),
            (3840, 2160),
            (3840, 2400),
        ] {
            let mut presentation = lyric_presentation("lyrics-blank-cue.json");
            let lyrics = presentation.lyrics.as_mut().unwrap();
            lyrics.timeline = [
                "Earlier", "Before", "One\nTwo", "", " ", "", "Returns", "After", "Further",
                "Last", "Beyond",
            ]
            .map(str::to_owned)
            .to_vec();
            lyrics.current_index = 3;
            let rendered = lyric_view(
                &presentation,
                presentation.lyrics.as_deref(),
                PresentationPalette::fallback(),
                PresentationBehavior::StaticFixture,
            );
            let layout =
                NowPlayingLayout::for_presentation(&presentation, Viewport::new(width, height));
            allocate_lyrics(&rendered, &layout);
            let cues = rendered.reel.visible_cues();
            let primary_y = f64::from(rendered.reel.widget.height()) / 3.0;
            assert!(
                cues.len() > 3,
                "blank must retain space-limited context at {width}x{height}"
            );
            assert!(cues.iter().any(|cue| cue.cue.text == "After"));
            assert!(cues.iter().all(|cue| cue.cue.emphasis == 0.0));
            assert!(
                cues.iter()
                    .all(|cue| cue.y + cue.height <= primary_y || cue.y > primary_y)
            );
            for pair in cues.windows(2) {
                assert!(pair[0].y + pair[0].height < pair[1].y);
            }
            let visible_geometry = |reel: &crate::lyric_reel::LyricReel| {
                reel.visible_cues()
                    .into_iter()
                    .map(|cue| (cue.cue.text, cue.y, cue.height, cue.scale))
                    .collect::<Vec<_>>()
            };
            let expected = visible_geometry(&rendered.reel);
            for index in [4, 5, 3] {
                let lyrics = presentation.lyrics.as_mut().unwrap();
                lyrics.current_index = index;
                rendered.motion.borrow_mut().update(
                    index as u64,
                    Some(lyrics),
                    std::time::Duration::ZERO,
                    true,
                );
                rendered.apply_frame(std::time::Duration::ZERO, &layout);
                assert_eq!(
                    visible_geometry(&rendered.reel),
                    expected,
                    "seeking within consecutive blanks preserves geometry"
                );
            }
            let lyrics = presentation.lyrics.as_mut().unwrap();
            lyrics.current_index = 2;
            rendered
                .motion
                .borrow_mut()
                .update(9, Some(lyrics), std::time::Duration::ZERO, false);
            lyrics.current_index = 3;
            rendered
                .motion
                .borrow_mut()
                .update(9, Some(lyrics), std::time::Duration::ZERO, true);
            assert_reel_motion_remains_ordered(&rendered, &layout, std::time::Duration::ZERO);
            assert_eq!(visible_geometry(&rendered.reel), expected);
            for index in [4, 5] {
                lyrics.current_index = index;
                let now = std::time::Duration::from_secs(index as u64);
                rendered
                    .motion
                    .borrow_mut()
                    .update(9, Some(lyrics), now, true);
                rendered.apply_frame(now, &layout);
                assert_eq!(
                    visible_geometry(&rendered.reel),
                    expected,
                    "consecutive blanks must hold without another lift"
                );
                assert!(!rendered.motion.borrow().frame_at(now).cue_motion_active);
            }
            // A short run of blanks can end while its departure is still moving.
            // Resuming lyrics must continue from exactly the visible geometry.
            let restarted_at = std::time::Duration::from_secs(10);
            lyrics.current_index = 2;
            rendered
                .motion
                .borrow_mut()
                .update(10, Some(lyrics), restarted_at, false);
            lyrics.current_index = 3;
            rendered
                .motion
                .borrow_mut()
                .update(10, Some(lyrics), restarted_at, true);
            let continued_at = restarted_at + std::time::Duration::from_millis(100);
            rendered.apply_frame(continued_at, &layout);
            let departing = visible_geometry(&rendered.reel);
            lyrics.current_index = 4;
            rendered
                .motion
                .borrow_mut()
                .update(10, Some(lyrics), continued_at, true);
            rendered.apply_frame(continued_at, &layout);
            assert_eq!(visible_geometry(&rendered.reel), departing);
            lyrics.current_index = 6;
            rendered
                .motion
                .borrow_mut()
                .update(10, Some(lyrics), continued_at, true);
            rendered.apply_frame(continued_at, &layout);
            assert_eq!(
                visible_geometry(&rendered.reel),
                departing,
                "resumption must not jump after consecutive blanks"
            );
            assert_reel_motion_remains_ordered(&rendered, &layout, continued_at);
        }
    }

    fn assert_complete_focal_cue(
        rendered: &super::RenderedLyrics,
        focal: &crate::lyric_reel::PositionedCue,
    ) {
        assert!(
            !focal.layout.is_ellipsized(),
            "complete active cue: {focal:?}"
        );
        let area_height = f64::from(rendered.reel.widget.height());
        let (ink, _) = focal.layout.pixel_extents();
        assert!(
            f64::from(ink.x() + ink.width()) * focal.scale
                <= f64::from(rendered.reel.widget.width()) + 1.0,
            "complete active text must fit the column width"
        );
        assert!((focal.y - area_height / 3.0).abs() < 1.0);
        let readable_bottom =
            area_height - f64::from(rendered.typography.get().lyric_spacing_px) * 0.65;
        assert!(
            focal.y + focal.height <= readable_bottom + 1.0,
            "complete active text must clear the edge fade: {focal:?}"
        );
        assert!(
            focal.y + f64::from(ink.y() + ink.height()) * focal.scale <= readable_bottom + 1.0,
            "active glyphs must clear the fade"
        );
    }

    fn shaped_lyric(text: &str, width: i32) -> crate::lyric_reel::PositionedCue {
        let mut presentation = lyric_presentation("lyrics-one-line.json");
        let lyrics = presentation.lyrics.as_mut().unwrap();
        lyrics.timeline = vec![text.to_owned()];
        lyrics.current_index = 0;
        let rendered = lyric_view(
            &presentation,
            presentation.lyrics.as_deref(),
            PresentationPalette::fallback(),
            PresentationBehavior::StaticFixture,
        );
        let layout = NowPlayingLayout::for_presentation(&presentation, Viewport::new(1600, 900));
        allocate_lyrics(&rendered, &layout);
        let frame = rendered.motion.borrow().frame_at(std::time::Duration::ZERO);
        rendered
            .reel
            .update(&frame, width, 1400, rendered.typography.get());
        rendered.reel.widget.allocate(width, 1400, -1, None);
        rendered.reel.visible_cues().remove(0)
    }

    fn lyric_lines(cue: &crate::lyric_reel::PositionedCue) -> Vec<String> {
        let text = cue.layout.text();
        cue.layout
            .lines_readonly()
            .iter()
            .map(|line| {
                text[line.start_index() as usize..(line.start_index() + line.length()) as usize]
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    fn assert_lyric_width(cue: &crate::lyric_reel::PositionedCue, width: i32) {
        assert!(!cue.layout.is_ellipsized());
        for line in cue.layout.lines_readonly() {
            let (ink, logical) = line.pixel_extents();
            assert!(
                ink.x() >= -1 && ink.x() + ink.width() <= width + 1,
                "glyphs must stay inside {width}px: {ink:?}, {:?}",
                lyric_lines(cue)
            );
            assert!(logical.width() <= width, "line must fit: {logical:?}");
        }
    }

    fn hyphenated_lyric_words_move_intact_to_the_next_line() {
        for word in [
            "D-Rock",
            "self-made",
            "mother-in-law",
            "D‐Rock",
            "mother‐in‐law",
            "cafe\u{301}-made",
        ] {
            let width = shaped_lyric(word, 2000).layout.pixel_size().0;
            let exact = shaped_lyric(word, width);
            assert_eq!(lyric_lines(&exact), [word], "exact fit");
            assert_lyric_width(&exact, width);
            for prefix in ["Hi", "É"] {
                let text = format!("{prefix} {word}");
                let cue = shaped_lyric(&text, width);
                assert_eq!(lyric_lines(&cue), [prefix, word]);
                assert_eq!(cue.scale, 1.0, "moving a word must not shrink the cue");
                assert_eq!(cue.layout.text(), text);
                assert_lyric_width(&cue, width);
            }
        }
        let width = shaped_lyric("mother-", 2000).layout.pixel_size().0;
        let cue = shaped_lyric("mother-in-law", width);
        assert_eq!(
            lyric_lines(&cue),
            ["mother-", "in-law"],
            "prefer existing hyphens"
        );
        assert_lyric_width(&cue, width);

        let text = format!("{}-Rock", "W".repeat(16));
        let cue = shaped_lyric(&text, width);
        assert!(cue.layout.line_count() > 2, "oversized segments wrap too");
        assert_eq!(lyric_lines(&cue).concat(), text);
        assert_eq!(cue.scale, 1.0);
        assert_lyric_width(&cue, width);

        for text in [
            "Hi - Rock",
            "Hi – Rock",
            "Hi — Rock",
            "Hi–Rock",
            "Hi—Rock",
            "Hi\nD-Rock",
            "D-\nRock",
        ] {
            let cue = shaped_lyric(text, width);
            let ordinary = cue.layout.copy();
            ordinary.set_attributes(None);
            let expected: Vec<_> = ordinary
                .lines_readonly()
                .iter()
                .map(|line| (line.start_index(), line.length()))
                .collect();
            let actual: Vec<_> = cue
                .layout
                .lines_readonly()
                .iter()
                .map(|line| (line.start_index(), line.length()))
                .collect();
            assert_eq!(
                actual, expected,
                "normal punctuation and explicit breaks: {text}"
            );
            assert_eq!(cue.layout.text(), text);
            assert_lyric_width(&cue, width);
        }
    }

    fn complete_cues_fit_below_the_primary_position() {
        let oversized = lyric_presentation("lyrics-four-lines.json");
        let oversized_text = oversized.lyrics.as_ref().unwrap().current();
        let unbroken = "W".repeat(512);
        for (width, height) in [
            (1280, 720),
            (1600, 900),
            (1600, 1200),
            (1920, 1200),
            (2560, 1080),
            (3840, 2160),
            (3840, 2400),
        ] {
            for text in [
                "One\nTwo\nThree\nFour\nFive",
                "One\nTwo\nThree\nFour\nFive\nSix\nSeven\nEight\nNine\nTen\nEleven\nTwelve",
                "We find the signal where the last blue horizon meets the dark and every distant answer turns slowly toward the room while the patient stars remember all the names we carried through the silence",
                "Short",
                "One\nTwo",
                oversized_text,
                &unbroken,
            ] {
                let mut presentation = lyric_presentation("lyrics-one-line.json");
                let lyrics = presentation.lyrics.as_mut().unwrap();
                lyrics.timeline = ["Earlier", "Before", text, "After", "Further"]
                    .map(str::to_owned)
                    .to_vec();
                lyrics.current_index = 2;
                let rendered = lyric_view(
                    &presentation,
                    presentation.lyrics.as_deref(),
                    PresentationPalette::fallback(),
                    PresentationBehavior::StaticFixture,
                );
                let layout =
                    NowPlayingLayout::for_presentation(&presentation, Viewport::new(width, height));
                allocate_lyrics(&rendered, &layout);
                let cues = rendered.reel.visible_cues();
                let focal = cues.iter().find(|cue| cue.index == 2).unwrap();
                assert_complete_focal_cue(&rendered, focal);
                assert_eq!(focal.layout.text(), text);
                if text == "Short" || text == "One\nTwo" {
                    assert_eq!(focal.scale, 1.0, "fitting cues retain focal size");
                }
                assert!(focal.layout.line_count() >= text.lines().count() as i32);
                if text == oversized_text || text == unbroken {
                    let (ink, _) = focal.layout.pixel_extents();
                    assert!(
                        f64::from(ink.width()) * focal.scale
                            > f64::from(rendered.reel.widget.width()) * 0.5,
                        "long cues must use available width: {width}x{height}, ink={ink:?}, focal={focal:?}, area={}",
                        rendered.reel.widget.width()
                    );
                }
                for pair in cues.windows(2) {
                    assert!(pair[0].y + pair[0].height < pair[1].y);
                }
                // The same complete native layout must survive both context roles,
                // including exceptional fitting below the preferred-size limits.
                for current_index in [1, 3] {
                    let lyrics = presentation.lyrics.as_mut().unwrap();
                    lyrics.current_index = current_index;
                    rendered.motion.borrow_mut().update(
                        current_index as u64,
                        Some(lyrics),
                        std::time::Duration::ZERO,
                        false,
                    );
                    rendered.apply_frame(std::time::Duration::ZERO, &layout);
                    let context = rendered.reel.visible_cues();
                    let cue = context.iter().find(|cue| cue.index == 2).unwrap();
                    assert_eq!(cue.layout.text(), text);
                    assert_eq!(lyric_lines(cue), lyric_lines(focal));
                    assert_eq!(
                        cue.layout.font_description(),
                        focal.layout.font_description()
                    );
                    assert_eq!((cue.scale, cue.height), (focal.scale, focal.height));
                    assert!(!cue.layout.is_ellipsized());
                }
            }
        }
    }

    fn reel_handoffs_keep_wrapping_and_outgoing_geometry() {
        let oversized = lyric_presentation("lyrics-four-lines.json");
        let oversized_text = oversized.lyrics.as_ref().unwrap().current();
        for (width, height) in [
            (1280, 720),
            (1600, 900),
            (1600, 1200),
            (1920, 1200),
            (2560, 1080),
            (3840, 2160),
            (3840, 2400),
        ] {
            let mut presentation = lyric_presentation("lyrics-one-line.json");
            let lyrics = presentation.lyrics.as_mut().unwrap();
            lyrics.timeline = [
                "Earlier",
                "Again",
                "Short",
                "One\nTwo\nThree",
                "Hi D-Rock",
                "One\nTwo\nThree\nFour",
                "Short",
                "One\nTwo\nThree\nFour\nFive",
                "Short",
                oversized_text,
                "Short",
                oversized_text,
                "Last",
                "Again",
                "After",
                "Further",
            ]
            .map(str::to_owned)
            .to_vec();
            lyrics.current_index = 2;

            let rendered = lyric_view(
                &presentation,
                presentation.lyrics.as_deref(),
                PresentationPalette::fallback(),
                PresentationBehavior::Dynamic,
            );
            let layout =
                NowPlayingLayout::for_presentation(&presentation, Viewport::new(width, height));
            allocate_lyrics(&rendered, &layout);
            for index in 3..=12 {
                let lyrics = presentation.lyrics.as_mut().unwrap();
                lyrics.current_index = index;

                let start = std::time::Duration::from_secs(index as u64);
                rendered
                    .motion
                    .borrow_mut()
                    .update(0, Some(lyrics), start, true);
                assert_reel_motion_remains_ordered(&rendered, &layout, start);
                for offset in [0, 100, 310, 500, 620] {
                    let now = start + std::time::Duration::from_millis(offset);
                    rendered.apply_frame(now, &layout);
                    let visible = rendered.reel.visible_cues();
                    let outgoing = visible
                        .iter()
                        .find(|cue| cue.index == index as i64 - 1)
                        .expect("the adjacent outgoing cue still intersects the lyric area");
                    assert_eq!(
                        outgoing.cue.opacity, 1.0,
                        "outgoing retention is geometric at {width}x{height}"
                    );
                }
                let cues = rendered.reel.visible_cues();
                let focal = cues.iter().find(|cue| cue.index == index as i64).unwrap();
                assert_complete_focal_cue(&rendered, focal);
                // Static Fixture Mode must agree with a completed natural handoff.
                let direct = lyric_view(
                    &presentation,
                    presentation.lyrics.as_deref(),
                    PresentationPalette::fallback(),
                    PresentationBehavior::StaticFixture,
                );
                allocate_lyrics(&direct, &layout);
                let settled = direct.reel.visible_cues();
                let destination = settled
                    .iter()
                    .find(|cue| cue.index == index as i64)
                    .unwrap();
                assert_eq!(
                    (focal.y, focal.height, focal.scale),
                    (destination.y, destination.height, destination.scale)
                );
            }
            // Interrupt a lift and compare the exact displayed native geometry
            // and color before continuing toward the newest destination.
            let start = std::time::Duration::from_secs(16);
            let lyrics = presentation.lyrics.as_mut().unwrap();
            lyrics.current_index = 2;
            rendered
                .motion
                .borrow_mut()
                .update(0, Some(lyrics), start, false);
            lyrics.current_index = 3;
            rendered
                .motion
                .borrow_mut()
                .update(0, Some(lyrics), start, true);
            let interrupted_at = start + std::time::Duration::from_millis(100);
            rendered.apply_frame(interrupted_at, &layout);
            let displayed = || {
                rendered
                    .reel
                    .visible_cues()
                    .iter()
                    .map(|cue| {
                        (
                            cue.index,
                            cue.y,
                            cue.height,
                            cue.scale,
                            cue.color,
                            lyric_lines(cue),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let before = displayed();
            lyrics.current_index = 4;
            rendered
                .motion
                .borrow_mut()
                .update(0, Some(lyrics), interrupted_at, true);
            rendered.apply_frame(interrupted_at, &layout);
            assert_eq!(
                displayed(),
                before,
                "interruption must retarget from the displayed frame"
            );
            assert_reel_motion_remains_ordered(&rendered, &layout, interrupted_at);

            // Reduced animation and seeks in either direction settle complete
            // destination text without replaying intervening cues.
            for (index, animate, revision) in [(9, false, 0), (11, true, 1), (9, true, 2)] {
                let lyrics = presentation.lyrics.as_mut().unwrap();
                lyrics.current_index = index;
                let now = std::time::Duration::from_secs(20 + revision);
                rendered
                    .motion
                    .borrow_mut()
                    .update(revision, Some(lyrics), now, animate);
                rendered.apply_frame(now, &layout);
                assert!(!rendered.motion.borrow().frame_at(now).cue_motion_active);
                let cues = rendered.reel.visible_cues();
                let focal = cues.iter().find(|cue| cue.index == index as i64).unwrap();
                assert_eq!(focal.layout.text(), oversized_text);
                assert_complete_focal_cue(&rendered, focal);
            }
        }
    }

    fn status_and_timing_replacements_preserve_the_existing_metadata() {
        use roonscape_renderer::{PresentationUpdate, classify_presentation_update};
        use std::time::Duration;
        for fixture in [
            include_str!("../../shared/fixtures/timing-stability.json"),
            include_str!("../../shared/fixtures/indeterminate-progress.json"),
        ] {
            let mut source = lyric_presentation("playing.json");
            source.progress = None;
            let unavailable = parse_snapshot(fixture).unwrap();
            let Presentation::NowPlaying(unavailable) =
                presentation_from_snapshot(&unavailable).unwrap()
            else {
                unreachable!()
            };
            source.status = unavailable.status;
            source.activity = unavailable.activity;
            let old_status = source.status.label;
            let target = lyric_presentation("playing.json");
            assert_eq!(
                classify_presentation_update(
                    &Presentation::NowPlaying(source.clone()),
                    &Presentation::NowPlaying(target.clone())
                ),
                PresentationUpdate::InPlace,
                "status and timing changes must preserve the rendered composition"
            );
            let mut rendered = rendered_now_playing(&source, PresentationBehavior::Dynamic);
            let title = rendered
                .now_playing
                .as_ref()
                .unwrap()
                .metadata
                .title
                .as_ref()
                .unwrap()
                .label
                .clone();
            let target = Presentation::NowPlaying(target);
            for millis in [0, 100, 225, 325, 450] {
                rendered.update_in_place(
                    1,
                    &target,
                    Duration::from_millis(millis),
                    Some(Viewport::new(1280, 720)),
                );
                let metadata = &rendered.now_playing.as_ref().unwrap().metadata;
                assert_eq!(metadata.title.as_ref().unwrap().label, title);
                assert_eq!(metadata.ordinary_metadata.opacity(), 1.0);
                let status = &metadata.presentation_status;
                if millis < 225 {
                    assert_eq!(status.label.text(), old_status);
                } else {
                    assert_eq!(status.label.text(), "PLAYING");
                }
                if millis == 225 {
                    assert_eq!(
                        status.root.opacity(),
                        if old_status == "PLAYING" { 1.0 } else { 0.0 }
                    );
                }
                assert!(
                    metadata.progress.is_some(),
                    "determinate timing appears immediately"
                );
                assert!(metadata.activity.is_none());
                assert_eq!(metadata.timing_slot.opacity(), 1.0);
                if let Some(child) = metadata.timing_slot.first_child() {
                    assert!(
                        child.next_sibling().is_none(),
                        "timing has only one rendered version"
                    );
                }
                if millis == 450 {
                    assert_eq!(status.root.opacity(), 1.0);
                    assert_eq!(metadata.timing_slot.opacity(), 1.0);
                }
            }
        }
    }

    fn invisible_destinations_keep_the_original_departure_clock() {
        use std::time::Duration;
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
        let playing = Presentation::NowPlaying(lyric_presentation("playing.json"));
        let lyrics = Presentation::NowPlaying(lyric_presentation("lyrics-one-line.json"));
        let full_field = presentation_from_snapshot(
            &parse_snapshot(include_str!("../../shared/fixtures/disconnected.json")).unwrap(),
        )
        .unwrap();
        for at_millis in [112, 225] {
            for (pending, latest) in [
                (&lyrics, &playing),
                (&lyrics, &full_field),
                (&full_field, &lyrics),
            ] {
                let mut view = super::PresentationView::new(
                    0,
                    &playing,
                    Viewport::new(1280, 720),
                    &repository,
                    super::install_style_providers(typography),
                    None,
                    RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
                );
                view.replace(1, pending, &repository);
                let start = view
                    .transition
                    .started_at()
                    .unwrap_or_else(|| view.transition_clock.elapsed());
                view.advance_transition(start + Duration::from_millis(at_millis));
                let original = view.transition.outgoing().unwrap().value().root.clone();
                let opacity = view.transition.outgoing().unwrap().value().text_opacity();
                let obsolete = view.transition.current().value().root.clone();
                assert_eq!(obsolete.opacity(), 0.0);
                view.replace(2, latest, &repository);
                assert!(
                    obsolete.parent().is_none(),
                    "invisible superseded destinations are released immediately"
                );
                assert_eq!(view.transition.outgoing().unwrap().value().root, original);
                assert_eq!(
                    view.transition.outgoing().unwrap().value().text_opacity(),
                    opacity
                );
                view.advance_transition(start + Duration::from_millis(225));
                assert_eq!(
                    view.transition.outgoing().unwrap().value().text_opacity(),
                    0.0,
                    "superseding destinations must not restart outgoing text departure"
                );
                view.advance_transition(start + Duration::from_millis(337));
                assert!(view.transition.current().value().root.opacity() > 0.4);
                assert_eq!(
                    view.transition.current().value().text_opacity(),
                    if matches!(latest, Presentation::FullField(_)) {
                        0.0
                    } else {
                        1.0
                    }
                );
                view.advance_transition(start + Duration::from_millis(675));
                assert!(!view.transition.is_active());
                assert_eq!(view.transition.current().value().text_opacity(), 1.0);
            }
        }
    }

    fn interrupted_reveal_preserves_the_visible_layers() {
        use std::time::Duration;
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        let first = Presentation::NowPlaying(lyric_presentation("playing.json"));
        let second = Presentation::NowPlaying(lyric_presentation("lyrics-one-line.json"));
        let full_field = presentation_from_snapshot(
            &parse_snapshot(include_str!("../../shared/fixtures/disconnected.json")).unwrap(),
        )
        .unwrap();
        for latest in [&first, &full_field] {
            for reduced_animation in [false, true] {
                gtk::Settings::default()
                    .unwrap()
                    .set_gtk_enable_animations(true);
                let mut view = super::PresentationView::new(
                    0,
                    &first,
                    Viewport::new(1280, 720),
                    &repository,
                    super::install_style_providers(typography),
                    None,
                    RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
                );
                let base = view.transition.current().value().root.clone();
                let mut contributing = vec![(base, 1.0)];
                view.replace(1, &second, &repository);
                for revision in 2..=5 {
                    let start = view
                        .transition
                        .started_at()
                        .unwrap_or_else(|| view.transition_clock.elapsed());
                    view.advance_transition(start + Duration::from_millis(337));
                    let revealed = view.transition.current().value().root.clone();
                    let opacity = revealed.opacity();
                    assert!(opacity > 0.4 && opacity < 0.6);
                    contributing.push((revealed, opacity));
                    let destination = if revision == 5 { latest } else { &second };
                    view.replace(revision, destination, &repository);
                    for (root, opacity) in &contributing {
                        assert!(
                            root.parent().is_some(),
                            "every contributing background survives repeated interruptions"
                        );
                        assert_eq!(
                            root.opacity(),
                            *opacity,
                            "retarget from the visible composition without promoting any layer"
                        );
                    }
                    assert_eq!(view.transition.current().value().root.opacity(), 0.0);
                    let start = view
                        .transition
                        .started_at()
                        .unwrap_or_else(|| view.transition_clock.elapsed());
                    view.advance_transition(start + Duration::from_millis(112));
                    assert!(view.transition.outgoing().unwrap().value().text_opacity() < 0.6);
                    assert_eq!(view.transition.current().value().root.opacity(), 0.0);
                    view.advance_transition(start + Duration::from_millis(225));
                    assert_eq!(
                        view.transition.outgoing().unwrap().value().text_opacity(),
                        0.0
                    );
                }
                if reduced_animation {
                    gtk::Settings::default()
                        .unwrap()
                        .set_gtk_enable_animations(false);
                    view.replace(6, &second, &repository);
                } else {
                    let start = view
                        .transition
                        .started_at()
                        .unwrap_or_else(|| view.transition_clock.elapsed());
                    view.advance_transition(start + Duration::from_millis(675));
                }
                for (root, _) in contributing {
                    assert!(
                        root.parent().is_none(),
                        "completion releases all obsolete layers"
                    );
                }
                assert!(view.retained_layers.is_empty());
                assert!(!view.transition.is_active());
                assert_eq!(view.transition.current().value().root.opacity(), 1.0);
                assert_eq!(view.transition.current().value().text_opacity(), 1.0);
            }
        }
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
    }

    fn metadata_availability_preserves_full_field_resolution() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut target = lyric_presentation("playing.json");
        target.artwork_path = Some("missing-artwork.jpg".into());
        target.title = None;
        target.artist = None;
        target.album = None;
        let mut view = artwork_view(&target, PresentationBehavior::StaticFixture);
        assert!(view.transition.current().value().full_field.is_some());
        target.title = Some("Available metadata".into());
        view.update_in_place(1, &Presentation::NowPlaying(target.clone()), &repository);
        assert!(view.transition.current().value().now_playing.is_some());
        target.title = None;
        view.update_in_place(2, &Presentation::NowPlaying(target.clone()), &repository);
        assert!(view.transition.current().value().full_field.is_some());
        let root = view.transition.current().value().root.clone();
        view.update_in_place(3, &Presentation::NowPlaying(target), &repository);
        assert_eq!(view.transition.current().value().root, root);
    }

    fn compatible_metadata_preserves_live_content_and_refits_invisibly() {
        use std::rc::Rc;
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
        for fixture in ["playing.json", "lyrics-one-line.json"] {
            for behavior in [
                PresentationBehavior::Dynamic,
                PresentationBehavior::StaticFixture,
            ] {
                let mut target = lyric_presentation(fixture);
                target.title = None;
                target.artist = None;
                target.album = None;
                let mut view = artwork_view(&target, behavior);
                let window = gtk::Window::new();
                window.set_default_size(1280, 720);
                window.set_child(Some(&view.root()));
                window.present();
                while gtk::glib::MainContext::default().iteration(false) {}
                let current = view.transition.current().value();
                let root = current.root.clone();
                let content = current.now_playing.as_ref().unwrap();
                let artwork = content.visuals.current().root.clone();
                let lyrics = Rc::clone(&content.metadata.lyrics);
                let footer = content.metadata.footer.clone();
                let status = content.metadata.presentation_status.root.clone();
                assert!(
                    content.metadata.ordinary_metadata_stage.width() > 0,
                    "absent metadata must not collapse the lyric overlay's allocation"
                );
                let long = lyric_presentation("long-metadata.json");
                target.title = long.title.clone();
                let start = Duration::from_secs(1);
                for (millis, artist, album) in [
                    (0, false, false),
                    (100, true, false),
                    (226, true, false),
                    (338, true, false),
                    (338, true, true),
                    (564, true, true),
                    (790, true, true),
                ] {
                    if artist {
                        target.artist = long.artist.clone();
                    }
                    if album {
                        target.album = long.album.clone();
                    }
                    let now = start + Duration::from_millis(millis);
                    view.transition_clock = Instant::now() - now;
                    view.update_in_place(1, &Presentation::NowPlaying(target.clone()), &repository);
                    let current = view.transition.current().value();
                    let content = current.now_playing.as_ref().unwrap();
                    let metadata = &content.metadata;
                    assert_eq!(current.root, root);
                    assert_eq!(content.visuals.current().root, artwork);
                    assert!(Rc::ptr_eq(&metadata.lyrics, &lyrics));
                    assert_eq!(metadata.footer, footer);
                    assert_eq!(metadata.presentation_status.root, status);
                    assert_eq!(footer.opacity(), 1.0);
                    assert!(!view.transition.is_active());
                    if behavior == PresentationBehavior::Dynamic && millis < 225 {
                        assert!(metadata.title.is_none(), "old group stays until invisible");
                    } else {
                        assert_eq!(
                            metadata.title.as_ref().unwrap().layout.text,
                            *long.title.as_ref().unwrap()
                        );
                        assert_eq!(
                            metadata.lyrics.masthead_title.text(),
                            *long.title.as_ref().unwrap()
                        );
                        if behavior == PresentationBehavior::Dynamic && [226, 564].contains(&millis)
                        {
                            assert_eq!(metadata.ordinary_metadata.opacity(), 0.0);
                            assert_eq!(metadata.lyrics.masthead.opacity(), 0.0);
                        }
                    }
                }
                let metadata = &view
                    .transition
                    .current()
                    .value()
                    .now_playing
                    .as_ref()
                    .unwrap()
                    .metadata;
                assert!(metadata.album.is_some());
                assert_eq!(metadata.metadata_fade.opacity(), 1.0);
                assert!(!metadata.metadata_fade.is_active());
                // A track replacement discards the pending metadata destination.
                target.album = None;
                view.update_in_place(2, &Presentation::NowPlaying(target), &repository);
                let replacement = Presentation::NowPlaying(lyric_presentation("playing.json"));
                view.replace(3, &replacement, &repository);
                let original_start = view.transition.started_at();
                let mut enriched = lyric_presentation("playing.json");
                enriched.album = None;
                view.update_in_place(4, &Presentation::NowPlaying(enriched), &repository);
                assert_eq!(view.transition.started_at(), original_start);
                let metadata = &view
                    .transition
                    .current()
                    .value()
                    .now_playing
                    .as_ref()
                    .unwrap()
                    .metadata;
                assert!(metadata.album.is_none());
                assert_eq!(metadata.metadata_fade.opacity(), 1.0);
                window.close();
            }
        }
    }

    fn artwork_view(
        presentation: &roonscape_renderer::NowPlayingPresentation,
        behavior: PresentationBehavior,
    ) -> super::PresentationView {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        super::PresentationView::new(
            0,
            &Presentation::NowPlaying(presentation.clone()),
            Viewport::new(1280, 720),
            &repository,
            super::install_style_providers(typography),
            None,
            RenderingConfiguration::live(typography, behavior),
        )
    }

    fn current_track_artwork_preserves_readable_text() {
        use std::rc::Rc;
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
        for fixture in ["playing.json", "lyrics-one-line.json"] {
            let mut supplied = lyric_presentation(fixture);
            let mut missing = supplied.clone();
            missing.artwork_path = None;
            missing.artwork_revision = None;
            if let Some(lyrics) = &mut missing.lyrics {
                lyrics.current_index = 0;
                missing.playback_position_seconds = Some(169.9);
                supplied.playback_position_seconds = Some(170.0);
            }
            // Prepare both appearances through the view before measuring motion,
            // so debug-build raster preparation is outside the handoff clock.
            let mut initial = missing.clone();
            initial.artwork_path.clone_from(&supplied.artwork_path);
            initial.artwork_revision = supplied.artwork_revision;
            let mut view = artwork_view(&initial, PresentationBehavior::Dynamic);
            gtk::Settings::default()
                .unwrap()
                .set_gtk_enable_animations(false);
            view.update_in_place(0, &Presentation::NowPlaying(missing.clone()), &repository);
            gtk::Settings::default()
                .unwrap()
                .set_gtk_enable_animations(true);
            // Start local cue progression before a new artwork publication.
            view.update_in_place(0, &Presentation::NowPlaying(missing.clone()), &repository);
            missing.lyrics.clone_from(&supplied.lyrics);
            missing.playback_position_seconds = supplied.playback_position_seconds;
            view.update_in_place(0, &Presentation::NowPlaying(missing), &repository);
            let rendered = view
                .transition
                .current()
                .value()
                .now_playing
                .as_ref()
                .unwrap();
            let text = rendered.metadata.root.clone();
            let timing = rendered.metadata.progress.as_ref().unwrap().root.clone();
            let identity = rendered.metadata.identity.root.clone();
            let lyrics = Rc::clone(&rendered.metadata.lyrics);
            view.update_in_place(1, &Presentation::NowPlaying(supplied.clone()), &repository);
            let start = view
                .transition
                .current()
                .value()
                .now_playing
                .as_ref()
                .unwrap()
                .visuals
                .transition
                .started_at()
                .unwrap();
            let mut previous_anchors = None;
            for millis in [0, 112, 180, 225] {
                let now = start + Duration::from_millis(millis);
                view.transition_clock = Instant::now() - now;
                supplied.progress.as_mut().unwrap().elapsed = format!("2:{millis:03}");
                view.update_in_place(2, &Presentation::NowPlaying(supplied.clone()), &repository);
                view.advance_transition(now);
                let current = view.transition.current().value();
                let rendered = current.now_playing.as_ref().unwrap();
                assert_eq!(
                    rendered.metadata.root, text,
                    "artwork preserves the live text widgets"
                );
                assert_eq!(text.opacity(), 1.0, "artwork must not retire readable text");
                assert_eq!(rendered.metadata.progress.as_ref().unwrap().root, timing);
                assert_eq!(
                    rendered.metadata.progress.as_ref().unwrap().elapsed.text(),
                    supplied.progress.as_ref().unwrap().elapsed
                );
                assert_eq!(rendered.metadata.identity.root, identity);
                assert_eq!(
                    rendered.metadata.presentation_status.label.text(),
                    "PLAYING"
                );
                assert!(Rc::ptr_eq(&lyrics, &rendered.metadata.lyrics));
                assert!(!view.transition.is_active(), "artwork is a partial update");
                if millis == 112 {
                    let opacity = rendered.visuals.current().root.opacity();
                    assert!(
                        (0.45..0.55).contains(&opacity),
                        "artwork and its background are halfway blended at 112ms"
                    );
                    assert_eq!(
                        rendered
                            .visuals
                            .transition
                            .outgoing()
                            .unwrap()
                            .value()
                            .root
                            .opacity(),
                        1.0
                    );
                }
                if supplied.lyrics.is_some() && millis > 0 {
                    let frame = lyrics.motion.borrow().frame_at(now);
                    assert!(
                        frame.cue_motion_active,
                        "artwork must not restart or settle a Natural Cue Handoff: now={now:?}, start={start:?}, frame={frame:?}"
                    );
                    if let Some(previous) = previous_anchors {
                        assert_ne!(frame.anchors, previous);
                    }
                    previous_anchors = Some(frame.anchors);
                }
            }
            let current = view.transition.current().value();
            assert_eq!(
                current.palette,
                roonscape_renderer::resolve_presentation(
                    &Presentation::NowPlaying(supplied),
                    &repository
                )
                .palette
            );
            let visuals = &current.now_playing.as_ref().unwrap().visuals;
            assert!(
                !visuals.transition.is_active(),
                "the artwork crossfade ends at 225ms"
            );
            assert_eq!(visuals.current().root.opacity(), 1.0);
        }
    }

    fn artwork_updates_recolor_the_existing_lyric_reel() {
        use std::time::Duration;
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for animated in [true, false] {
            gtk::Settings::default()
                .unwrap()
                .set_gtk_enable_animations(animated);
            let mut target = lyric_presentation("lyrics-one-line.json");
            let mut view = artwork_view(&target, PresentationBehavior::Dynamic);
            let window = gtk::Window::new();
            window.set_default_size(1280, 720);
            window.set_child(Some(&view.root()));
            window.present();
            for _ in 0..40 {
                while gtk::glib::MainContext::default().iteration(false) {}
                std::thread::sleep(Duration::from_millis(5));
            }
            let reel = std::rc::Rc::clone(
                &view
                    .transition
                    .current()
                    .value()
                    .now_playing
                    .as_ref()
                    .unwrap()
                    .metadata
                    .lyrics
                    .reel,
            );
            let before = reel.visible_cues();
            target.artwork_path = Some("src/shared/fixtures/artwork/light.jpg".into());
            target.artwork_revision = Some(9);
            view.update_in_place(1, &Presentation::NowPlaying(target.clone()), &repository);
            if animated {
                let start = view
                    .transition
                    .current()
                    .value()
                    .now_playing
                    .as_ref()
                    .unwrap()
                    .visuals
                    .transition
                    .started_at()
                    .unwrap();
                for millis in [112, 225] {
                    view.advance_transition(start + Duration::from_millis(millis));
                    let cues = reel.visible_cues();
                    let active = cues
                        .iter()
                        .find(|cue| cue.cue.color_weights == [0.0, 1.0, 0.0])
                        .unwrap();
                    assert_eq!(
                        active.color,
                        view.transition.current().value().palette.primary_text,
                        "lyric colors follow intermediate palettes too"
                    );
                }
            }
            let current = view.transition.current().value();
            let after = reel.visible_cues();
            assert!(
                !after.is_empty(),
                "the lyric palette check needs painted cues"
            );
            let active = after
                .iter()
                .find(|cue| cue.cue.color_weights == [0.0, 1.0, 0.0])
                .unwrap();
            assert_eq!(
                active.color, current.palette.primary_text,
                "painted lyrics must use the destination artwork's text palette"
            );
            assert_eq!(
                before
                    .iter()
                    .map(|cue| (&cue.cue.text, cue.y, cue.height, cue.layout.as_ptr()))
                    .collect::<Vec<_>>(),
                after
                    .iter()
                    .map(|cue| (&cue.cue.text, cue.y, cue.height, cue.layout.as_ptr()))
                    .collect::<Vec<_>>(),
                "recoloring must preserve the fitted reel and its positions"
            );
            window.destroy();
        }
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
    }

    fn repeated_artwork_updates_preserve_visible_appearance() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut target = lyric_presentation("playing.json");
        let mut view = artwork_view(&target, PresentationBehavior::Dynamic);
        for (revision, path) in [
            (2, Some("src/shared/fixtures/artwork/revised.jpg")),
            (3, None),
            (4, Some("src/shared/fixtures/artwork/playing.jpg")),
            (5, Some("src/shared/fixtures/artwork/unavailable.jpg")),
        ] {
            let before = view.transition.current().value();
            let palette = before.palette;
            let visible: Vec<_> = before
                .now_playing
                .as_ref()
                .unwrap()
                .visuals
                .layers()
                .map(|(_, layer)| (layer.root.clone(), layer.root.opacity()))
                .collect();
            target.artwork_path = path.map(str::to_owned);
            target.artwork_revision = path.map(|_| revision);
            view.update_in_place(
                revision,
                &Presentation::NowPlaying(target.clone()),
                &repository,
            );
            let current = view.transition.current().value();
            assert_eq!(
                current.palette, palette,
                "retargeting starts at the visible palette"
            );
            for (root, opacity) in &visible {
                assert!(
                    root.parent().is_some(),
                    "visible artwork is retained beneath the new destination"
                );
                assert_eq!(
                    root.opacity(),
                    *opacity,
                    "a repeated update must not jump to an opaque intermediate artwork"
                );
            }
            let start = current
                .now_playing
                .as_ref()
                .unwrap()
                .visuals
                .transition
                .started_at()
                .unwrap();
            let now = start + Duration::from_millis(112);
            view.advance_transition(now);
            view.transition_clock = Instant::now() - now;
            assert_eq!(view.transition.current().value().text_opacity(), 1.0);
        }
        let visuals = &view
            .transition
            .current()
            .value()
            .now_playing
            .as_ref()
            .unwrap()
            .visuals;
        let obsolete: Vec<_> = visuals
            .layers()
            .skip(1)
            .map(|(_, layer)| layer.root.clone())
            .collect();
        let end = visuals.transition.started_at().unwrap() + Duration::from_millis(225);
        view.advance_transition(end);
        let current = view.transition.current().value();
        assert_eq!(current.palette, PresentationPalette::fallback());
        assert!(
            current
                .now_playing
                .as_ref()
                .unwrap()
                .visuals
                .current()
                .artwork
                .source_key
                .is_none()
        );
        for root in obsolete {
            assert!(
                root.parent().is_none(),
                "completed updates release obsolete artwork"
            );
        }

        for behavior in [
            PresentationBehavior::Dynamic,
            PresentationBehavior::StaticFixture,
        ] {
            gtk::Settings::default()
                .unwrap()
                .set_gtk_enable_animations(false);
            let mut view = artwork_view(&target, behavior);
            target = lyric_presentation("playing.json");
            view.update_in_place(10, &Presentation::NowPlaying(target.clone()), &repository);
            let current = view.transition.current().value();
            assert_eq!(current.text_opacity(), 1.0);
            assert!(
                !current
                    .now_playing
                    .as_ref()
                    .unwrap()
                    .visuals
                    .transition
                    .is_active()
            );
            assert_eq!(
                current.palette,
                roonscape_renderer::resolve_presentation(
                    &Presentation::NowPlaying(target.clone()),
                    &repository
                )
                .palette
            );
            target.artwork_path = None;
            target.artwork_revision = None;
            view.update_in_place(11, &Presentation::NowPlaying(target.clone()), &repository);
            assert_eq!(
                view.transition.current().value().palette,
                PresentationPalette::fallback()
            );
        }
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
    }

    fn track_replacement_supersedes_artwork_without_restarting_its_reveal() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut target = lyric_presentation("playing.json");
        let mut view = artwork_view(&target, PresentationBehavior::Dynamic);
        target.artwork_path = None;
        target.artwork_revision = None;
        view.update_in_place(1, &Presentation::NowPlaying(target.clone()), &repository);
        let start = view
            .transition
            .current()
            .value()
            .now_playing
            .as_ref()
            .unwrap()
            .visuals
            .transition
            .started_at()
            .unwrap();
        let now = start + Duration::from_millis(112);
        view.advance_transition(now);
        view.transition_clock = Instant::now() - now;
        let obsolete: Vec<_> = view
            .transition
            .current()
            .value()
            .now_playing
            .as_ref()
            .unwrap()
            .visuals
            .layers()
            .map(|(_, layer)| (layer.root.clone(), layer.root.opacity()))
            .collect();
        target.title = Some("A new track".into());
        view.replace(2, &Presentation::NowPlaying(target.clone()), &repository);
        let track_start = view.transition.started_at().unwrap();
        let text = view
            .transition
            .current()
            .value()
            .now_playing
            .as_ref()
            .unwrap()
            .metadata
            .root
            .clone();
        for millis in [100, 337] {
            let now = track_start + Duration::from_millis(millis);
            view.advance_transition(now);
            view.transition_clock = Instant::now() - now;
            let before = view.transition.current().value().root.opacity();
            target.artwork_path = Some(
                if millis == 100 {
                    "src/shared/fixtures/artwork/playing.jpg"
                } else {
                    "src/shared/fixtures/artwork/revised.jpg"
                }
                .into(),
            );
            target.artwork_revision = Some(millis);
            view.update_in_place(
                3 + millis,
                &Presentation::NowPlaying(target.clone()),
                &repository,
            );
            assert_eq!(
                view.transition.started_at(),
                Some(track_start),
                "artwork must not restart the Now Playing Transition"
            );
            let current = view.transition.current().value();
            assert_eq!(current.root.opacity(), before);
            assert_eq!(current.now_playing.as_ref().unwrap().metadata.root, text);
            assert_eq!(
                text.opacity(),
                1.0,
                "the coordinated reveal still owns text visibility"
            );
            if millis == 100 {
                assert!(
                    !current
                        .now_playing
                        .as_ref()
                        .unwrap()
                        .visuals
                        .transition
                        .is_active(),
                    "artwork joins an invisible destination immediately"
                );
            } else {
                assert!(
                    current
                        .now_playing
                        .as_ref()
                        .unwrap()
                        .visuals
                        .transition
                        .is_active()
                );
            }
            for (root, opacity) in &obsolete {
                assert_eq!(
                    root.opacity(),
                    *opacity,
                    "obsolete artwork cannot finish revealing on a new track"
                );
            }
        }
        view.advance_transition(track_start + Duration::from_millis(450));
        assert!(
            !view.transition.is_active(),
            "the original coordinated reveal finishes on time"
        );
        let artwork_end = view
            .transition
            .current()
            .value()
            .now_playing
            .as_ref()
            .unwrap()
            .visuals
            .transition
            .started_at()
            .unwrap()
            + Duration::from_millis(225);
        view.advance_transition(artwork_end);
        assert_eq!(
            view.transition.current().value().palette,
            roonscape_renderer::resolve_presentation(
                &Presentation::NowPlaying(target),
                &repository
            )
            .palette
        );
        assert_eq!(view.transition.current().value().text_opacity(), 1.0);
    }

    fn now_playing_artwork_and_text_reveal_together() {
        use std::time::Duration;
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let presentation = |fixture: &str| {
            presentation_from_snapshot(
                &parse_snapshot(
                    &std::fs::read_to_string(repository.join("src/shared/fixtures").join(fixture))
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
        for source in ["stopped.json", "loading-empty.json", "playing.json"] {
            for target in ["lyrics-one-line.json", "missing-artwork.json"] {
                let mut view = super::PresentationView::new(
                    0,
                    &presentation(source),
                    Viewport::new(1280, 720),
                    &repository,
                    super::install_style_providers(typography),
                    None,
                    RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
                );
                view.replace(1, &presentation(target), &repository);
                let start = view
                    .transition
                    .started_at()
                    .unwrap_or_else(|| view.transition_clock.elapsed());
                for millis in [0, 112, 225, 337, 449] {
                    view.advance_transition(start + Duration::from_millis(millis));
                    let outgoing = view.transition.outgoing().unwrap().value();
                    let incoming = view.transition.current().value();
                    assert_eq!(
                        outgoing.root.opacity(),
                        1.0,
                        "the outgoing background remains opaque"
                    );
                    if millis < 225 {
                        assert_eq!(
                            incoming.root.opacity(),
                            0.0,
                            "incoming artwork and text must both wait for outgoing text"
                        );
                    } else {
                        assert_eq!(
                            outgoing.text_opacity(),
                            0.0,
                            "outgoing text retires before the reveal"
                        );
                        assert_eq!(
                            incoming.text_opacity(),
                            1.0,
                            "text shares the artwork's layer opacity without a second fade"
                        );
                    }
                    if millis == 337 {
                        assert!(
                            (incoming.root.opacity() - 0.5).abs() < 0.02,
                            "artwork and text are halfway revealed together"
                        );
                    }
                }
                view.advance_transition(start + Duration::from_millis(450));
                assert!(
                    !view.transition.is_active(),
                    "the reveal completes at 450ms"
                );
                assert_eq!(view.transition.current().value().root.opacity(), 1.0);

                gtk::Settings::default()
                    .unwrap()
                    .set_gtk_enable_animations(false);
                view.replace(2, &presentation(source), &repository);
                view.replace(3, &presentation(target), &repository);
                assert!(!view.transition.is_active());
                assert_eq!(view.transition.current().value().root.opacity(), 1.0);
                assert_eq!(view.transition.current().value().text_opacity(), 1.0);
                gtk::Settings::default()
                    .unwrap()
                    .set_gtk_enable_animations(true);
            }
        }
    }

    fn opening_lyrics_enter_from_regular_composition() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        let snapshot =
            parse_snapshot(include_str!("../../shared/fixtures/lyrics-opening.json")).unwrap();
        let incoming = |millis: u64, lyrics_available: bool| {
            let mut snapshot = snapshot.clone();
            snapshot
                .timing
                .as_mut()
                .unwrap()
                .position
                .as_mut()
                .unwrap()
                .seconds = millis as f64 / 1_000.0;
            if !lyrics_available {
                snapshot.lyrics = None;
            }
            presentation_from_snapshot(&snapshot).unwrap()
        };
        for (behavior, platform_animations) in [
            (PresentationBehavior::Dynamic, true),
            (PresentationBehavior::Dynamic, false),
            (PresentationBehavior::StaticFixture, true),
        ] {
            gtk::Settings::default()
                .unwrap()
                .set_gtk_enable_animations(platform_animations);
            let animated = behavior == PresentationBehavior::Dynamic && platform_animations;
            for discovery_millis in [0, 100, 300] {
                let mut view = super::PresentationView::new(
                    0,
                    &Presentation::NowPlaying(lyric_presentation("playing.json")),
                    Viewport::new(1280, 720),
                    &repository,
                    super::install_style_providers(typography),
                    None,
                    RenderingConfiguration::live(typography, behavior),
                );
                view.replace(1, &incoming(0, discovery_millis == 0), &repository);
                let transition_start = view.transition.started_at();
                let start = transition_start.unwrap_or_else(|| view.transition_clock.elapsed());
                let initial = lyric_motion_frame(view.transition.current().value(), start);
                assert_eq!(
                    initial.composition_motion_active,
                    animated && discovery_millis == 0
                );
                assert_eq!(view.transition.is_active(), animated);
                if animated || discovery_millis != 0 {
                    assert_eq!(
                        view.transition
                            .current()
                            .value()
                            .now_playing
                            .as_ref()
                            .unwrap()
                            .metadata
                            .rendered_composition_progress(),
                        0.0,
                        "the incoming song inherits regular geometry before its first layout"
                    );
                } else {
                    assert_eq!(initial.composition_progress, 1.0);
                }
                for millis in [100, 200, 300, 337, 400, 450, 600, 900] {
                    let now = start + Duration::from_millis(millis);
                    view.transition_clock = Instant::now() - now;
                    let available = millis >= discovery_millis;
                    let before = view.transition.current().value().root.opacity();
                    view.update_in_place(2, &incoming(millis, available), &repository);
                    assert_eq!(
                        view.transition.started_at(),
                        transition_start.filter(|_| millis <= 450),
                        "discovery must not restart the Now Playing Transition"
                    );
                    assert_eq!(view.transition.current().value().root.opacity(), before);
                    view.advance_transition(now);
                    let current = view.transition.current().value();
                    let frame = lyric_motion_frame(current, now);
                    if animated && millis == discovery_millis {
                        assert_eq!(
                            current
                                .now_playing
                                .as_ref()
                                .unwrap()
                                .metadata
                                .rendered_composition_progress(),
                            0.0,
                            "discovery begins at the displayed geometry"
                        );
                        assert_rendered_composition_ownership(current, 1.0, 0.0, 0.0);
                    }
                    if available {
                        let expected_index = if millis < 200 {
                            0
                        } else if millis < 400 {
                            1
                        } else {
                            2
                        };
                        let focal = frame
                            .cues
                            .iter()
                            .find(|cue| cue.role == crate::lyric_motion::LyricColorRole::Focal)
                            .unwrap();
                        assert_eq!(
                            focal.index, expected_index,
                            "cue selection follows playback even before readability"
                        );
                        if !animated {
                            assert_eq!(frame.composition_progress, 1.0);
                            assert!(!frame.composition_motion_active);
                        }
                    }
                    if animated && millis == 337 {
                        assert!(current.root.opacity() > 0.0 && current.root.opacity() < 1.0);
                        assert!(
                            frame.composition_progress > 0.0 && frame.composition_progress < 1.0
                        );
                        assert!(
                            frame.composition_motion_active,
                            "entry overlaps the Now Playing reveal"
                        );
                    }
                    if animated && millis == 450 {
                        assert!(!view.transition.is_active());
                        assert!(
                            frame.composition_motion_active,
                            "the 580 ms entry continues after the reveal"
                        );
                    }
                    if millis == 900 {
                        assert_eq!(frame.composition_progress, 1.0);
                        assert!(!frame.composition_motion_active);
                    }
                }
            }
        }
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
    }

    fn unknown_incoming_lyrics_retain_composition_until_expiry() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        let mut view = super::PresentationView::new(
            0,
            &Presentation::NowPlaying(lyric_presentation("lyrics-one-line.json")),
            Viewport::new(1280, 720),
            &repository,
            super::install_style_providers(typography),
            None,
            RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
        );
        let incoming = Presentation::NowPlaying(lyric_presentation("paused.json"));
        view.replace(1, &incoming, &repository);
        let start = view.transition_clock.elapsed();
        assert_eq!(
            lyric_motion_frame(view.transition.current().value(), start).composition_progress,
            1.0
        );
        for (millis, expected) in [(450, 1.0), (4_500, 1.0), (5_100, 1.0), (5_700, 0.0)] {
            let now = start + Duration::from_millis(millis);
            view.transition_clock = Instant::now() - now;
            view.update_in_place(2, &incoming, &repository);
            view.advance_transition(now);
            let frame = lyric_motion_frame(view.transition.current().value(), now);
            assert!(frame.cues.is_empty());
            assert_eq!(frame.composition_progress, expected);
        }
    }

    fn rapid_replacement_inherits_the_visible_composition() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        let mut view = super::PresentationView::new(
            0,
            &Presentation::NowPlaying(lyric_presentation("playing.json")),
            Viewport::new(1280, 720),
            &repository,
            super::install_style_providers(typography),
            None,
            RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
        );
        let hidden = Presentation::NowPlaying(lyric_presentation("lyrics-one-line.json"));
        view.replace(1, &hidden, &repository);
        let now = view.transition.started_at().unwrap() + Duration::from_millis(200);
        view.transition_clock = Instant::now() - now;
        view.update_in_place(1, &hidden, &repository);
        view.advance_transition(now);
        assert_eq!(view.transition.current().value().root.opacity(), 0.0);
        assert!(
            lyric_motion_frame(view.transition.current().value(), now).composition_progress > 0.0
        );
        view.replace(
            2,
            &Presentation::NowPlaying(lyric_presentation("paused.json")),
            &repository,
        );
        let current = view.transition.current().value();
        assert_eq!(
            lyric_motion_frame(current, view.transition_clock.elapsed()).composition_progress,
            0.0
        );
        assert_eq!(
            current
                .now_playing
                .as_ref()
                .unwrap()
                .metadata
                .rendered_composition_progress(),
            0.0
        );
    }

    fn replacement_preserves_rendered_intermediate_composition() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        for entering in [false, true] {
            let mut source = lyric_presentation("lyrics-one-line.json");
            let cues = source.lyrics.clone();
            if entering {
                source.lyrics = None;
            }
            let mut view = super::PresentationView::new(
                0,
                &Presentation::NowPlaying(source.clone()),
                Viewport::new(1280, 720),
                &repository,
                super::install_style_providers(typography),
                None,
                RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
            );
            source.lyrics = if entering { cues } else { None };
            let start = Duration::from_secs(1);
            view.transition_clock = Instant::now() - start;
            view.update_in_place(1, &Presentation::NowPlaying(source.clone()), &repository);
            let midpoint = start + Duration::from_millis(290);
            view.transition_clock = Instant::now() - midpoint;
            view.update_in_place(1, &Presentation::NowPlaying(source), &repository);
            let before = view
                .transition
                .current()
                .value()
                .now_playing
                .as_ref()
                .unwrap()
                .metadata
                .rendered_composition_progress();
            assert!(before > 0.1 && before < 0.9);
            view.replace(
                2,
                &Presentation::NowPlaying(lyric_presentation("paused.json")),
                &repository,
            );
            let after = view
                .transition
                .current()
                .value()
                .now_playing
                .as_ref()
                .unwrap()
                .metadata
                .rendered_composition_progress();
            assert!(
                (before - after).abs() < 0.01,
                "replacement geometry changed: {before} -> {after}"
            );
        }
    }

    fn song_replacement_removes_every_previous_lyric_reel() {
        use std::time::Duration;
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        for behavior in [
            PresentationBehavior::Dynamic,
            PresentationBehavior::StaticFixture,
        ] {
            for platform_animations in [true, false] {
                gtk::Settings::default()
                    .unwrap()
                    .set_gtk_enable_animations(platform_animations);
                for has_incoming_lyrics in [false, true] {
                    let mut source = lyric_presentation("lyrics-one-line.json");
                    let mut view = super::PresentationView::new(
                        0,
                        &Presentation::NowPlaying(source.clone()),
                        Viewport::new(1280, 720),
                        &repository,
                        super::install_style_providers(typography),
                        None,
                        RenderingConfiguration::live(typography, behavior),
                    );
                    // Replace during a Natural Cue Handoff, so the old reel
                    // includes its departure as well as earlier/upcoming cues.
                    source.lyrics.as_mut().unwrap().current_index = 2;
                    view.update_in_place(1, &Presentation::NowPlaying(source), &repository);
                    let mut target = lyric_presentation("lyrics-one-line.json");
                    if !has_incoming_lyrics {
                        target.lyrics = None;
                    }
                    // Interrupt both the invisible departure and partially
                    // revealed destinations, building a retained composite.
                    for (revision, millis) in [(2, 0), (3, 112), (4, 337), (5, 337)] {
                        if let Some(start) = view.transition.started_at() {
                            view.advance_transition(start + Duration::from_millis(millis));
                        }
                        target.title = Some(format!("Incoming song {revision}"));
                        view.replace(
                            revision,
                            &Presentation::NowPlaying(target.clone()),
                            &repository,
                        );
                        // Assert before advancing the transition or yielding
                        // to GTK: the very first replacement frame must be clean.
                        for layer in view
                            .transition
                            .outgoing()
                            .into_iter()
                            .chain(view.retained_layers.iter())
                        {
                            let old = layer.value().now_playing.as_ref().unwrap();
                            assert_eq!(
                                old.metadata.lyrics.reel.widget.opacity(),
                                0.0,
                                "no previous-song cue may paint in any outgoing or retained layer"
                            );
                        }
                        let current = view.transition.current().value();
                        let lyrics = &current.now_playing.as_ref().unwrap().metadata.lyrics;
                        assert_eq!(lyrics.reel.widget.opacity(), 1.0);
                        assert_eq!(
                            lyrics
                                .motion
                                .borrow()
                                .frame_at(Duration::ZERO)
                                .cues
                                .is_empty(),
                            !has_incoming_lyrics,
                            "only the incoming song supplies its lyric destination"
                        );
                        // A resize must not restore retired lyrics or remove
                        // the layout that the artwork/metadata transition uses.
                        view.apply_viewport(if revision % 2 == 0 {
                            Viewport::new(1600, 900)
                        } else {
                            Viewport::new(1280, 720)
                        });
                        for layer in view
                            .transition
                            .outgoing()
                            .into_iter()
                            .chain(view.retained_layers.iter())
                        {
                            let old = layer.value().now_playing.as_ref().unwrap();
                            assert_eq!(old.metadata.lyrics.reel.widget.opacity(), 0.0);
                        }
                    }
                }
            }
        }
        gtk::Settings::default()
            .unwrap()
            .set_gtk_enable_animations(true);
    }

    fn incoming_foreground_updates_during_the_coordinated_reveal() {
        use std::time::{Duration, Instant};
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        for interrupt in [false, true] {
            let mut incoming = lyric_presentation("lyrics-one-line.json");
            incoming.status = lyric_presentation("paused.json").status;
            let mut view = super::PresentationView::new(
                0,
                &Presentation::NowPlaying(lyric_presentation("playing.json")),
                Viewport::new(1280, 720),
                &repository,
                super::install_style_providers(typography),
                None,
                RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
            );
            if interrupt {
                let mut retiring = lyric_presentation("lyrics-one-line.json");
                retiring.title = Some("Retiring track".into());
                view.replace(1, &Presentation::NowPlaying(retiring), &repository);
                let now = view.transition_clock.elapsed() + Duration::from_millis(337);
                view.advance_transition(now);
            }
            view.replace(1, &Presentation::NowPlaying(incoming.clone()), &repository);
            let retiring_timing = view
                .transition
                .outgoing()
                .unwrap()
                .value()
                .now_playing
                .as_ref()
                .unwrap()
                .metadata
                .progress
                .as_ref()
                .unwrap()
                .elapsed
                .clone();
            let retiring_elapsed = retiring_timing.text();
            let start = view
                .transition
                .started_at()
                .unwrap_or_else(|| view.transition_clock.elapsed());
            incoming.status = lyric_presentation("playing.json").status;
            let mut previous_anchors = None;
            for millis in [0, 226, 337, 400] {
                // Drive both transition and in-place updates from the same test clock.
                let now = start + Duration::from_millis(millis);
                view.transition_clock = Instant::now() - now;
                incoming.progress.as_mut().unwrap().elapsed = format!("3:{:02}", millis / 10);
                if millis >= 226 {
                    incoming.lyrics.as_mut().unwrap().current_index = 2;
                }
                view.update_in_place(2, &Presentation::NowPlaying(incoming.clone()), &repository);
                view.advance_transition(now);
                assert_eq!(
                    retiring_timing.text(),
                    retiring_elapsed,
                    "new timing must never be applied to a retiring track"
                );
                let current = view.transition.current().value();
                let metadata = &current.now_playing.as_ref().unwrap().metadata;
                assert_eq!(
                    metadata.progress.as_ref().unwrap().elapsed.text(),
                    incoming.progress.as_ref().unwrap().elapsed
                );
                if millis >= 337 {
                    assert!(current.root.opacity() > 0.0 && current.root.opacity() < 1.0);
                    assert_eq!(metadata.presentation_status.label.text(), "PLAYING");
                    assert!(metadata.presentation_status.root.opacity() > 0.0);
                    let frame = metadata.lyrics.motion.borrow().frame_at(now);
                    assert!(
                        frame.cue_motion_active,
                        "Natural Cue Handoff stays live during the reveal"
                    );
                    if let Some(previous) = previous_anchors {
                        assert_ne!(
                            frame.anchors, previous,
                            "the incoming Lyric Reel keeps moving"
                        );
                    }
                    previous_anchors = Some(frame.anchors);
                }
            }
        }
    }

    fn full_field_replacement_never_superimposes_messages() {
        use std::time::Duration;
        let presentation = |fixture: &str| {
            let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            presentation_from_snapshot(
                &parse_snapshot(
                    &std::fs::read_to_string(repository.join("src/shared/fixtures").join(fixture))
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let typography = roonscape_renderer::select_typography(&HashSet::new());
        let mut view = super::PresentationView::new(
            0,
            &presentation("disconnected.json"),
            Viewport::new(1280, 720),
            &repository,
            super::install_style_providers(typography),
            None,
            RenderingConfiguration::live(typography, PresentationBehavior::Dynamic),
        );
        view.replace(1, &presentation("stopped.json"), &repository);
        let start = view
            .transition
            .started_at()
            .unwrap_or_else(|| view.transition_clock.elapsed());
        for millis in [0, 100, 200, 250, 350, 449, 500, 600] {
            view.advance_transition(start + Duration::from_millis(millis));
            let outgoing = view.transition.outgoing().unwrap().value();
            let incoming = view.transition.current().value();
            let old_opacity = outgoing.full_field.as_ref().unwrap().copy.opacity();
            let new_opacity = incoming.full_field.as_ref().unwrap().copy.opacity();
            if millis == 350 {
                assert_eq!(
                    new_opacity, 0.0,
                    "Full-field text waits until the background crossfade finishes"
                );
            }
            if millis == 600 {
                assert!(
                    new_opacity > 0.0 && new_opacity < 1.0,
                    "Full-field replacement text fades in after the background"
                );
            }
            assert!(
                old_opacity == 0.0 || new_opacity == 0.0,
                "Full-field messages must not coexist at {millis}ms"
            );
            if millis < 225 {
                assert_eq!(incoming.root.opacity(), 0.0);
            } else {
                assert_eq!(old_opacity, 0.0);
                assert!(incoming.root.opacity() > 0.0);
            }
        }
        view.advance_transition(start + Duration::from_millis(750));
        assert!(!view.transition.is_active());
        assert_eq!(
            view.transition
                .current()
                .value()
                .full_field
                .as_ref()
                .unwrap()
                .copy
                .opacity(),
            1.0
        );
    }

    fn blank_promotion_preserves_context_and_an_interrupted_departure() {
        let mut before = lyric_presentation("lyrics-one-line.json");
        before
            .lyrics
            .as_mut()
            .unwrap()
            .timeline
            .insert(2, String::new());
        let mut blank = before.clone();
        let cue = blank.lyrics.as_mut().unwrap();
        cue.current_index += 1;
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
            departing.cues.iter().any(|cue| cue.role
                == crate::lyric_motion::LyricColorRole::Upcoming
                && cue.opacity == 1.0),
            "blank entry should retain anticipation"
        );
        let mut after = blank;
        let cue = after.lyrics.as_mut().unwrap();
        cue.current_index += 1;
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
                .find(|cue| cue.role == crate::lyric_motion::LyricColorRole::Earlier)
                .unwrap()
                .clone()
        };
        assert_eq!(
            outgoing(&departing).color_weights,
            outgoing(&promoting).color_weights
        );
        assert_eq!(
            departing.anchors,
            promoting
                .anchors
                .iter()
                .copied()
                .filter(|(_, weight)| *weight > 0.0)
                .collect::<Vec<_>>()
        );
        assert_eq!(outgoing(&departing).emphasis, outgoing(&promoting).emphasis);
    }

    fn continuous_snapshot_at_a_cue_boundary_animates() {
        for viewport in [Viewport::new(1280, 720), Viewport::new(3840, 2400)] {
            let mut before = lyric_presentation("lyrics-one-line.json");
            before.playback_position_seconds = Some(179.95);
            let mut rendered = rendered_now_playing(&before, PresentationBehavior::Dynamic);
            rendered.apply_viewport(viewport);
            rendered.update_in_place(
                1,
                &Presentation::NowPlaying(before.clone()),
                std::time::Duration::ZERO,
                Some(viewport),
            );
            let mut incoming = before;
            incoming.playback_position_seconds = Some(180.0);
            incoming.lyrics.as_mut().unwrap().current_index += 1;
            let boundary = std::time::Duration::from_millis(50);
            rendered.update_in_place(
                2,
                &Presentation::NowPlaying(incoming.clone()),
                boundary,
                Some(viewport),
            );
            let midpoint =
                lyric_motion_frame(&rendered, boundary + std::time::Duration::from_millis(150));
            assert_eq!(midpoint.cause, LyricMotionCause::NaturalCueHandoff);
            assert!(midpoint.cue_motion_active);
            assert!(
                midpoint
                    .anchors
                    .iter()
                    .all(|(_, weight)| *weight > 0.0 && *weight < 1.0)
            );

            incoming.playback_position_seconds = Some(180.62);
            rendered.update_in_place(
                3,
                &Presentation::NowPlaying(incoming),
                boundary + std::time::Duration::from_millis(620),
                Some(viewport),
            );
            assert!(
                !lyric_motion_frame(&rendered, boundary + std::time::Duration::from_millis(620))
                    .cue_motion_active
            );
        }
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

    fn authoritative_relocations_render_destination_content_with_timing() {
        use roonscape_renderer::{LyricCue, Playback, SynchronizedLyrics};
        use std::time::Duration;

        for behavior in [
            PresentationBehavior::Dynamic,
            PresentationBehavior::StaticFixture,
        ] {
            for (first_at, destination, expected_index, preparing) in [
                (8.0, 0.0, None, false),
                (2.0, 0.0, Some(0), true),
                (0.0, 0.0, Some(0), false),
                (8.0, 12.0, Some(1), false),
                (8.0, 8.0, Some(0), false),
                (8.0, 30.0, None, false),
            ] {
                let mut snapshot =
                    parse_snapshot(include_str!("../../shared/fixtures/lyrics-one-line.json"))
                        .unwrap();
                snapshot.lyrics = Some(SynchronizedLyrics {
                    cues: [
                        (first_at, "Opening"),
                        (12.0, ""),
                        (16.0, "Later passage"),
                        (20.0, "Upcoming"),
                    ]
                    .map(|(at_seconds, text)| LyricCue {
                        at_seconds,
                        text: text.into(),
                    })
                    .to_vec(),
                });
                snapshot
                    .timing
                    .as_mut()
                    .unwrap()
                    .position
                    .as_mut()
                    .unwrap()
                    .seconds = 18.0;
                let Presentation::NowPlaying(before) =
                    presentation_from_snapshot(&snapshot).unwrap()
                else {
                    panic!("Now Playing");
                };
                let viewport = Viewport::new(1280, 720);
                let mut rendered = rendered_now_playing(&before, behavior);
                rendered.apply_viewport(viewport);
                rendered.update_in_place(
                    1,
                    &Presentation::NowPlaying(before),
                    Duration::ZERO,
                    Some(viewport),
                );

                // STARTING preserves the passage until Authoritative Timing resets.
                snapshot.playback = Some(Playback::Loading);
                rendered.update_in_place(
                    2,
                    &presentation_from_snapshot(&snapshot).unwrap(),
                    Duration::from_millis(100),
                    Some(viewport),
                );
                assert_rendered_lyric_roles(
                    &rendered,
                    Some("Opening"),
                    Some("Later passage"),
                    Some("Upcoming"),
                );

                snapshot.playback = Some(Playback::Playing);
                snapshot
                    .timing
                    .as_mut()
                    .unwrap()
                    .position
                    .as_mut()
                    .unwrap()
                    .seconds = destination;
                let after = presentation_from_snapshot(&snapshot).unwrap();
                let Presentation::NowPlaying(destination_presentation) = &after else {
                    panic!("Now Playing")
                };
                assert_eq!(
                    destination_presentation
                        .lyrics
                        .as_ref()
                        .map(|lyrics| lyrics.current_index),
                    expected_index
                );
                assert_eq!(
                    destination_presentation
                        .lyrics
                        .as_ref()
                        .is_some_and(|lyrics| lyrics.preparing),
                    preparing
                );
                let reset_at = Duration::from_millis(200);
                rendered.update_in_place(3, &after, reset_at, Some(viewport));
                let frame = lyric_motion_frame(&rendered, reset_at);
                assert!(!frame.cue_motion_active);
                let metadata = &rendered.now_playing.as_ref().unwrap().metadata;
                assert_eq!(
                    metadata.progress.as_ref().unwrap().elapsed.text(),
                    destination_presentation.progress.as_ref().unwrap().elapsed
                );
                if expected_index.is_none() {
                    assert!(frame.cues.is_empty());
                    assert_rendered_lyric_roles(&rendered, None, None, None);
                } else {
                    let focal = frame.cues.iter().find(|cue| cue.emphasis == 1.0).unwrap();
                    assert_eq!(
                        focal.text,
                        if preparing || expected_index == Some(1) {
                            ""
                        } else {
                            "Opening"
                        }
                    );
                    assert_eq!(frame.anchors.len(), 1);
                    assert_rendered_lyric_roles(
                        &rendered,
                        if expected_index == Some(1) {
                            Some("Opening")
                        } else {
                            None
                        },
                        if preparing || expected_index == Some(1) {
                            None
                        } else {
                            Some("Opening")
                        },
                        Some(if preparing {
                            "Opening"
                        } else {
                            "Later passage"
                        }),
                    );
                }
                let exits = expected_index.is_none();
                let animated_exit = exits && behavior == PresentationBehavior::Dynamic;
                assert_eq!(frame.composition_motion_active, animated_exit);
                assert_eq!(
                    frame.composition_progress,
                    if exits && !animated_exit { 0.0 } else { 1.0 }
                );
                if animated_exit {
                    assert_rendered_composition_ownership(&rendered, 0.0, 1.0, 1.0);
                    let artwork_start = rendered
                        .now_playing
                        .as_ref()
                        .unwrap()
                        .metadata
                        .rendered_composition_progress();
                    assert_eq!(artwork_start, 1.0);
                    rendered.update_in_place(
                        3,
                        &after,
                        reset_at + Duration::from_millis(290),
                        Some(viewport),
                    );
                    let metadata = &rendered.now_playing.as_ref().unwrap().metadata;
                    assert!(
                        metadata.rendered_composition_progress() > 0.0
                            && metadata.rendered_composition_progress() < 1.0
                    );
                    assert_eq!(metadata.ordinary_metadata.opacity(), 0.0);
                    assert!(
                        metadata.lyrics.masthead.opacity() > 0.0
                            && metadata.lyrics.masthead.opacity() < 1.0
                    );
                    assert_rendered_lyric_roles(&rendered, None, None, None);
                    rendered.update_in_place(
                        3,
                        &after,
                        reset_at + Duration::from_millis(580),
                        Some(viewport),
                    );
                }
                assert_rendered_composition_ownership(
                    &rendered,
                    if exits { 1.0 } else { 0.0 },
                    if exits { 0.0 } else { 1.0 },
                    if exits { 0.0 } else { 1.0 },
                );
            }
        }
    }

    fn short_blanks_return_during_their_departure() {
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
            .seconds = 9.9;
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
        for (seconds, milliseconds, expected_cause) in [
            (9.999, 99, LyricMotionCause::Settled),
            (10.0, 100, LyricMotionCause::IntentionalBlankEntry),
            (10.299, 399, LyricMotionCause::IntentionalBlankEntry),
            (10.3, 400, LyricMotionCause::IntentionalBlankExit),
        ] {
            snapshot
                .timing
                .as_mut()
                .unwrap()
                .position
                .as_mut()
                .unwrap()
                .seconds = seconds;
            let after = presentation_from_snapshot(&snapshot).unwrap();
            let at = std::time::Duration::from_millis(milliseconds);
            let departing = lyric_motion_frame(&rendered, at);
            rendered.update_in_place(1, &after, at, Some(viewport));
            let frame = lyric_motion_frame(&rendered, at);
            assert_eq!(frame.cause, expected_cause, "at {seconds}s");
            assert_eq!(frame.cue_motion_active, milliseconds >= 100);
            if expected_cause == LyricMotionCause::IntentionalBlankExit {
                assert_eq!(
                    departing.anchors,
                    frame
                        .anchors
                        .into_iter()
                        .filter(|(_, weight)| *weight > 0.0)
                        .collect::<Vec<_>>(),
                    "returning lyrics continue from the unfinished blank departure"
                );
            }
        }
    }

    #[test]
    fn composition_text_groups_transfer_ownership_without_overlap() {
        let mut previous = composition_ownership(0.0);
        for step in 0..=100 {
            let progress = f64::from(step) / 100.0;
            let (ordinary, reel, masthead) = composition_ownership(progress);
            assert!(
                ordinary == 0.0 || masthead == 0.0,
                "large and compact Titles must never coexist at progress {progress}"
            );
            assert!(
                ordinary == 0.0 || reel == 0.0,
                "returning ordinary text must not cover departing lyric cues at progress {progress}"
            );
            assert!(masthead <= reel);
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
