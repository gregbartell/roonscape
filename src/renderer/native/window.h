#pragma once
#include <stddef.h>
#include <stdint.h>

// The window owns Qt and the render loop. Only render runs on the render thread;
// all other callbacks execute on the GUI thread.
// Texture preparation is blocking and must run on a preparation thread; a
// returned texture is immutable and usable from every scene until released.
// Scene submission copies descriptions and retains every referenced texture.
extern "C" {
struct RsWindow;
struct RsTexture;
struct RsImage;
struct RsUpload;
struct RsRect { float x, y, width, height; };
struct RsColor { float red, green, blue, alpha; };
struct RsGraphic {
    const RsTexture *artwork;
    const RsTexture *gradient;
    const RsTexture *noise;
    RsRect canvas;
    RsRect artwork_bounds;
    RsRect plate_bounds;
    RsColor background, border, plate, quiet, muted;
    uint32_t origin, step_x, step_y;
    float border_width, shadow_radius, shadow_y, shadow_alpha, weight;
};
struct RsSprite {
    const RsTexture *texture;
    const RsTexture *foreground;
    RsRect bounds, uv, clip;
    RsColor color;
    RsColor secondary;
    float radius, angle, fade_top, fade_top_origin, fade_bottom, dimming;
    // 0: rectangle; 1: mask; 2: RGBA; 3: colored glyph base + tintable foreground.
    uint32_t kind;
};
struct RsCallbacks {
    void *context;
    void (*update)(void *, int64_t, uint32_t, uint32_t, uint32_t);
    void (*input)(void *, int32_t);
    void (*painted)(void *, uint64_t, uint64_t, int64_t);
    void (*render)(void *, int64_t, uint32_t, uint32_t, uint32_t, double);
};
RsWindow *rs_window_new(uint32_t width, uint32_t height, bool fullscreen, RsCallbacks callbacks);
void rs_window_callbacks(RsWindow *, RsCallbacks);
int rs_window_run(RsWindow *);
void rs_window_delete(RsWindow *);
void rs_window_quit(RsWindow *);
void rs_window_wake(RsWindow *);
void rs_window_animate(RsWindow *, bool);
void rs_window_render_animate(RsWindow *, bool);
void rs_window_watch(RsWindow *, int fd, int32_t event);
double rs_window_scale(RsWindow *);
void rs_window_size(RsWindow *, uint32_t *, uint32_t *);
void rs_window_icon(RsWindow *, const uint8_t *, uint32_t, uint32_t);
void rs_window_timer(RsWindow *, uint32_t, int32_t);
void rs_window_resize(RsWindow *, uint32_t, uint32_t);
uint64_t rs_window_scene(RsWindow *, const RsGraphic *, size_t, const RsSprite *, size_t, bool direct);
int64_t rs_clock_micros();
// Read-only identities for associating external presentation observations.
// Nonzero only on the thread that drew this frame, until its next frame begins.
uint64_t roonscape_submission_frame();
uint64_t roonscape_submission_scene();
uint64_t roonscape_frame_serial();
int64_t roonscape_frame_time_micros();
// Formats: 0 RGBA8, 1 BGRA8, 2 R8, 3 RGBA32I, 4 RGB16I, 5 RGB8.
// RGB8 rows are padded to four bytes; all other formats are tightly packed.
// Pixel storage must remain alive until upload_finish, including cancellation.
RsUpload *rs_texture_upload_start(RsWindow *, uint32_t, uint32_t, uint32_t, const void *);
RsTexture *rs_texture_upload_finish(RsUpload *);
RsTexture *rs_texture_upload(RsWindow *, uint32_t, uint32_t, uint32_t, const void *);
RsImage *rs_image_decode(const char *, uint32_t *, uint32_t *);
RsImage *rs_image_palette(const RsImage *);
uint32_t rs_image_channels(const RsImage *);
uint32_t rs_image_stride(const RsImage *);
bool rs_image_has_alpha(const RsImage *);
const uint8_t *rs_image_pixels(const RsImage *);
void rs_image_delete(RsImage *);
RsTexture *rs_texture_clone(const RsTexture *);
void rs_texture_delete(RsTexture *);
const char *rs_window_error(RsWindow *);
bool rs_window_capture(RsWindow *, uint8_t *, size_t);
void rs_window_request_capture(RsWindow *);
}
