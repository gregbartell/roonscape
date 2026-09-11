//! Qt owns windowing and display scheduling; Rust submits immutable scene data.
//! Uploads borrow the window so every preparation thread and texture must finish
//! before Qt tears down its shared graphics context.

use std::ffi::{CStr, CString, c_char, c_void};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use roonscape_renderer::{Rgb, Viewport};

#[repr(C)]
struct NativeWindow {
    _opaque: [u8; 0],
}
#[repr(C)]
struct NativeTexture {
    _opaque: [u8; 0],
}

#[repr(C)]
struct NativeUpload {
    _opaque: [u8; 0],
}

#[repr(C)]
struct NativeImage {
    _opaque: [u8; 0],
}

pub(crate) struct DecodedImage {
    native: NonNull<NativeImage>,
    pub size: Viewport,
    pub has_alpha: bool,
    pub stride: u32,
    pub channels: u32,
}

impl DecodedImage {
    pub fn open(path: &Path) -> Result<Self, String> {
        use std::os::unix::ffi::OsStrExt;
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let (mut width, mut height) = (0, 0);
        let native =
            NonNull::new(unsafe { rs_image_decode(path.as_ptr(), &mut width, &mut height) })
                .ok_or_else(|| "Artwork could not be decoded".to_owned())?;
        Ok(Self {
            native,
            size: Viewport::new(width, height),
            has_alpha: unsafe { rs_image_has_alpha(native.as_ptr()) },
            stride: unsafe { rs_image_stride(native.as_ptr()) },
            channels: unsafe { rs_image_channels(native.as_ptr()) },
        })
    }

    pub fn palette_image(&self) -> Result<Self, String> {
        let native = NonNull::new(unsafe { rs_image_palette(self.native.as_ptr()) })
            .ok_or("Cannot prepare artwork palette pixels")?;
        Ok(Self {
            native,
            size: self.size,
            has_alpha: self.has_alpha,
            stride: unsafe { rs_image_stride(native.as_ptr()) },
            channels: unsafe { rs_image_channels(native.as_ptr()) },
        })
    }

    pub fn pixels(&self) -> &[u8] {
        // The owned RGB/RGBA image is immutable, including initialized row padding.
        unsafe {
            std::slice::from_raw_parts(
                rs_image_pixels(self.native.as_ptr()),
                self.stride as usize * self.size.height_px as usize,
            )
        }
    }
}

// QImage owns immutable pixels and supports destruction on another thread.
unsafe impl Send for DecodedImage {}

impl AsRef<[u8]> for DecodedImage {
    fn as_ref(&self) -> &[u8] {
        self.pixels()
    }
}

impl Drop for DecodedImage {
    fn drop(&mut self) {
        unsafe { rs_image_delete(self.native.as_ptr()) };
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[repr(C)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn viewport(viewport: Viewport) -> Self {
        Self::new(
            0.0,
            0.0,
            viewport.width_px as f32,
            viewport.height_px as f32,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[repr(C)]
pub(crate) struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub fn new(rgb: Rgb, alpha: f32) -> Self {
        Self {
            red: f32::from(rgb.red) / 255.0,
            green: f32::from(rgb.green) / 255.0,
            blue: f32::from(rgb.blue) / 255.0,
            alpha,
        }
    }
}

#[derive(Clone, Copy, Default, serde::Serialize)]
#[repr(C)]
pub(crate) struct GraphicGeometry {
    pub canvas: Rect,
    pub artwork_bounds: Rect,
    pub plate_bounds: Rect,
    pub background: Color,
    pub border: Color,
    pub plate: Color,
    pub quiet: Color,
    pub muted: Color,
    pub origin: u32,
    pub step_x: u32,
    pub step_y: u32,
    pub border_width: f32,
    pub shadow_radius: f32,
    pub shadow_y: f32,
    pub shadow_alpha: f32,
    pub weight: f32,
}

#[derive(Clone, Default)]
pub(crate) struct Graphic<'window> {
    pub artwork: Option<Texture<'window>>,
    pub gradient: Option<Texture<'window>>,
    pub noise: Option<Texture<'window>>,
    pub geometry: GraphicGeometry,
}

// Numeric values are shared with the native sprite shader and evidence JSON.
#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[repr(u32)]
#[serde(into = "u32")]
pub(crate) enum SpriteKind {
    #[default]
    RoundedRect = 0,
    AlphaMask = 1,
    #[cfg(test)]
    Image = 2,
    TextWithForeground = 3,
    Progress = 4,
}

impl From<SpriteKind> for u32 {
    fn from(kind: SpriteKind) -> Self {
        kind as Self
    }
}

#[derive(Clone, Copy, Default, serde::Serialize)]
#[repr(C)]
pub(crate) struct SpriteGeometry {
    pub bounds: Rect,
    pub uv: Rect,
    pub clip: Rect,
    pub color: Color,
    pub secondary: Color,
    pub radius: f32,
    pub angle: f32,
    pub fade_top: f32,
    pub fade_bottom: f32,
    pub dimming: f32,
    pub kind: SpriteKind,
}

#[derive(Clone, Default)]
pub(crate) struct Sprite<'window> {
    pub texture: Option<Texture<'window>>,
    pub foreground: Option<Texture<'window>>,
    pub geometry: SpriteGeometry,
}

#[derive(Clone, Default)]
pub(crate) struct Scene<'window> {
    pub graphics: Vec<Graphic<'window>>,
    pub sprites: Vec<Sprite<'window>>,
}

#[repr(C)]
struct RawGraphic {
    artwork: *const NativeTexture,
    gradient: *const NativeTexture,
    noise: *const NativeTexture,
    geometry: GraphicGeometry,
}
#[repr(C)]
struct RawSprite {
    texture: *const NativeTexture,
    foreground: *const NativeTexture,
    geometry: SpriteGeometry,
}
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    update: Option<unsafe extern "C" fn(*mut c_void, i64, u32, u32, u32)>,
    input: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    painted: Option<unsafe extern "C" fn(*mut c_void, u64, u64, i64)>,
    render: Option<unsafe extern "C" fn(*mut c_void, i64, u32, u32, u32, f64)>,
}

unsafe extern "C" {
    fn rs_window_new(
        width: u32,
        height: u32,
        fullscreen: bool,
        callbacks: Callbacks,
    ) -> *mut NativeWindow;
    fn rs_window_callbacks(window: *mut NativeWindow, callbacks: Callbacks);
    fn rs_window_run(window: *mut NativeWindow) -> i32;
    fn rs_window_delete(window: *mut NativeWindow);
    fn rs_window_quit(window: *mut NativeWindow);
    fn rs_window_wake(window: *mut NativeWindow);
    fn rs_window_animate(window: *mut NativeWindow, active: bool);
    fn rs_window_render_animate(window: *mut NativeWindow, active: bool);
    fn rs_window_watch(window: *mut NativeWindow, fd: i32, event: i32);
    fn rs_window_size(window: *mut NativeWindow, width: *mut u32, height: *mut u32);
    fn rs_window_scale(window: *mut NativeWindow) -> f64;
    fn rs_window_icon(window: *mut NativeWindow, rgba: *const u8, width: u32, height: u32);
    fn rs_window_timer(window: *mut NativeWindow, milliseconds: u32, event: i32);
    #[cfg(test)]
    fn rs_window_resize(window: *mut NativeWindow, width: u32, height: u32);
    fn rs_window_scene(
        window: *mut NativeWindow,
        graphics: *const RawGraphic,
        graphic_count: usize,
        sprites: *const RawSprite,
        sprite_count: usize,
        direct: bool,
    ) -> u64;
    fn rs_clock_micros() -> i64;
    fn rs_texture_upload(
        window: *mut NativeWindow,
        width: u32,
        height: u32,
        format: u32,
        pixels: *const c_void,
    ) -> *mut NativeTexture;
    fn rs_texture_upload_start(
        window: *mut NativeWindow,
        width: u32,
        height: u32,
        format: u32,
        pixels: *const c_void,
    ) -> *mut NativeUpload;
    fn rs_texture_upload_finish(pending: *mut NativeUpload) -> *mut NativeTexture;
    fn rs_image_palette(image: *const NativeImage) -> *mut NativeImage;
    fn rs_image_channels(image: *const NativeImage) -> u32;
    fn rs_image_decode(path: *const c_char, width: *mut u32, height: *mut u32) -> *mut NativeImage;
    fn rs_image_stride(image: *const NativeImage) -> u32;
    fn rs_image_has_alpha(image: *const NativeImage) -> bool;
    fn rs_image_pixels(image: *const NativeImage) -> *const u8;
    fn rs_image_delete(image: *mut NativeImage);
    fn rs_texture_delete(texture: *mut NativeTexture);
    fn rs_window_error(window: *mut NativeWindow) -> *const c_char;
    #[cfg(test)]
    fn rs_window_capture(window: *mut NativeWindow, output: *mut u8, length: usize) -> bool;
    #[cfg(test)]
    fn rs_window_request_capture(window: *mut NativeWindow);
}

pub(crate) fn clock_micros() -> i64 {
    unsafe { rs_clock_micros() }
}

#[derive(Clone, Copy)]
pub(crate) struct PaintedFrame {
    pub scene: u64,
    pub frame: u64,
    pub time_micros: i64,
}

pub(crate) trait WindowEvents: Send {
    fn render(&mut self, _frame: &RenderFrame) {}
    fn update(&mut self, window: &Window, now: i64, viewport: Viewport, refresh_millihertz: u32);
    fn input(&mut self, window: &Window, event: i32);
    fn painted(&mut self, window: &Window, frame: PaintedFrame);
}

pub(crate) struct Window {
    native: NonNull<NativeWindow>,
    _gui_thread: PhantomData<Rc<()>>,
}

impl Window {
    pub fn new(viewport: Viewport, fullscreen: bool) -> Result<Self, String> {
        let native = NonNull::new(unsafe {
            rs_window_new(
                viewport.width_px,
                viewport.height_px,
                fullscreen,
                Callbacks::default(),
            )
        })
        .ok_or("Cannot initialize the Renderer window")?;
        Ok(Self {
            native,
            _gui_thread: PhantomData,
        })
    }

    pub fn run(&self, events: &mut impl WindowEvents) -> Result<(), String> {
        struct Context<'a> {
            window: &'a Window,
            events: Mutex<&'a mut dyn WindowEvents>,
            panic: Mutex<Option<Box<dyn std::any::Any + Send>>>,
            native: NonNull<NativeWindow>,
        }
        unsafe fn dispatch(
            pointer: *mut c_void,
            event: impl FnOnce(&mut dyn WindowEvents, &Window),
        ) {
            let context = unsafe { &*pointer.cast::<Context<'_>>() };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                event(
                    &mut **context.events.lock().unwrap_or_else(|e| e.into_inner()),
                    context.window,
                );
            }));
            if let Err(panic) = result {
                *context.panic.lock().unwrap_or_else(|e| e.into_inner()) = Some(panic);
                context.window.quit();
            }
        }
        unsafe extern "C" fn update(
            pointer: *mut c_void,
            now: i64,
            width: u32,
            height: u32,
            rate: u32,
        ) {
            if width == 0 || height == 0 {
                return;
            }
            unsafe {
                dispatch(pointer, |events, window| {
                    events.update(window, now, Viewport::new(width, height), rate)
                })
            }
        }
        unsafe extern "C" fn input(pointer: *mut c_void, event: i32) {
            unsafe { dispatch(pointer, |events, window| events.input(window, event)) }
        }
        unsafe extern "C" fn painted(
            pointer: *mut c_void,
            scene: u64,
            frame: u64,
            time_micros: i64,
        ) {
            unsafe {
                dispatch(pointer, |events, window| {
                    events.painted(
                        window,
                        PaintedFrame {
                            scene,
                            frame,
                            time_micros,
                        },
                    )
                })
            }
        }
        unsafe extern "C" fn render(
            pointer: *mut c_void,
            now: i64,
            width: u32,
            height: u32,
            rate: u32,
            scale: f64,
        ) {
            let context = unsafe { &*pointer.cast::<Context<'_>>() };
            let frame = RenderFrame {
                native: context.native,
                now,
                viewport: Viewport::new(width, height),
                rate,
                scale,
                _render_thread: PhantomData,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                context
                    .events
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .render(&frame);
            }));
            if let Err(panic) = result {
                *context.panic.lock().unwrap_or_else(|e| e.into_inner()) = Some(panic);
                frame.quit();
            }
        }
        let mut context = Context {
            window: self,
            events: Mutex::new(events),
            panic: Mutex::new(None),
            native: self.native,
        };
        let result = unsafe {
            rs_window_callbacks(
                self.native.as_ptr(),
                Callbacks {
                    context: (&mut context as *mut Context<'_>).cast(),
                    update: Some(update),
                    input: Some(input),
                    painted: Some(painted),
                    render: Some(render),
                },
            );
            let result = rs_window_run(self.native.as_ptr());
            rs_window_callbacks(self.native.as_ptr(), Callbacks::default());
            result
        };
        if let Some(panic) = context
            .panic
            .into_inner()
            .unwrap_or_else(|e| e.into_inner())
        {
            std::panic::resume_unwind(panic);
        }
        if result == 0 {
            Ok(())
        } else {
            Err(
                unsafe { CStr::from_ptr(rs_window_error(self.native.as_ptr())) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    pub fn quit(&self) {
        unsafe { rs_window_quit(self.native.as_ptr()) }
    }
    pub fn wake(&self) {
        unsafe { rs_window_wake(self.native.as_ptr()) }
    }
    pub fn animate(&self, active: bool) {
        unsafe { rs_window_animate(self.native.as_ptr(), active) }
    }
    pub fn watch(&self, fd: i32, event: i32) {
        unsafe { rs_window_watch(self.native.as_ptr(), fd, event) }
    }
    pub fn viewport(&self) -> Viewport {
        let (mut width, mut height) = (0, 0);
        unsafe {
            rs_window_size(self.native.as_ptr(), &mut width, &mut height);
        }
        Viewport::new(width, height)
    }
    pub fn scale(&self) -> f64 {
        unsafe { rs_window_scale(self.native.as_ptr()) }
    }
    pub fn icon(&self, path: &Path) -> Result<(), String> {
        let icon = gdk_pixbuf::Pixbuf::from_file_at_scale(path, 128, 128, true)
            .map_err(|e| e.to_string())?;
        let rgba = roonscape_renderer::pixbuf_rgba(&icon);
        unsafe {
            rs_window_icon(
                self.native.as_ptr(),
                rgba.as_ptr(),
                icon.width() as u32,
                icon.height() as u32,
            );
        }
        Ok(())
    }
    pub fn timer(&self, milliseconds: u32, event: i32) {
        unsafe { rs_window_timer(self.native.as_ptr(), milliseconds, event) }
    }
    #[cfg(test)]
    pub fn resize(&self, viewport: Viewport) {
        unsafe { rs_window_resize(self.native.as_ptr(), viewport.width_px, viewport.height_px) }
    }
    pub fn uploader(&self) -> Uploader<'_> {
        Uploader {
            native: self.native,
            _window: PhantomData,
        }
    }

    #[cfg(test)]
    pub fn submit(&self, scene: &Scene<'_>) -> u64 {
        submit_scene(self.native, scene, false)
    }

    #[cfg(test)]
    pub fn capture(&self, viewport: Viewport) -> Result<Vec<u8>, String> {
        let mut pixels = vec![0; viewport.width_px as usize * viewport.height_px as usize * 4];
        if unsafe { rs_window_capture(self.native.as_ptr(), pixels.as_mut_ptr(), pixels.len()) } {
            Ok(pixels)
        } else {
            Err("Cannot capture the Renderer window at the requested viewport".into())
        }
    }

    #[cfg(test)]
    pub fn request_capture(&self) {
        unsafe { rs_window_request_capture(self.native.as_ptr()) }
    }
}

/// Available only for the duration of Qt's render callback. It exposes immutable
/// frame properties, immediate scene adoption, and queued GUI requests.
pub(crate) struct RenderFrame {
    native: NonNull<NativeWindow>,
    pub now: i64,
    pub viewport: Viewport,
    pub rate: u32,
    pub scale: f64,
    _render_thread: PhantomData<Rc<()>>,
}
impl RenderFrame {
    pub fn submit(&self, scene: &Scene<'_>) -> u64 {
        submit_scene(self.native, scene, true)
    }
    pub fn animate(&self, active: bool) {
        unsafe { rs_window_render_animate(self.native.as_ptr(), active) };
    }
    pub fn quit(&self) {
        unsafe { rs_window_quit(self.native.as_ptr()) };
    }
}

fn submit_scene(native: NonNull<NativeWindow>, scene: &Scene<'_>, direct: bool) -> u64 {
    let pointer = |texture: &Option<Texture<'_>>| {
        texture
            .as_ref()
            .map_or(std::ptr::null(), |t| t.native.0.as_ptr())
    };
    let graphics: Vec<_> = scene
        .graphics
        .iter()
        .map(|g| RawGraphic {
            artwork: pointer(&g.artwork),
            gradient: pointer(&g.gradient),
            noise: pointer(&g.noise),
            geometry: g.geometry,
        })
        .collect();
    let sprites: Vec<_> = scene
        .sprites
        .iter()
        .map(|s| RawSprite {
            texture: pointer(&s.texture),
            foreground: pointer(&s.foreground),
            geometry: s.geometry,
        })
        .collect();
    unsafe {
        rs_window_scene(
            native.as_ptr(),
            graphics.as_ptr(),
            graphics.len(),
            sprites.as_ptr(),
            sprites.len(),
            direct,
        )
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { rs_window_delete(self.native.as_ptr()) }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Uploader<'window> {
    native: NonNull<NativeWindow>,
    _window: PhantomData<&'window Window>,
}

// Only the native upload queue is accessed here. Borrowing Window bounds the
// queue's lifetime, while the native implementation serializes graphics work.
unsafe impl Send for Uploader<'_> {}
unsafe impl Sync for Uploader<'_> {}

#[derive(Clone)]
pub(crate) struct Texture<'window> {
    native: Arc<TextureHandle>,
    _window: PhantomData<&'window Window>,
}
struct TextureHandle(NonNull<NativeTexture>, u64);
// Pixels are immutable after upload. Native scene ownership and retirement
// fences protect the allocation independently of these reference counts.
unsafe impl Send for TextureHandle {}
unsafe impl Sync for TextureHandle {}
unsafe impl Send for Texture<'_> {}
unsafe impl Sync for Texture<'_> {}

static NEXT_TEXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
impl Texture<'_> {
    pub fn identity(&self) -> u64 {
        self.native.1
    }
}

impl Drop for TextureHandle {
    fn drop(&mut self) {
        crate::content_evidence::record(
            || serde_json::json!({"event":"resource-released","resource":self.1}),
        );
        unsafe { rs_texture_delete(self.0.as_ptr()) }
    }
}

pub(crate) struct PendingTexture<'pixels, 'window> {
    native: Option<NonNull<NativeUpload>>,
    uploader: Uploader<'window>,
    _pixels: PhantomData<&'pixels DecodedImage>,
}
impl<'window> PendingTexture<'_, 'window> {
    pub fn finish(mut self) -> Result<Texture<'window>, String> {
        self.uploader
            .texture(unsafe { rs_texture_upload_finish(self.native.take().unwrap().as_ptr()) })
    }
}
impl Drop for PendingTexture<'_, '_> {
    fn drop(&mut self) {
        if let Some(native) = self.native.take() {
            let texture = unsafe { rs_texture_upload_finish(native.as_ptr()) };
            if !texture.is_null() {
                unsafe { rs_texture_delete(texture) };
            }
        }
    }
}

impl<'window> Uploader<'window> {
    fn texture(&self, pointer: *mut NativeTexture) -> Result<Texture<'window>, String> {
        let pointer = NonNull::new(pointer).ok_or("Cannot prepare a Renderer texture")?;
        let identity = NEXT_TEXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        crate::content_evidence::record(
            || serde_json::json!({"event":"resource-created","resource":identity}),
        );
        Ok(Texture {
            native: Arc::new(TextureHandle(pointer, identity)),
            _window: PhantomData,
        })
    }

    fn upload(
        &self,
        width: u32,
        height: u32,
        format: u32,
        pixels: *const c_void,
    ) -> Result<Texture<'window>, String> {
        self.texture(unsafe {
            rs_texture_upload(self.native.as_ptr(), width, height, format, pixels)
        })
    }

    pub fn start_image<'pixels>(
        &self,
        image: &'pixels DecodedImage,
    ) -> Result<PendingTexture<'pixels, 'window>, String> {
        let native = NonNull::new(unsafe {
            rs_texture_upload_start(
                self.native.as_ptr(),
                image.size.width_px,
                image.size.height_px,
                if image.channels == 4 { 0 } else { 5 },
                image.pixels().as_ptr().cast(),
            )
        })
        .ok_or("Cannot start artwork upload")?;
        Ok(PendingTexture {
            native: Some(native),
            uploader: *self,
            _pixels: PhantomData,
        })
    }

    #[cfg(test)]
    pub fn image(&self, image: &DecodedImage) -> Result<Texture<'window>, String> {
        self.upload(
            image.size.width_px,
            image.size.height_px,
            if image.channels == 4 { 0 } else { 5 },
            image.pixels().as_ptr().cast(),
        )
    }

    pub fn rgba(&self, width: u32, height: u32, pixels: &[u8]) -> Result<Texture<'window>, String> {
        assert_eq!(pixels.len(), width as usize * height as usize * 4);
        self.upload(width, height, 0, pixels.as_ptr().cast())
    }

    pub fn mask(&self, width: u32, height: u32, pixels: &[u8]) -> Result<Texture<'window>, String> {
        assert_eq!(pixels.len(), width as usize * height as usize);
        self.upload(width, height, 2, pixels.as_ptr().cast())
    }

    pub fn lookup(&self, colors: &[[i32; 4]]) -> Result<Texture<'window>, String> {
        assert_eq!(colors.len(), 256 * 257);
        self.upload(256, 257, 3, colors.as_ptr().cast())
    }

    pub fn noise(&self, noise: &[[i16; 3]]) -> Result<Texture<'window>, String> {
        assert_eq!(noise.len(), 128 * 128);
        self.upload(128, 128, 4, noise.as_ptr().cast())
    }
}
