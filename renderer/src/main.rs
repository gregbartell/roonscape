mod view;

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant, SystemTime};

use gtk::glib;
use gtk::prelude::*;
use roonscape_renderer::{
    ConnectionState, Diagnostics, DiagnosticsConfiguration, Presentation, PresentationState,
    PresentationTime, SnapshotEvent, SnapshotSubscription, current_process_memory_bytes,
};

use view::{diagnostics_view, install_style_provider, presentation_view};

const APPLICATION_ID: &str = "io.roonscape.Renderer";
const SNAPSHOT_RETRY_DELAY: Duration = Duration::from_millis(250);

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
    let progress_clock = Instant::now();
    let presentation = Rc::new(RefCell::new(PresentationState::disconnected()));
    let diagnostics = DiagnosticsConfiguration::from_environment()?
        .enabled()
        .then(|| Rc::new(RefCell::new(Diagnostics::default())));
    let updates = Rc::new(SnapshotSubscription::start(
        socket_path,
        SNAPSHOT_RETRY_DELAY,
    ));
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
            diagnostics.clone(),
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
    updates: Rc<SnapshotSubscription>,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
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
    let display = gtk::Overlay::new();
    display.set_child(Some(&rendered_presentation.borrow().root));
    let rendered_diagnostics = diagnostics.as_ref().map(|diagnostics| {
        let rendered = diagnostics_view(
            &diagnostics
                .borrow()
                .overlay_text(current_process_memory_bytes()),
        );
        display.add_overlay(rendered.widget());
        rendered
    });
    window.set_child(Some(&display));

    let updating_display = display.clone();
    let repository_root = repository_root.to_path_buf();
    let updating_style_provider = style_provider.clone();
    let updating_diagnostics = diagnostics.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let now = progress_clock.elapsed();
        let mut presentation_changed = false;
        loop {
            match updates.try_recv() {
                Ok(SnapshotEvent::Snapshot(snapshot)) => {
                    if let Some(diagnostics) = updating_diagnostics.as_ref() {
                        diagnostics
                            .borrow_mut()
                            .observe_snapshot(&snapshot, &repository_root);
                    }
                    match presentation
                        .borrow_mut()
                        .update(snapshot, PresentationTime::new(now, SystemTime::now()))
                    {
                        Ok(()) => presentation_changed = true,
                        Err(error) => eprintln!("RoonScape renderer: {error}"),
                    }
                }
                Ok(SnapshotEvent::ConnectionChanged(ConnectionState::Disconnected)) => {
                    if let Some(diagnostics) = updating_diagnostics.as_ref() {
                        diagnostics
                            .borrow_mut()
                            .observe_connection(ConnectionState::Disconnected);
                    }
                    presentation.borrow_mut().disconnect();
                    presentation_changed = true;
                }
                Ok(SnapshotEvent::ConnectionChanged(ConnectionState::Connected)) => {
                    if let Some(diagnostics) = updating_diagnostics.as_ref() {
                        diagnostics
                            .borrow_mut()
                            .observe_connection(ConnectionState::Connected);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        match presentation.borrow().presentation_at(now) {
            Ok(current_presentation) if presentation_changed => {
                let next_view = presentation_view(
                    &current_presentation,
                    &repository_root,
                    &updating_style_provider,
                );
                updating_display.set_child(Some(&next_view.root));
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

    if let (Some(diagnostics), Some(rendered_diagnostics)) = (diagnostics, rendered_diagnostics) {
        let frame_diagnostics = diagnostics.clone();
        window.add_tick_callback(move |_, frame_clock| {
            if let Ok(frame_time) = u64::try_from(frame_clock.frame_time()) {
                frame_diagnostics
                    .borrow_mut()
                    .observe_frame(Duration::from_micros(frame_time));
            }
            glib::ControlFlow::Continue
        });
        glib::timeout_add_local(Duration::from_millis(500), move || {
            rendered_diagnostics.update(
                &diagnostics
                    .borrow()
                    .overlay_text(current_process_memory_bytes()),
            );
            glib::ControlFlow::Continue
        });
    }

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
