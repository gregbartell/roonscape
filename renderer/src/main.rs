mod view;

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use gtk::glib;
use gtk::prelude::*;
use roonscape_renderer::{
    Presentation, PresentationSnapshot, PresentationState, PresentationTime, SnapshotReader,
};

use view::{install_style_provider, presentation_view};

const APPLICATION_ID: &str = "io.roonscape.Renderer";

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
    let style_provider = install_style_provider();

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .decorated(false)
        .default_width(1600)
        .default_height(900)
        .show_menubar(false)
        .title("RoonScape")
        .build();
    let initial_presentation = presentation
        .borrow()
        .presentation_at(progress_clock.elapsed())
        .expect("the initial presentation was validated before GTK started");
    let rendered_presentation = Rc::new(RefCell::new(presentation_view(
        &initial_presentation,
        repository_root,
        &style_provider,
    )));
    window.set_child(Some(&rendered_presentation.borrow().root));

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
                let next_view = presentation_view(
                    &current_presentation,
                    &repository_root,
                    &updating_style_provider,
                );
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
