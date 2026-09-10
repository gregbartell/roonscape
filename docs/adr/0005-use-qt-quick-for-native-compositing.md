# Use Qt Quick for native compositing

The Renderer uses Qt Quick/OpenGL for windowing and layered compositing, with
Rust presentation logic and Pango text shaping. Text and artwork textures are
prepared off the presentation loop and uploaded through a shared graphics
context so animation continues during content preparation. This design accepts
a C++ boundary and Qt runtime dependencies; GTK supplies the platform animation
preference.
