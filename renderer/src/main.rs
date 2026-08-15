use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use roonscape_renderer::{
    NowPlayingPresentation, Presentation, PresentationProgress, PresentationSnapshot,
    PresentationState, PresentationTime, SnapshotReader, UnavailablePresentation,
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
    let progress_clock = Instant::now();
    let presentation = Rc::new(RefCell::new(PresentationState::new(
        snapshot,
        PresentationTime::new(progress_clock.elapsed(), SystemTime::now()),
    )?));
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
            progress_clock,
        );
    });
    application.run();

    Ok(())
}

fn build_window(
    application: &gtk::Application,
    presentation: Rc<RefCell<PresentationState>>,
    updates: Rc<Receiver<PresentationSnapshot>>,
    repository_root: &Path,
    progress_clock: Instant,
) {
    install_styles();

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .default_width(1600)
        .default_height(900)
        .title("RoonScape")
        .build();
    let initial_presentation = presentation
        .borrow()
        .presentation_at(progress_clock.elapsed())
        .expect("the initial presentation was validated before GTK started");
    let rendered_presentation = Rc::new(RefCell::new(presentation_view(
        &initial_presentation,
        repository_root,
    )));
    window.set_child(Some(&rendered_presentation.borrow().root));

    let updating_window = window.clone();
    let repository_root = repository_root.to_path_buf();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let mut latest_snapshot = None;
        loop {
            match updates.try_recv() {
                Ok(snapshot) => latest_snapshot = Some(snapshot),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }

        let now = progress_clock.elapsed();
        let mut presentation_changed = false;
        if let Some(snapshot) = latest_snapshot {
            match presentation
                .borrow_mut()
                .update(snapshot, PresentationTime::new(now, SystemTime::now()))
            {
                Ok(()) => presentation_changed = true,
                Err(error) => eprintln!("RoonScape renderer: {error}"),
            }
        }

        match presentation.borrow().presentation_at(now) {
            Ok(current_presentation) if presentation_changed => {
                let next_view = presentation_view(&current_presentation, &repository_root);
                updating_window.set_child(Some(&next_view.root));
                *rendered_presentation.borrow_mut() = next_view;
            }
            Ok(Presentation::NowPlaying(current_presentation)) => {
                if let (Some(progress), Some(progress_view)) = (
                    current_presentation.progress.as_ref(),
                    rendered_presentation.borrow().progress.as_ref(),
                ) {
                    progress_view.update(progress);
                }
            }
            Ok(Presentation::Unavailable(_)) => {}
            Err(error) => eprintln!("RoonScape renderer: {error}"),
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

struct RenderedPresentation {
    root: gtk::Box,
    progress: Option<RenderedProgress>,
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

fn presentation_view(presentation: &Presentation, repository_root: &Path) -> RenderedPresentation {
    match presentation {
        Presentation::NowPlaying(presentation) => gallery_split(presentation, repository_root),
        Presentation::Unavailable(presentation) => RenderedPresentation {
            root: unavailable(presentation),
            progress: None,
        },
    }
}

fn gallery_split(
    presentation: &NowPlayingPresentation,
    repository_root: &Path,
) -> RenderedPresentation {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("gallery-split");

    let artwork_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_column.add_css_class("artwork-column");
    artwork_column.set_hexpand(true);
    artwork_column.set_vexpand(true);
    artwork_column.append(&artwork(presentation, repository_root));

    let metadata = metadata(presentation);
    set_responsive_column_width(&root, &metadata.root);

    root.append(&artwork_column);
    root.append(&metadata.root);
    RenderedPresentation {
        root,
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

    let frame = gtk::AspectFrame::new(0.5, 0.5, 1.0, false);
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

    set_responsive_column_width(&root, &copy);

    root.append(&copy);
    root
}

fn set_responsive_column_width(root: &gtk::Box, column: &gtk::Box) {
    column.set_width_request(672);
    let responsive_column = column.clone();
    root.connect_notify_local(Some("width"), move |root, _| {
        let width = root.width();
        if width > 0 {
            responsive_column.set_width_request(width * 42 / 100);
        }
    });
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
