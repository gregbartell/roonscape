use std::path::Path;
use std::time::Duration;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    ArtworkFit, GalleryField, GallerySplitLayout, GallerySplitRole, INACTIVE_HORIZONTAL_BOUND,
    INACTIVE_VERTICAL_BOUND, IdentityPlacement, InactivityTransform, MetadataFontSizes,
    MetadataLineLayout, MetadataTypography, NowPlayingPresentation, Presentation,
    PresentationPalette, PresentationProgress, PresentationRevision, PresentationTransition,
    TypographyPair, UnavailablePresentation, Viewport, metadata_layout_for_viewport,
    presentation_palette_styles,
};

const STYLES: &str = include_str!("style.css");
const CURRENT_LAYER_CLASS: &str = "presentation-current";
const OUTGOING_LAYER_CLASS: &str = "presentation-outgoing";

pub(crate) struct PresentationView {
    stack: gtk::Stack,
    transition: PresentationTransition<RenderedPresentation>,
    palette_provider: gtk::CssProvider,
    viewport: Option<Viewport>,
}

struct RenderedPresentation {
    root: gtk::Widget,
    progress: Option<RenderedProgress>,
    palette: PresentationPalette,
    gallery_split: Option<RenderedGallerySplit>,
}

pub(crate) struct RenderedDiagnostics {
    label: gtk::Label,
}

impl RenderedDiagnostics {
    pub(crate) fn update(&self, text: &str) {
        self.label.set_text(text);
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
    playback_state: RenderedPlaybackState,
    title: Option<RenderedMetadataLine>,
    artist: Option<RenderedMetadataLine>,
    album: Option<RenderedMetadataLine>,
    progress: Option<RenderedProgress>,
    identity: RenderedIdentity,
}

struct RenderedGallerySplit {
    content: gtk::Box,
    artwork_column: gtk::Box,
    artwork_frame: gtk::AspectFrame,
    metadata_slot: gtk::Box,
    metadata: RenderedMetadata,
}

struct RenderedPlaybackState {
    root: gtk::Box,
    dot: gtk::Box,
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
    ) -> Self {
        let rendered = render_presentation(presentation, repository_root);
        rendered.root.add_css_class(CURRENT_LAYER_CLASS);
        let transition = PresentationTransition::new(revision, rendered);
        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(transition.duration().as_millis() as u32);
        stack.add_child(&transition.current().value().root);

        let mut view = Self {
            stack,
            transition,
            palette_provider,
            viewport: None,
        };
        view.apply_viewport(Viewport::WINDOWED_FIXTURE);
        view
    }

    pub(crate) fn root(&self) -> gtk::Widget {
        self.stack.clone().upcast()
    }

    pub(crate) fn apply_inactivity(&self, transform: InactivityTransform) {
        self.stack.set_opacity(transform.opacity);
        let (horizontal_bound, vertical_bound) = if transform == InactivityTransform::default() {
            (0, 0)
        } else {
            (INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND)
        };
        self.stack
            .set_margin_start(horizontal_bound + transform.offset.x);
        self.stack
            .set_margin_end(horizontal_bound - transform.offset.x);
        self.stack
            .set_margin_top(vertical_bound + transform.offset.y);
        self.stack
            .set_margin_bottom(vertical_bound - transform.offset.y);
    }

    pub(crate) fn apply_viewport(&mut self, viewport: Viewport) {
        if self.viewport == Some(viewport) {
            return;
        }

        self.transition.current().value().apply_viewport(viewport);
        if let Some(outgoing) = self.transition.outgoing() {
            outgoing.value().apply_viewport(viewport);
        }
        self.viewport = Some(viewport);
        self.install_palette_styles();
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
        outgoing.value().root.remove_css_class(CURRENT_LAYER_CLASS);
        outgoing.value().root.add_css_class(OUTGOING_LAYER_CLASS);
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

    fn remove_layer(&self, layer: PresentationRevision<RenderedPresentation>) {
        self.stack.remove(&layer.value().root);
    }

    fn render_current_at_viewport(
        &self,
        presentation: &Presentation,
        repository_root: &Path,
    ) -> RenderedPresentation {
        let rendered = render_current(presentation, repository_root);
        if let Some(viewport) = self.viewport {
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
        let layout =
            GallerySplitLayout::for_viewport(self.viewport.unwrap_or(Viewport::WINDOWED_FIXTURE));
        let mut styles = presentation_palette_styles(
            CURRENT_LAYER_CLASS,
            self.transition.current().value().palette,
            &layout,
        );
        if let Some(outgoing) = self.transition.outgoing() {
            styles.push_str(&presentation_palette_styles(
                OUTGOING_LAYER_CLASS,
                outgoing.value().palette,
                &layout,
            ));
        }
        self.palette_provider.load_from_data(&styles);
    }
}

impl RenderedPresentation {
    fn apply_viewport(&self, viewport: Viewport) {
        if let Some(gallery_split) = self.gallery_split.as_ref() {
            gallery_split.apply_layout(&GallerySplitLayout::for_viewport(viewport));
        }
    }
}

fn render_current(presentation: &Presentation, repository_root: &Path) -> RenderedPresentation {
    let rendered = render_presentation(presentation, repository_root);
    rendered.root.add_css_class(CURRENT_LAYER_CLASS);
    rendered
}

fn render_presentation(
    presentation: &Presentation,
    repository_root: &Path,
) -> RenderedPresentation {
    let palette = palette_for_presentation(presentation, repository_root);

    match presentation {
        Presentation::NowPlaying(presentation) => {
            gallery_split(presentation, repository_root, palette)
        }
        Presentation::Unavailable(presentation) => RenderedPresentation {
            root: unavailable(presentation).upcast(),
            progress: None,
            palette,
            gallery_split: None,
        },
    }
}

pub(crate) fn diagnostics_view(text: &str) -> RenderedDiagnostics {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("diagnostics");
    label.set_halign(gtk::Align::End);
    label.set_valign(gtk::Align::Start);
    label.set_xalign(0.0);
    label.set_selectable(false);
    RenderedDiagnostics { label }
}

impl RenderedDiagnostics {
    pub(crate) fn widget(&self) -> &gtk::Widget {
        self.label.upcast_ref()
    }
}

fn gallery_split(
    presentation: &NowPlayingPresentation,
    repository_root: &Path,
    palette: PresentationPalette,
) -> RenderedPresentation {
    let layout = GallerySplitLayout::for_presentation(presentation, Viewport::WINDOWED_FIXTURE);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    match layout.field {
        GalleryField::Cohesive => root.add_css_class("gallery-split"),
    }
    root.set_hexpand(true);
    root.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.add_css_class("gallery-content");
    content.set_hexpand(true);
    content.set_vexpand(true);
    root.append(&content);

    let artwork_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_hexpand(false);
    artwork_column.set_vexpand(true);
    let artwork_frame = artwork(presentation, repository_root, layout.artwork_fit);
    artwork_column.append(&artwork_frame);

    let metadata = metadata(presentation, &layout);
    let metadata_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    metadata_slot.add_css_class("metadata-slot");
    metadata_slot.set_hexpand(false);
    metadata_slot.set_vexpand(true);
    metadata_slot.append(&metadata.root);

    content.append(&artwork_column);
    content.append(&metadata_slot);
    let progress = metadata.progress.clone();
    let gallery_split = RenderedGallerySplit {
        content,
        artwork_column,
        artwork_frame,
        metadata_slot,
        metadata,
    };
    RenderedPresentation {
        root: root.upcast(),
        progress,
        palette,
        gallery_split: Some(gallery_split),
    }
}

fn artwork(
    presentation: &NowPlayingPresentation,
    repository_root: &Path,
    fit: ArtworkFit,
) -> gtk::AspectFrame {
    let picture = match presentation.artwork_path.as_deref() {
        Some(path) => gtk::Picture::for_filename(repository_root.join(path)),
        None => gtk::Picture::new(),
    };
    picture.set_alternative_text(Some("Current album artwork"));
    picture.add_css_class("artwork");
    picture.set_can_shrink(true);
    match fit {
        ArtworkFit::Contain => picture.set_keep_aspect_ratio(true),
    }
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    if presentation.artwork_path.is_none() {
        picture.add_css_class("artwork-missing");
    }

    let frame = gtk::AspectFrame::new(0.5, 0.5, 1.0, false);
    frame.add_css_class("artwork-frame");
    frame.set_halign(gtk::Align::Start);
    frame.set_valign(gtk::Align::Center);
    frame.set_hexpand(false);
    frame.set_vexpand(false);
    frame.set_child(Some(&picture));
    frame
}

fn metadata(
    presentation: &NowPlayingPresentation,
    gallery_layout: &GallerySplitLayout,
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

    let playback_state = playback_state(&presentation.playback_state);

    let layout = metadata_layout_for_viewport(presentation, Viewport::WINDOWED_FIXTURE);
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

    for role in &gallery_layout.metadata_roles {
        match role {
            GallerySplitRole::PlaybackStatus => root.add_overlay(&playback_state.root),
            GallerySplitRole::Title => {
                copy.append(&title.as_ref().expect("Title role requires a label").label)
            }
            GallerySplitRole::Artist => {
                copy.append(&artist.as_ref().expect("Artist role requires a label").label)
            }
            GallerySplitRole::Album => {
                copy.append(&album.as_ref().expect("Album role requires a label").label)
            }
            GallerySplitRole::Progress => copy.append(
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
        gallery_layout.identity_placement,
    );
    column.append(&identity.root);
    RenderedMetadata {
        root,
        playback_state,
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
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    set_label_font_size(&label, layout.font_sizes.preferred_px);

    RenderedMetadataLine { label }
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

impl RenderedGallerySplit {
    fn apply_layout(&self, layout: &GallerySplitLayout) {
        let gutter = dimension(layout.outer_gutter_px);
        self.content.set_margin_start(gutter);
        self.content.set_margin_end(gutter);
        self.content.set_margin_top(gutter);
        self.content.set_margin_bottom(gutter);
        self.content.set_spacing(dimension(layout.column_gap_px));

        self.artwork_column
            .set_width_request(dimension(layout.artwork_column_width_px));
        self.artwork_frame.set_size_request(
            dimension(layout.artwork_field_width_px),
            dimension(layout.artwork_field_height_px),
        );
        self.metadata_slot
            .set_width_request(dimension(layout.metadata_column_width_px));
        self.metadata
            .root
            .set_margin_end(dimension(layout.metadata_right_inset_px));
        self.metadata.apply_layout(layout);
    }
}

impl RenderedMetadata {
    fn apply_layout(&self, layout: &GallerySplitLayout) {
        self.playback_state
            .root
            .set_spacing(dimension(layout.state_dot_size_px));
        self.playback_state
            .root
            .set_margin_top(dimension(layout.status_top_inset_px));
        let dot_size = dimension(layout.state_dot_size_px);
        self.playback_state.dot.set_size_request(dot_size, dot_size);
        set_status_label_typography(
            &self.playback_state.label,
            layout.typography.status_px,
            layout.typography.status_letter_spacing_px,
        );

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
            .root
            .set_column_spacing(layout.identity_gap_px);
        let identity_label_size = ((layout.typography.identity_px as f64) * 0.84).round() as u32;
        set_label_font_size(&self.identity.output_label, identity_label_size);
        set_label_font_size(&self.identity.output_name, layout.typography.identity_px);
        set_label_font_size(&self.identity.zone_label, identity_label_size);
        set_label_font_size(&self.identity.zone_name, layout.typography.identity_px);
        self.identity
            .output_label
            .set_margin_end(dimension(identity_label_size / 2));
        self.identity
            .zone_label
            .set_margin_end(dimension(identity_label_size / 2));
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

fn unavailable(presentation: &UnavailablePresentation) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("unavailable");

    let quiet_field = gtk::Box::new(gtk::Orientation::Vertical, 0);
    quiet_field.add_css_class("unavailable-field");
    quiet_field.set_hexpand(true);
    quiet_field.set_vexpand(true);
    root.append(&quiet_field);

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.add_css_class("unavailable-copy");
    copy.set_hexpand(false);
    copy.set_vexpand(true);

    let state = metadata_label(presentation.state_label, "unavailable-state");
    copy.append(&state);

    let message = gtk::Box::new(gtk::Orientation::Vertical, 0);
    message.add_css_class("unavailable-message");
    message.set_valign(gtk::Align::Center);
    message.set_vexpand(true);

    let heading = metadata_label(presentation.heading, "unavailable-heading");
    heading.set_lines(3);
    heading.set_max_width_chars(12);
    heading.set_wrap(true);
    heading.set_wrap_mode(pango::WrapMode::WordChar);
    message.append(&heading);

    let explanation = metadata_label(presentation.explanation, "unavailable-explanation");
    explanation.set_max_width_chars(26);
    explanation.set_wrap(true);
    explanation.set_wrap_mode(pango::WrapMode::WordChar);
    message.append(&explanation);

    copy.append(&message);
    copy.set_width_request(672);

    root.append(&copy);
    root
}

fn playback_state(state: &str) -> RenderedPlaybackState {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("playback-state");
    row.set_halign(gtk::Align::Start);
    row.set_valign(gtk::Align::Start);

    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("state-dot");
    dot.set_halign(gtk::Align::Center);
    dot.set_valign(gtk::Align::Center);
    row.append(&dot);
    let label = metadata_label(&state.to_uppercase(), "state-label");
    row.append(&label);
    RenderedPlaybackState {
        root: row,
        dot,
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
) -> RenderedIdentity {
    let row = gtk::Grid::new();
    row.add_css_class("tracked-identity");
    row.set_column_homogeneous(true);
    row.set_hexpand(true);
    match placement {
        IdentityPlacement::BottomRight => {
            row.set_halign(gtk::Align::Fill);
            row.set_valign(gtk::Align::End);
        }
    }

    let output = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    output.set_hexpand(true);
    output.set_halign(gtk::Align::Fill);
    let output_label = metadata_label("OUTPUT", "identity-label");
    let output_name = identity_name(tracked_output);
    output.append(&output_label);
    output.append(&output_name);

    let zone = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    zone.set_halign(gtk::Align::End);
    let zone_label = metadata_label("ZONE", "identity-label");
    let zone_name = identity_name(tracked_zone);
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

fn palette_for_presentation(
    presentation: &Presentation,
    repository_root: &Path,
) -> PresentationPalette {
    let Presentation::NowPlaying(presentation) = presentation else {
        return PresentationPalette::fallback();
    };
    let artwork_path = presentation
        .artwork_path
        .as_deref()
        .map(|path| repository_root.join(path));

    PresentationPalette::for_artwork(artwork_path.as_deref())
}

pub(crate) fn install_style_providers(typography: TypographyPair) -> gtk::CssProvider {
    let static_provider = gtk::CssProvider::new();
    static_provider.load_from_data(&format!("{STYLES}\n{}", typography_styles(typography)));
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

fn typography_styles(typography: TypographyPair) -> String {
    format!(
        ".editorial-text, .unavailable-heading {{ font-family: \"{}\", serif; }}\n\
         .utility-text, .state-label, .identity-label, .identity-name, .time, .unavailable-state, .unavailable-explanation {{ font-family: \"{}\", sans-serif; }}\n",
        typography.editorial_family(),
        typography.utility_family(),
    )
}
