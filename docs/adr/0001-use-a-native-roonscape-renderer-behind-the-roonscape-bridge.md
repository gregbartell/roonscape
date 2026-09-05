# Use a native RoonScape Renderer behind the RoonScape Bridge

The RoonScape Bridge integrates with Roon through Roon's supported JavaScript
interface, while a separate native RoonScape Renderer presents playback; both
ship as one application. The RoonScape Bridge is distinct from Roon Bridge,
Roon's separate application. This adds a process seam and shared contract, but
avoids embedding a browser engine, reimplementing Roon's private protocol, or
coupling rendering to the integration runtime.
