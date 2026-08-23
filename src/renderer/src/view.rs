use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFit,
    ArtworkLayout, FullFieldFontSize, FullFieldLayout, FullFieldLineLayout, FullFieldPresentation,
    IdentityLineLayout, IdentityPhraseAlignment, IdentityPlacement, IdentityRowLayout,
    InactivityLayout, InactivityTransform, MetadataGroupPlan, MetadataLayout, MetadataLineLayout,
    MetadataTypography, NowPlayingField, NowPlayingFooterContent, NowPlayingGradient,
    NowPlayingGradientCacheKey, NowPlayingLayout, NowPlayingPresentation, NowPlayingRole,
    Presentation, PresentationActivity, PresentationPalette, PresentationProgress,
    PresentationRevision, PresentationStatus, PresentationStatusEmphasis, PresentationStatusLayout,
    PresentationStyleLayer, PresentationTransition, PresentationTransitionStyles, TextOverflow,
    TypographySelection, TypographyStyles, Viewport, metadata_layout, resolve_presentation,
};

use crate::activity_waveform::activity_waveform;
use crate::status_symbol::presentation_status_symbol;

const STYLES: &str = include_str!("style.css");

pub(crate) struct PresentationView {
    root: gtk::Overlay,
    stack: gtk::Stack,
    transition: PresentationTransition<RenderedPresentation>,
    palette_provider: gtk::CssProvider,
    typography: TypographySelection,
    display_viewport: Viewport,
    layout_viewport: Option<Viewport>,
    inactivity: InactivityTransform,
}

struct RenderedPresentation {
    root: gtk::Widget,
    progress: Option<RenderedProgress>,
    palette: PresentationPalette,
    layout_source: PresentationLayoutSource,
    now_playing: Option<RenderedNowPlaying>,
    full_field: Option<RenderedFullField>,
    diagnostics: Option<gtk::Label>,
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
    bar: gtk::ProgressBar,
    times: gtk::Box,
    elapsed: gtk::Label,
    remaining: gtk::Label,
}

impl RenderedProgress {
    fn update(&self, progress: &PresentationProgress) {
        self.bar.set_fraction(progress.fraction);
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
    palette: PresentationPalette,
    cache_key: Rc<Cell<Option<NowPlayingGradientCacheKey>>>,
}

struct RenderedNowPlayingGradient {
    logical_viewport: Viewport,
    physical_viewport: Viewport,
    stride_bytes: usize,
    rgba8: Vec<u8>,
}

trait NowPlayingGradientTarget {
    fn scale_factor(&self) -> u32;

    fn install_gradient(&self, gradient: RenderedNowPlayingGradient);
}

impl NowPlayingGradientTarget for gtk::Picture {
    fn scale_factor(&self) -> u32 {
        u32::try_from(gtk::prelude::WidgetExt::scale_factor(self))
            .expect("GTK display scale factor must be positive")
    }

    fn install_gradient(&self, gradient: RenderedNowPlayingGradient) {
        let bytes = gtk::glib::Bytes::from_owned(gradient.rgba8);
        let texture = gdk::MemoryTexture::new(
            dimension(gradient.physical_viewport.width_px),
            dimension(gradient.physical_viewport.height_px),
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            gradient.stride_bytes,
        );
        self.set_size_request(
            dimension(gradient.logical_viewport.width_px),
            dimension(gradient.logical_viewport.height_px),
        );
        self.set_paintable(Some(&texture));
    }
}

struct RenderedArtwork {
    reservation: gtk::AspectFrame,
    print_plate: gtk::Box,
    decoration: gtk::AspectFrame,
    surface: gtk::Picture,
    source: Option<gdk_pixbuf::Pixbuf>,
    layout: ArtworkLayout,
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
}

struct RenderedPresentationStatus {
    root: gtk::Box,
    symbol: gtk::Box,
    label: gtk::Label,
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
    separator: gtk::Box,
    zone: gtk::Box,
    zone_label: gtk::Label,
    zone_name: gtk::Label,
}

impl PresentationView {
    pub(crate) fn new(
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
        palette_provider: gtk::CssProvider,
        typography: TypographySelection,
        diagnostics_text: Option<&str>,
    ) -> Self {
        let rendered =
            render_presentation(presentation, repository_root, typography, diagnostics_text);
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
            typography,
            display_viewport: Viewport::WINDOWED_FIXTURE,
            layout_viewport: None,
            inactivity: InactivityTransform::default(),
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
        started_at: Duration,
    ) {
        if let Some(discarded) = self.transition.discard_outgoing() {
            self.remove_layer(discarded);
        }
        let rendered = self.render_current_at_viewport(presentation, repository_root);
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

    pub(crate) fn finish_transition(&mut self, now: Duration) {
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

    pub(crate) fn update_diagnostics(&self, text: &str) {
        self.transition.current().value().update_diagnostics(text);
        if let Some(outgoing) = self.transition.outgoing() {
            outgoing.value().update_diagnostics(text);
        }
    }

    fn remove_layer(&self, layer: PresentationRevision<RenderedPresentation>) {
        self.stack.remove(&layer.value().root);
    }

    fn render_current_at_viewport(
        &self,
        presentation: &Presentation,
        repository_root: &Path,
    ) -> RenderedPresentation {
        let diagnostics_text = self.transition.current().value().diagnostics_text();
        let rendered = render_current(
            presentation,
            repository_root,
            self.typography,
            diagnostics_text.as_deref(),
        );
        if let Some(viewport) = self.layout_viewport {
            rendered.apply_viewport(viewport);
        }
        rendered
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

impl RenderedPresentation {
    fn apply_viewport(&self, viewport: Viewport) {
        if let (Some(now_playing), Some(layout)) = (
            self.now_playing.as_ref(),
            self.layout_source.now_playing(viewport),
        ) {
            now_playing.apply_layout(&layout, viewport);
        }
        if let Some(full_field) = self.full_field.as_ref() {
            full_field.apply_layout(&FullFieldLayout::for_viewport(viewport));
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

fn render_current(
    presentation: &Presentation,
    repository_root: &Path,
    typography: TypographySelection,
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let rendered = render_presentation(presentation, repository_root, typography, diagnostics_text);
    rendered
        .root
        .add_css_class(PresentationStyleLayer::Current.class_name());
    rendered
}

fn render_presentation(
    presentation: &Presentation,
    repository_root: &Path,
    typography: TypographySelection,
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let resolved = resolve_presentation(presentation, repository_root);
    let layout_source = PresentationLayoutSource::for_presentation(&resolved.presentation);

    match &resolved.presentation {
        Presentation::NowPlaying(presentation) => now_playing(
            presentation,
            repository_root,
            resolved.palette,
            layout_source,
            typography,
            diagnostics_text,
        ),
        Presentation::FullField(presentation) => full_field(
            presentation,
            resolved.palette,
            layout_source,
            diagnostics_text,
        ),
    }
}

fn full_field(
    presentation: &FullFieldPresentation,
    palette: PresentationPalette,
    layout_source: PresentationLayoutSource,
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
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
    let rendered_status =
        presentation_status(&presentation.status, layout.presentation_status.decoration);
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
        let identity = tracked_identity(
            &presentation_identity.tracked_output,
            &presentation_identity.tracked_zone,
            layout.identity_placement,
            layout.identity_line,
        );
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
        }),
        diagnostics,
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
    typography: TypographySelection,
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let layout = NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE);
    let surface = gtk::Overlay::new();
    surface.set_hexpand(true);
    surface.set_vexpand(true);

    let background = RenderedNowPlayingBackground::new(palette);
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
    let artwork = artwork(presentation, repository_root);
    artwork_column.set_center_widget(Some(&artwork.reservation));

    let metadata = metadata(presentation, &layout, typography);
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

fn artwork(presentation: &NowPlayingPresentation, repository_root: &Path) -> RenderedArtwork {
    let source = presentation
        .artwork_path
        .as_deref()
        .and_then(|path| gdk_pixbuf::Pixbuf::from_file(repository_root.join(path)).ok());
    let intrinsic_dimensions = source.as_ref().map(|artwork| {
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
        source,
        layout,
    }
}

fn metadata(
    presentation: &NowPlayingPresentation,
    now_playing_layout: &NowPlayingLayout,
    typography: TypographySelection,
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
    );
    rendered_status.root.set_halign(gtk::Align::Start);
    rendered_status.root.set_valign(gtk::Align::Start);
    root.add_overlay(&rendered_status.root);
    root.set_measure_overlay(&rendered_status.root, false);

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
    let progress = presentation.progress.as_ref().map(progress_view);
    let activity = presentation.activity.as_deref().map(activity_view);
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
        &presentation.tracked_zone,
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
    fn apply_layout(&self, layout: &NowPlayingLayout, viewport: Viewport) {
        self.background.apply_viewport(viewport);
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
    fn new(palette: PresentationPalette) -> Self {
        let picture = gtk::Picture::new();
        picture.set_can_shrink(false);
        picture.set_keep_aspect_ratio(false);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        let cache_key = Rc::new(Cell::new(None::<NowPlayingGradientCacheKey>));
        picture.connect_scale_factor_notify({
            let cache_key = Rc::clone(&cache_key);
            move |picture| {
                let Some(current_key) = cache_key.get() else {
                    return;
                };
                apply_now_playing_gradient(
                    picture,
                    palette,
                    &cache_key,
                    current_key.logical_viewport(),
                );
            }
        });
        Self {
            picture,
            palette,
            cache_key,
        }
    }

    fn apply_viewport(&self, viewport: Viewport) {
        apply_now_playing_gradient(&self.picture, self.palette, &self.cache_key, viewport);
    }
}

fn apply_now_playing_gradient<T: NowPlayingGradientTarget>(
    target: &T,
    palette: PresentationPalette,
    cache_key: &Cell<Option<NowPlayingGradientCacheKey>>,
    logical_viewport: Viewport,
) {
    let scale_factor = target.scale_factor();
    let next_key = NowPlayingGradientCacheKey::new(logical_viewport, scale_factor);
    if cache_key.get() == Some(next_key) {
        return;
    }

    // GTK lays widgets out in logical pixels, then rasterizes them at the
    // widget scale factor. Giving the Picture one texture pixel per physical
    // output pixel avoids resampling the spatial dither during rasterization.
    let physical_viewport = next_key.physical_viewport();
    let gradient = NowPlayingGradient::new(palette, physical_viewport);
    target.install_gradient(RenderedNowPlayingGradient {
        logical_viewport,
        physical_viewport,
        stride_bytes: physical_viewport.width_px as usize * 4,
        rgba8: gradient.into_rgba8(),
    });
    cache_key.set(Some(next_key));
}

impl RenderedArtwork {
    fn apply_layout(&self, now_playing: &NowPlayingLayout) {
        let reservation = now_playing.artwork_print_plate.footprint;
        self.reservation.set_size_request(
            dimension(reservation.width_px),
            dimension(reservation.height_px),
        );
        self.print_plate.set_size_request(
            dimension(now_playing.artwork_print_plate.footprint.width_px),
            dimension(now_playing.artwork_print_plate.footprint.height_px),
        );
        let plate_offset = dimension(now_playing.artwork_print_plate.offset_px);
        self.print_plate.set_margin_start(plate_offset);
        self.print_plate.set_margin_top(plate_offset);
        let visible = self.layout.visible_decoration(reservation);
        self.decoration
            .set_ratio(visible.width_px as f32 / visible.height_px as f32);
        self.surface
            .set_size_request(dimension(visible.width_px), dimension(visible.height_px));
        if let Some(source) = self.source.as_ref() {
            let image = self
                .layout
                .fitted_image(reservation)
                .expect("supplied artwork should have fitted image dimensions");
            let scaled = source
                .scale_simple(
                    dimension(image.width_px),
                    dimension(image.height_px),
                    gdk_pixbuf::InterpType::Bilinear,
                )
                .expect("positive artwork dimensions should produce a scaled image");
            self.surface.set_pixbuf(Some(&scaled));
        }
    }
}

impl RenderedFullField {
    fn apply_layout(&self, layout: &FullFieldLayout) {
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
        apply_full_field_font_size(&self.heading, layout.heading_font);
        apply_full_field_line_layout(&self.heading, layout.heading_line);
        if let (Some(slot), Some(explanation)) =
            (self.explanation_slot.as_ref(), self.explanation.as_ref())
        {
            slot.set_margin_top(dimension(layout.explanation_spacing_px));
            slot.set_height_request(dimension(layout.explanation_slot.height_px));
            apply_full_field_font_size(explanation, layout.explanation_font);
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
                .bar
                .set_height_request(dimension(layout.progress_height_px));
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
                for label in [
                    &self.output_label,
                    &self.output_name,
                    &self.zone_label,
                    &self.zone_name,
                ] {
                    label.set_valign(gtk::Align::Baseline);
                }
            }
        }
        for (phrase, maximum_width_px) in [
            (&self.output, layout.output_phrase_max_width_px),
            (&self.zone, layout.zone_phrase_max_width_px),
        ] {
            phrase.set_hexpand(false);
            phrase.set_size_request(-1, -1);
            let (_, natural_width, _, _) = phrase.measure(gtk::Orientation::Horizontal, -1);
            phrase.set_size_request(natural_width.min(dimension(maximum_width_px)), -1);
        }
        self.zone.set_halign(gtk::Align::Start);
        self.output_name.set_hexpand(true);
        self.zone_name.set_hexpand(true);
        self.zone_name.set_xalign(0.0);
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
        set_tracked_label_typography(&self.zone_label, label_px, label_letter_spacing_px);
        set_label_font_size(&self.zone_name, name_px);
        self.separator
            .set_size_request(dimension(separator_px), dimension(separator_px));
        self.output_label.set_margin_end(dimension(label_gap_px));
        self.zone_label.set_margin_end(dimension(label_gap_px));
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

fn apply_full_field_font_size(label: &gtk::Label, sizes: FullFieldFontSize) {
    set_label_font_size(label, sizes.preferred_px);
    let fitted_label = label.clone();
    label.add_tick_callback(move |_, _| {
        if fitted_label.width() <= 0 {
            return gtk::glib::ControlFlow::Continue;
        }
        fit_full_field_line(&fitted_label, sizes);
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
) -> RenderedPresentationStatus {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("presentation-status");
    row.add_css_class(match status.emphasis {
        PresentationStatusEmphasis::FullAccent => "status-full",
        PresentationStatusEmphasis::MutedAccent => "status-muted",
    });
    row.set_halign(gtk::Align::Start);
    row.set_valign(gtk::Align::Start);

    let symbol = presentation_status_symbol(status, decoration);
    row.append(&symbol);
    let label = metadata_label(status.label, "status-label");
    row.append(&label);
    RenderedPresentationStatus {
        root: row,
        symbol,
        label,
    }
}

fn progress_view(progress: &PresentationProgress) -> RenderedProgress {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
    group.add_css_class("progress-group");

    let bar = gtk::ProgressBar::new();
    bar.set_fraction(progress.fraction);
    bar.set_show_text(false);
    group.append(&bar);

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
        bar,
        times,
        elapsed,
        remaining,
    }
}

fn activity_view(activity: &PresentationActivity) -> RenderedActivity {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("activity-group");
    root.set_halign(gtk::Align::Start);
    root.set_valign(gtk::Align::Center);

    let waveform = activity_waveform(activity.waveform);
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
    tracked_zone: &str,
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

    row.attach(&output, 0, 0, 1, 1);
    row.attach(&separator, 1, 0, 1, 1);
    row.attach(&zone, 2, 0, 1, 1);
    RenderedIdentity {
        root: row,
        output,
        output_label,
        output_name,
        separator,
        zone,
        zone_label,
        zone_name,
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
    use std::cell::{Cell, RefCell};

    use super::{
        NowPlayingGradientTarget, PresentationLayoutSource, RenderedNowPlayingGradient, STYLES,
        apply_now_playing_gradient,
    };
    use roonscape_renderer::{
        NowPlayingFooterContent, NowPlayingGradientCacheKey, PresentationPalette, Viewport,
        parse_snapshot, presentation_from_snapshot,
    };

    #[derive(Debug, PartialEq, Eq)]
    struct GradientInstallation {
        logical_viewport: Viewport,
        physical_viewport: Viewport,
        stride_bytes: usize,
        byte_count: usize,
    }

    struct RecordingGradientTarget {
        scale_factor: Cell<u32>,
        installations: RefCell<Vec<GradientInstallation>>,
    }

    impl RecordingGradientTarget {
        fn new(scale_factor: u32) -> Self {
            Self {
                scale_factor: Cell::new(scale_factor),
                installations: RefCell::new(Vec::new()),
            }
        }
    }

    impl NowPlayingGradientTarget for RecordingGradientTarget {
        fn scale_factor(&self) -> u32 {
            self.scale_factor.get()
        }

        fn install_gradient(&self, gradient: RenderedNowPlayingGradient) {
            self.installations.borrow_mut().push(GradientInstallation {
                logical_viewport: gradient.logical_viewport,
                physical_viewport: gradient.physical_viewport,
                stride_bytes: gradient.stride_bytes,
                byte_count: gradient.rgba8.len(),
            });
        }
    }

    #[test]
    fn installs_and_caches_the_physical_now_playing_gradient() {
        let target = RecordingGradientTarget::new(2);
        let cache_key = Cell::new(None::<NowPlayingGradientCacheKey>);
        let first_viewport = Viewport::new(16, 9);

        apply_now_playing_gradient(
            &target,
            PresentationPalette::fallback(),
            &cache_key,
            first_viewport,
        );
        assert_eq!(
            *target.installations.borrow(),
            [GradientInstallation {
                logical_viewport: first_viewport,
                physical_viewport: Viewport::new(32, 18),
                stride_bytes: 32 * 4,
                byte_count: 32 * 18 * 4,
            }]
        );

        apply_now_playing_gradient(
            &target,
            PresentationPalette::fallback(),
            &cache_key,
            first_viewport,
        );
        assert_eq!(target.installations.borrow().len(), 1);

        let second_viewport = Viewport::new(20, 12);
        apply_now_playing_gradient(
            &target,
            PresentationPalette::fallback(),
            &cache_key,
            second_viewport,
        );
        assert_eq!(target.installations.borrow().len(), 2);

        target.scale_factor.set(3);
        apply_now_playing_gradient(
            &target,
            PresentationPalette::fallback(),
            &cache_key,
            second_viewport,
        );
        assert_eq!(
            target.installations.borrow().last(),
            Some(&GradientInstallation {
                logical_viewport: second_viewport,
                physical_viewport: Viewport::new(60, 36),
                stride_bytes: 60 * 4,
                byte_count: 60 * 36 * 4,
            })
        );
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
}
