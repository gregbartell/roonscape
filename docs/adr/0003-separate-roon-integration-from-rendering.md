# Separate Roon integration from rendering

The runtime will use a small Node.js bridge for Roon's supported JavaScript
API and a separate native renderer process. The extra process boundary costs
some memory and operational complexity, but it isolates Roon connectivity from
presentation, keeps browser engines out of the runtime, and avoids depending
on immature native UI bindings for Node.js or reimplementing Roon's private
protocol.

The native renderer will use Rust with GTK 4 and Pango. The processes will be
supervised independently and exchange complete, versioned state snapshots over
a private Unix-domain socket; artwork will move through bounded, atomically
replaced files rather than the state payload. Either process must tolerate the
other restarting without terminating its own session.

Both modules will live in one repository and one coordinated release. The
bridge will use TypeScript and the renderer Rust; a language-neutral JSON
Schema and shared fixtures will define their contract instead of either module
importing the other's framework types.
