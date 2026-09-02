# Use directed SOOD for an explicit Roon Server

RoonScape accepts an optional, invocation-only Roon Server Host for networks
where ordinary Roon discovery cannot reach an otherwise accessible server. It
sends that host a directed SOOD query to learn the dynamic API port, then
connects through the supported Roon JavaScript interface. The selection is
authoritative: RoonScape retries until discovery succeeds or is cancelled and
does not fall back to ordinary discovery, search other hosts, or add new
viewer-facing behavior. Discovery does not restart after the resulting
connection is lost.

The pinned Roon SDK does not expose directed discovery, so RoonScape owns the
smallest necessary SOOD codec behind a narrow module instead of maintaining an
SDK fork. This discovery-only exception qualifies ADR-0001's broader decision
not to reimplement Roon's private protocol; all subsequent Roon integration
continues through the supported SDK interface.
