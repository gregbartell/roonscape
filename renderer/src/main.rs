use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::thread;
use std::time::Duration;

use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    NowPlayingPresentation, Presentation, PresentationPalette, PresentationProgress,
    PresentationSnapshot, SnapshotReader, UnavailablePresentation, presentation_from_snapshot,
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
    let mut snapshot_reader = SnapshotReader::connect(&socket_path)?;
    let snapshot = snapshot_reader.read_snapshot()?;
    let presentation = Rc::new(presentation_from_snapshot(&snapshot)?);
    let updates = Rc::new(start_snapshot_reader(snapshot_reader));
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("renderer manifest should be inside the repository")?
        .to_path_buf();

    let application = gtk::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    application.connect_activate(move |application| {
        build_window(
            application,
            presentation.clone(),
            updates.clone(),
            &repository_root,
        );
    });
    application.run();

    Ok(())
}

fn build_window(
    application: &gtk::Application,
    presentation: Rc<Presentation>,
    updates: Rc<Receiver<PresentationSnapshot>>,
    repository_root: &Path,
) {
    let style_provider = install_style_provider();

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .decorated(false)
        .default_width(1600)
        .default_height(900)
        .show_menubar(false)
        .title("RoonScape")
        .build();
    window.set_child(Some(&presentation_view(
        &presentation,
        repository_root,
        &style_provider,
    )));

    let updating_window = window.clone();
    let repository_root = repository_root.to_path_buf();
    let updating_style_provider = style_provider.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let mut latest_snapshot = None;
        loop {
            match updates.try_recv() {
                Ok(snapshot) => latest_snapshot = Some(snapshot),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }

        if let Some(snapshot) = latest_snapshot {
            match presentation_from_snapshot(&snapshot) {
                Ok(presentation) => updating_window.set_child(Some(&presentation_view(
                    &presentation,
                    &repository_root,
                    &updating_style_provider,
                ))),
                Err(error) => eprintln!("RoonScape renderer: {error}"),
            }
        }

        glib::ControlFlow::Continue
    });

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

fn start_snapshot_reader(mut reader: SnapshotReader) -> Receiver<PresentationSnapshot> {
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        loop {
            match reader.read_snapshot() {
                Ok(snapshot) => {
                    if sender.send(snapshot).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    eprintln!("RoonScape renderer: {error}");
                    break;
                }
            }
        }
    });
    receiver
}

fn presentation_view(
    presentation: &Presentation,
    repository_root: &Path,
    style_provider: &gtk::CssProvider,
) -> gtk::Widget {
    let palette = palette_for_presentation(presentation, repository_root);
    install_styles(style_provider, palette);

    match presentation {
        Presentation::NowPlaying(presentation) => {
            gallery_split(presentation, repository_root).upcast()
        }
        Presentation::Unavailable(presentation) => unavailable(presentation).upcast(),
    }
}

fn gallery_split(presentation: &NowPlayingPresentation, repository_root: &Path) -> gtk::Grid {
    let root = gtk::Grid::new();
    root.add_css_class("gallery-split");
    root.set_column_homogeneous(true);

    let artwork_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_hexpand(true);
    artwork_column.set_vexpand(true);
    artwork_column.append(&artwork(presentation, repository_root));

    let metadata_column = metadata(presentation);

    root.attach(&artwork_column, 0, 0, 58, 1);
    root.attach(&metadata_column, 58, 0, 42, 1);
    root
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

fn metadata(presentation: &NowPlayingPresentation) -> gtk::Box {
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

fn install_style_provider() -> gtk::CssProvider {
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
