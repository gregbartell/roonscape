mod activity_waveform;
mod artwork_cache;
mod bounded_lru_cache;
mod gradient_cache;
mod lyric_motion;
mod status_symbol;
mod view;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant, SystemTime};

use gtk::glib;
use gtk::prelude::*;
use roonscape_renderer::{
    CaptureControl, CaptureControlEvent, ConnectionState, Diagnostics, DiagnosticsConfiguration,
    FixtureNavigation, FixtureSelection, FixtureSelectionIdentity, InactivityConfiguration,
    NowPlayingTitleFace, PaintedFixtureSelection, Presentation, PresentationBehavior,
    PresentationState, PresentationTime, PresentationUpdate, RendererAction, RendererKey,
    RendererKeyboard, SnapshotEvent, SnapshotSubscription, Viewport, classify_presentation_update,
    current_process_memory_bytes, display_configuration_file_path, load_inactivity_configuration,
    register_packaged_fallback_fonts, select_capture_typography, select_typography,
};

use view::{PresentationView, RenderingConfiguration, install_style_providers};

const APPLICATION_ID: &str = "io.roonscape.Renderer";
const SNAPSHOT_RETRY_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
struct CaptureConfiguration {
    viewport: Option<Viewport>,
    typography: Option<NowPlayingTitleFace>,
    reduced_animation: bool,
}

#[derive(Clone, Copy)]
struct RendererConfiguration {
    capture: CaptureConfiguration,
    behavior: PresentationBehavior,
}

#[derive(Clone)]
struct RendererStartup {
    progress_clock: Instant,
    configuration: RendererConfiguration,
    initial_capture: Option<FixtureSelectionIdentity>,
    runtime_error: Rc<RefCell<Option<String>>>,
}

#[derive(Clone)]
struct RendererConnections {
    snapshots: Option<Rc<SnapshotSubscription>>,
    fixture_navigation: Option<Rc<RefCell<FixtureNavigation>>>,
    capture_control: Option<Rc<CaptureControl>>,
}

#[derive(Clone, Copy)]
enum FirstRevealedPaintPhase {
    WaitingForReveal,
    WaitingForRepaint,
    Complete,
}

struct PresentationRuntime {
    presentation: Rc<RefCell<PresentationState>>,
    rendered_presentation: RefCell<Presentation>,
    reported_lyric_visibility: Cell<Option<u64>>,
    presentation_view: Rc<RefCell<PresentationView>>,
    updates: Option<Rc<SnapshotSubscription>>,
    capture_control: Option<Rc<CaptureControl>>,
    painted_capture: RefCell<PaintedFixtureSelection>,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
    display: gtk::Overlay,
    repository_root: PathBuf,
    progress_clock: Instant,
    fixture_navigation_enabled: bool,
    capture_viewport: Option<Viewport>,
    application: gtk::Application,
    runtime_error: Rc<RefCell<Option<String>>>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("RoonScape Renderer: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let configuration_file = configuration_file_from_arguments()?;
    let inactivity_configuration = host_inactivity_configuration(&configuration_file);
    let renderer_configuration = renderer_configuration_from_environment()?;
    let progress_clock = Instant::now();
    let capture_session = env::var_os("ROONSCAPE_CAPTURE_CONTROL")
        .map(PathBuf::from)
        .map(|control_socket_path| CaptureControl::connect(&control_socket_path))
        .transpose()?;
    if capture_session.is_some()
        && (renderer_configuration.behavior != PresentationBehavior::StaticFixture
            || renderer_configuration.capture.viewport.is_none())
    {
        return Err(
            "ROONSCAPE_CAPTURE_CONTROL requires static Fixture Mode and an exact capture viewport"
                .into(),
        );
    }
    let (capture_control, initial_capture) = match capture_session {
        Some((control, selection)) => (Some(Rc::new(control)), Some(selection)),
        None => (None, None),
    };
    let initial_capture_identity = initial_capture.as_ref().map(FixtureSelection::identity);
    let presentation = Rc::new(RefCell::new(match initial_capture {
        Some(selection) => PresentationState::new_with_behavior(
            selection.into_snapshot(),
            PresentationTime::new(progress_clock.elapsed(), SystemTime::now()),
            inactivity_configuration,
            renderer_configuration.behavior,
        )?,
        None => PresentationState::disconnected_with_behavior(
            progress_clock.elapsed(),
            inactivity_configuration,
            renderer_configuration.behavior,
        ),
    }));
    let diagnostics = DiagnosticsConfiguration::from_environment()?
        .enabled()
        .then(|| Rc::new(RefCell::new(Diagnostics::default())));
    let connections = RendererConnections {
        snapshots: if capture_control.is_some() {
            None
        } else {
            let socket_path = env::var_os("ROONSCAPE_SOCKET")
                .map(PathBuf::from)
                .ok_or("ROONSCAPE_SOCKET must name the private Unix socket")?;
            Some(Rc::new(SnapshotSubscription::start(
                socket_path,
                SNAPSHOT_RETRY_DELAY,
            )))
        },
        fixture_navigation: env::var_os("ROONSCAPE_FIXTURE_CONTROL")
            .map(PathBuf::from)
            .map(|control_socket_path| FixtureNavigation::connect(&control_socket_path))
            .transpose()?
            .map(|navigation| Rc::new(RefCell::new(navigation))),
        capture_control,
    };
    let repository_root = resource_root()?;
    register_packaged_fallback_fonts(&repository_root.join("src/renderer"))?;

    // Keep X11 WM_CLASS aligned with the desktop entry and Wayland application ID.
    glib::set_prgname(Some(APPLICATION_ID));
    let application = gtk::Application::builder()
        .application_id(APPLICATION_ID)
        .build();
    let activation_error = Rc::new(RefCell::new(None));
    let runtime_error = Rc::new(RefCell::new(None));
    let captured_activation_error = activation_error.clone();
    let startup = RendererStartup {
        progress_clock,
        configuration: renderer_configuration,
        initial_capture: initial_capture_identity,
        runtime_error: runtime_error.clone(),
    };

    application.connect_activate(move |application| {
        if let Err(error) = build_window(
            application,
            presentation.clone(),
            connections.clone(),
            diagnostics.clone(),
            &repository_root,
            startup.clone(),
        ) {
            *captured_activation_error.borrow_mut() = Some(error);
            application.quit();
        }
    });
    application.run_with_args(&["roonscape-renderer"]);

    if let Some(error) = activation_error.borrow_mut().take() {
        return Err(error);
    }
    if let Some(error) = runtime_error.borrow_mut().take() {
        return Err(error.into());
    }

    Ok(())
}

fn resource_root() -> Result<PathBuf, io::Error> {
    env::current_exe()?
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "renderer executable should be inside target/release",
            )
        })
}

fn build_window(
    application: &gtk::Application,
    presentation: Rc<RefCell<PresentationState>>,
    connections: RendererConnections,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
    repository_root: &Path,
    startup: RendererStartup,
) -> Result<(), Box<dyn Error>> {
    let renderer_configuration = startup.configuration;
    let progress_clock = startup.progress_clock;
    if (renderer_configuration.behavior == PresentationBehavior::StaticFixture
        || renderer_configuration.capture.reduced_animation)
        && let Some(settings) = gtk::Settings::default()
    {
        settings.set_gtk_enable_animations(false);
    }
    let configured_viewport = renderer_configuration
        .capture
        .viewport
        .unwrap_or(Viewport::WINDOWED_FIXTURE);
    let fullscreen = renderer_configuration.capture.viewport.is_none()
        && env::var_os("ROONSCAPE_WINDOWED").is_none();
    let first_revealed_paint_control = env::var_os("ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL")
        .map(PathBuf::from)
        .map(|control_socket_path| UnixStream::connect(&control_socket_path))
        .transpose()?;
    if first_revealed_paint_control.is_some() && !fullscreen {
        return Err(
            "ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL requires fullscreen operation".into(),
        );
    }
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .decorated(false)
        .default_width(configured_viewport.width_px as i32)
        .default_height(configured_viewport.height_px as i32)
        .show_menubar(false)
        .title("RoonScape")
        .icon_name(APPLICATION_ID)
        .build();
    gtk::IconTheme::for_display(&WidgetExt::display(&window))
        .add_search_path(repository_root.join("src/desktop/icons"));
    let fullscreen_viewport = if fullscreen {
        present_fullscreen(&window)
    } else {
        None
    };
    let initial_viewport = fullscreen_viewport.unwrap_or(configured_viewport);
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
    let typography = match renderer_configuration.capture.typography {
        Some(requested) => select_capture_typography(&available_families, requested)?,
        None => select_typography(&available_families),
    };
    let palette_provider = install_style_providers(typography);
    let initial_frame = presentation
        .borrow()
        .frame_at(progress_clock.elapsed())
        .expect("the initial presentation was validated before GTK started");
    let initial_diagnostics = diagnostics.as_ref().map(|diagnostics| {
        diagnostics
            .borrow()
            .overlay_text(current_process_memory_bytes())
    });
    let rendering = if connections.capture_control.is_some() {
        RenderingConfiguration::capture(typography, renderer_configuration.behavior)
    } else if connections.fixture_navigation.is_some() {
        RenderingConfiguration::fixture(typography, renderer_configuration.behavior)
    } else {
        RenderingConfiguration::live(typography, renderer_configuration.behavior)
    };
    let presentation_view = Rc::new(RefCell::new(PresentationView::new(
        presentation.borrow().revision(),
        &initial_frame.presentation,
        initial_viewport,
        repository_root,
        palette_provider,
        initial_diagnostics.as_deref(),
        rendering,
    )));
    presentation_view
        .borrow_mut()
        .apply_inactivity(initial_frame.inactivity);
    let display = gtk::Overlay::new();
    if fullscreen {
        display.set_opacity(0.0);
    }
    display.set_child(Some(&presentation_view.borrow().root()));
    window.set_child(Some(&display));

    let key_controller = gtk::EventControllerKey::new();
    let keyboard = Rc::new(RefCell::new(RendererKeyboard::new(
        connections.fixture_navigation.is_some(),
    )));
    keyboard.borrow_mut().set_focused(window.is_active());
    let controlled_window = window.clone();
    let pressed_keyboard = keyboard.clone();
    let navigation = connections.fixture_navigation.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        let action = pressed_keyboard.borrow_mut().press(renderer_key(key));
        match action {
            RendererAction::Close => {
                controlled_window.close();
                glib::Propagation::Stop
            }
            RendererAction::Navigate(intent) => {
                if let Some(navigation) = navigation.as_ref()
                    && let Err(error) = navigation.borrow_mut().send(intent)
                {
                    eprintln!("RoonScape Renderer: could not navigate Fixture Mode: {error}");
                }
                glib::Propagation::Stop
            }
            RendererAction::None => glib::Propagation::Proceed,
        }
    });
    let released_keyboard = keyboard.clone();
    key_controller.connect_key_released(move |_, key, _, _| {
        released_keyboard.borrow_mut().release(renderer_key(key));
    });
    window.add_controller(key_controller);
    window.connect_is_active_notify(move |window| {
        keyboard.borrow_mut().set_focused(window.is_active());
    });

    let runtime = Rc::new(PresentationRuntime {
        presentation,
        rendered_presentation: RefCell::new(initial_frame.presentation.clone()),
        reported_lyric_visibility: Cell::new(None),
        presentation_view: presentation_view.clone(),
        updates: connections.snapshots,
        capture_control: connections.capture_control,
        painted_capture: RefCell::new(PaintedFixtureSelection::new(startup.initial_capture)),
        diagnostics: diagnostics.clone(),
        display: display.clone(),
        repository_root: repository_root.to_path_buf(),
        progress_clock,
        fixture_navigation_enabled: connections.fixture_navigation.is_some(),
        capture_viewport: renderer_configuration.capture.viewport,
        application: application.clone(),
        runtime_error: startup.runtime_error,
    });
    if let Some(updates) = runtime.updates.as_ref() {
        let wakeup_runtime = Rc::clone(&runtime);
        let updates = Rc::clone(updates);
        glib_unix::unix_fd_add_local(updates.wakeup_fd(), glib::IOCondition::IN, move |_, _| {
            if let Err(error) = updates.clear_wakeup() {
                wakeup_runtime.fail(format!("could not clear snapshot wakeup: {error}"));
                return glib::ControlFlow::Break;
            }
            wakeup_runtime.apply_pending_updates();
            glib::ControlFlow::Continue
        });
    }
    if let Some(control) = runtime.capture_control.as_ref() {
        let wakeup_runtime = Rc::clone(&runtime);
        let control = Rc::clone(control);
        glib_unix::unix_fd_add_local(control.wakeup_fd(), glib::IOCondition::IN, move |_, _| {
            if let Err(error) = control.clear_wakeup() {
                wakeup_runtime.fail(format!("could not clear capture control wakeup: {error}"));
                return glib::ControlFlow::Break;
            }
            if wakeup_runtime.apply_capture_commands() {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
    }
    let timer_runtime = Rc::clone(&runtime);
    glib::timeout_add_local(Duration::from_millis(50), move || {
        timer_runtime.tick();
        glib::ControlFlow::Continue
    });

    install_diagnostics_updates(&window, diagnostics, presentation_view);

    if fullscreen {
        let mapped_runtime = Rc::clone(&runtime);
        window.connect_map(move |_| {
            mapped_runtime.prepare_first_visible_frame();
        });
        let first_frame_runtime = Rc::clone(&runtime);
        window.add_tick_callback(move |_, _| {
            if first_frame_runtime.prepare_first_visible_frame() {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
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

    if let Some(first_revealed_paint_control) = first_revealed_paint_control {
        let first_revealed_paint_runtime = Rc::clone(&runtime);
        let first_revealed_paint_control = Rc::new(RefCell::new(first_revealed_paint_control));
        let first_revealed_paint_phase =
            Rc::new(Cell::new(FirstRevealedPaintPhase::WaitingForReveal));
        let first_revealed_paint_display = display.clone();
        window.connect_map(move |window| {
            let Some(frame_clock) = window.frame_clock() else {
                first_revealed_paint_runtime
                    .fail("fullscreen renderer window has no frame clock after mapping".into());
                return;
            };
            let first_revealed_paint_runtime = Rc::clone(&first_revealed_paint_runtime);
            let first_revealed_paint_control = Rc::clone(&first_revealed_paint_control);
            let first_revealed_paint_phase = Rc::clone(&first_revealed_paint_phase);
            let first_revealed_paint_display = first_revealed_paint_display.clone();
            let observed_frame_clock = frame_clock.clone();
            frame_clock.begin_updating();
            frame_clock.connect_after_paint(move |_| match first_revealed_paint_phase.get() {
                FirstRevealedPaintPhase::WaitingForReveal => {
                    if first_revealed_paint_display.opacity() == 0.0 {
                        return;
                    }
                    first_revealed_paint_phase.set(FirstRevealedPaintPhase::WaitingForRepaint);
                    let result = {
                        let mut control = first_revealed_paint_control.borrow_mut();
                        let mut release = [0];
                        control
                            .write_all(b"painted\n")
                            .and_then(|()| control.read_exact(&mut release))
                    };
                    if let Err(error) = result {
                        first_revealed_paint_phase.set(FirstRevealedPaintPhase::Complete);
                        observed_frame_clock.end_updating();
                        first_revealed_paint_runtime.fail(format!(
                            "could not synchronize the first painted frame: {error}"
                        ));
                        return;
                    }
                    first_revealed_paint_display.queue_draw();
                }
                FirstRevealedPaintPhase::WaitingForRepaint => {
                    first_revealed_paint_phase.set(FirstRevealedPaintPhase::Complete);
                    if let Err(error) = first_revealed_paint_control
                        .borrow_mut()
                        .write_all(b"repainted\n")
                    {
                        first_revealed_paint_runtime.fail(format!(
                            "could not report the post-release painted frame: {error}"
                        ));
                    }
                    observed_frame_clock.end_updating();
                }
                FirstRevealedPaintPhase::Complete => {}
            });
        });
    }
    window.present();
    if runtime.capture_control.is_some() {
        let ready_runtime = Rc::clone(&runtime);
        window.add_tick_callback(move |_, _| {
            ready_runtime.prepare_capture_acknowledgement();
            glib::ControlFlow::Continue
        });
    }
    let painted_runtime = Rc::clone(&runtime);
    window
        .frame_clock()
        .ok_or("renderer window has no frame clock after presentation")?
        .connect_after_paint(move |_| painted_runtime.after_paint());
    Ok(())
}

fn present_fullscreen(window: &gtk::ApplicationWindow) -> Option<Viewport> {
    let monitors = gtk::prelude::WidgetExt::display(window).monitors();
    if monitors.n_items() == 1
        && let Some(monitor) = monitors.item(0).and_downcast::<gtk::gdk::Monitor>()
    {
        let geometry = monitor.geometry();
        window.set_default_size(geometry.width(), geometry.height());
        window.fullscreen_on_monitor(&monitor);
        Some(Viewport::new(
            geometry.width() as u32,
            geometry.height() as u32,
        ))
    } else {
        window.fullscreen();
        None
    }
}

fn renderer_key(key: gtk::gdk::Key) -> RendererKey {
    if key == gtk::gdk::Key::Escape {
        RendererKey::Escape
    } else if key == gtk::gdk::Key::Left {
        RendererKey::Left
    } else if key == gtk::gdk::Key::Right {
        RendererKey::Right
    } else {
        RendererKey::Other
    }
}

fn capture_configuration_from_environment() -> Result<CaptureConfiguration, Box<dyn Error>> {
    let viewport = env::var("ROONSCAPE_CAPTURE_VIEWPORT")
        .ok()
        .map(|value| parse_capture_viewport(&value))
        .transpose()?;
    let typography = env::var("ROONSCAPE_CAPTURE_TYPOGRAPHY")
        .ok()
        .map(|value| match value.as_str() {
            "preferred" => Ok(NowPlayingTitleFace::Preferred),
            "fallback" => Ok(NowPlayingTitleFace::Fallback),
            _ => Err("ROONSCAPE_CAPTURE_TYPOGRAPHY must be preferred or fallback"),
        })
        .transpose()?;
    if typography.is_some() && viewport.is_none() {
        return Err("ROONSCAPE_CAPTURE_TYPOGRAPHY requires ROONSCAPE_CAPTURE_VIEWPORT".into());
    }
    let reduced_animation = env::var("ROONSCAPE_CAPTURE_REDUCED_ANIMATION").as_deref() == Ok("1");
    if reduced_animation && viewport.is_none() {
        return Err(
            "ROONSCAPE_CAPTURE_REDUCED_ANIMATION requires ROONSCAPE_CAPTURE_VIEWPORT".into(),
        );
    }

    Ok(CaptureConfiguration {
        viewport,
        typography,
        reduced_animation,
    })
}

fn renderer_configuration_from_environment() -> Result<RendererConfiguration, Box<dyn Error>> {
    Ok(RendererConfiguration {
        capture: capture_configuration_from_environment()?,
        behavior: if env::var("ROONSCAPE_STATIC_FIXTURE").as_deref() == Ok("1") {
            PresentationBehavior::StaticFixture
        } else {
            PresentationBehavior::Dynamic
        },
    })
}

fn parse_capture_viewport(value: &str) -> Result<Viewport, Box<dyn Error>> {
    let (width, height) = value
        .split_once('x')
        .ok_or("ROONSCAPE_CAPTURE_VIEWPORT must use WIDTHxHEIGHT")?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "ROONSCAPE_CAPTURE_VIEWPORT width must be a positive integer")?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "ROONSCAPE_CAPTURE_VIEWPORT height must be a positive integer")?;
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("ROONSCAPE_CAPTURE_VIEWPORT dimensions must fit positive GTK sizes".into());
    }

    Ok(Viewport::new(width, height))
}

impl PresentationRuntime {
    fn tick(&self) {
        self.apply_viewport();
        let now = self.progress_clock.elapsed();
        self.apply_snapshot_events(now);
        self.render(now);
        self.presentation_view.borrow_mut().finish_transition();
    }

    fn apply_pending_updates(&self) {
        self.apply_viewport();
        let now = self.progress_clock.elapsed();
        self.apply_snapshot_events(now);
        self.render(now);
    }

    fn apply_viewport(&self) -> bool {
        let width = self.display.width();
        let height = self.display.height();
        if width <= 0 || height <= 0 {
            return false;
        }

        self.presentation_view
            .borrow_mut()
            .apply_viewport(Viewport::new(width as u32, height as u32));
        true
    }

    fn prepare_first_visible_frame(&self) -> bool {
        if !self.apply_viewport() || !self.presentation_view.borrow().layout_ready() {
            return false;
        }
        self.display.set_opacity(1.0);
        true
    }

    fn apply_snapshot_events(&self, now: Duration) {
        let Some(updates) = self.updates.as_ref() else {
            return;
        };
        loop {
            match updates.try_recv() {
                Ok(SnapshotEvent::Snapshot(snapshot)) => {
                    if let Some(diagnostics) = self.diagnostics.as_ref() {
                        diagnostics
                            .borrow_mut()
                            .observe_snapshot(&snapshot, &self.repository_root);
                    }
                    let anchored_at = PresentationTime::new(now, SystemTime::now());
                    let update = if self.fixture_navigation_enabled {
                        self.presentation
                            .borrow_mut()
                            .update_for_fixture_selection(*snapshot, anchored_at)
                    } else {
                        self.presentation
                            .borrow_mut()
                            .update(*snapshot, anchored_at)
                    };
                    if let Err(error) = update {
                        eprintln!("RoonScape Renderer: {error}");
                    }
                }
                Ok(SnapshotEvent::ConnectionChanged(connection)) => {
                    if let Some(diagnostics) = self.diagnostics.as_ref() {
                        diagnostics.borrow_mut().observe_connection(connection);
                    }
                    if connection == ConnectionState::Disconnected {
                        self.presentation.borrow_mut().disconnect(now);
                    }
                }
                Ok(SnapshotEvent::RevisionRejected { incoming, accepted }) => {
                    eprintln!(
                        "RoonScape Renderer: ignored presentation revision {incoming}; current revision is {accepted}"
                    );
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    fn apply_capture_commands(&self) -> bool {
        let Some(control) = self.capture_control.as_ref() else {
            return true;
        };
        let now = self.progress_clock.elapsed();
        loop {
            match control.try_recv() {
                Ok(CaptureControlEvent::Selection(selection)) => {
                    let identity = selection.identity();
                    let anchored_at = PresentationTime::new(now, SystemTime::now());
                    match self
                        .presentation
                        .borrow_mut()
                        .update_for_fixture_selection((*selection).into_snapshot(), anchored_at)
                    {
                        Ok(_) => {
                            self.painted_capture.borrow_mut().select(identity);
                        }
                        Err(error) => {
                            self.fail(format!("could not select Fixture Scenario: {error}"));
                            return false;
                        }
                    }
                }
                Ok(CaptureControlEvent::Disconnected) => {
                    self.fail("capture control channel disconnected".to_owned());
                    return false;
                }
                Ok(CaptureControlEvent::Failed(error)) => {
                    self.fail(error);
                    return false;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.fail("capture control reader stopped unexpectedly".to_owned());
                    return false;
                }
            }
        }
        self.apply_viewport();
        self.render(now);
        self.display.queue_draw();
        true
    }

    fn prepare_capture_acknowledgement(&self) {
        let Some(revision) = self.painted_capture.borrow().pending_revision() else {
            return;
        };
        let Some(viewport) = self.capture_viewport else {
            return;
        };
        match self
            .presentation_view
            .borrow()
            .capture_ready(revision, viewport)
        {
            Ok(false) => {}
            Ok(true) => self
                .painted_capture
                .borrow_mut()
                .presentation_completed(revision),
            Err(error) => self.fail(error),
        }
    }

    fn after_paint(&self) {
        self.report_visible_lyrics();
        let Some(selection) = self.painted_capture.borrow_mut().after_paint() else {
            return;
        };
        let Some(control) = self.capture_control.as_ref() else {
            return;
        };
        if let Err(error) = control.acknowledge(&selection) {
            self.fail(format!(
                "could not acknowledge painted Fixture Scenario: {error}"
            ));
        }
    }

    fn report_visible_lyrics(&self) {
        let revision = self.presentation.borrow().revision();
        let has_visible_lyrics = matches!(
            &*self.rendered_presentation.borrow(),
            Presentation::NowPlaying(now_playing) if now_playing.lyrics.is_some()
        );
        if !has_visible_lyrics {
            self.reported_lyric_visibility.set(None);
            return;
        }
        if self.reported_lyric_visibility.get() == Some(revision) {
            return;
        }
        let Some(updates) = self.updates.as_ref() else {
            return;
        };
        match updates.report_lyrics_visible(revision) {
            Ok(true) => self.reported_lyric_visibility.set(Some(revision)),
            Ok(false) => {}
            Err(error) => {
                eprintln!("RoonScape Renderer: could not report visible lyrics: {error}");
            }
        }
    }

    fn fail(&self, error: String) {
        if self.runtime_error.borrow().is_none() {
            *self.runtime_error.borrow_mut() = Some(error);
        }
        self.application.quit();
    }

    fn render(&self, now: Duration) {
        let current_frame = match self.presentation.borrow().frame_at(now) {
            Ok(current_frame) => current_frame,
            Err(error) => {
                eprintln!("RoonScape Renderer: {error}");
                return;
            }
        };

        let presentation_update = classify_presentation_update(
            &self.rendered_presentation.borrow(),
            &current_frame.presentation,
        );
        match presentation_update {
            PresentationUpdate::TransitionRequired => {
                self.presentation_view.borrow_mut().replace(
                    self.presentation.borrow().revision(),
                    &current_frame.presentation,
                    &self.repository_root,
                );
            }
            PresentationUpdate::InPlace => {
                self.presentation_view.borrow_mut().update_in_place(
                    self.presentation.borrow().revision(),
                    &current_frame.presentation,
                );
            }
        }
        self.rendered_presentation
            .replace(current_frame.presentation.clone());
        self.presentation_view
            .borrow_mut()
            .apply_inactivity(current_frame.inactivity);
    }
}

fn install_diagnostics_updates(
    window: &gtk::ApplicationWindow,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
    presentation_view: Rc<RefCell<PresentationView>>,
) {
    let Some(diagnostics) = diagnostics else {
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
        let text = diagnostics
            .borrow()
            .overlay_text(current_process_memory_bytes());
        presentation_view.borrow().update_diagnostics(&text);
        glib::ControlFlow::Continue
    });
}

fn configuration_file_from_arguments() -> Result<PathBuf, Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    match (arguments.next(), arguments.next(), arguments.next()) {
        (None, None, None) => Ok(display_configuration_file_path()?),
        (Some(option), Some(configuration_file), None)
            if option == "--config" && !configuration_file.is_empty() =>
        {
            Ok(PathBuf::from(configuration_file))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "RoonScape Renderer accepts only a launcher-provided --config PATH",
        )
        .into()),
    }
}

fn host_inactivity_configuration(configuration_file: &Path) -> InactivityConfiguration {
    let configuration = load_inactivity_configuration(configuration_file);
    match configuration {
        Ok(configuration) => configuration,
        Err(error) => {
            eprintln!("RoonScape Renderer: {error}; using default OLED inactivity calibration");
            InactivityConfiguration::default()
        }
    }
}
