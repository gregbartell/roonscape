use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    Presentation, PresentationProgress, presentation_from_snapshot, read_snapshot_from_socket,
};

const APPLICATION_ID: &str = "io.roonscape.Renderer";
const STYLES: &str = include_str!("style.css");

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("RoonScape renderer: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let socket_path = env::var_os("ROONSCAPE_SOCKET")
        .map(PathBuf::from)
        .ok_or("ROONSCAPE_SOCKET must name the private Unix socket")?;
    let snapshot = read_snapshot_from_socket(&socket_path)?;
    let presentation = Rc::new(presentation_from_snapshot(&snapshot)?);
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("renderer manifest should be inside the repository")?
        .to_path_buf();

    let application = gtk::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    application.connect_activate(move |application| {
        build_window(application, presentation.clone(), &repository_root);
    });
    application.run();

    Ok(())
}

fn build_window(
    application: &gtk::Application,
    presentation: Rc<Presentation>,
    repository_root: &Path,
) {
    install_styles();

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .default_width(1600)
        .default_height(900)
        .title("RoonScape")
        .build();
    window.set_child(Some(&gallery_split(&presentation, repository_root)));

    if env::var_os("ROONSCAPE_WINDOWED").is_none() {
        window.fullscreen();
    }

    if let Some(milliseconds) = env::var("ROONSCAPE_FIXTURE_AUTO_CLOSE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        let window = window.clone();
        glib::timeout_add_local_once(Duration::from_millis(milliseconds), move || {
            window.close();
        });
    }

    window.present();
}

fn gallery_split(presentation: &Presentation, repository_root: &Path) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("gallery-split");

    let artwork_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_hexpand(true);
    artwork_column.set_vexpand(true);
    artwork_column.append(&artwork(presentation, repository_root));

    let metadata_column = metadata(presentation);
    metadata_column.set_width_request(672);
    let responsive_metadata = metadata_column.clone();
    root.connect_notify_local(Some("width"), move |root, _| {
        let width = root.width();
        if width > 0 {
            responsive_metadata.set_width_request(width * 42 / 100);
        }
    });

    root.append(&artwork_column);
    root.append(&metadata_column);
    root
}

fn artwork(presentation: &Presentation, repository_root: &Path) -> gtk::AspectFrame {
    let picture = match presentation.artwork_path.as_deref() {
        Some(path) => gtk::Picture::for_filename(repository_root.join(path)),
        None => gtk::Picture::new(),
    };
    picture.set_alternative_text(Some("Current album artwork"));
    picture.add_css_class("artwork");
    picture.set_can_shrink(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);

    let frame = gtk::AspectFrame::new(0.5, 0.5, 1.0, false);
    frame.set_hexpand(true);
    frame.set_vexpand(true);
    frame.set_child(Some(&picture));
    frame
}

fn metadata(presentation: &Presentation) -> gtk::Box {
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
    if let Some(progress) = presentation.progress.as_ref() {
        copy.append(&progress_view(progress));
    }

    column.append(&copy);
    column.append(&display_zone(&presentation.display_zone));
    column
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

fn progress_view(progress: &PresentationProgress) -> gtk::Box {
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
    group
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

fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(STYLES);
    let display = gdk::Display::default().expect("GTK should have a display");
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
