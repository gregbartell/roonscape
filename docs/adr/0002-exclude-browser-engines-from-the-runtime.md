# Exclude browser engines from the runtime

The RoonScape runtime will not embed Chromium, Electron, WebKit, or another
browser engine. A native display requires more new implementation work, but it
fits the product's lightweight, always-on role and avoids retaining the
resource-heavy browser process that motivated this replacement.
