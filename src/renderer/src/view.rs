use std::path::Path;
use std::time::Duration;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    ArtworkAlignment, ArtworkContent, ArtworkDecoration, ArtworkDimensions, ArtworkFit,
    ArtworkLayout, FullFieldLayout, FullFieldLineLayout, FullFieldPresentation, IdentityLineLayout,
    IdentityPlacement, InactivityLayout, InactivityTransform, MetadataFontSizes,
    MetadataLineLayout, MetadataTypography, NowPlayingField, NowPlayingLayout,
    NowPlayingPresentation, NowPlayingRole, Presentation, PresentationPalette,
    PresentationProgress, PresentationRevision, PresentationStatus, PresentationStatusEmphasis,
    PresentationStatusLayout, PresentationStyleLayer, PresentationTransition,
    PresentationTransitionStyles, TextOverflow, TypographyPair, TypographyStyles, Viewport,
    metadata_layout, resolve_presentation,
};

use crate::status_symbol::presentation_status_symbol;

const STYLES: &str = include_str!("style.css");

pub(crate) struct PresentationView {
    root: gtk::Overlay,
    stack: gtk::Stack,
    transition: PresentationTransition<RenderedPresentation>,
    palette_provider: gtk::CssProvider,
    display_viewport: Viewport,
    layout_viewport: Option<Viewport>,
    inactivity: InactivityTransform,
}

struct RenderedPresentation {
    root: gtk::Widget,
    progress: Option<RenderedProgress>,
    palette: PresentationPalette,
    now_playing: Option<RenderedNowPlaying>,
    full_field: Option<RenderedFullField>,
    diagnostics: Option<gtk::Label>,
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
    presentation_status: RenderedPresentationStatus,
    title: Option<RenderedMetadataLine>,
    artist: Option<RenderedMetadataLine>,
    album: Option<RenderedMetadataLine>,
    progress: Option<RenderedProgress>,
    identity: RenderedIdentity,
}

struct RenderedNowPlaying {
    content: gtk::Box,
    artwork_column: gtk::Box,
    artwork: RenderedArtwork,
    metadata_slot: gtk::Box,
    metadata: RenderedMetadata,
}

struct RenderedArtwork {
    reservation: gtk::AspectFrame,
    decoration: gtk::AspectFrame,
    surface: gtk::Picture,
    source: Option<gdk_pixbuf::Pixbuf>,
    layout: ArtworkLayout,
}

struct RenderedFullField {
    copy: gtk::Box,
    message: gtk::Box,
    presentation_status: RenderedPresentationStatus,
    heading: gtk::Label,
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
}

struct RenderedIdentity {
    root: gtk::Grid,
    output_label: gtk::Label,
    output_name: gtk::Label,
    zone_label: gtk::Label,
    zone_name: gtk::Label,
}

impl PresentationView {
    pub(crate) fn new(
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
        palette_provider: gtk::CssProvider,
        diagnostics_text: Option<&str>,
    ) -> Self {
        let rendered = render_presentation(presentation, repository_root, diagnostics_text);
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

    pub(crate) fn replace_immediately(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
    ) {
        let rendered = self.render_current_at_viewport(presentation, repository_root);
        let (discarded_current, discarded_outgoing) =
            self.transition.replace_immediately(revision, rendered);
        self.remove_layer(discarded_current);
        if let Some(discarded_outgoing) = discarded_outgoing {
            self.remove_layer(discarded_outgoing);
        }
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
        let rendered = render_current(presentation, repository_root, diagnostics_text.as_deref());
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
        if let Some(now_playing) = self.now_playing.as_ref() {
            now_playing.apply_layout(&NowPlayingLayout::for_viewport(viewport));
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
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let rendered = render_presentation(presentation, repository_root, diagnostics_text);
    rendered
        .root
        .add_css_class(PresentationStyleLayer::Current.class_name());
    rendered
}

fn render_presentation(
    presentation: &Presentation,
    repository_root: &Path,
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let resolved = resolve_presentation(presentation, repository_root);

    match &resolved.presentation {
        Presentation::NowPlaying(presentation) => now_playing(
            presentation,
            repository_root,
            resolved.palette,
            diagnostics_text,
        ),
        Presentation::FullField(presentation) => {
            full_field(presentation, resolved.palette, diagnostics_text)
        }
    }
}

fn full_field(
    presentation: &FullFieldPresentation,
    palette: PresentationPalette,
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let layout = FullFieldLayout::for_viewport(Viewport::WINDOWED_FIXTURE);
    let content = gtk::Overlay::new();
    content.set_hexpand(true);
    content.set_vexpand(true);

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.add_css_class("full-copy");
    copy.set_halign(gtk::Align::Center);
    copy.set_valign(gtk::Align::Center);

    let message = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let rendered_status = presentation_status(&presentation.status);
    message.append(&rendered_status.root);

    let heading = metadata_label(presentation.heading, "full-field-heading");
    heading.add_css_class("editorial-text");
    message.append(&heading);

    let explanation = presentation.explanation.map(|explanation| {
        let explanation = metadata_label(explanation, "full-field-explanation");
        explanation.add_css_class("utility-text");
        message.append(&explanation);
        explanation
    });
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
        now_playing: None,
        full_field: Some(RenderedFullField {
            copy,
            message,
            presentation_status: rendered_status,
            heading,
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
    diagnostics_text: Option<&str>,
) -> RenderedPresentation {
    let layout = NowPlayingLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE);
    let surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
    surface.set_hexpand(true);
    surface.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.add_css_class("now-playing-content");
    content.set_hexpand(true);
    content.set_vexpand(true);
    surface.append(&content);

    let artwork_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_hexpand(false);
    artwork_column.set_vexpand(true);
    let artwork = artwork(presentation, repository_root);
    artwork_column.append(&artwork.reservation);

    let metadata = metadata(presentation, &layout);
    let metadata_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    metadata_slot.add_css_class("metadata-slot");
    metadata_slot.set_hexpand(false);
    metadata_slot.set_vexpand(true);
    metadata_slot.append(&metadata.root);

    content.append(&artwork_column);
    content.append(&metadata_slot);
    let progress = metadata.progress.clone();
    let now_playing = RenderedNowPlaying {
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

    let reservation = gtk::AspectFrame::new(horizontal_alignment, vertical_alignment, 1.0, false);
    reservation.add_css_class("artwork-reservation");
    reservation.set_halign(gtk::Align::Start);
    reservation.set_valign(gtk::Align::Center);
    reservation.set_hexpand(false);
    reservation.set_vexpand(false);
    reservation.set_child(Some(&decoration));

    RenderedArtwork {
        reservation,
        decoration,
        surface: picture,
        source,
        layout,
    }
}

fn metadata(
    presentation: &NowPlayingPresentation,
    now_playing_layout: &NowPlayingLayout,
) -> RenderedMetadata {
    let root = gtk::Overlay::new();
    root.add_css_class("metadata-column");
    root.set_hexpand(true);
    root.set_vexpand(true);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_hexpand(true);
    column.set_vexpand(true);
    root.set_child(Some(&column));

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.add_css_class("metadata-copy");
    copy.set_valign(gtk::Align::Center);
    copy.set_vexpand(true);

    let rendered_status = presentation_status(&presentation.status);

    let layout = metadata_layout(presentation, Viewport::WINDOWED_FIXTURE);
    let title = layout
        .title
        .as_ref()
        .map(|layout| metadata_line(layout, "title"));
    let artist = layout
        .artist
        .as_ref()
        .map(|layout| metadata_line(layout, "artist"));
    let album = layout
        .album
        .as_ref()
        .map(|layout| metadata_line(layout, "album"));
    let progress = presentation.progress.as_ref().map(progress_view);

    for role in &now_playing_layout.metadata_roles {
        match role {
            NowPlayingRole::PresentationStatus => root.add_overlay(&rendered_status.root),
            NowPlayingRole::Title => {
                copy.append(&title.as_ref().expect("Title role requires a label").label)
            }
            NowPlayingRole::Artist => {
                copy.append(&artist.as_ref().expect("Artist role requires a label").label)
            }
            NowPlayingRole::Album => {
                copy.append(&album.as_ref().expect("Album role requires a label").label)
            }
            NowPlayingRole::Progress => copy.append(
                &progress
                    .as_ref()
                    .expect("progress role requires a timeline")
                    .root,
            ),
        }
    }

    column.append(&copy);
    let identity = tracked_identity(
        &presentation.tracked_output,
        &presentation.tracked_zone,
        now_playing_layout.identity_placement,
        now_playing_layout.identity_line,
    );
    identity.root.set_halign(gtk::Align::Fill);
    column.append(&identity.root);
    RenderedMetadata {
        root,
        presentation_status: rendered_status,
        title,
        artist,
        album,
        progress,
        identity,
    }
}

fn metadata_line(layout: &MetadataLineLayout, class_name: &str) -> RenderedMetadataLine {
    let label = metadata_label(&layout.text, class_name);
    label.add_css_class(match layout.typography {
        MetadataTypography::EditorialSerif => "editorial-text",
        MetadataTypography::UtilitySans => "utility-text",
    });
    label.set_lines(layout.maximum_lines as i32);
    apply_text_overflow(&label, layout.overflow);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    set_label_font_size(&label, layout.font_sizes.preferred_px);

    RenderedMetadataLine { label }
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
    label.set_wrap(layout.wrap);
}

fn set_label_font_size(label: &gtk::Label, font_size_px: u32) {
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrSize::new_size_absolute(
        font_size_px as i32 * pango::SCALE,
    ));
    label.set_attributes(Some(&attributes));
}

fn set_status_label_typography(label: &gtk::Label, font_size_px: u32, letter_spacing_px: u32) {
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
    fn apply_layout(&self, layout: &NowPlayingLayout) {
        let gutter = dimension(layout.outer_gutter_px);
        self.content.set_margin_start(gutter);
        self.content.set_margin_end(gutter);
        self.content.set_margin_top(gutter);
        self.content.set_margin_bottom(gutter);
        self.content.set_spacing(dimension(layout.column_gap_px));

        self.artwork_column
            .set_width_request(dimension(layout.artwork_column_width_px));
        self.artwork.apply_layout(ArtworkDimensions::new(
            layout.artwork_field_width_px,
            layout.artwork_field_height_px,
        ));
        self.metadata_slot
            .set_width_request(dimension(layout.metadata_column_width_px));
        self.metadata
            .root
            .set_margin_end(dimension(layout.metadata_right_inset_px));
        self.metadata.apply_layout(layout);
    }
}

impl RenderedArtwork {
    fn apply_layout(&self, reservation: ArtworkDimensions) {
        self.reservation.set_size_request(
            dimension(reservation.width_px),
            dimension(reservation.height_px),
        );
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
        self.copy.set_width_request(dimension(layout.copy_width_px));
        self.message
            .set_margin_start(dimension(layout.accent_padding_px));
        self.presentation_status
            .apply_layout(layout.presentation_status);
        self.presentation_status
            .root
            .set_margin_bottom(dimension(layout.status_spacing_px));
        set_label_font_size(&self.heading, layout.heading_px);
        apply_full_field_line_layout(&self.heading, layout.heading_line);
        if let Some(explanation) = self.explanation.as_ref() {
            explanation.set_margin_top(dimension(layout.explanation_spacing_px));
            set_label_font_size(explanation, layout.explanation_px);
            apply_full_field_line_layout(explanation, layout.explanation_line);
        }
        if let Some(identity) = self.identity.as_ref() {
            let gutter = dimension(layout.outer_gutter_px);
            identity
                .root
                .set_margin_end(gutter + dimension(layout.identity_right_inset_px));
            identity.root.set_margin_bottom(gutter);
            identity
                .root
                .set_width_request(dimension(layout.identity_width_px));
            identity.apply_layout(layout.identity_gap_px, layout.identity_px);
        }
    }
}

impl RenderedMetadata {
    fn apply_layout(&self, layout: &NowPlayingLayout) {
        self.presentation_status
            .apply_layout(layout.presentation_status);
        self.presentation_status
            .root
            .set_margin_top(dimension(layout.status_top_inset_px));

        if let Some(title) = self.title.as_ref() {
            title.apply_font_sizes(layout.typography.title);
        }
        if let Some(artist) = self.artist.as_ref() {
            artist
                .label
                .set_margin_top(dimension(layout.artist_spacing_px));
            artist.apply_font_sizes(layout.typography.artist);
        }
        if let Some(album) = self.album.as_ref() {
            album
                .label
                .set_margin_top(dimension(layout.album_spacing_px));
            album.apply_font_sizes(layout.typography.album);
        }
        if let Some(progress) = self.progress.as_ref() {
            progress
                .root
                .set_margin_top(dimension(layout.progress_spacing_px));
            progress
                .bar
                .set_height_request(dimension(layout.progress_height_px));
            progress
                .times
                .set_margin_top(dimension(layout.time_spacing_px));
            set_label_font_size(&progress.elapsed, layout.typography.time_px);
            set_label_font_size(&progress.remaining, layout.typography.time_px);
        }

        self.identity
            .apply_layout(layout.identity_gap_px, layout.typography.identity_px);
    }
}

impl RenderedPresentationStatus {
    fn apply_layout(&self, layout: PresentationStatusLayout) {
        self.root.set_spacing(dimension(layout.symbol_gap_px));
        let symbol_size = dimension(layout.symbol_size_px);
        self.symbol.set_size_request(symbol_size, symbol_size);
        set_status_label_typography(&self.label, layout.font_px, layout.letter_spacing_px);
    }
}

impl RenderedIdentity {
    fn apply_layout(&self, gap_px: u32, name_px: u32) {
        self.root.set_column_spacing(gap_px);
        let label_px = ((name_px as f64) * 0.84).round() as u32;
        set_label_font_size(&self.output_label, label_px);
        set_label_font_size(&self.output_name, name_px);
        set_label_font_size(&self.zone_label, label_px);
        set_label_font_size(&self.zone_name, name_px);
        self.output_label.set_margin_end(dimension(label_px / 2));
        self.zone_label.set_margin_end(dimension(label_px / 2));
    }
}

impl RenderedMetadataLine {
    fn apply_font_sizes(&self, sizes: MetadataFontSizes) {
        set_label_font_size(&self.label, sizes.preferred_px);
        let label = self.label.clone();
        self.label.add_tick_callback(move |_, _| {
            fit_metadata_line(&label, sizes);
            gtk::glib::ControlFlow::Break
        });
    }
}

fn fit_metadata_line(label: &gtk::Label, sizes: MetadataFontSizes) {
    let _ = sizes.fitting_font_size(|font_size_px| {
        set_label_font_size(label, font_size_px);
        !label.layout().is_ellipsized()
    });
}

fn dimension(value: u32) -> i32 {
    i32::try_from(value).expect("supported viewport dimensions fit GTK's signed sizes")
}

fn presentation_status(status: &PresentationStatus) -> RenderedPresentationStatus {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("presentation-status");
    row.add_css_class(match status.emphasis {
        PresentationStatusEmphasis::FullAccentWithGlow => "status-glow",
        PresentationStatusEmphasis::FullAccent => "status-full",
        PresentationStatusEmphasis::MutedAccent => "status-muted",
    });
    row.set_halign(gtk::Align::Start);
    row.set_valign(gtk::Align::Start);

    let symbol = presentation_status_symbol(status);
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

fn tracked_identity(
    tracked_output: &str,
    tracked_zone: &str,
    placement: IdentityPlacement,
    line_layout: IdentityLineLayout,
) -> RenderedIdentity {
    let row = gtk::Grid::new();
    row.add_css_class("tracked-identity");
    row.set_column_homogeneous(true);
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
    output.append(&output_label);
    output.append(&output_name);

    let zone = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    zone.set_halign(gtk::Align::End);
    let zone_label = metadata_label("ZONE", "identity-label");
    let zone_name = identity_name(tracked_zone, line_layout);
    zone_name.set_xalign(1.0);
    zone.append(&zone_label);
    zone.append(&zone_name);

    row.attach(&output, 0, 0, 1, 1);
    row.attach(&zone, 1, 0, 1, 1);
    RenderedIdentity {
        root: row,
        output_label,
        output_name,
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

pub(crate) fn install_style_providers(typography: TypographyPair) -> gtk::CssProvider {
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
