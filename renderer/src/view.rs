use std::path::Path;
use std::time::Duration;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityTransform, MetadataLineLayout,
    MetadataOverflow, MetadataTypography, NowPlayingPresentation, Presentation,
    PresentationPalette, PresentationProgress, PresentationTransition, UnavailablePresentation,
    metadata_layout,
};

const STYLES: &str = include_str!("style.css");
const CURRENT_LAYER_CLASS: &str = "presentation-current";
const OUTGOING_LAYER_CLASS: &str = "presentation-outgoing";

pub(crate) struct PresentationView {
    stack: gtk::Stack,
    transition: PresentationTransition<RenderedPresentation>,
    palette_provider: gtk::CssProvider,
}

struct RenderedPresentation {
    root: gtk::Widget,
    progress: Option<RenderedProgress>,
    palette: PresentationPalette,
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
    bar: gtk::ProgressBar,
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
    root: gtk::Box,
    progress: Option<RenderedProgress>,
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

        let view = Self {
            stack,
            transition,
            palette_provider,
        };
        view.install_palette_styles();
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

    pub(crate) fn replace(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
        started_at: Duration,
    ) {
        if let Some(discarded) = self.transition.discard_outgoing() {
            self.stack.remove(&discarded.value().root);
        }
        let rendered = render_presentation(presentation, repository_root);
        rendered.root.add_css_class(CURRENT_LAYER_CLASS);
        let discarded = self.transition.begin(revision, rendered, started_at);
        debug_assert!(discarded.is_none());

        let outgoing = self
            .transition
            .outgoing()
            .expect("a started presentation transition has an outgoing layer");
        outgoing.value().root.remove_css_class(CURRENT_LAYER_CLASS);
        outgoing.value().root.add_css_class(OUTGOING_LAYER_CLASS);

        let current = self.transition.current();
        self.stack.add_child(&current.value().root);
        self.install_palette_styles();
        self.stack.set_visible_child(&current.value().root);
    }

    pub(crate) fn replace_immediately(
        &mut self,
        revision: u64,
        presentation: &Presentation,
        repository_root: &Path,
    ) {
        let rendered = render_presentation(presentation, repository_root);
        rendered.root.add_css_class(CURRENT_LAYER_CLASS);
        let (discarded_current, discarded_outgoing) =
            self.transition.replace_immediately(revision, rendered);
        self.stack.remove(&discarded_current.value().root);
        if let Some(discarded_outgoing) = discarded_outgoing {
            self.stack.remove(&discarded_outgoing.value().root);
        }

        let current = self.transition.current();
        self.stack.add_child(&current.value().root);
        self.install_palette_styles();
        self.stack.set_visible_child(&current.value().root);
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

    fn install_palette_styles(&self) {
        let mut styles = palette_styles(
            CURRENT_LAYER_CLASS,
            self.transition.current().value().palette,
        );
        if let Some(outgoing) = self.transition.outgoing() {
            styles.push_str(&palette_styles(
                OUTGOING_LAYER_CLASS,
                outgoing.value().palette,
            ));
        }
        self.palette_provider.load_from_data(&styles);
    }
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
    let root = gtk::Grid::new();
    root.add_css_class("gallery-split");
    root.set_column_homogeneous(true);

    let artwork_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_hexpand(true);
    artwork_column.set_vexpand(true);
    artwork_column.append(&artwork(presentation, repository_root));

    let metadata = metadata(presentation);

    root.attach(&artwork_column, 0, 0, 58, 1);
    root.attach(&metadata.root, 58, 0, 42, 1);
    RenderedPresentation {
        root: root.upcast(),
        progress: metadata.progress,
        palette,
    }
}

fn artwork(presentation: &NowPlayingPresentation, repository_root: &Path) -> gtk::AspectFrame {
    let picture = match presentation.artwork_path.as_deref() {
        Some(path) => gtk::Picture::for_filename(repository_root.join(path)),
        None => gtk::Picture::new(),
    };
    picture.set_alternative_text(Some("Current album artwork"));
    picture.add_css_class("artwork");
    picture.set_can_shrink(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    if presentation.artwork_path.is_none() {
        picture.add_css_class("artwork-missing");
    }

    let frame = gtk::AspectFrame::new(0.5, 0.5, 1.0, false);
    frame.add_css_class("artwork-frame");
    frame.set_hexpand(true);
    frame.set_vexpand(true);
    frame.set_child(Some(&picture));
    frame
}

fn metadata(presentation: &NowPlayingPresentation) -> RenderedMetadata {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.add_css_class("metadata-column");
    column.set_hexpand(true);
    column.set_vexpand(true);

    column.append(&playback_state(&presentation.playback_state));

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.add_css_class("metadata-copy");
    copy.set_valign(gtk::Align::Center);
    copy.set_vexpand(true);

    let layout = metadata_layout(presentation);
    if let Some(title) = layout.title.as_ref() {
        copy.append(&metadata_line(title, "title"));
    }
    if let Some(artist) = layout.artist.as_ref() {
        copy.append(&metadata_line(artist, "artist"));
    }
    if let Some(album) = layout.album.as_ref() {
        copy.append(&metadata_line(album, "album"));
    }
    let progress = presentation.progress.as_ref().map(|progress| {
        let (group, rendered_progress) = progress_view(progress);
        copy.append(&group);
        rendered_progress
    });

    column.append(&copy);
    column.append(&display_zone(&presentation.display_zone));
    RenderedMetadata {
        root: column,
        progress,
    }
}

fn metadata_line(layout: &MetadataLineLayout, class_name: &str) -> gtk::Label {
    let label = metadata_label(&layout.text, class_name);
    label.add_css_class(match layout.typography {
        MetadataTypography::EditorialSerif => "editorial-text",
        MetadataTypography::UtilitySans => "utility-text",
    });
    label.set_lines(layout.maximum_lines as i32);
    label.set_ellipsize(match layout.overflow {
        MetadataOverflow::EllipsizeEnd => pango::EllipsizeMode::End,
    });
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    set_label_font_size(&label, layout.preferred_font_size_px);

    let fitting_layout = layout.clone();
    label.connect_map(move |label| {
        let _ = fitting_layout.fitting_font_size(|font_size_px| {
            set_label_font_size(label, font_size_px);
            !label.layout().is_ellipsized()
        });
    });
    label
}

fn set_label_font_size(label: &gtk::Label, font_size_px: u32) {
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrSize::new_size_absolute(
        font_size_px as i32 * pango::SCALE,
    ));
    label.set_attributes(Some(&attributes));
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

fn playback_state(state: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("playback-state");
    row.set_halign(gtk::Align::Start);

    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("state-dot");
    row.append(&dot);
    row.append(&metadata_label(state, "state-label"));
    row
}

fn progress_view(progress: &PresentationProgress) -> (gtk::Box, RenderedProgress) {
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
    (
        group,
        RenderedProgress {
            bar,
            elapsed,
            remaining,
        },
    )
}

fn display_zone(display_zone: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    row.add_css_class("display-zone");
    row.append(&metadata_label("ZONE", "zone-label"));
    row.append(&metadata_label(display_zone, "zone-name"));
    row
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
        return PresentationPalette::neutral();
    };
    let artwork_path = presentation
        .artwork_path
        .as_deref()
        .map(|path| repository_root.join(path));

    match PresentationPalette::for_artwork(artwork_path.as_deref()) {
        Ok(palette) => palette,
        Err(error) => {
            eprintln!("RoonScape renderer: {error}");
            PresentationPalette::neutral()
        }
    }
}

pub(crate) fn install_style_providers() -> gtk::CssProvider {
    let static_provider = gtk::CssProvider::new();
    static_provider.load_from_data(STYLES);
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

fn palette_styles(class_name: &str, palette: PresentationPalette) -> String {
    let background = palette.background.to_hex();
    let artwork_field = palette.artwork_field.to_hex();
    let metadata_field = palette.metadata_field.to_hex();
    let primary_text = palette.primary_text.to_hex();
    let secondary_text = palette.secondary_text.to_hex();
    let accent = palette.accent.to_hex();
    format!(
        ".{class_name} {{ background-color: {background}; color: {primary_text}; }}\n\
         .{class_name}.gallery-split {{ background-image: linear-gradient(112deg, {artwork_field} 0%, {background} 58%, {metadata_field} 100%); }}\n\
         .{class_name} .artwork-column {{ background-color: alpha({artwork_field}, 0.48); }}\n\
         .{class_name} .artwork-frame {{ box-shadow: 0 36px 96px alpha({background}, 0.88); }}\n\
         .{class_name} .artwork {{ border-color: alpha({primary_text}, 0.16); background-color: {artwork_field}; }}\n\
         .{class_name} .artwork-missing {{ border-color: alpha({secondary_text}, 0.22); background-image: linear-gradient(142deg, alpha({secondary_text}, 0.09), {artwork_field} 52%, {background}); box-shadow: inset 0 0 0 24px alpha({background}, 0.16); }}\n\
         .{class_name} .metadata-column, .{class_name}.unavailable .unavailable-copy {{ background-color: {metadata_field}; }}\n\
         .{class_name} .playback-state, .{class_name} .zone-label, .{class_name} .unavailable-state {{ color: {accent}; }}\n\
         .{class_name} .state-dot {{ background-color: {accent}; box-shadow: 0 0 18px alpha({accent}, 0.72); }}\n\
         .{class_name} .title, .{class_name} .unavailable-heading {{ color: {primary_text}; }}\n\
         .{class_name} .artist, .{class_name} .album, .{class_name} .time, .{class_name} .display-zone, .{class_name} .unavailable-explanation {{ color: {secondary_text}; }}\n\
         .{class_name} progressbar trough {{ background-color: alpha({secondary_text}, 0.22); }}\n\
         .{class_name} progressbar progress {{ background-color: {accent}; }}\n\
         .{class_name}.unavailable {{ background-color: {background}; }}\n\
         .{class_name} .unavailable-field {{ border-color: alpha({secondary_text}, 0.12); background-image: linear-gradient(142deg, {artwork_field}, {background} 72%); }}\n"
    )
}
