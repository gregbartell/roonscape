# Implement Reel Lift for synchronized lyrics

## Problem Statement

During a Natural Cue Handoff, the current native RoonScape Renderer fades the
complete lyric reel away, replaces all three cue roles, and fades the reel back
in. This creates an intermediate state with no prominent current lyric. At room
distance, the missing focal cue reads as lost or unavailable lyrics rather than
as intentional motion.

The current transition into and out of the Synchronized Lyric Composition also
replaces a complete rendered presentation. Stable artwork, Presentation Status,
progress or activity, and Output/Zone identity can be duplicated across the
outgoing and incoming layers, while captured entry behavior has appeared as an
abrupt layout cut. Neither behavior gives lyric playback a continuous visual
center.

The selected design direction is **Reel Lift**, option B from the local lyric
transition prototype. Reel Lift treats the lyric roles as a spatial sequence:
the Next Cue approaches from anticipation, becomes the room-scale focal cue,
and—when its rendered height permits—moves into the Previous Cue role as the
following cue takes focus. The implementation must preserve that idea without
copying prototype-only assumptions into the native GTK/Pango Renderer.

## Solution

Implement Reel Lift entirely within the native RoonScape Renderer. Preserve the
RoonScape Bridge contract and the Lyric Feed's bounded normalized timeline.

For a Natural Cue Handoff between compact nonblank cues, promote the incoming
Next Cue into the focal position while demoting the outgoing focal cue into the
Previous Cue position. Transfer position, scale, and artwork-derived semantic
color roles continuously so that one readable lyric owns focus throughout the
handoff. When either cue occupies three or four rendered lines, use an
abbreviated height-aware lift that removes the outgoing cue without compressing
it through the compact Previous Cue position and brings the incoming cue to
focus on a shorter path.

For the same Now Playing identity, implement entry into and exit from the
Synchronized Lyric Composition as an in-place Lyric Composition Transition. Keep
artwork, Presentation Status, footer timing or activity, and Output/Zone
identity as persistent rendered objects. Animate artwork geometry, information
rail geometry, and ownership between ordinary Title/Artist/Album copy and the
compact lyric masthead plus lyric reel. Genuine Now Playing identity changes
continue to use the existing complete composition replacement.

Treat a continuously available timeline containing nonblank lyrics as one
continuous composition interval, recomputed when the timeline is revised. Ignore
leading blanks for presentation; a timeline containing only blanks does not
enter the lyric composition. When the timeline is known early enough, begin
preparing the composition before its first nonblank cue becomes focal, allowing
focal arrival to overlap artwork and information-rail movement; exit after its
final entry, including trailing blanks. Late availability and external seeks may
introduce the destination cue during composition entry without an artificial
waiting period. Never exit merely because the track contains a long internal gap
or an Intentional Blank. A settled internal or trailing blank leaves the focal
position empty while retaining available Previous Cue and Next Cue context.
Short blanks preserve normal advance promotion even when no perceptibly settled
empty focal interval remains. Do not force a visible pause at the expense of
the song's visual flow.
Exact lead, hold, duration, and easing values are left to the implementing
agent, subject to the semantic and perceptual requirements in this
specification.

## User Stories

1. As a listener viewing RoonScape from across a room, I want one prominent
   lyric to remain readable during every Natural Cue Handoff, so that I never
   mistake ordinary progression for missing lyrics.
2. As a listener following a short lyric cue, I want the anticipated next cue to
   rise into the established focal position, so that progression is easy to
   follow without moving my attention around the display.
3. As a listener following a short lyric cue, I want the outgoing focal cue to
   settle into the Previous Cue role, so that the movement communicates the
   relationship between what was sung and what is current.
4. As a listener, I want the incoming cue to change from the brighter
   anticipation role to the focal text role during its lift, so that visual
   emphasis and spatial movement tell the same story.
5. As a listener, I want the outgoing cue to change from focal text to the more
   muted Previous Cue role, so that old lyrics remain contextual rather than
   competing with the current line.
6. As a listener, I want Previous Cue and Next Cue to retain their distinct
   artwork-derived hierarchy, so that past and upcoming context are not visually
   interchangeable.
7. As a listener, I want a three- or four-line incoming cue to remain large and
   readable, so that retaining contextual neighbors never reduces the focal
   tier.
8. As a listener, I want a three- or four-line outgoing cue to leave cleanly
   instead of being squeezed through a small memory position, so that tall
   lyrics do not form a dense or illegible block.
9. As a listener, I want the lyric anchor to remain perceptually stable as cue
   height changes, so that one-, two-, three-, and four-line lyrics all feel
   like the same composition.
10. As a listener, I want animated lyrics to remain clipped to the lyric reel,
    so that moving text never enters the masthead or footer.
11. As a listener, I want a settled Intentional Blank to empty the focal position
    while retaining the Synchronized Lyric Composition, so that a deliberate
    musical pause reads as breathing room rather than a layout change.
12. As a listener, I want available Previous Cue and Next Cue to remain
    contextual during a settled Intentional Blank, with neither highlighted, so
    that I can follow the surrounding lyrics without implying that they are sung.
13. As a listener, I want adjacent blank timeline entries to form one
    uninterrupted blank interval, so that invisible cue indices do not cause
    repeated or decorative motion.
14. As a listener, I want leading blanks to behave as though they were absent,
    so that ordinary Now Playing remains until preparation for the first
    nonblank cue. A timeline containing only blanks retains ordinary Now
    Playing.
15. As a listener, I want a long timestamp gap without an explicit blank to
    retain the current cue, so that RoonScape does not invent lyric timing that
    the Lyric Feed did not provide.
16. As a listener, I want composition entry to begin ahead of the first nonblank
    cue when possible and overlap its arrival as needed, so that preparation
    never requires an empty-looking entry or delays the first focal lyric.
17. As a listener, I want the Synchronized Lyric Composition to remain active
    across internal gaps and trailing blanks, so that instrumental gaps do not
    repeatedly shrink and enlarge the artwork or replace the metadata layout.
18. As a listener, I want the Synchronized Lyric Composition to exit once after
    the final timeline entry, so that ordinary Now Playing returns as one
    deliberate composition change.
19. As a listener, I want late lyric availability for the same Now Playing
    identity to use the normal in-place entry choreography, so that a delayed
    Lyric Feed does not cause an abrupt or duplicated presentation.
20. As a listener, I want loss of lyric availability for the same Now Playing
    identity to use the normal in-place exit choreography, so that ordinary Now
    Playing returns coherently.
21. As a listener, I want stable artwork, Presentation Status, progress or
    activity, and Output/Zone identity to remain singular during lyric entry and
    exit while artwork resizes and the information rail moves, so that unchanged
    content stays crisp without duplication or replacement flashes.
22. As a listener, I want ordinary Title/Artist/Album copy and the compact lyric
    masthead to exchange ownership without visibly duplicating the same Title
    and Artist, so that the composition remains calm during entry and exit.
23. As a listener, I want an external seek within the lyric interval to install
    the destination cue immediately, so that skipped cues are never replayed as
    animation.
24. As a listener, I want the Previous Cue after a seek to be the nearest
    nonblank predecessor in the destination timeline, so that seeking and
    playing naturally to the same position produce the same lyric state.
25. As a listener, I want a seek across the ordinary/lyric boundary to animate
    the relevant Lyric Composition Transition while installing destination cue
    content directly, so that the layout change remains coherent without
    pretending skipped cues were sung.
26. As a listener, I want a corrected or retimed lyric timeline to install its
    current destination state directly, so that a data revision does not
    impersonate natural sung progression.
27. As a listener, I want two adjacent cue entries containing identical text to
    perform Reel Lift, so that separately sung repetitions remain visibly
    distinct timeline events.
28. As a listener, I want a new cue arriving during an active handoff to take
    priority immediately, so that the display never queues stale lyrics behind
    playback.
29. As a listener, I want an interrupted handoff to preserve smooth continuity
    when safe and otherwise choose the newest stable endpoint, so that visual
    polish never outranks semantic correctness.
30. As a listener, I want an interrupted lyric entry or exit to reverse or
    retarget from its current geometry when the Now Playing identity is
    unchanged, so that asynchronous Lyric Feed availability does not create a
    second abrupt layout change.
31. As a listener, I want an active Reel Lift to finish when playback pauses and
    then remain frozen, so that the paused presentation never stops in a
    half-promoted state.
32. As a listener using reduced animation, I want every lyric update and
    composition change to install a complete semantic endpoint immediately, so
    that the hierarchy remains useful without motion.
33. As a listener, I want Reel Lift to use crisp native text throughout, so that
    animated scale or weight changes do not make room-scale lyrics soft or
    unstable.
34. As a listener on any supported landscape display, I want the same cue roles
    and focal continuity, so that Reel Lift works consistently from the minimum
    supported viewport through the peer 4K viewports.
35. As a listener, I want missing artwork or optional metadata to retain their
    established fallback behavior, so that Reel Lift does not introduce new
    placeholders or layout exceptions.
36. As a listener, I want a genuine Now Playing change to retain the established
    complete composition replacement, so that Reel Lift does not visually
    connect unrelated tracks or artwork.
37. As a maintainer, I want natural progression, Intentional Blanks, external
    seeks, revised timelines, and composition entry/exit to be represented as
    distinct transition causes, so that future motion changes cannot silently
    merge their semantics.
38. As a maintainer, I want deterministic coverage for interruptions and reduced
    animation, so that asynchronous callbacks cannot overwrite a newer cue or
    leave the presentation between states.
39. As a designer reviewing the native Renderer, I want synchronized full-rate
    evidence for short, tall, blank, seek, entry, exit, and interrupted cases,
    so that focal continuity is judged in motion rather than from endpoints
    alone.
40. As an implementing agent, I want prototype timing values documented as
    references rather than requirements, so that I can calibrate native GTK and
    Pango motion without changing the agreed semantic ordering.
41. As a listener, I want short Intentional Blanks to preserve normal advance
    promotion without a forced visible pause, so that abrupt or accelerated
    movement does not break the flow of the song.
42. As a listener entering the lyric composition directly into a blank, I want
    artwork to carry visual dominance while the focal position stays empty,
    so that neither a highlighted neighbor nor lingering ordinary metadata
    falsely fills the pause.
43. As a listener, I want the lyric composition to remain through even a long
    instrumental blank, so that a duration threshold does not trigger an extra
    artwork resize or metadata replacement.
44. As a designer, I want native calibration to retain the selected Reel Lift
    character, so that implementation adjustments do not silently replace the
    chosen motion with a cut or dissolve.

## Implementation Decisions

### Ownership and architectural boundary

- The RoonScape Renderer owns cue selection consequences, transition
  classification, layout, animation, interruption, and reduced-animation
  behavior.
- Do not change the RoonScape Bridge contract, move presentation policy into the
  Bridge, or expose visual transition state through the Presentation Snapshot.
- Reel Lift is native GTK/Pango behavior. Do not embed or port the browser
  prototype runtime.
- The current Now Playing identity remains the boundary between in-place lyric
  behavior and complete composition replacement. Lyric availability changing for
  the same identity uses persistent in-place objects. A changed Title, Artist,
  Album, artwork identity, or other established composition identity input
  continues through the complete replacement path.
- A same-identity in-place Lyric Composition Transition must preserve one
  rendered artwork object, one Presentation Status, one footer, and one
  Output/Zone identity. The internal widget structure may change as necessary,
  but stable roles must not be recreated as simultaneous outgoing and incoming
  layers.

### Transition classification

The Renderer must distinguish these externally meaningful cases before choosing
motion:

| Case                                                                     | Cue behavior                                                                 | Composition behavior                                                                                 |
| ------------------------------------------------------------------------ | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Local playback advances to the immediately adjacent nonblank cue         | Reel Lift                                                                    | Remains in place                                                                                     |
| Local playback advances into an Intentional Blank after nonblank lyrics  | Contextual blank endpoint when time permits; preserve advance promotion through short blanks | Remains in place                                                                                     |
| Local playback leaves an Intentional Blank                               | Dedicated blank transition to the selected nonblank cue                      | Remains in place                                                                                     |
| Local playback advances by more than one cue before rendering catches up | Retarget directly to the newest selected cue; do not visit skipped cues      | Remains in place                                                                                     |
| External seek within the lyric interval                                  | Install destination cue and neighbors immediately                            | Remains in place                                                                                     |
| External seek across an ordinary/lyric boundary                          | Install destination cue state without cue travel                             | Animate the in-place lyric entry or exit                                                             |
| Same-identity timeline text or timing revision                           | Install the corrected destination cue state immediately                      | Animate in-place entry/exit if revised interval membership changes; otherwise retain the composition |
| Same-identity lyric timeline becomes available or unavailable            | Install the destination lyric state                                          | Animate in-place entry/exit only if interval membership changes                                      |
| Genuine Now Playing identity changes                                     | Destination state belongs to the new identity                                | Use the established complete composition replacement                                                 |
| Reduced animation or deterministic Presentation Capture                  | Install the complete semantic endpoint immediately                           | Install the complete semantic endpoint immediately                                                   |

- A Natural Cue Handoff is determined by adjacent timeline identity and local
  playback progression, not text equality. Identical adjacent strings still
  lift.
- A changed Presentation Snapshot revision must not by itself be interpreted as
  natural progression. External seeks and revised timelines are direct
  destination updates.
- If local progression skips one or more indices because of a delayed frame or
  exceptionally short cues, do not animate through the skipped cue sequence.
  Apply the newest-cue interruption policy once.

### Synchronized Lyric Composition lifecycle

- A continuously available lyric timeline containing at least one nonblank cue
  defines one composition interval from its first nonblank cue and final entry,
  with the selected entry lead and post-final hold. Recompute that interval
  after a timeline revision. If the current playback position changes membership
  in the interval, perform the corresponding in-place composition entry or exit
  even though the timeline remains available. Composition membership must not
  depend on the history of timeline revisions.
- Do not infer additional entry or exit boundaries from the time between cues.
  Long instrumental passages and Intentional Blanks remain inside the same
  composition interval, including a 90-second internal blank. Accept the smaller
  artwork during these passages; do not add duration-based composition exits.
- Ignore all consecutive leading blanks for presentation and composition entry.
  Present exactly as if those entries were absent, preparing for the first
  nonblank cue. A timeline containing only blanks defines no lyric composition
  interval and retains ordinary Now Playing. These are Renderer selection rules;
  do not discard entries from the Lyric Feed or its contract.
- Consecutive blank entries do not retrigger blank motion. They extend one
  interval whose settled endpoint has an empty focal position and available
  contextual neighbors, subject to normal advance promotion of the next cue.
- A final blank remains the final entry for lifecycle purposes. Exit only after
  the selected post-final hold policy. During a trailing blank, show Previous
  Cue context and an empty focal position, with no Next Cue. Consecutive
  trailing blanks extend this interval through the hold after the final blank
  timestamp.
- The implementing agent chooses the exact entry lead and post-final hold. When
  the timeline is known early enough, begin preparation before its first
  nonblank cue becomes focal. Focal arrival may overlap artwork and
  information-rail movement; settled geometry is never a prerequisite for it.
  Do not require a visibly empty preparatory composition. When advance
  preparation is impossible, including a first cue at track time zero,
  introduce the destination cue during entry
  without waiting for artwork and masthead motion to finish. Natural exit begins
  after the final timeline entry; seeks, revisions, and availability changes may
  cross a composition boundary independently of natural playback.
- If the timeline becomes unavailable and later returns for the same Now Playing
  identity, each availability change may create an exit or entry even in the
  middle of the track. This is a new availability interval, not an inferred
  boundary caused by musical silence.

### Reel Lift geometry and hierarchy

- Derive transition geometry from the native laid-out lyric roles and actual
  Pango rendered line counts. Prototype-relative coordinates are reference
  evidence, not production constants.
- When both outgoing and incoming cues occupy at most two rendered lines, use
  literal promotion:
  - the outgoing focal cue begins at the current anchor and ends at the Previous
    Cue geometry;
  - the incoming cue begins at the Next Cue geometry and ends at the current
    anchor;
  - after settlement, the following cue occupies the Next Cue role when one is
    available.
- When either outgoing or incoming cue occupies three or four rendered lines,
  use the abbreviated height-aware lift:
  - the outgoing cue exits upward without being compressed into the Previous Cue
    role;
  - the incoming cue starts from the anticipation side on a shorter path and
    becomes focal;
  - settled neighbor visibility is based on the incoming cue's rendered height.
- After a tall outgoing cue departs toward a short incoming cue, gently reveal
  its compact contextual rendering at the Previous Cue position as the incoming
  cue settles. Do not compress the departing focal rendering into that position.
  The settled Previous Cue remains the nearest nonblank predecessor, matching
  the state obtained by seeking to the same destination.
- Preserve the existing settled hierarchy: cues of one or two rendered lines may
  show Previous Cue and Next Cue; a three-line focal cue omits Previous Cue; a
  four-line focal cue omits both neighbors. Never shrink the focal typography
  merely to retain context.
- Preserve the four-line defensive cap and end ellipsis.
- Keep the perceptual current-line anchor stable. Height-aware motion may adapt
  its path, but the settled focal position must not drift by cue height.
- Clip all traveling lyric content to the established lyric reel region. It must
  not overlap the compact masthead, Presentation Status, or footer.
- Interpolate the incoming cue from the actual Next Cue semantic color toward
  the focal color. Interpolate the outgoing cue from the focal color toward the
  actual Previous Cue semantic color. Preserve the distinct previous/next
  hierarchy supplied by the artwork-derived palette.
- Position, scale, semantic color ownership, and uninterrupted focal continuity
  are required parts of a Natural Cue Handoff. Opacity may support the handoff,
  but there must never be an all-transparent or all-subordinate focal interval.
- The focal and contextual weight endpoints remain required. Continuous weight
  interpolation is optional: switch weight discretely near ownership transfer if
  native interpolation causes Pango relayout, jitter, or softened text.
- Prefer native text clarity over a mathematically exact reproduction of the
  prototype transform. Do not rasterize lyric text merely to simulate smooth
  weight interpolation.

### Composition entry and exit choreography

- On lyric entry for the same Now Playing identity:
  - move the persistent artwork from ordinary to lyric geometry;
  - move the persistent information rail to its lyric geometry;
  - retire ordinary Title/Artist/Album copy while introducing the compact
    Title/Artist masthead and destination lyric state;
  - keep Presentation Status and the complete footer, including Output/Zone
    identity, singular and continuously visible as they move horizontally with
    the information rail; persistence does not mean fixed screen coordinates;
  - ensure ordinary copy has substantially relinquished ownership before the
    compact masthead becomes fully visible, avoiding duplicate Title/Artist
    emphasis;
  - maintain at least one dominant content group throughout the transition.
- Allow the first focal lyric to arrive while artwork and information-rail
  geometry are still moving, with ordinary-copy departure and lyric arrival
  overlapping as needed to preserve visual dominance.
- Exit reverses the same ownership and geometry relationship, returning to
  ordinary artwork and Title/Artist/Album without duplicating stable objects.
- An external seek across a composition boundary uses this choreography. Its
  destination cue and neighbor contents are installed directly; they do not
  traverse skipped cue positions.
- Same-identity late Lyric Feed arrival and loss use this choreography rather
  than complete-layer replacement or an immediate cut.
- On late availability or an external seek into the lyric interval, introduce
  the destination lyric state during entry, including an empty focal position
  with contextual neighbors when the destination is a settled blank. Do not
  defer it until composition geometry settles or replay a preparatory interval
  that playback has already passed.
- When entry lands in a settled blank, persistent artwork may provide the
  dominant content group. Present contextual neighbors around the empty focal
  position without highlighting either neighbor or retaining ordinary metadata
  after entry merely to fill the space.
- If entry and exit interrupt one another, reverse or retarget from the current
  rendered geometry. Do not jump back to the prior endpoint before beginning the
  opposite transition.
- If a genuine Now Playing identity replacement arrives during entry or exit,
  cancel the in-place motion and let the established complete replacement path
  present the newest identity. Do not visually connect unrelated content.

### Intentional Blank behavior

- After the first nonblank cue, a blank cue is a real timeline state but not a
  composition boundary. Leading blanks are ignored as specified above.
- The settled blank endpoint has an empty focal position. Retire focal emphasis
  toward that endpoint when normal advance promotion leaves time to settle;
  short blanks need not visibly reach it. Retain the nearest nonblank predecessor
  as Previous Cue and the nearest nonblank successor as Next Cue when available.
  Both use their
  established contextual typography and distinct artwork-derived semantic
  colors; neither is highlighted in the settled blank state.
- Reserve empty space at the established focal anchor, without placeholder text
  or a drawn line. Do not collapse the neighbors into that space. Blank neighbor
  visibility must not inherit suppression from a preceding tall cue.
- On exit from a blank, promote the upcoming nonblank cue toward focus using the
  usual advance timing so it is fully focal around its sung timestamp. Motion
  may begin before that timestamp: the blank interval need not remain visually
  settled until the next sung timestamp. Do not replay intervening blank indices
  or promote the Previous Cue as though it were being sung again.
- Preserve normal advance promotion for short blanks even when it consumes the
  entire perceptible pause. Promotion may already be underway when the blank
  starts. Do not force a cut, abbreviated or accelerated movement, minimum
  empty dwell, or extended blank solely to make each explicit blank visible.
  Keep the blank's timeline identity and timestamps unchanged. Longer blanks
  still settle with contextual neighbors around an empty focal position.
- Consecutive blanks retain one continuous contextual presentation without
  repeated motion. A trailing blank has Previous Cue but no Next Cue and remains
  within the composition until the post-final exit boundary.
- Do not infer a blank from a long timestamp gap. In the absence of an explicit
  blank entry, retain the selected current cue until the timeline advances.
- The implementing agent may choose blank-transition timing and easing. Settled
  blanks must remain visibly distinct from nonblank focal states and composition
  exit; a short blank need not create a separately perceptible visual event.
  The uninterrupted focal-emphasis requirement applies to Natural Cue Handoffs,
  not to settled Intentional Blanks.

### External seek and timeline revision behavior

- At a seek destination inside the lyric interval, compute the same complete
  lyric state that uninterrupted playback would produce at that position.
- Previous Cue is the nearest nonblank predecessor in the destination timeline;
  it is not the last cue actually displayed before the seek. Next Cue follows
  the same destination-relative selection and settled visibility rules.
- Do not animate cue content through skipped indices, even when the seek lands
  exactly one index ahead.
- A seek crossing into or out of the lyric interval still performs the in-place
  Lyric Composition Transition selected above.
- If a timeline revision changes current text, timing, or selected index for the
  same Now Playing identity, install the corrected destination cue state
  directly without Reel Lift. Recompute the composition interval from the
  revised timeline: crossing a revised boundary performs normal in-place entry
  or exit; remaining inside the interval preserves the lyric composition.
  Timeline availability changing from absent to present or present to absent
  remains subject to normal composition entry/exit and interval selection.

### Interruption, pause, and reduced animation

- Store enough current visual state to retarget an active Natural Cue Handoff
  toward the newest selected cue without first restoring an obsolete endpoint.
- Prefer a smooth retarget from current geometry. If the current state cannot
  produce a valid, legible trajectory, cancel motion and install the newest
  stable endpoint immediately.
- Never queue handoffs. Any delayed work must be generation-safe or otherwise
  unable to overwrite a newer cue, seek, timeline revision, or Now Playing
  identity.
- If playback pauses during a Reel Lift, allow the active lift to settle on its
  already selected incoming cue, then keep that lyric state frozen until
  playback progression selects another state.
- When system animation is disabled, or when deterministic static Fixture Mode
  is active, bypass intermediate motion and install the final hierarchy,
  visibility, geometry, and text roles immediately.

### Timing and calibration

- Exact motion values are intentionally delegated to the implementing agent.
  Calibrate them against native GTK/Pango rendering and the full-rate visual
  evidence.
- The existing and prototype values are starting references only: approximately
  700 ms cue lookahead, 620 ms Natural Cue Handoff, 580 ms lyric composition
  entry/exit, 440 ms blank transition, and a three-second final hold.
- The qualitative ordering is normative:
  - a naturally incoming cue should be fully focal by approximately its sung
    timestamp;
  - begin preparation early when possible, allowing the first focal cue to
    arrive during geometry movement; introduce destination content without an
    artificial waiting period even when advance preparation is impossible;
  - natural lyric composition exit begins after the final timeline entry;
  - Natural Cue Handoffs must not create a focal opacity valley or an
    intermediate state mistaken for an Intentional Blank; settled blank states
    have no focal emphasis, while short blanks preserve normal advance
    promotion without a mandatory visible pause;
  - no timing choice may create stale queued motion.
- Easing is also a native calibration choice. It must support the selected
  musical but restrained Reel Lift character and the legibility requirements
  above.
- Native duration and discrete-weight adjustments are acceptable only while
  compact-cue promotion, upward departure, readable focal continuity, and
  coordinated composition movement remain recognizable as the selected Reel
  Lift. Replacing that motion with a cut or dissolve requires renewed design
  agreement, except for the defined interruption and reduced-animation or
  deterministic-static endpoint fallbacks.

## Testing Decisions

### Test seams

- Use the native Renderer presentation-update boundary as the primary automated
  seam. Drive complete presentation states and transition causes through the
  view, then assert viewer-observable lyric roles, composition ownership, and
  newest-state behavior. Avoid tests coupled only to CSS class names, callback
  counts, or a particular widget decomposition.
- Use the existing pure presentation-selection seam only for lifecycle and
  destination-state rules that precede rendering: first-nonblank/final timeline
  boundaries, explicit blanks, long unmarked gaps, destination-relative seek
  context, and revised timelines.
- Use the existing full-rate native lyric capture workflow as the perceptual
  integration seam. Static endpoints alone cannot establish focal continuity,
  text clarity, or the quality of height-aware travel.
- Add a new lower-level animation seam only if deterministic interruption
  behavior cannot be exercised at the presentation-update boundary. If one is
  necessary, expose semantic state and geometry rather than toolkit-specific
  class choreography.

### Automated behavior coverage

- A compact adjacent nonblank progression performs one Reel Lift and settles
  with the former current cue as Previous Cue, incoming cue as focal, and the
  following cue as Next Cue.
- The handoff never exposes a state in which every visible lyric is contextual
  or transparent.
- Identical text at adjacent cue indices still counts as progression.
- A three- or four-line outgoing cue uses the abbreviated path even when the
  incoming cue is short.
- A tall-to-short handoff reveals the departed cue in its compact Previous Cue
  role as the incoming cue settles, without squeezing the outgoing focal
  rendering into that position. Its settled context matches a direct seek.
- A three- or four-line incoming cue uses the abbreviated path and settles with
  the established neighbor-visibility rules.
- Actual Pango rendered line count, not newline count or source text length,
  chooses the height-aware branch.
- A continuously available timeline containing nonblank lyrics creates one lyric
  composition interval; internal gaps do not generate exits or entries.
- Removing any consecutive leading blanks leaves presentation and entry timing
  unchanged. A timeline containing only blanks retains ordinary Now Playing.
- Internal blanks with time to settle expose an empty focal position and
  available destination-relative Previous Cue and Next Cue in contextual roles,
  including after tall focal cues. Consecutive blanks do not retrigger motion.
- Blank exit permits advance promotion of the upcoming cue so it becomes focal
  around its timestamp. Trailing blanks retain Previous Cue without Next Cue
  until the hold after the final blank timestamp ends.
- Short blanks retain their timeline entries and timestamps while normal advance
  promotion proceeds without a forced empty dwell, compressed motion, or delayed
  focal arrival. Do not assert that every blank produces an empty rendered frame.
- Long internal blanks retain the lyric composition regardless of duration.
- A long gap without a blank retains the current cue.
- A seek within the interval installs destination text and destination-relative
  neighbors immediately without cue travel.
- A seek across the boundary installs destination cue state directly while
  starting the corresponding in-place composition entry or exit.
- A revised same-identity timeline installs corrected cue state directly. Late
  availability or timeline loss starts same-identity composition entry or exit
  when interval membership changes.
- Retiming the first nonblank cue or final entry across the current playback
  position recomputes interval membership and starts the appropriate in-place
  entry or exit. Revisions that retain membership install corrected cue content
  without cue travel or composition replacement. Revising all nonblank cues to
  blanks removes the interval; adding a nonblank cue to an all-blank timeline
  creates an interval subject to the same destination selection rules.
- A timeline known early enough permits preparation to begin before the first
  nonblank cue becomes focal without requiring settled geometry. Late
  availability, a seek into the interval, and a first cue at track time zero
  introduce destination content during entry
  without waiting for geometry to settle.
- Entry into a settled blank installs empty focal content and contextual
  neighbors, with no highlighted neighbor or ordinary metadata left after entry.
- Presentation Status and the complete footer remain singular and travel with
  the information rail during entry and exit.
- A second natural cue arriving before settlement retargets to the newest cue or
  takes the defined immediate fallback. No stale delayed work can overwrite it.
- A multi-index local advance reaches only the newest cue and never animates
  through skipped indices.
- Pausing during a lift permits settlement and then freezes progression.
- Entry interrupted by exit, and exit interrupted by entry, retain persistent
  stable objects and settle on the newest composition state.
- A genuine Now Playing identity replacement during lyric motion uses complete
  replacement and cannot inherit cue state from the old identity.
- Reduced animation and deterministic Presentation Capture install correct
  endpoints with the full settled hierarchy.

### Native visual acceptance

- Capture and review Natural Cue Handoffs for every short/tall direction:
  one-to-one, one-to-two, two-to-one, two-to-two, short-to-three, short-to-four,
  three-to-short, and four-to-short. Include wrapping-derived line counts rather
  than only source-authored line breaks.
- Capture a genuinely interrupted handoff in which the second cue arrives before
  the first lift can settle. The existing prototype tour's roughly one-second
  spacing is not sufficient evidence for this case.
- Capture ordinary-to-lyrics entry and lyrics-to-ordinary exit with stable Now
  Playing identity. Confirm that artwork, Presentation Status, footer, and
  identities remain singular and that Title/Artist ownership does not double.
  Confirm that status and footer travel with the rail, and that first-cue arrival
  can overlap geometry movement without an empty-looking entry.
- Capture late timeline arrival, timeline loss, and an entry/exit reversal for
  the same Now Playing identity.
- Capture leading blanks, an all-blank timeline, consecutive internal blanks, a
  middle blank, trailing blanks, and a long unmarked gap. Confirm that leading
  blanks do not affect entry, an all-blank timeline stays ordinary, internal and
  trailing blanks that have time to settle retain context around an empty focal
  position, and long unmarked gaps retain the current cue. Review advance promotion out of a blank
  and the single exit after the final trailing blank's hold.
- Review short blanks at normal playback speed, centered on a 300 ms case and
  including shorter and longer intervals, a no-blank baseline, and tall-to-short
  cues. Confirm normal advance promotion preserves flow without a forced cut,
  accelerated jump, or extra dwell. A short blank need not visibly settle;
  longer blanks must retain the empty focal position and contextual neighbors.
  These durations are review examples, not production thresholds. Paused frames
  or slow motion alone cannot establish the quality of this behavior.
- Review entry directly into a settled blank after late availability and a seek:
  artwork carries visual dominance without invented focal emphasis. Include a
  long internal blank to confirm that no duration-based composition exit occurs.
- Capture an in-range external seek and seeks across both composition
  boundaries. Confirm direct destination cue state and destination-relative
  Previous Cue.
- Capture identical adjacent strings and a revised current timeline entry.
- Review reduced-animation endpoints for every semantic case.
- Review settled and moving behavior across all equal-authority supported
  landscape viewports, including the 4:3, ultrawide, and 4K representatives.
- Use the existing full-rate review sheets or recording rather than a single
  overview image. Judge focal continuity, readable text rasterization, clipping,
  masthead/footer separation, and stable geometry throughout motion.
- Presentation Captures remain human-review artifacts, not pixel-golden test
  inputs. Automated tests should assert semantic and geometric outcomes rather
  than exact screenshots.

### Prior art

- Extend the existing presentation-classification coverage that distinguishes
  in-place updates from complete transitions.
- Extend the native lyric-view coverage for Pango-derived neighbor visibility,
  immediate external seek, reduced animation, and cancellation of pending lyric
  work.
- Reuse the existing transition-state coverage for newest-presentation wins and
  bounded outgoing state.
- Extend the existing deterministic lyric transition, wrapping progression, and
  external-seek capture tours rather than creating an unrelated visual harness.
- Keep the existing static Fixture Scenario and peer-viewport coverage as
  settled-layout regression evidence.

## Out of Scope

- Changing the RoonScape Bridge/Renderer boundary or adding transition state to
  the Presentation Snapshot contract.
- Changing how the private Lyric Feed connects to Roon, parses timelines, or
  publishes normalized cues.
- Fixing the separate timing-only progress-versus-activity footer duplication
  defect.
- Replacing or redesigning complete composition transitions for genuine Now
  Playing identity changes.
- Inferring blank intervals from long timestamp gaps or adding synthetic cue end
  times.
- Guaranteeing the prototype's exact colors, coordinates, durations, easing,
  lead time, or final hold.
- Implementing Anchored Dissolve or Crisp Relay.
- Adding playback controls, lyric scrolling, karaoke highlighting, or changes to
  Roon playback.
- Redesigning Presentation Status, progress/activity, Output/Zone identity,
  inactivity movement, artwork decoration, or ordinary metadata typography.
- Publishing this specification as a GitHub issue.

## Further Notes

- In the short Intentional Blank prototype comparison, the user selected **No**:
  preserve normal advance promotion rather than guarantee a perceptible pause
  for every explicit blank. The forced-pause treatment introduced an awkward,
  jumpy transition that broke the flow of the song. This approves the behavior,
  not the prototype's literal timing constants. The disposable source has been
  removed. Local decision evidence remains in the
  [comparison recording](/var/tmp/codex/roonscape/short-blank-comparison-2026-09-04.webm)
  and [review notes](/var/tmp/codex/roonscape/short-blank-review-notes.md).
  The notes record experimental timings and capture limitations.
- Option B, Reel Lift, in the local disposable lyric transition prototype is the
  primary visual reference. Its musical spatial promotion is intentional; its
  browser implementation and literal token values are not production
  architecture.
- The prototype's final external-seek state shows the last actually displayed
  cue as visual memory. Do **not** copy that behavior. The agreed production
  state uses the nearest nonblank Previous Cue in the destination timeline so
  seeking and uninterrupted playback produce the same state at the same
  position.
- The current native cue animation fades and translates the complete
  previous/current/next cluster out before replacing its text. Reel Lift must
  replace that group opacity valley rather than layer additional motion on top
  of it.
- The existing presentation documentation describes a restrained cue fade and
  complete composition crossfade. For this feature, the decisions in this
  specification supersede that motion description while preserving the rest of
  the established presentation system.
- The Lyric Feed can technically republish a changed timeline for the same Now
  Playing identity, although it is unknown whether live Roon sessions commonly
  correct lyric text or timing. Direct destination behavior is required as a
  defensive rule, not because it is known to be a frequent event.
- The feature resolves both the lost-current-cue behavior and the abrupt lyric
  composition entry behavior under one coherent motion direction. Keep their
  evidence available during implementation and native review.
