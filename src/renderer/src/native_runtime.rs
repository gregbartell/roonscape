use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::{Duration, Instant, SystemTime};

use gtk::prelude::*;
use roonscape_renderer::{
    CaptureControl, CaptureControlEvent, ConnectionState, FixtureNavigation, FramePacing,
    InactivityLayout, PaintedFixtureSelection, Presentation, PresentationBehavior,
    PresentationState, PresentationTime, PresentationUpdate, RendererAction, RendererKey,
    RendererKeyboard, SnapshotEvent, SnapshotSubscription, Viewport, classify_presentation_update,
    register_packaged_fallback_fonts, select_capture_typography, select_typography,
};

use crate::displayed_state::DisplayedState;
use crate::native_view::NativeView;
use crate::prepared_presentation::{
    PreparedPresentation, PresentationPreparation, preparation_key,
};
use crate::qt_window::{PaintedFrame, RenderFrame, Window, WindowEvents};

#[derive(Clone, PartialEq)]
struct PreparationKey {
    presentation: Presentation,
    viewport: Viewport,
    scale: f64,
    diagnostics: Option<String>,
    generation: u64,
}
struct Request {
    key: PreparationKey,
    presentation: Presentation,
    token: u64,
}
enum PreparationWork {
    Artwork(roonscape_renderer::ArtworkReference),
    Presentation(Box<Request>),
}
struct Reply<'window> {
    token: u64,
    ready_at: Duration,
    result: Result<PreparedPresentation<'window>, String>,
}

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let configuration_file = crate::configuration_file_from_arguments()?;
    let inactivity = crate::host_inactivity_configuration(&configuration_file);
    let configuration = crate::renderer_configuration_from_environment()?;
    let diagnostics = roonscape_renderer::DiagnosticsConfiguration::from_environment()?
        .enabled()
        .then(roonscape_renderer::Diagnostics::default);
    let repository = crate::resource_root()?;
    register_packaged_fallback_fonts(&repository.join("src/renderer"))?;
    // GTK settings remain the platform adapter for the existing reduced-motion
    // preference. GTK creates no window and does not drive rendering.
    gtk::init()?;
    let clock = Instant::now();
    let capture = env::var_os("ROONSCAPE_CAPTURE_CONTROL")
        .map(PathBuf::from)
        .map(|path| CaptureControl::connect(&path))
        .transpose()?;
    if capture.is_some()
        && (configuration.behavior != PresentationBehavior::StaticFixture
            || configuration.capture.viewport.is_none())
    {
        return Err(
            "ROONSCAPE_CAPTURE_CONTROL requires static Fixture Mode and an exact capture viewport"
                .into(),
        );
    }
    let initial_capture = capture.as_ref().map(|(_, selection)| selection.identity());
    let (capture, state) = match capture {
        Some((control, selection)) => (
            Some(control),
            PresentationState::new_with_behavior(
                selection.into_snapshot(),
                PresentationTime::new(clock.elapsed(), SystemTime::now()),
                inactivity,
                configuration.behavior,
            )?,
        ),
        None => (
            None,
            PresentationState::disconnected_with_behavior(
                clock.elapsed(),
                inactivity,
                configuration.behavior,
            ),
        ),
    };
    let navigation = env::var_os("ROONSCAPE_FIXTURE_CONTROL")
        .map(PathBuf::from)
        .map(|path| FixtureNavigation::connect(&path))
        .transpose()?;
    let families = pangocairo::FontMap::new()
        .list_families()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<HashSet<_>>();
    let typography = match configuration.capture.typography {
        Some(face) => select_capture_typography(&families, face)?,
        None => select_typography(&families),
    };
    let viewport = configuration
        .capture
        .viewport
        .unwrap_or(Viewport::WINDOWED_FIXTURE);
    let fullscreen = configuration.capture.viewport.is_none()
        && env::var("ROONSCAPE_WINDOWED").as_deref() != Ok("1");
    let window = Window::new(viewport, fullscreen)?;
    window.icon(
        &repository.join("src/desktop/icons/hicolor/scalable/apps/io.roonscape.Renderer.svg"),
    )?;
    let viewport = window.viewport();
    let (send, receive) = mpsc::channel::<PreparationWork>();
    let send = Arc::new(send);
    let artwork_send = Arc::downgrade(&send);
    let snapshots = if capture.is_some() {
        None
    } else {
        let socket = env::var_os("ROONSCAPE_SOCKET")
            .ok_or("ROONSCAPE_SOCKET must name the private Unix socket")?;
        Some(SnapshotSubscription::start_with_observer(
            PathBuf::from(socket),
            Duration::from_millis(250),
            move |snapshot| {
                if let Some(artwork) = &snapshot.artwork
                    && let Some(send) = artwork_send.upgrade()
                {
                    let _ = send.send(PreparationWork::Artwork(artwork.clone()));
                }
            },
        ))
    };
    let (wake, mut notify) = UnixStream::pair()?;
    wake.set_nonblocking(true)?;
    notify.set_nonblocking(true)?;
    let (reply_send, replies) = mpsc::channel();
    let latest = Arc::new(AtomicU64::new(0));
    let worker_latest = latest.clone();
    let strict = capture.is_some();
    let uploader = window.uploader();
    let initial = state.frame_at(clock.elapsed())?;
    let displayed = DisplayedState::new(&state, &initial.presentation);
    let generation = state.now_playing_generation();
    let revision = state.revision();
    let first_paint = env::var_os("ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL")
        .map(PathBuf::from)
        .map(UnixStream::connect)
        .transpose()?;
    if first_paint.is_some() && !fullscreen {
        return Err(
            "ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL requires fullscreen operation".into(),
        );
    }
    let mut runtime = Runtime {
        state,
        displayed,
        shown: initial.presentation.clone(),
        rendered: initial.presentation,
        generation,
        revision,
        view: None,
        ready: None,
        ready_at: Duration::ZERO,
        installed_palette: None,
        send,
        replies,
        wake,
        latest,
        key: None,
        installed_key: None,
        token: 0,
        snapshots,
        capture,
        navigation,
        painted: PaintedFixtureSelection::new(initial_capture),
        paint_records: std::collections::VecDeque::new(),
        reported_lyrics: None,
        keyboard: RendererKeyboard::new(false),
        diagnostics,
        diagnostics_text: None,
        diagnostics_at: None,
        repository: repository.clone(),
        clock,
        configuration,
        viewport,
        refresh: 60000,
        pacing: FramePacing::default(),
        error: None,
        dirty: true,
        motion: false,
        animating: false,
        platform_animations: true,
        last_inactivity: initial.inactivity,
        first_paint,
        first_paint_sent: false,
        first_paint_complete: false,
        auto_close: env::var("ROONSCAPE_FIXTURE_AUTO_CLOSE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_millis),
    };
    runtime.keyboard = RendererKeyboard::new(runtime.navigation.is_some());
    window.watch(runtime.wake.as_raw_fd(), 1);
    if let Some(snapshots) = &runtime.snapshots {
        window.watch(snapshots.wakeup_fd(), 2);
    }
    if let Some(capture) = &runtime.capture {
        window.watch(capture.wakeup_fd(), 3);
    }
    window.timer(50, 4);
    std::thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        let worker = scope.spawn(move || {
            let mut scale = 1.0;
            let mut preparation = match PresentationPreparation::new(uploader, typography, scale) {
                Ok(preparation) => preparation,
                Err(error) => {
                    let _ = reply_send.send(Reply {
                        token: 0,
                        ready_at: clock.elapsed(),
                        result: Err(error),
                    });
                    let _ = notify.write(&[1]);
                    return;
                }
            };
            while let Ok(work) = receive.recv() {
                let mut artwork = None;
                let mut request = None;
                for work in std::iter::once(work).chain(receive.try_iter()) {
                    match work {
                        PreparationWork::Artwork(next) => artwork = Some(next),
                        PreparationWork::Presentation(next) => request = Some(next),
                    }
                }
                let Some(request) = request else {
                    if let Some(artwork) = artwork {
                        preparation.prepare_arriving_artwork(artwork, &repository);
                    }
                    continue;
                };
                if request.token != worker_latest.load(Ordering::Acquire) {
                    continue;
                }
                let result = (|| {
                    if scale != request.key.scale {
                        preparation =
                            PresentationPreparation::new(uploader, typography, request.key.scale)?;
                        scale = request.key.scale;
                    }
                    let mut prepared = preparation.prepare(
                        &request.presentation,
                        request.key.viewport,
                        &repository,
                        strict,
                    )?;
                    prepared.diagnostics = request
                        .key
                        .diagnostics
                        .as_deref()
                        .map(|text| preparation.diagnostics(text))
                        .transpose()?;
                    Ok(prepared)
                })();
                if request.token != worker_latest.load(Ordering::Acquire) {
                    continue;
                }
                if reply_send
                    .send(Reply {
                        token: request.token,
                        ready_at: clock.elapsed(),
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                let _ = notify.write(&[1]);
            }
        });
        runtime.refresh(&window);
        let result = window.run(&mut runtime);
        let error = runtime.error.take();
        drop(runtime);
        worker
            .join()
            .map_err(|_| "Presentation preparation thread panicked")?;
        result?;
        if let Some(error) = error {
            return Err(error.into());
        }
        Ok(())
    })
}

struct PaintedScene {
    serial: u64,
    revision: u64,
    visible_lyrics: bool,
    capture_ready: bool,
    values: Option<Arc<crate::animation_evidence::SceneValues>>,
    active: bool,
}

struct Runtime<'window> {
    state: PresentationState,
    displayed: DisplayedState,
    rendered: Presentation,
    shown: Presentation,
    installed_palette: Option<roonscape_renderer::PresentationPalette>,
    generation: u64,
    revision: u64,
    view: Option<NativeView<'window>>,
    ready: Option<PreparedPresentation<'window>>,
    ready_at: Duration,
    send: Arc<mpsc::Sender<PreparationWork>>,
    replies: mpsc::Receiver<Reply<'window>>,
    wake: UnixStream,
    latest: Arc<AtomicU64>,
    key: Option<PreparationKey>,
    installed_key: Option<PreparationKey>,
    token: u64,
    snapshots: Option<SnapshotSubscription>,
    capture: Option<CaptureControl>,
    navigation: Option<FixtureNavigation>,
    painted: PaintedFixtureSelection,
    paint_records: std::collections::VecDeque<PaintedScene>,
    reported_lyrics: Option<u64>,
    keyboard: RendererKeyboard,
    diagnostics: Option<roonscape_renderer::Diagnostics>,
    diagnostics_text: Option<String>,
    diagnostics_at: Option<Duration>,
    repository: PathBuf,
    clock: Instant,
    configuration: crate::RendererConfiguration,
    viewport: Viewport,
    refresh: u32,
    pacing: FramePacing,
    error: Option<String>,
    dirty: bool,
    motion: bool,
    animating: bool,
    platform_animations: bool,
    last_inactivity: roonscape_renderer::InactivityTransform,
    first_paint: Option<UnixStream>,
    first_paint_sent: bool,
    first_paint_complete: bool,
    auto_close: Option<Duration>,
}

impl Runtime<'_> {
    fn animated(&self) -> bool {
        self.configuration.behavior.animations_enabled(
            !self.configuration.capture.reduced_animation && self.platform_animations,
        )
    }

    fn fail(&mut self, window: &Window, error: impl ToString) {
        self.error = Some(error.to_string());
        window.quit();
    }

    fn refresh_gui_state(&mut self) {
        let platform_animations =
            gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
        self.dirty |= self.platform_animations != platform_animations;
        self.platform_animations = platform_animations;
        let now = self.clock.elapsed();
        if let Some(diagnostics) = &self.diagnostics
            && self
                .diagnostics_at
                .is_none_or(|previous| now.saturating_sub(previous) >= Duration::from_millis(500))
        {
            self.diagnostics_text =
                Some(diagnostics.overlay_text(roonscape_renderer::current_process_memory_bytes()));
            self.diagnostics_at = Some(now);
        }
    }

    fn refresh(&mut self, window: &Window) {
        self.refresh_gui_state();
        if let Err(error) = self.request_preparation(window.scale()) {
            self.fail(window, error);
        }
        if !self.animating {
            self.animating = true;
            window.animate(true);
        }
        window.wake();
    }

    fn request_preparation(
        &mut self,
        scale: f64,
    ) -> Result<roonscape_renderer::PresentationFrame, String> {
        let incoming = self
            .state
            .frame_at(self.clock.elapsed())
            .map_err(|e| e.to_string())?;
        let layout = InactivityLayout::for_viewport(self.viewport, incoming.inactivity);
        let key = PreparationKey {
            presentation: preparation_key(&incoming.presentation),
            viewport: layout.content_viewport,
            scale,
            diagnostics: self.diagnostics_text.clone(),
            generation: self.state.now_playing_generation(),
        };
        if self.key.as_ref() != Some(&key) {
            self.token += 1;
            self.latest.store(self.token, Ordering::Release);
            crate::animation_evidence::record(|| {
                serde_json::json!({
                    "event": "preparation-requested", "token": self.token,
                    "revision": self.state.revision(), "observedMicros": crate::qt_window::clock_micros(),
                })
            });
            self.send
                .send(PreparationWork::Presentation(Box::new(Request {
                    key: key.clone(),
                    presentation: incoming.presentation.clone(),
                    token: self.token,
                })))
                .map_err(|e| e.to_string())?;
            self.key = Some(key);
            self.ready = None;
        }
        Ok(incoming)
    }

    fn render_inner(&mut self, frame: &RenderFrame) -> Result<(), String> {
        let now = self.clock.elapsed();
        let animated = self.animated();
        let incoming = self.request_preparation(frame.scale)?;
        while let Ok(reply) = self.replies.try_recv() {
            if reply.token == 0 {
                return Err(reply
                    .result
                    .err()
                    .unwrap_or_else(|| "Preparation stopped".into()));
            }
            if reply.token == self.token {
                crate::animation_evidence::record(|| {
                    serde_json::json!({
                        "event": "preparation-ready", "token": reply.token,
                        "readyMicros": crate::qt_window::clock_micros() - self.clock.elapsed().saturating_sub(reply.ready_at).as_micros() as i64,
                        "artworkTexture": reply.result.as_ref().ok().and_then(|prepared| prepared.artwork.as_ref()).map(|texture| texture.identity()),
                    })
                });
                self.ready_at = reply.ready_at;
                self.ready = Some(reply.result?);
            }
        }
        let incoming_palette = if let Some(ready) = &self.ready {
            ready.artwork.as_ref().map(|_| ready.palette)
        } else if self.installed_key == self.key {
            self.installed_palette
        } else if matches!(&incoming.presentation,Presentation::NowPlaying(value) if value.artwork_path.is_some())
        {
            // Until decoding finishes, retain the useful Now Playing candidate.
            Some(roonscape_renderer::PresentationPalette::fallback())
        } else {
            None
        };
        let destination = roonscape_renderer::resolve_presentation_with_palette(
            &incoming.presentation,
            incoming_palette,
        )
        .presentation;
        let replacement = self.generation != self.state.now_playing_generation()
            || classify_presentation_update(&self.shown, &destination)
                == PresentationUpdate::TransitionRequired;
        if replacement && let Some(view) = &mut self.view {
            view.begin_departure(now, animated);
        }
        let reveal_ready = (self.ready.is_some() || self.installed_key == self.key)
            && (!replacement
                || self
                    .view
                    .as_ref()
                    .is_none_or(|view| view.departure_complete(now, animated)));
        let selected = self
            .displayed
            .frame_at(&self.state, now, |presentation| {
                (
                    reveal_ready,
                    self.view
                        .as_ref()
                        .is_none_or(|view| view.lyrics_ready(presentation)),
                )
            })
            .map_err(|e| e.to_string())?;
        let mut presentation = selected.frame.presentation;
        if !reveal_ready
            && let (Presentation::NowPlaying(value), Presentation::NowPlaying(old)) =
                (&mut presentation, &self.rendered)
        {
            value.title.clone_from(&old.title);
            value.artist.clone_from(&old.artist);
            value.album.clone_from(&old.album);
            value.tracked_output.clone_from(&old.tracked_output);
            value.tracked_zone.clone_from(&old.tracked_zone);
        }
        let resolved = roonscape_renderer::resolve_presentation_with_palette(
            &presentation,
            if reveal_ready {
                incoming_palette
            } else {
                self.installed_palette
            },
        )
        .presentation;
        if reveal_ready && let Some(prepared) = self.ready.take() {
            self.installed_key = self.key.clone();
            self.installed_palette = prepared.artwork.as_ref().map(|_| prepared.palette);
            match &mut self.view {
                Some(view) if replacement => view.replace(
                    prepared,
                    &resolved,
                    selected.revision,
                    view.reveal_at(self.ready_at),
                    animated,
                ),
                Some(view) => view.install(prepared, self.ready_at, animated),
                None => self.view = Some(NativeView::new(prepared, selected.revision)),
            }
            self.generation = selected.generation;
        }
        if let Some(view) = &mut self.view {
            view.update(&resolved, selected.revision);
            let mut scene = view.render(now, animated);
            let inactivity = selected.frame.inactivity;
            let layout = InactivityLayout::for_viewport(self.viewport, inactivity);
            for graphic in &mut scene.graphics {
                graphic.geometry.canvas.x = layout.margin_start_px as f32;
                graphic.geometry.canvas.y = layout.margin_top_px as f32;
                graphic.geometry.weight *= inactivity.opacity as f32;
            }
            for sprite in &mut scene.sprites {
                sprite.geometry.bounds.x += layout.margin_start_px as f32;
                sprite.geometry.bounds.y += layout.margin_top_px as f32;
                sprite.geometry.clip.x += layout.margin_start_px as f32;
                sprite.geometry.clip.y += layout.margin_top_px as f32;
                sprite.geometry.dimming = 1.0 - inactivity.opacity as f32;
            }
            let serial = frame.submit(&scene);
            self.revision = selected.revision;
            self.paint_records.push_back(PaintedScene {
                serial,
                revision: self.revision,
                visible_lyrics: view.visible_lyrics(),
                values: crate::animation_evidence::scene_values(&scene),
                active: false,
                capture_ready: self.configuration.behavior == PresentationBehavior::StaticFixture
                    && self.installed_key == self.key
                    && self.generation == self.state.now_playing_generation(),
            });
            while self.paint_records.len() > 16 {
                self.paint_records.pop_front();
            }
            let continuous = match &presentation {
                Presentation::NowPlaying(value) => {
                    value.activity.is_some()
                        || value.status.motion
                            != roonscape_renderer::PresentationStatusMotion::Static
                        || (value.status.symbol
                            == roonscape_renderer::PresentationStatusSymbol::Playing
                            && value.progress.as_ref().is_some_and(|p| p.fraction < 1.0))
                }
                Presentation::FullField(value) => {
                    value.status.motion != roonscape_renderer::PresentationStatusMotion::Static
                }
            };
            self.motion = animated && (continuous || view.active(now));
            if let Some(painted) = self.paint_records.back_mut() {
                painted.active = self.motion;
            }
        }
        self.last_inactivity = selected.frame.inactivity;
        self.shown = resolved;
        self.rendered = presentation;
        self.dirty = false;
        if self.animating != self.motion {
            self.animating = self.motion;
            frame.animate(self.motion);
        }
        Ok(())
    }

    fn snapshots(&mut self) -> Result<(), String> {
        let Some(snapshots) = &self.snapshots else {
            return Ok(());
        };
        snapshots.clear_wakeup().map_err(|e| e.to_string())?;
        let now = self.clock.elapsed();
        while let Ok(event) = snapshots.try_recv() {
            match event {
                SnapshotEvent::Snapshot {
                    snapshot,
                    received_at,
                } => {
                    crate::animation_evidence::record(|| {
                        serde_json::json!({
                            "event": "snapshot-received", "revision": snapshot.revision,
                            "artwork": snapshot.artwork.as_ref().map(|artwork| serde_json::json!({
                                "path": artwork.path, "revision": artwork.revision,
                            })),
                            "receivedMicros": crate::qt_window::clock_micros() - received_at.elapsed().as_micros() as i64,
                        })
                    });
                    if let Some(diagnostics) = &mut self.diagnostics {
                        diagnostics.observe_snapshot(&snapshot, &self.repository);
                    }
                    let time = PresentationTime::new(now, SystemTime::now());
                    let result = if self.navigation.is_some() {
                        self.state.update_for_fixture_selection(*snapshot, time)
                    } else {
                        self.state.update(*snapshot, time)
                    };
                    if let Err(error) = result {
                        eprintln!("RoonScape Renderer: {error}");
                    }
                }
                SnapshotEvent::ConnectionChanged(connection) => {
                    if let Some(diagnostics) = &mut self.diagnostics {
                        diagnostics.observe_connection(connection);
                    }
                    if connection == ConnectionState::Disconnected {
                        self.state.disconnect(now);
                    }
                }
                SnapshotEvent::RevisionRejected { incoming, accepted } => eprintln!(
                    "RoonScape Renderer: ignored presentation revision {incoming}; current revision is {accepted}"
                ),
            }
        }
        Ok(())
    }
}

impl WindowEvents for Runtime<'_> {
    fn render(&mut self, frame: &RenderFrame) {
        self.viewport = frame.viewport;
        self.refresh = frame.rate;
        if (self.dirty || self.motion)
            && self.pacing.update_due(frame.now, frame.rate)
            && let Err(error) = self.render_inner(frame)
        {
            self.error = Some(error);
            frame.quit();
        }
    }

    fn update(&mut self, window: &Window, now: i64, viewport: Viewport, rate: u32) {
        self.refresh = rate;
        if let Some(diagnostics) = &mut self.diagnostics {
            diagnostics.observe_frame(Duration::from_micros(now.max(0) as u64));
        }
        if viewport != self.viewport
            || self
                .key
                .as_ref()
                .is_some_and(|key| key.scale != window.scale())
        {
            self.viewport = viewport;
            self.dirty = true;
        }
        // Start any newly needed text before Qt begins rendering. Prepared
        // replies are adopted by render_inner at the last possible moment.
        self.refresh_gui_state();
        if self.dirty && !self.animating {
            self.animating = true;
            window.animate(true);
        }
        if let Err(error) = self.request_preparation(window.scale()) {
            self.fail(window, error);
        }
    }

    fn input(&mut self, window: &Window, event: i32) {
        if event == 4 {
            self.refresh_gui_state();
            if self
                .auto_close
                .is_some_and(|deadline| self.clock.elapsed() >= deadline)
            {
                window.quit();
                return;
            }
            if !self.dirty && self.motion {
                return;
            }
            if !self.dirty
                && self.diagnostics.is_none()
                && self
                    .state
                    .frame_at(self.clock.elapsed())
                    .is_ok_and(|frame| {
                        frame.presentation == self.rendered
                            && frame.inactivity == self.last_inactivity
                    })
            {
                return;
            }
        }
        let result = (|| -> Result<(), String> {
            match event {
                1 => {
                    let mut bytes = [0; 256];
                    while self.wake.read(&mut bytes).is_ok_and(|count| count > 0) {}
                }
                2 => self.snapshots()?,
                3 => {
                    if let Some(control) = &self.capture {
                        control.clear_wakeup().map_err(|e| e.to_string())?;
                        while let Ok(event) = control.try_recv() {
                            match event {
                                CaptureControlEvent::Selection(selection) => {
                                    let identity = selection.identity();
                                    self.state
                                        .update_for_fixture_selection(
                                            (*selection).into_snapshot(),
                                            PresentationTime::new(
                                                self.clock.elapsed(),
                                                SystemTime::now(),
                                            ),
                                        )
                                        .map_err(|e| e.to_string())?;
                                    self.painted.select(identity);
                                }
                                CaptureControlEvent::Disconnected => {
                                    return Err("capture control channel disconnected".into());
                                }
                                CaptureControlEvent::Failed(error) => return Err(error),
                            }
                        }
                    }
                }
                4 => {
                    if self
                        .auto_close
                        .is_some_and(|deadline| self.clock.elapsed() >= deadline)
                    {
                        window.quit();
                    }
                }
                10 => self.keyboard.set_focused(true),
                11 => self.keyboard.set_focused(false),
                100..=102 => {
                    let key = match event {
                        100 => RendererKey::Escape,
                        101 => RendererKey::Left,
                        _ => RendererKey::Right,
                    };
                    match self.keyboard.press(key) {
                        RendererAction::Close => window.quit(),
                        RendererAction::Navigate(intent) => {
                            if let Some(navigation) = &mut self.navigation {
                                navigation.send(intent).map_err(|e| e.to_string())?;
                            }
                        }
                        RendererAction::None => {}
                    }
                }
                201 => self.keyboard.release(RendererKey::Left),
                202 => self.keyboard.release(RendererKey::Right),
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(window, error);
            return;
        }
        self.dirty = true;
        self.refresh(window);
    }

    fn painted(&mut self, window: &Window, frame: PaintedFrame) {
        let Some(painted) = self
            .paint_records
            .iter()
            .find(|painted| painted.serial == frame.scene)
        else {
            return;
        };
        if let Some(values) = &painted.values {
            crate::animation_evidence::painted(
                frame,
                self.viewport,
                self.refresh,
                painted.active,
                values,
            );
        }
        let revision = painted.revision;
        let visible_lyrics = painted.visible_lyrics;
        if painted.capture_ready {
            self.painted.presentation_completed(revision);
        }
        if let Some(selection) = self.painted.after_paint()
            && let Some(control) = &self.capture
            && let Err(error) = control.acknowledge(&selection)
        {
            self.fail(window, error);
            return;
        }
        if visible_lyrics {
            if self.reported_lyrics != Some(revision)
                && let Some(snapshots) = &self.snapshots
            {
                match snapshots.report_lyrics_visible(revision) {
                    Ok(true) => self.reported_lyrics = Some(revision),
                    Ok(false) => {}
                    Err(error) => {
                        eprintln!("RoonScape Renderer: could not report visible lyrics: {error}")
                    }
                }
            }
        } else {
            self.reported_lyrics = None;
        }
        if self.view.is_some()
            && !self.first_paint_complete
            && let Some(control) = &mut self.first_paint
        {
            let result = if self.first_paint_sent {
                control.write_all(b"repainted\n")
            } else {
                control
                    .write_all(b"painted\n")
                    .and_then(|()| control.read_exact(&mut [0]))
            };
            if let Err(error) = result {
                self.fail(window, error);
                return;
            }
            if self.first_paint_sent {
                self.first_paint_complete = true;
            } else {
                self.first_paint_sent = true;
                self.dirty = true;
                self.refresh(window);
            }
        }
    }
}
