# Build a greenfield read-only RoonScape

RoonScape will be built as a clean implementation in a new repository. The
new product will not preserve controller interfaces or any ability to mutate
Roon state; only the accepted design documents and relevant operational
findings will migrate. This local legacy repository can then be deleted, with
the upstream Web Controller repository remaining available as its historical
reference. Official Roon clients already provide control, while a greenfield
boundary avoids carrying forward obsolete browser code, dependencies, global
state, and defects.

RoonScape will register under a new extension identity with separate
authorization state, accepting a one-time enable step instead of inheriting
the legacy Web Controller identity. Its core will avoid `roll`-specific paths
and names, while `roll` remains the only initially supported and tested
deployment.
