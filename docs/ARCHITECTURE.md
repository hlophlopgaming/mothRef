# Architecture

`mothref` owns the event loop and wires together independent crates:

- `moth-scene`: pixel-free image metadata, UUIDv7 identity, fractional ordering,
  reversible commands, an R-tree and an ordered index for top-item lookup.
  Commands return their inverse; the storage
  worker journals undo/redo before acknowledgment.
- `moth-format`: byte-stream storage contracts, opaque handles, schema-one
  records, checkpoint replay and atomic compaction. See [FORMAT.md](FORMAT.md).
- `moth-io`: bounded Rayon decode work, header checks and panic isolation. JPEG
  uses allocation-budgeted DCT scaling, EXIF orientation and CMYK conversion; PNG
  and clipboard previews use alpha-weighted area averages, including Adam7 passes.
  Original dimensions determine geometry; pixels live only in worker results
  and the renderer. Preview pyramids stop at 2048 pixels on the long side.
- `moth-platform`: native path resolution, clipboard access and durable staging.
  Filesystem and clipboard calls run in workers. Registration on the event
  thread records a path without opening it. The storage worker copies imported
  originals into the journal before acknowledging their scene commands.
- `moth-import`: foreign board extraction and conversion into scene data;
  `.pur` behavior, compatibility and limits are specified in
  [PUR-IMPORT.md](PUR-IMPORT.md).
- `moth-render`: instanced quads and a GPU resource registry, shared by the window
  and headless checks. Texture pages use size classes, full mip pyramids and
  page-level LRU eviction. Original bytes do not depend on a GPU texture format.
- `moth-ui`: theme tokens, bundled licensed font, localized string lookup and
  responsive wrapped controls. Pan/select, fit, zoom and paste have visible
  controls for touch input.

The native input adapter is the only app code consuming raw mouse events.
Normalized pointers carry identity, kind and pressure; two-contact pan/pinch is
implemented by a separate gesture layer. On native Linux Wayland, the adapter
supplements winit 0.30 with `tablet-v2` on the same display connection. A worker
batches tool motion, pressure and contact phases at protocol frames; logical
surface coordinates are already scaled. Pen input reaches both canvas selection
and egui controls. Proximity loss/removal cancels contacts. Touch input continues
through winit and the separate gesture layer; real touch-device qualification is
still pending.

## Budgets and lifetimes

| Resource | Bound |
| --- | --- |
| App board admission / waiting import backlog | 1,000,000 items / 1,024 handles |
| Desktop source catalog | 1,000,000 handles |
| Texture arrays, including every layer and mip | 96 MiB |
| Instance buffer | 10,000 instances, 880,000 bytes, reused between ordered batch submissions |
| Decode requests / completed images | 8 / 2 |
| Concurrent decode consumers | min(cores − 1, 4), at least 1 |
| Rayon pool threads | cores − 1, at least 1 |
| Compressed source / raw scratch input | 64 MiB |
| Header dimensions / non-JPEG pixel count | 32,768 per side / 32 × 1024² pixels |
| JPEG reduced planes + estimated progressive coefficients | 128 MiB admission budget |
| Area accumulator | At most 2048² × 32 bytes |
| PNG decoder internal allocations | 8 MiB |
| Clipboard requests / results | 1 / 1 |
| Clipboard staging directory, per board | 8 GiB and 10,000 entries |
| Native clipboard file list | 1,024 absolute paths |
| Storage requests / results | 32 / 32 |
| Undo/redo | 256 session commands |

The decode concurrency cap bounds temporary pixels independently of CPU count.
Texture lookup entries exist only for resident slots. Texture pages and lookup
entries are evicted together; debug teardown checks allocation accounting returns
to zero. There is no retained CPU pixel cache.

The clipboard worker stages original pixels as lossless PNG (RGB for wholly
opaque images, RGBA otherwise) with a bounded 256 KiB output buffer before reporting
success and passes an already-built preview with the result. The preview hash
matches the staged PNG, so eviction reloads the same content. PNG bytes are hashed
while writing and the preview reuses that identity, avoiding redundant pixel
hashing and a file reread. File and directory syncs finish before staging returns.
Recovery files are
private to the user on Unix and never overwrite existing files. Staging rejects
new pastes when its budget is full; it never evicts originals. Native clipboard
file lists enter the same bounded import path as file drops. Unsupported/empty
payloads and saturated queues have localized feedback. When no native file/image
payload exists, explicit Paste can import a direct HTTP(S) image URL or the first
HTML `img[src]`. The worker parses at most 1 MiB of captured text/HTML and validates
URLs up to 8 KiB. Downloads have a shared 20-second HTTP deadline, at most five
redirects and a 64 MiB input limit; existing decoder limits also apply. Every
redirect is validated, embedded credentials and HTTPS downgrades are rejected,
and TLS uses OS trust roots. Original downloaded bytes are synced in the same
budgeted recovery directory, then pass through the existing command/journal path.
No web pages or scripts execute, and clipboard polling never starts downloads.

Explicit discard removes only this board's staged originals and journal; crashes
retain them for the recovery browser. Legacy checksummed `.rgba` scratch records
remain readable, but do not preserve layout by themselves.

The event loop waits between input, worker completions and requested UI animation
frames. Zero-sized windows do not render. Surface loss, suspend/resume and device
loss replace GPU/UI resources while retaining scene metadata, camera and source
handles. Adapter creation retries a fallback adapter; `--software` forces it.
RGBA8 is the portable baseline; BC7 is chosen at runtime on adapters that
support it, with RGBA8 as the fallback.

## Current limits

Large-image visible tiles and full-resolution inspection beyond the 2048-pixel
preview limit are not implemented. Rendering uses fixed-size ordered batches
instead of dropping items beyond the first 10,000. App admission, import backlog,
source catalog, input-byte, journal and snapshot budgets still apply; this does
not promise that every million-item board fits a 64 MiB checkpoint.

winit 0.30 does not deliver native Wayland drops itself. The input adapter binds
`wl_data_device_manager` on winit's existing connection, negotiates copy-only
`text/uri-list` offers for its own surface and receives local paths on a transfer
worker. Transfers allow at most 1 MiB, 1,024 paths and five seconds; remote URLs
and malformed paths fail without importing a partial list. Offers survive
`leave` until a successful read finishes the protocol operation. Both native
workers sleep when idle, queues are bounded (256 events, one waiting transfer),
and seats/tools/tablets/offers are capped at 64 each. The display handle outlives
all guest proxies and workers; suspension retains the adapter rather than
rebinding native devices. Clipboard interoperability and pen/touch behavior still
need qualification on other desktops.
The Wayland decoration backend omits its title font to avoid an unmaintained font
parser; the application title is shown in the toolbar.

## M1 persistence

The storage worker has 32 request and 32 result slots. Scene mutations are
validated, journaled and synced before the UI receives their command. Originals
are copied into private recovery storage before acknowledgment, so external source
changes cannot destroy an acknowledged image. Scene metadata is duplicated in the
worker to validate commands and provide checkpoints without blocking frames.

The journal is the durable original store. Private `.source` files are working
copies reconstructed when opening/recovering a board; they need no separate
`fsync`. Foreign imports reuse their first working copy. A new blob and its insert
share one journal sync, and opening/foreign import sync all pending records with the final checkpoint.
Neither path acknowledges anything before that sync; append/sync failures poison
the worker. Clipboard staging still syncs its original before handing it to the
storage worker. No on-disk format or admission budget changes.

Each explicit save appends missing original blobs and one atomic checkpoint.
Three immutable backups per destination survive session discard. Compaction uses
a sibling temporary object and atomic replacement. Recovery, recent-file discovery
and source extraction run on the worker. The path field, recent/recovery buttons,
Save/Open and undo/redo controls provide pointer paths for all new operations.
Undo/redo retains 256 commands per session; it is not restored from disk.

RGBA8 preview uploads remain the portable baseline; the runtime image format is
chosen from adapter capabilities (BC7 on desktop, RGBA8 fallback, ASTC planned
for M8). Board I/O never runs on the event thread.
See [M0 validation](M0-VALIDATION.md), [M1 validation](M1-VALIDATION.md) and the
[debt backlog](TECH-DEBT.md) for historical measurements, executed checks and
remaining gates. M0's earlier volatile-canvas and raw-staging descriptions are
historical, not the current persistence contract.

GIF previews decode only the first frame, row by row, including interlace passes
and transparent offsets. BMP previews reuse the existing codec's palette/bitfield
conversion on bounded single-row records; supported DIB headers are 12, 40, 52,
56, 108 and 124 bytes. Pixel offsets are capped at 64 KiB. RLE/JPEG/PNG-compressed
BMPs are rejected rather than routed through full-frame decoding. Both feed the
same alpha-weighted area accumulator as PNG/clipboard previews.

WebP uses the maintainer-approved full-frame exception in
[ADR 0003](decisions/0003-bounded-webp-previews.md): static files only, encoded input
and full RGBA frame each at most 8 MiB. Embedded VP8/VP8L dimensions are validated
against the canvas before pixel decoding. Area averaging and EXIF orientation
produce the preview, and the full frame is immediately released. These are hard
input/frame limits, not a cap on every private allocation inside the dependency.

## Reference workspace

The desktop shell parks up to four independent `Session`s, each with its own
journal, scene, decode requests and camera. Switching clears image residency;
inactive completions are drained without GPU uploads. Destination leases prevent
two sessions in this process from writing the same canonical native path.

An independent library worker indexes at most twenty recent and twenty pinned
boards. It produces four 128-pixel previews per entry (at most 10 MiB of egui
textures). Leaving the library drops those handles. Pins and bounded tag metadata
use atomic replacement in app-owned storage. Core crates still see byte streams
and opaque handles, never native paths.

Overlay mode composites the complete board once, then applies opacity. Its
registry-managed intermediate texture has a separate 64 MiB ceiling and is freed
on renderer clear. A separate interactive controller restores the main window's
input and pin state on exit. Native capability flags disable unsupported actions.
See [decision 0004](decisions/0004-reference-workspace.md).
