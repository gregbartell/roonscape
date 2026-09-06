# Use layered compositing for Now Playing Transitions

Now Playing Transitions must reveal incoming content together while timing,
status, and lyric motion remain live, and interrupted transitions must continue
from their visible appearance. Use explicit layered compositing with a shared
transition clock rather than the Renderer's existing cached whole-screen GTK
crossfade. This accepts additional rendering and lifecycle complexity to support
coordinated reveals, partial content updates, and continuous retargeting.
