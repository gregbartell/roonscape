# Curating a Live Capture Session

The raw recording is working material. Mechanical filtering narrows exact and
near duplicates; semantic selection remains the agent's responsibility.

## Review candidates

Generate review sheets:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs review --session <session-directory>
```

The `full-rate-page-*` sheets contain every recorded 20 fps frame in 10x10
grids. Inspect every full-rate sheet in order; the small number in each tile is
its zero-based position within that page. `review/review-index.json` maps each
page and tile to an exact time from recording start. These sheets are the
authoritative defense against a brief change being hidden by similarity
filtering.

When a full-rate thumbnail is ambiguous, extract that exact tick at the source
resolution before deciding whether to retain it:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs inspect --session <session-directory> --at <seconds-from-recording-start>
```

The `candidate-page-*` sheets are a faster, mechanically filtered index of
likely changes. Use them to navigate, but never substitute them for inspecting
the full-rate sheets. Candidate labels are relative to recording start and are
not the final timeline timestamps.

Retain the before/transition/after bookends of every meaningful presentation
change relevant to the requested event. Discard frames whose only differences
are routine progress, waveform, or incidental continuous motion unless that
motion is itself under examination. Do not let similarity filtering remove a
one-tick state merely because it is brief.

The first retained frame must be the last captured stable frame immediately
before the event. It becomes `T+000.00s`; discard all earlier frames. If a
change survives mechanical filtering, its immediately preceding 0.05-second
tick is also included as a candidate so this boundary can be inspected. An
event already underway at recording start has no valid origin and yields an
incomplete session.

Retain the first stable concluding frame. The two-second post-roll establishes
that it stayed stable; later post-roll frames are omitted unless they contain
another meaningful, event-relevant change.

## Describe the selection

Create a JSON file inside the session directory:

```json
{
  "title": "Track A → Track B",
  "complete": true,
  "summary": "The visible transition ordering and any notable anomaly.",
  "frames": [
    {
      "at": 3.25,
      "name": "track-a-before-transition",
      "observation": "Track A is stable immediately before the transition."
    },
    {
      "at": 4.55,
      "name": "first-track-b-state",
      "observation": "Track B metadata first appears."
    }
  ]
}
```

`at` is seconds from recording start and comes from `review-index.json` or
`candidates.json`. Keep frames chronological. The first frame defines the
final time origin. Names are short semantic slugs; the helper adds the ordered
numeric prefix.

For an incomplete observation, set `complete` to `false`, add
`incompleteReason`, and include only useful diagnostic frames. Set
`preserveDiagnostics` to `true` only when the raw recording or process log has
diagnostic value.

When explicit acceptance criteria were supplied, the selection may add:

```json
{
  "acceptance": {
    "criteria": "The supplied criterion.",
    "verdict": "pass",
    "rationale": "Evidence supporting pass, fail, or inconclusive."
  }
}
```

Omit `acceptance` when criteria were not supplied.

## Publish

Publish the selection:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs publish --session <session-directory> --selection <selection.json>
```

The helper creates a collision-safe dated directory directly beneath
`/var/tmp/codex/roonscape`, extracts pristine ordered PNGs, writes the README
timeline, and builds a five-column `overview.png` containing every selected
frame. Overview thumbnails receive bottom-left timestamps such as
`T+001.25s`; annotation failure falls back to an unannotated overview and is
disclosed in the README.

Inspect every published PNG, the README, and the overview. Confirm that file
order, relative times, descriptions, and findings agree with the visible
evidence. Publication deliberately retains the temporary recording during this
inspection. If correction is needed, retract the generated output, revise the
selection, and publish again:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs retract --session <session-directory>
```

After visual and semantic validation succeeds, remove the temporary recording:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs finalize --session <session-directory>
```

Report the output directory and the most important observations to the user.
