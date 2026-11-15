# Soulseek

Independent native package `player.soulseek`, for desktop and Android.
Rust implements its UI, search, pagination, cover scheduling, preview, import,
replacement actions, and persistent transfer intentions.

| File | Purpose |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Native package crate |
| [plugin.json](plugin.json) | API 2 permissions and contributions |
| [src/lib.rs](src/lib.rs) | NativePlugin implementation and behavior tests |
| [assets/README.md](assets/README.md) | Logo attribution |

| Contribution | Placement |
| --- | --- |
| `connection` | Settings: username and credential form |
| `search` | Global search provider |
| `replacement` | Internal replacement page, hidden from the sidebar |
| `find-replacement` | Track-menu action opening the internal page |
| `buffer` | Transfer status in `player.status` |

The package owns its network client, Tokio runtime, transfers and cover downloads in
[src/service.rs](src/service.rs). The modified protocol client is preserved under
[vendor/soulseek-rs-lib](vendor/soulseek-rs-lib). Player contains neither dependency.
Generic host storage/media services issue source tokens for staged downloads; normal
library import/replacement APIs save completed files. Credentials use host secrets.
Workers queue completion events; pollIntervalMs drains them on serialized native calls.

Remote queue status means the peer has not started sending the file; advertised
speed and free slots do not guarantee immediate upload. Queue position is shown
when supplied by the peer. Preview starts only after enough audio is buffered;
library saves wait for completion. Cancel preview preserves a separately requested
save, and Cancel save preserves a pending preview. Cancelling the last request
cancels the package-owned transfer. The player status uses the Soulseek icon for
active transfers, errors and completed downloads. Its menu lists active transfer
titles, progress and queue status, with Preview/Cancel preview and Save to library/
Cancel save actions sharing the search results' transfer intentions. The menu credits
the author declared in `plugin.json`.

Search results do not normally include artwork. For each visible result folder,
the package requests that exact directory listing from the same peer (not a
global image search or a browse of the peer's entire library). Two artwork jobs
at a time cover discovery and download; requests and results are deduplicated by
peer plus directory. The first eligible JPG/JPEG/PNG/WebP image in the folder
listing is selected; extensions are case-insensitive. Empty files, files over
32 MiB, other peers and child/sibling folders
are excluded. The selected image is downloaded separately through Soulseek and
stored through `media.cover`; no third-party artwork service or unsolicited
audio download is used. Unavailable/empty folders retain a placeholder. Embedded
audio artwork remains available through metadata extraction on library import.
While discovery or transfer is pending, search thumbnails use the player's
existing animated loading stripes. Missing artwork and failures stop the
animation. Completed preview status follows the active Soulseek track and
disappears when playback moves elsewhere; ongoing downloads remain visible.
Cached folder artwork is reapplied when searches refresh, and the current artwork
is forwarded to preview resources and library imports even from older result rows.

## Diagnostics

The package writes newline-delimited JSON under its host-provided staging directory:
`diagnostics/soulseek.jsonl` and one rotated `soulseek.jsonl.1`, each at most 512 KiB.
On macOS the default path is
`~/Library/Application Support/dev.verncat.player/plugin-system/staging/player.soulseek/diagnostics/soulseek.jsonl`.
Read both files (archive first) to follow connection attempts, transfer enqueue,
local/remote queue state and position, byte progress, completion/failure/cancellation,
preview readiness/start, library import and artwork transfers. Progress is sampled
at most every five seconds except state/queue changes. `timeMs` is Unix time;
`resource` is SHA-256 of the internal resource ID, allowing correlation without
writing usernames or remote/local file paths. Credentials, source tokens, queries,
protocol payloads and raw errors are never logged. `failureCategory` records safe
stage/kind categories (for example `peer_connection_refused`,
`peer_truncated_transfer`, `peer_disconnected`, or `peer_upload_failed`), never
peer-provided text. Vendor tracing is not enabled as a diagnostics subscriber.
Logs are best-effort after initialization and do not replace user-visible errors.

Direct file sockets send PeerInit(F), **receive** the uploader's raw uint32 transfer
token, then send the uint64 resume offset. The uploader owns FileTransferInit even
when the downloader initiates TCP; see the [Nicotine+ primary implementation](https://github.com/nicotine-plus/nicotine-plus/blob/master/pynicotine/slskproto.py)
(`_process_file_init_message` and `_process_file_output`). Incoming init frames
are read without consuming the following raw transfer token or file bytes.
Independent literal wire fixtures cover this direction and framing. Downloads
complete only at their advertised byte size; early EOF is a categorized failure.
Closing the separate P control socket does not cancel an F transfer or remote queue.
Structured `file_*` events identify socket direction/handshake/data phases and byte
counts without peer identity or payloads. `cover_media_ready` confirms that the
download reached `media.cover`; `cover_media_unavailable` distinguishes conversion failure.
Folder artwork discovery records `artwork_folder_listed` after a successful
same-peer directory response, `artwork_folder_found` when an eligible image is
selected, and `artwork_folder_failed` on a protocol/connection timeout or error.
`artwork_folder_unavailable` leaves a placeholder rather than substituting
artwork from another peer or folder. These events retain only hashed resource IDs.
Successful empty listings are cached for the activation. Transient listing errors
are not cached as missing artwork, so a later explicit search can try again;
there is no automatic retry loop.
Folder control connections try the peer's address directly before falling back
to one server-mediated connection attempt. Timeout categories identify only the
last control phase, without recording peer identities or protocol payloads.
Reverse F handshakes also record `file_reverse_handshake_started` and
`file_reverse_handshake_failed_before_token` against their initiating transfer:
previously a peer closing before its raw token produced a truncated-transfer
error without any file-phase event. The F stream has no separate filesize header;
size is negotiated on the P connection, and raw file bytes follow the resume offset.
Closed peer actors are removed when their command channel rejects an upload,
allowing a fresh peer-address response to create a replacement. Address recovery
and upload response share one bounded deadline rather than resetting on reconnect.
Replacing a disconnected client session first cancels its actor system, including
the old listener and reconnecting server actor; it cannot leave an orphan session
reconnecting with the same account after a subsequent connection attempt.
Search-result P sockets close as soon as their response has been dispatched;
non-transfer P sockets also expire after fifteen seconds without incoming data.
Previously a popular search retained hundreds of established responder sockets
(over 650 were observed in the native application). Actual transfer control
sockets are excluded from this search-only lifecycle, preserving remote queues.
Artwork already has two workers and is not an unbounded connection source.
Peer input drains up to 1 MiB per polling cycle with 32 KiB reads, rather than
one 1 KiB read every 100 ms, so large search responses do not retain sockets
artificially at about 10 KiB/s while hundreds of other peers arrive.
Direct P connections identify themselves with `PeerInit(P)`; only a server-mediated
reverse rendezvous uses `PierceFireWall` with its nonzero rendezvous token.
Both wire forms have independent literal socket fixtures. Expired actors drain
connection signals before checking idle age so queued sockets cannot be retained.
The listener admits at most 16 concurrent initialization handshakes and 64 inbound
P sockets. Server-rendezvous/direct outgoing non-transfer P actors have a separate
64-slot per-client budget shared across registry clones; transfer control has eight
reserved slots. Excess connections are rejected immediately, not queued. Permits
release on disconnect and actor drop; classified F transfers do not consume P slots.
These limits bound observed thousand-socket search fan-in in both directions.


Desktop: install the compiled package from the catalog. Android: install the separate
[APK](android/README.md), then select Find APK plugins in Player. Disabling cancels
owned tasks. Uninstallation follows the normal plugin manager flow.

Run `cargo test -p player-soulseek-plugin`. Offline tests cover pagination, navigation,
preview/import intentions, status without credential prompts and remote path validation.
Live network/account behavior needs separate testing. See [SDK](../docs/sdk.md).
