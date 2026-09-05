# Roon Web Display lyrics observations

Observed on 2026-09-01 against Roon Web Display assets served by Roon Server
build `207101683`. The interface described here is undocumented and
version-specific.

## Sources and retained evidence

The first-party application shell, JavaScript bundle, markup, and styles were
fetched directly from the observed Roon Server. Protocol and payload behavior
was also recorded through controlled observations of:

- a browser Web Display connection during playback with synchronized lyrics,
  untimed lyrics, and no lyrics;
- a duplicate client using the browser Web Display's registry token;
- an ordinary authorized extension identity;
- an ordinary identity that additionally provided
  `com.roonlabs.zonedisplay:1`;
- fresh `com.roonlabs.display_zone` registrations with and without an active
  Web Display; and
- tracks known to have synchronized lyrics, untimed lyrics, and no lyrics.

Raw captures were not retained because they contained registry authentication
material and copyrighted lyric text. The retained observations contain only
protocol names, redacted payload shapes, timings, counts, hashes, and aggregate
measurements.

## Served application

An unauthenticated `GET /display/` returned the application shell with
`Server: RoonServer/207101683`, `Access-Control-Allow-Origin: *`, and a
`Last-Modified` timestamp of 2026-08-05 21:21:17 GMT. The shell loads
`jquery-3.3.1.min.js` and `display_ui.js`; the bundle subsequently loads
`display_ui.html` into `#uiParent`.[^http-headers]

The observed first-party assets were:

| Asset                          |   Bytes | SHA-256                                                            |
| ------------------------------ | ------: | ------------------------------------------------------------------ |
| `/display/`                    |   3,751 | `12da3df1d1a5166db21ea5e9dcaa7ebf5ede81e43bbc6a98d1e1f38b6aba1c3f` |
| `/display/jquery-3.3.1.min.js` |  86,927 | `160a426ff2894252cd7cebbdd6d6b7da8fcd319c65b70468f10b6690c45d02ef` |
| `/display/display_ui.js`       | 788,171 | `b373c0f3d9dc5d474ad84e4bc78401cec640bf0f0ef64572b1dbe2f67c219741` |
| `/display/display_ui.html`     |  10,528 | `4906ea4852744620f5fdafbaa7cab2fa616720492725d59f5f76d9bc7beb340c` |

## Connection and registration

The bundle opens `ws://<server>:<port>/api`, exchanges binary WebSocket frames,
and encodes requests, continuations, and completions as `MOO/1` headers with
optional JSON bodies. Its connection sequence is:

1. Request `com.roonlabs.registry:1/info`.
2. Read any saved token for the returned Core ID from same-origin
   `localStorage`.
3. Request `com.roonlabs.registry:1/register`, including that token when one is
   available.
4. On `Registered`, instantiate the Core services and invoke the paired-Core
   callback.
5. Request `com.roonlabs.transport:2/subscribe_zones` with a generated
   `subscription_key`.

The Web Display identifies itself as:

```json
{
  "extension_id": "com.roonlabs.display_zone",
  "display_name": "Roon API Display Zone",
  "display_version": "1.0.0",
  "publisher": "Roon Labs, LLC"
}
```

It requires `com.roonlabs.transport:2` and `com.roonlabs.image:1`. It provides
`com.roonlabs.zonedisplay:1`; the bundled Roon API client also adds
`com.roonlabs.pairing:1` and `com.roonlabs.ping:1` to the registration.

The zone-display service exposes `get_displays`, `activate`, `deactivate`, and
`update_settings`, plus a `subscribe_displays` subscription. Activation
supplies an active Roon zone and settings to the page. The captured setting
that enabled lyrics was `{"enable_lyrics":true}`.[^lyrics-capture]

## Endpoint behavior

`/display/` is the browser application's bootstrap page. The application
connects directly to `/api` on the same host and port as the HTTP application.
The observed port was `9330`; it is not fixed.

The inspected first-party Node API discovers the endpoint through SOOD. Its
`start_discovery()` implementation filters for Roon's SOOD service, takes the
response address as `host`, and takes the advertisement's `http_port` property
as `port`; the SOOD parser also honors an advertised `_replyaddr` override. The
WebSocket transport then opens `ws://<host>:<port>/api`.[Roon API discovery
source][roon-api-source] [Roon SOOD parser source][roon-sood-parser-source]
[Roon WebSocket transport][roon-websocket-source]

A Roon engineering response states that the API port is selected from a range
and obtained through discovery.[Roon API port guidance][roon-port-guidance]

## Zone subscription and lyric payload

The standard `subscribe_zones` helper explicitly handles `Subscribed`,
`Changed`, and `Unsubscribed` for its internal zone cache, but invokes the
application callback for every response name. The Web Display callback also
handles two undocumented continuation names: `WaveformChanged` and
`LyricsChanged`.[Roon Transport source][transport-source]

The observed `LyricsChanged` body has this shape:

```json
{
  "zone_id": "<opaque zone id>",
  "key": "<opaque lyric key>",
  "lrc": "[00:14.21]<redacted>\n..."
}
```

For one synchronized-lyrics track, one continuation delivered the whole track
rather than streaming individual lines. The `lrc` value was 1,816 characters
and contained 36 timestamped lines. Its timestamps were monotonic from 14.21
seconds through 232.00 seconds. The opaque key was 24 characters.[^lyrics-capture]

### Three-way lyric availability comparison

A controlled comparison used one track from each observed lyric-availability
state. In all three cases, the one `LyricsChanged` body had exactly the keys
`zone_id`, `key`, and `lrc`:

| Known track state   | First event | `key`                   | `lrc`  | Additional observation                  |
| ------------------- | ----------: | ----------------------- | ------ | --------------------------------------- |
| Synchronized lyrics |    `147 ms` | Non-null                | String | Complete synchronized LRC arrived       |
| Untimed lyrics      |    `114 ms` | Non-null, 26 characters | `null` | No alternate payload during 12 seconds  |
| No lyrics           |    `129 ms` | `null`                  | `null` | No later lyric payload during 12 seconds |

In this sample, key presence distinguished the known untimed-lyrics track from
the no-lyrics track, while a string `lrc` distinguished synchronized lyrics
from both. No retrieval behavior for the untimed key was observed.[^lyric-availability-capture]

## Parsing and presentation behavior

The client splits a non-null `lrc` value on newlines and recognizes lines
matching `[MM:SS.CC]text`, where minutes have at least two digits and seconds
and centiseconds have exactly two. Nonmatching lines are ignored. Every
recognized line becomes a DOM paragraph immediately.

The client adds presentation-only entries around the parsed lyrics:

- If the first timestamp is before three seconds, it inserts a blank entry at
  zero.
- Otherwise it inserts a blank countdown entry four seconds before the first
  timestamp and another blank at the first timestamp.
- If the last parsed line is blank, it marks that entry as a fade-out point.

It chooses the active line using the zone seek position plus a 700 ms visual
look-ahead, adds `current` and `focused` classes to that line, adds `prevnext`
to its neighbors, and translates the full lyric column to center the active
line. Countdown indicators use a separate 200 ms look-ahead.

On a new non-null payload, the client marks the old lyric tree as `dead`,
removes its ID, creates a new `#currentLyrics`, and reaps dead trees later. On
a `null` payload it does not rebuild that tree: the old `#currentLyrics` can
remain in the DOM while `#lyricscontainer` is hidden.

When lyrics first become visible for a newly accepted payload, the client
sends `com.roonlabs.transport:2/report_lrc_viewed` with
`{"key":"<opaque lyric key>"}` and records locally that it has reported the
view.

## Opaque lyric key behavior

An exhaustive trace of the served bundle found these uses of a
`LyricsChanged` key:

1. The handler compares `msg.key` with its per-zone `lrcKey`. A changed key—or
   changed LRC when both old and new keys are null—invalidates parsed lyrics
   and resets the local view-reported flag.
2. It logs the raw key and zone ID to the browser console, then stores the key
   as `lrcKey`.
3. Once synchronized lyrics are visible, it passes `lrcKey` to
   `reportLrcView`, which sends
   `com.roonlabs.transport:2/report_lrc_viewed` with a body containing only
   `key`. A null key is ignored, and the local flag prevents another report
   for the same accepted payload.

The served bundle did not substitute the key into a URL, request another
resource with it, or call a lyric-retrieval method. An exhaustive
service/method-string scan of the bundle found no other lyric-related route.

The published first-party Transport wrapper defines no lyric method or
lyric-key schema. It passes unrecognized `subscribe_zones` continuation names
to the caller, while the Web Display sends the view report through the raw MOO
request API.[Roon Transport source][transport-source]

Two narrow unauthenticated HTTP checks—`GET /api/lyrics` and `GET /api/lrc`,
both without a key—returned empty `404` responses on this server build. No
other paths were checked.[^lyric-route-controls]

## Access and authentication observations

The static application and its assets were readable without authentication.
Separate WebSocket clients connected both without an `Origin` header and with
an arbitrary cross-origin `Origin`; the server did not reject either
handshake. Both connections still required successful Roon registry
registration before receiving Core services and zone-subscription data.

The page persists its Roon registry state in same-origin `localStorage`. That
registry token is distinct from the page's `zone_key`, which identifies the
Web Display instance to `com.roonlabs.zonedisplay:1`.

## Same-token connection observation

A duplicate client registered with the browser Web Display's existing registry
token and extension identity. The observed timeline was:[^token-capture]

| Time from duplicate start | Observation |
| ------------------------: | ----------- |
| `t+104 ms` | The duplicate client registered with the existing token and identity. |
| `t+105 ms` | The original browser connection disconnected; its active zone became undefined and its UI faded out. |
| Before `t+10 s` | The duplicate received lyric and seek updates. |
| `t+10.1 s` | The browser's reconnect registered again; the duplicate closed with WebSocket code `1006`. |
| Following minute | The browser reauthenticated and received display settings, but did not recover active-zone presentation or visible UI. |

No manual stop or restart occurred during this run.

## Identity and service observations

### Ordinary authorized identity

An ordinary authorized extension identity registered with the supported image
and transport services and subscribed to zones while a browser Web Display
visibly advanced synchronized lyrics. `Subscribed` arrived after 167 ms with
one zone, followed by 12 `Changed` continuations containing one seek update
each during 12.151 seconds. No `LyricsChanged` or `WaveformChanged`
continuation arrived.[^ordinary-identity-capture]

### Ordinary identity providing the zone-display service

The ordinary identity experiment was repeated while additionally providing
`com.roonlabs.zonedisplay:1`. The inert service advertised zero displays and
could answer every zone-display method without activating or modifying a
display.

`Subscribed` arrived after 104 ms with one zone, followed by 12 `Changed`
continuations with seek updates during 12.078 seconds. No `LyricsChanged` or
`WaveformChanged` continuation arrived. The server made no request to the
advertised zone-display service: it did not call `subscribe_displays`,
`get_displays`, `activate`, `deactivate`, or `update_settings`.

The browser Web Display remained active and rolled over to the beginning of
the looping synchronized-lyrics track during the observation.[^zone-display-service-capture]

### Fresh official identity

A disposable client registered as `com.roonlabs.display_zone` with the
first-party metadata and service set, no existing registry token, and zero
advertised displays. It never activated a display.

The server registered this identity without user approval. `Registered`
arrived 74 ms after observation began and included a fresh 36-character token.
The server immediately requested `subscribe_displays` and
`subscribe_pairing`.[^fresh-official-identity-capture]

The subsequent zone subscription received:

| Time from start | Continuation      | Observation |
| --------------: | ----------------- | ----------- |
| `98 ms`  | `Subscribed`      | One zone |
| `99 ms`  | `WaveformChanged` | A 2,000-sample waveform |
| `176 ms` | `LyricsChanged`   | A non-null 1,966-character LRC with 65 monotonic timestamped lines |

The lyric key was present and 24 characters. The LRC timestamps ran from 4.58
through 233.29 seconds. The existing browser Web Display remained connected
and visibly advanced lyrics throughout and after this observation.

### Fresh official identity with no active display

The official-identity observation was repeated after every activated Web
Display had been stopped. Immediately beforehand, the still-open browser page
had no active zone, its `#uiParent` and `#lyricscontainer` both had computed
opacity zero, its title was `Roon`, and it showed neither a current lyric line
nor playback progress.

The disposable client supplied no token, advertised zero displays, and never
sent `activate`. The observed timeline was:[^no-active-display-capture]

| Time from start | Event                        | Observation |
| --------------: | ---------------------------- | ----------- |
| `99 ms`  | `Registered`                 | A fresh 36-character token was returned and discarded |
| `100 ms` | `subscribe_displays` request | The client advertised an empty display list |
| `101 ms` | `subscribe_pairing` request  | The server subscribed to the built-in pairing service |
| `126 ms` | `Subscribed`                 | The transport subscription reported one Roon zone |
| `146 ms` | `WaveformChanged`            | A 2,000-sample waveform arrived |
| `147 ms` | `LyricsChanged`              | A complete, non-null synchronized LRC payload arrived |
| `240 ms` | `Changed`                    | A zone seek update arrived |

The LRC was 2,958 characters with 75 monotonic timestamped lines, including
four blank timed lines. Its timestamps ran from 129.41 through 522.45 seconds,
and its opaque key was present and 26 characters. The client disconnected
after 548 ms. Afterward, the browser still had no zone, current lyric, or
progress; `#uiParent` and `#lyricscontainer` remained at opacity zero.

## Limits of the observations

- `LyricsChanged`, `WaveformChanged`, `report_lrc_viewed`,
  `com.roonlabs.zonedisplay:1`, and the MOO wire format are absent from the
  inspected published Transport API surface.[Roon Transport
  source][transport-source]
- The lyric-availability comparison covered one known track per state. It did
  not test providers, translations, multiple lyric versions, edits during
  playback, or every form of untimed lyric.
- The opaque key was observed only in `LyricsChanged` and
  `report_lrc_viewed`. Its wider semantics and stability were not observed.
- Origin permissiveness did not bypass registry registration.
- Token displacement and identity behavior were observed against one Roon
  Server build.

[^http-headers]:
    Direct unauthenticated HTTP requests on 2026-09-01 returned the recorded
    headers and byte-identical assets identified by the hashes in this note.

[^lyrics-capture]:
    Controlled browser-network observation on 2026-09-01. Secret values,
    opaque IDs, registry tokens, and lyric text were discarded.

[^token-capture]:
    Controlled duplicate-connection observation on 2026-09-01. The user did
    not stop or restart the Web Display during the run.

[^ordinary-identity-capture]:
    Controlled disposable-client observation on 2026-09-01 using an ordinary
    authorized extension identity while a browser Web Display visibly
    advanced synchronized lyrics. Lyric text, registry tokens, and opaque IDs
    were not retained.

[^zone-display-service-capture]:
    Controlled disposable-client observation on 2026-09-01 using an ordinary
    authorized identity plus an inert `com.roonlabs.zonedisplay:1` provider.
    The browser Web Display remained the positive control.

[^fresh-official-identity-capture]:
    Controlled disposable-client observation on 2026-09-01 using a fresh
    official Web Display identity. The returned token and all lyric text and
    opaque IDs were discarded.

[^no-active-display-capture]:
    Controlled disposable-client observation on 2026-09-01 after all
    activated Web Displays had been stopped. The returned token and all lyric
    text and opaque IDs were discarded.

[^lyric-availability-capture]:
    Three controlled disposable-client observations on 2026-09-01 using one
    track known to be in each lyric state. Each null case was observed for 12
    seconds after subscription. Only event timing, payload field names, value
    types, nullness, and key length were retained.

[^lyric-route-controls]:
    Two direct unauthenticated HTTP requests on 2026-09-01, made without a
    lyric key or another identifier. Both exact paths returned an empty `404`
    response.

[roon-api-source]: https://github.com/RoonLabs/node-roon-api/blob/055dae6c2ac45b6c738aa3b9e5ac1fd722ed60e8/lib.js#L123-L176
[roon-port-guidance]: https://community.roonlabs.com/t/roon-api-on-build-880-connection-refused-error/181619/3
[roon-sood-parser-source]: https://github.com/RoonLabs/node-roon-api/blob/055dae6c2ac45b6c738aa3b9e5ac1fd722ed60e8/sood.js#L33-L76
[roon-websocket-source]: https://github.com/RoonLabs/node-roon-api/blob/055dae6c2ac45b6c738aa3b9e5ac1fd722ed60e8/transport-websocket.js#L6-L14
[transport-source]: https://github.com/RoonLabs/node-roon-api-transport/blob/2ee60008a4cdb90c34ff3de58bb4b949067f1d20/lib.js
