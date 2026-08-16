# Use a native renderer behind the Roon bridge

RoonScape integrates with Roon through a Node.js process using Roon's
supported JavaScript interface and presents playback through a separate native
renderer; both ship as one application. This adds a process seam and shared
contract, but avoids embedding a browser engine, reimplementing Roon's private
protocol, or coupling rendering to the integration runtime.
