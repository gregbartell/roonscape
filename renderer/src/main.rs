mod view;

use std::cell::RefCell;
use std::collections::HashSet;
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
    ConnectionState, Diagnostics, DiagnosticsConfiguration, InactivityConfiguration, Presentation,
    PresentationState, PresentationTime, PresentationUpdate, RendererKey, SnapshotEvent,
    SnapshotSubscription, Viewport, current_process_memory_bytes, display_configuration_file_path,
    load_inactivity_configuration, register_packaged_fallback_fonts, select_typography,
    should_close_renderer,
};

use view::{PresentationView, RenderedDiagnostics, diagnostics_view, install_style_providers};

const APPLICATION_ID: &str = "io.roonscape.Renderer";
const SNAPSHOT_RETRY_DELAY: Duration = Duration::from_millis(250);

struct PresentationRuntime {
    presentation: Rc<RefCell<PresentationState>>,
    presentation_view: Rc<RefCell<PresentationView>>,
    updates: Rc<SnapshotSubscription>,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
    display: gtk::Overlay,
    repository_root: PathBuf,
    progress_clock: Instant,
}

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
    let inactivity_configuration = host_inactivity_configuration();
    let progress_clock = Instant::now();
    let presentation = Rc::new(RefCell::new(
        PresentationState::disconnected_with_inactivity(
            progress_clock.elapsed(),
            inactivity_configuration,
        ),
    ));
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
    register_packaged_fallback_fonts(&repository_root.join("renderer"))?;

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
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .decorated(false)
        .default_width(1600)
        .default_height(900)
        .show_menubar(false)
        .title("RoonScape")
        .build();
    let available_families = window
        .pango_context()
        .font_map()
        .map(|font_map| {
            font_map
                .list_families()
                .into_iter()
                .map(|family| family.name().to_string())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let typography = select_typography(&available_families);
    let palette_provider = install_style_providers(typography);
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
    let display = gtk::Overlay::new();
    display.set_child(Some(&presentation_view.borrow().root()));
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

    let key_controller = gtk::EventControllerKey::new();
    let controlled_window = window.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        let key = if key == gtk::gdk::Key::Escape {
            RendererKey::Escape
        } else {
            RendererKey::Other
        };
        if should_close_renderer(key) {
            controlled_window.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);

    let runtime = PresentationRuntime {
        presentation,
        presentation_view,
        updates,
        diagnostics: diagnostics.clone(),
        display: display.clone(),
        repository_root: repository_root.to_path_buf(),
        progress_clock,
    };
    glib::timeout_add_local(Duration::from_millis(50), move || {
        runtime.tick();
        glib::ControlFlow::Continue
    });

    install_diagnostics_updates(&window, diagnostics, rendered_diagnostics);

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

impl PresentationRuntime {
    fn tick(&self) {
        self.apply_viewport();
        let now = self.progress_clock.elapsed();
        let presentation_update = self.apply_snapshot_events(now);
        self.render(now, presentation_update);
        self.presentation_view.borrow_mut().finish_transition(now);
    }

    fn apply_viewport(&self) {
        let width = self.display.width();
        let height = self.display.height();
        if width <= 0 || height <= 0 {
            return;
        }

        self.presentation_view
            .borrow_mut()
            .apply_viewport(Viewport::new(width as u32, height as u32));
    }

    fn apply_snapshot_events(&self, now: Duration) -> Option<PresentationUpdate> {
        let mut presentation_update = None;
        loop {
            let update = match self.updates.try_recv() {
                Ok(SnapshotEvent::Snapshot(snapshot)) => {
                    if let Some(diagnostics) = self.diagnostics.as_ref() {
                        diagnostics
                            .borrow_mut()
                            .observe_snapshot(&snapshot, &self.repository_root);
                    }
                    match self
                        .presentation
                        .borrow_mut()
                        .update(*snapshot, PresentationTime::new(now, SystemTime::now()))
                    {
                        Ok(update) => Some(update),
                        Err(error) => {
                            eprintln!("RoonScape renderer: {error}");
                            None
                        }
                    }
                }
                Ok(SnapshotEvent::ConnectionChanged(connection)) => {
                    if let Some(diagnostics) = self.diagnostics.as_ref() {
                        diagnostics.borrow_mut().observe_connection(connection);
                    }
                    match connection {
                        ConnectionState::Disconnected => {
                            Some(self.presentation.borrow_mut().disconnect(now))
                        }
                        ConnectionState::Connected => None,
                    }
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            if let Some(update) = update {
                presentation_update = combine_presentation_update(presentation_update, update);
            }
        }
        presentation_update
    }

    fn render(&self, now: Duration, presentation_update: Option<PresentationUpdate>) {
        let current_frame = match self.presentation.borrow().frame_at(now) {
            Ok(current_frame) => current_frame,
            Err(error) => {
                eprintln!("RoonScape renderer: {error}");
                return;
            }
        };

        match presentation_update {
            Some(PresentationUpdate::ReplaceImmediately) => {
                self.presentation_view.borrow_mut().replace_immediately(
                    self.presentation.borrow().revision(),
                    &current_frame.presentation,
                    &self.repository_root,
                );
            }
            Some(PresentationUpdate::TransitionRequired) => {
                self.presentation_view.borrow_mut().replace(
                    self.presentation.borrow().revision(),
                    &current_frame.presentation,
                    &self.repository_root,
                    now,
                );
            }
            Some(PresentationUpdate::ProgressOnly) | None => {
                if let Presentation::NowPlaying(current_presentation) = &current_frame.presentation
                    && let Some(progress) = current_presentation.progress.as_ref()
                {
                    self.presentation_view.borrow().update_progress(progress);
                }
            }
        }
        self.presentation_view
            .borrow()
            .apply_inactivity(current_frame.inactivity);
    }
}

fn install_diagnostics_updates(
    window: &gtk::ApplicationWindow,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
    rendered_diagnostics: Option<RenderedDiagnostics>,
) {
    let (Some(diagnostics), Some(rendered_diagnostics)) = (diagnostics, rendered_diagnostics)
    else {
        return;
    };

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

fn combine_presentation_update(
    current: Option<PresentationUpdate>,
    next: PresentationUpdate,
) -> Option<PresentationUpdate> {
    Some(match (current, next) {
        (Some(PresentationUpdate::ReplaceImmediately), _)
        | (_, PresentationUpdate::ReplaceImmediately) => PresentationUpdate::ReplaceImmediately,
        (Some(PresentationUpdate::TransitionRequired), _)
        | (_, PresentationUpdate::TransitionRequired) => PresentationUpdate::TransitionRequired,
        _ => PresentationUpdate::ProgressOnly,
    })
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
