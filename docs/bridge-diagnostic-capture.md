# Bridge Diagnostic Capture

Bridge Diagnostic Capture retains a chronology of decoded messages received
from Roon and Presentation Snapshots published by the RoonScape Bridge. It is
disabled by default. Enable it explicitly when launching Live Mode:

```sh
./roonscape --capture-bridge /path/to/bridge-capture
```

In a source checkout, use `./src/launcher/roonscape` after building. These
operational flags are invocation-only: they are not Display Configuration,
interactive setup choices, or Renderer options. They cannot accompany
`--setup`. Capture does not enable a disabled Lyric Feed, open any additional
Roon connection, request additional state or artwork, or alter playback.

The default total retained budget is **100 MiB**. To select another budget,
pass a positive whole number of MiB along with the destination:

```sh
./roonscape --capture-bridge /path/to/bridge-capture --capture-budget-mib 32
```

## Output and reading

Choose a dedicated directory. RoonScape creates it if missing and restricts its
permissions to `0700`; capture files use `0600`. JSONL files are named
`bridge-capture-<launch-time>-<session-uuid>-<segment-number>.jsonl`. This
filename namespace belongs to capture. Other files are not removed. A private
`.bridge-capture.lock` prevents concurrent writers to the same directory; a
second writer reports failures and retries when recording more input. After an
unclean process exit, remove that lock only after confirming no capture writer
is using the directory, or select a new destination.

Each line has a UTC `timestamp`, a monotonically increasing `sequence` within
its `sessionId`, a `type`, and `data`. Record types are:

- `session`: format version and configured budget.
- `discovery`: available ordinary discovery messages or directed discovery
  context, including the resolved endpoint.
- `connection`: connection UUID, role (`ordinary` or `lyricFeed`), endpoint,
  available Roon server identity, and lifecycle state.
- `inbound`: the same connection context and the complete SDK-decoded `message`,
  subject to secret redaction, artwork omission, and bounded recording limits.
- `snapshot`: `snapshot` is the Presentation Snapshot actually published by
  the Bridge; `contributors` and `trigger` explain its diagnostic provenance.
- `checkpoint`: retained local state and the latest previously captured
  publication, placed at the start of every file. This is not received Roon
  data and does not query Roon for a baseline.
- `gap`: omitted record sequence bounds/count, or a specific metadata omission.

Connection UUIDs distinguish reconnects. Format version 2 identifies contributing
inputs separately for `zone`, `timing`, `lyrics`, `artwork`, and `availability`.
Each available source has a `sessionId`, `sequence`, record `type`, and bounded,
redacted `context` copied from that input. Multiple fields may reference the
same input. A seek update can supply position while an earlier zone input
still supplies duration and Now Playing. Accepted lyric inputs survive pending
compositions, and artwork references follow the accepted asynchronous response.
Received inputs that are stale or rejected remain `inbound` records but do not
become contributors merely because they arrived recently.

`trigger` describes the publication path, including startup, received input,
artwork completion, and local error recovery. `localInterpretation` preserves
the reason and available input reference when the Bridge omits oversized lyrics,
including while artwork is pending. It is separate from the received
sources: a local failure may publish a snapshot using earlier Roon observations.
A missing contributor means no received source reference is retained for that
part of the publication; it does not prove that Roon reported an empty value.
These are Bridge interpretations of accepted inputs, not claims that Roon sent
the normalized snapshot or that the most recent message caused it.

Checkpoints include accumulated ordinary zone state with its sources, the
private Lyric Feed's accepted and pending context when active, any pending
Presentation Snapshot, and the latest published snapshot with its contributors.
The startup and disconnected states explicitly identify unavailable information;
an observed subscription with `zones: []` means an observed empty baseline.
The Lyric Feed similarly distinguishes no accepted lyric input, an observed
empty response, and an input that produced no usable timeline.
Incremental state received before a baseline is marked partial. Sources retain
bounded message context, including connection identity and artwork metadata, so
references remain interpretable after their original files have been deleted.
An `unavailable` marker identifies context that could not fit or be serialized.

A checkpoint's `beforeSequence` places it immediately before the accompanying
ordinary record. It shares that record's envelope sequence; it is not another
received input or another publication. Its contents describe retained local
observations at recording time, including observations made during a recording
gap. `earlierRecordingGap`, `earlierWriteGap`, and available `recordingGap` bounds
mark missing evidence. Retained context cannot reconstruct discarded history,
intermediate states never retained, or all messages lost during a gap. A new
launch does not reconstruct the previous launch's local state.

Read files with ordinary text tools. For example, inspect decoded inbound
messages with `jq`:

```sh
cat /path/to/bridge-capture/bridge-capture-*.jsonl |
  jq -c 'select(.type == "inbound")'
```

File names sort by launch time and segment; use `sessionId` and `sequence` for
ordering within a launch, since wall-clock timestamps can change. Recording
includes early registration/pairing exchanges, incoming service requests,
responses and continuations, all-zone updates, and decoded inputs subsequently
ignored, rejected, stale, or deduplicated by the SDK or application. It excludes
malformed traffic and WebSocket network control frames. This is not packet
capture or a replay format.

## Secrets and artwork

Capture redacts credential-bearing fields recursively, including registry
and authorization tokens, passwords, authentication headers, cookies, and API
keys. It also strips recognizable bearer/basic credentials, URL user/password
pairs, and credential query parameters. The private Lyric Feed registry token
remains in memory for the existing connection lifetime; capture never persists
it. Nonsecret identifiers, including image keys, remain readable. Error logs
use fixed descriptions and never include raw messages, filesystem errors, or
credential-bearing paths. Captures still contain listening history and server
identifiers; handle them as private diagnostic evidence.

Artwork responses retain their existing request's image key and parameters,
Roon server association, response status/error, content type, and byte count.
Artwork bytes are omitted. Capture associates the response with a request
already issued by the Bridge and never fetches an image. A retained image key
supports a later retrieval attempt against Roon; availability is not guaranteed.
Non-artwork binary message bodies use a base64 representation.

## Retention, omissions, and failure

Segments rotate at up to 1 MiB (or one quarter of the configured budget).
Before appending, the writer discards the oldest capture-owned segments as
needed to keep their total logical file sizes within the budget. This includes
files retained from previous launches in the same directory. Filesystem block
allocation and directory metadata are outside that byte accounting. Do not
modify capture-owned files while recording.

Disk work runs asynchronously in a worker thread, targeting writes within one
second of observation. No `fsync` or crash/power-loss durability is promised.
Orderly shutdown attempts to flush pending output, but waits at most one second
for capture. The Bridge's progress and timely shutdown do not depend on the
writer or destination being responsive.

Recording admission is bounded to 1 MiB of queued JSONL and 1,024 records, with
small reserved space for gap metadata. Queued checkpoint context counts toward
that byte bound, and checkpoint bytes count toward both segment and total disk
limits. Each checkpoint section has a share of the bounded context allowance;
large sections are explicitly omitted while other sections can survive.
Individual records are limited to 256 KiB or one eighth of the budget, whichever is smaller. Serialization also
bounds traversal depth and input size; oversized, cyclic, or otherwise
unserializable input is omitted rather than truncated into apparently complete
evidence. Artwork request metadata is bounded to 128 outstanding requests and
8 KiB per request. Retained source context uses a 16 KiB traversal allowance
per input; checkpoint limits apply again before writing. Exceeding these limits
emits an omission notice or an explicit unavailable
context marker. If artwork correlation is incomplete, subsequent uncorrelated binary bodies on that connection are
omitted conservatively as well.

Destination/write failures and write pressure are logged through operational
stderr, with repeated notices rate-limited. Input omitted before admission is
summarized in a gap record when queue space is available. After a write failure,
the worker retries no more than once per second while receiving input. On
recovery it starts a new segment with a checkpoint and uses the first subsequent
record slot to write a gap identifying omitted sequence bounds/count; that
slot's original input is also omitted.
Admission-gap recovery also starts a new segment. Shutdown makes one final
best-effort gap write, with explicitly omitted checkpoint context when needed
to preserve the queue bound. A partial append is truncated back when possible; a final incomplete JSONL line can remain after an
unrecoverable write failure or abrupt exit. Sequence discontinuities and gap
records identify missing evidence, but gaps themselves may be lost when storage
is unavailable or older files rotate away. Operational failure logs remain
necessary when no capture output can be written.
