# Roon Web Display protocol knowledge

These findings describe the undocumented Web Display interface in Roon Server
build `207101683`. Confidence applies to that build, not to all Roon versions.
**Established** means supported by the inspected client or SDK implementation;
**observed** means demonstrated server behavior without a published guarantee;
**inferred** means a plausible interpretation; **unknown** means unresolved.

## Connection and discovery

**Established:** `/display/` bootstraps the browser application, which connects
to `ws://<server>:<port>/api` on the same host and port. The API port is dynamic.
The Node SDK discovers it through SOOD using the response host and advertised
`http_port`, honoring `_replyaddr` when present. RoonScape must discover the
endpoint rather than assume a fixed port.
See the [SDK discovery implementation][roon-api-source],
[SOOD parser][roon-sood-parser-source], and
[WebSocket transport][roon-websocket-source].

**Established:** The connection uses binary WebSocket frames carrying `MOO/1`
headers and optional JSON bodies. The client requests
`com.roonlabs.registry:1/info`, registers through
`com.roonlabs.registry:1/register`, then subscribes through
`com.roonlabs.transport:2/subscribe_zones` with a `subscription_key`.

## Identity, authorization, and display independence

**Established:** The Web Display registers with this identity:

```json
{
  "extension_id": "com.roonlabs.display_zone",
  "display_name": "Roon API Display Zone",
  "display_version": "1.0.0",
  "publisher": "Roon Labs, LLC"
}
```

It requires `com.roonlabs.transport:2` and `com.roonlabs.image:1`, provides
`com.roonlabs.zonedisplay:1`, and includes the SDK's pairing and ping services.
The zone-display service supports `get_displays`, `subscribe_displays`,
`activate`, `deactivate`, and `update_settings`. Activation supplies a zone
and display settings; `enable_lyrics` controls browser lyric presentation.

**Observed:** An ordinary authorized extension received zone and seek updates
but no lyric continuations. Advertising the zone-display service under an
ordinary identity did not enable them. A fresh `com.roonlabs.display_zone`
registration received lyric continuations without user approval, without
advertising concrete displays, and without an activated Web Display.

**Inferred:** The official identity participates in lyric access selection;
advertising the service alone is insufficient. The server's exact selection
rules and behavior across versions are unknown.

**Observed:** Reusing an active Web Display's registry token displaced its
connection and disrupted its presentation. A fresh token allowed independent
observation. Registry tokens must not be shared between RoonScape's Lyric Feed
and a Web Display. See [ADR 0002](../adr/0002-isolate-the-private-lyric-feed.md)
for RoonScape's integration boundary and token lifetime.

**Established:** The registry token authenticates the connection. The browser's
`zone_key` identifies a display instance; it is distinct from both the registry
token and the lyric key.

**Observed:** Static assets were accessible without authentication and the
WebSocket handshake accepted arbitrary or absent Origin headers. Registry
registration was still required for service and zone data; Origin acceptance
does not imply authorization bypass.

## Lyric continuations

**Established:** The [Transport wrapper][transport-source] maintains its zone
cache for `Subscribed`, `Changed`, and `Unsubscribed`, but forwards other
continuation names to the application callback. The Web Display handles
`LyricsChanged` and `WaveformChanged`; the published wrapper exposes no lyric
retrieval method or lyric-key schema.

**Observed:** `LyricsChanged` carries this shape:

```json
{
  "zone_id": "<opaque zone id>",
  "key": "<opaque lyric key or null>",
  "lrc": "<complete timestamped LRC string or null>"
}
```

The placeholders describe types: absent lyric keys and LRC values are JSON
`null`, not the string `"null"`. Synchronized lyric payloads contain a complete
timeline, including possible blank timed lines, rather than individual streamed
cues.

| Lyric availability | Observed `key` | Observed `lrc` |
| --- | --- | --- |
| Synchronized lyrics | Non-null | Timestamped string |
| Untimed lyrics | Non-null | `null` |
| No lyrics | `null` | `null` |

**Inferred:** A non-null key with null LRC may indicate untimed lyrics. The
sample is too narrow to make that a reliable classification rule. Provider
variation, translations, multiple versions, and edits during playback remain
unknown. RoonScape should rely on usable timed content rather than key presence
for synchronized presentation.

## Parsing and view reporting

**Established:** The browser recognizes `[MM:SS.CC]text` lines, with at least
two minute digits and exactly two second and centisecond digits. It ignores
nonmatching lines. Browser countdowns, lookahead, and animation are presentation
choices, not additional protocol data or RoonScape design requirements.

**Established:** A changed lyric key, or changed LRC when both keys are null,
invalidates the browser's parsed lyrics and resets its view-reported flag.
When synchronized lyrics become visible, it sends
`com.roonlabs.transport:2/report_lrc_viewed` with
`{"key":"<opaque lyric key>"}` once for the accepted payload. A null key is
not reported.

**Established within the inspected client:** The key is used for payload
identity and view reporting, not substituted into a URL or sent to a lyric
retrieval method. **Unknown:** whether other clients or private endpoints can
retrieve untimed lyrics, and what stability guarantees the key has. No general
claim that such retrieval is impossible follows from the inspected client.

[roon-api-source]: https://github.com/RoonLabs/node-roon-api/blob/055dae6c2ac45b6c738aa3b9e5ac1fd722ed60e8/lib.js#L123-L176
[roon-sood-parser-source]: https://github.com/RoonLabs/node-roon-api/blob/055dae6c2ac45b6c738aa3b9e5ac1fd722ed60e8/sood.js#L33-L76
[roon-websocket-source]: https://github.com/RoonLabs/node-roon-api/blob/055dae6c2ac45b6c738aa3b9e5ac1fd722ed60e8/transport-websocket.js#L6-L14
[transport-source]: https://github.com/RoonLabs/node-roon-api-transport/blob/2ee60008a4cdb90c34ff3de58bb4b949067f1d20/lib.js
