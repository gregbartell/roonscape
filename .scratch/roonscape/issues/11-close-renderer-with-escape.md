# 11 — Close the renderer with Escape

**What to build:** Let a person previewing or operating RoonScape close the
renderer cleanly by pressing Escape while its window has focus. The shortcut
works consistently in windowed and fullscreen presentations, including both
fixture and live Roon operation.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Pressing Escape while the renderer has focus closes the renderer in
      windowed mode.
- [x] Pressing Escape while the renderer has focus closes the renderer in
      fullscreen mode.
- [x] The shortcut behaves the same with fixture snapshots and live Roon
      snapshots and does not introduce Roon Control.
- [x] Other keys do not close the renderer, and the existing window-close
      behavior remains unchanged.
- [x] Closing a fixture preview with Escape lets its launcher stop both
      processes and remove the temporary runtime directory.
- [x] Automated checks cover Escape handling and protection against unrelated
      key presses without depending on toolkit implementation details.
