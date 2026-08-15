use std::path::Path;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityTransform,
    NowPlayingPresentation, Presentation, PresentationPalette, PresentationProgress,
    UnavailablePresentation,
};

const STYLES: &str = include_str!("style.css");

pub(crate) struct RenderedPresentation {
    pub(crate) root: gtk::Widget,
    content: gtk::Widget,
    pub(crate) progress: Option<RenderedProgress>,
}

impl RenderedPresentation {
    pub(crate) fn apply_inactivity(&self, transform: InactivityTransform) {
        self.content.set_opacity(transform.opacity);
        let (horizontal_bound, vertical_bound) = if transform == InactivityTransform::default() {
            (0, 0)
        } else {
            (INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND)
        };
        self.content
            .set_margin_start(horizontal_bound + transform.offset.x);
        self.content
            .set_margin_end(horizontal_bound - transform.offset.x);
        self.content
            .set_margin_top(vertical_bound + transform.offset.y);
        self.content
            .set_margin_bottom(vertical_bound - transform.offset.y);
    }
}

#[derive(Clone)]
pub(crate) struct RenderedProgress {
    bar: gtk::ProgressBar,
    elapsed: gtk::Label,
    remaining: gtk::Label,
}

impl RenderedProgress {
    pub(crate) fn update(&self, progress: &PresentationProgress) {
        self.bar.set_fraction(progress.fraction);
        self.elapsed.set_text(&progress.elapsed);
        self.remaining.set_text(&progress.remaining);
    }
}

struct RenderedMetadata {
    root: gtk::Box,
    progress: Option<RenderedProgress>,
}

struct RenderedContent {
    root: gtk::Widget,
    progress: Option<RenderedProgress>,
}

pub(crate) fn presentation_view(
    presentation: &Presentation,
    repository_root: &Path,
    style_provider: &gtk::CssProvider,
) -> RenderedPresentation {
    let palette = palette_for_presentation(presentation, repository_root);
    install_styles(style_provider, palette);

    let rendered_content = match presentation {
        Presentation::NowPlaying(presentation) => gallery_split(presentation, repository_root),
        Presentation::Unavailable(presentation) => RenderedContent {
            root: unavailable(presentation).upcast(),
            progress: None,
        },
    };
    let content = rendered_content.root;
    content.set_hexpand(true);
    content.set_vexpand(true);

    let stage = gtk::Box::new(gtk::Orientation::Vertical, 0);
    stage.add_css_class("presentation-stage");
    stage.set_hexpand(true);
    stage.set_vexpand(true);
    stage.append(&content);

    RenderedPresentation {
        root: stage.upcast(),
        content,
        progress: rendered_content.progress,
    }
}

fn gallery_split(presentation: &NowPlayingPresentation, repository_root: &Path) -> RenderedContent {
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
    RenderedContent {
        root: root.upcast(),
        progress: metadata.progress,
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

    let title = metadata_label(presentation.title.as_deref().unwrap_or(""), "title");
    title.set_lines(3);
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_wrap(true);
    title.set_wrap_mode(pango::WrapMode::WordChar);
    copy.append(&title);

    if let Some(artist) = presentation.artist.as_deref() {
        copy.append(&metadata_label(artist, "artist"));
    }
    if let Some(album) = presentation.album.as_deref() {
        let album = metadata_label(album, "album");
        album.set_lines(2);
        album.set_ellipsize(pango::EllipsizeMode::End);
        album.set_wrap(true);
        copy.append(&album);
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

pub(crate) fn install_style_provider() -> gtk::CssProvider {
    let provider = gtk::CssProvider::new();
    let display = gdk::Display::default().expect("GTK should have a display");
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    provider
}

fn install_styles(provider: &gtk::CssProvider, palette: PresentationPalette) {
    provider.load_from_data(&format!("{}\n{STYLES}", palette.css_definitions()));
}
