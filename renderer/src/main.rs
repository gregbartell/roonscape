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
    InactivityConfiguration, Presentation, PresentationSnapshot, PresentationState,
    PresentationTime, PresentationUpdate, SnapshotReader, display_configuration_file_path,
    load_inactivity_configuration,
};

use view::{PresentationView, install_style_providers};

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
    let inactivity_configuration = host_inactivity_configuration();
    let progress_clock = Instant::now();
    let presentation = Rc::new(RefCell::new(PresentationState::new_with_inactivity(
        snapshot,
        PresentationTime::new(progress_clock.elapsed(), SystemTime::now()),
        inactivity_configuration,
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
    let palette_provider = install_style_providers();

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .decorated(false)
        .default_width(1600)
        .default_height(900)
        .show_menubar(false)
        .title("RoonScape")
        .build();
    let initial_frame = presentation
        .borrow()
        .frame_at(progress_clock.elapsed())
        .expect("the initial presentation was validated before GTK started");
    let presentation_view = Rc::new(RefCell::new(PresentationView::new(
        presentation.borrow().revision(),
        &initial_frame.presentation,
        repository_root,
        palette_provider,
    )));
    presentation_view
        .borrow()
        .apply_inactivity(initial_frame.inactivity);
    window.set_child(Some(&presentation_view.borrow().root()));

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
        let mut presentation_update = None;
        if let Some(snapshot) = latest_snapshot {
            match presentation
                .borrow_mut()
                .update(snapshot, PresentationTime::new(now, SystemTime::now()))
            {
                Ok(update) => presentation_update = Some(update),
                Err(error) => eprintln!("RoonScape renderer: {error}"),
            }
        }

        match presentation.borrow().frame_at(now) {
            Ok(current_frame) => {
                if presentation_update == Some(PresentationUpdate::TransitionRequired) {
                    presentation_view.borrow_mut().replace(
                        presentation.borrow().revision(),
                        &current_frame.presentation,
                        &repository_root,
                        now,
                    );
                } else if let Presentation::NowPlaying(current_presentation) =
                    &current_frame.presentation
                    && let Some(progress) = current_presentation.progress.as_ref()
                {
                    presentation_view.borrow().update_progress(progress);
                }
                presentation_view
                    .borrow()
                    .apply_inactivity(current_frame.inactivity);
            }
            Err(error) => eprintln!("RoonScape renderer: {error}"),
        }
        presentation_view.borrow_mut().finish_transition(now);

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

fn host_inactivity_configuration() -> InactivityConfiguration {
    let configuration =
        display_configuration_file_path().and_then(|path| load_inactivity_configuration(&path));
    match configuration {
        Ok(configuration) => configuration,
        Err(error) => {
            eprintln!("RoonScape renderer: {error}; using default OLED inactivity calibration");
            InactivityConfiguration::default()
        }
    }
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
