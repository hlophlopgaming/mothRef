# Architecture

`mothref` owns the event loop and wires together independent crates:

- `moth-scene`: pixel-free image metadata, UUIDv7 identity, fractional ordering,
  reversible commands and an R-tree. Commands return their inverse; the storage
  worker journals undo/redo before acknowledgment.
- `moth-format`: byte-stream storage contracts, opaque handles, schema-one
  records, checkpoint replay and atomic compaction. See [FORMAT.md](FORMAT.md).
- `moth-io`: bounded Rayon decode work, header checks and panic isolation. JPEG
  uses DCT scaling; PNG is downsampled from scanlines, including Adam7 passes.
  Original dimensions determine geometry; pixels live only in worker results
  and the renderer. Preview pyramids stop at 2048 pixels on the long side.
- `moth-platform`: native path resolution, clipboard access and durable staging.
  Filesystem and clipboard calls run in workers. Registration on the event
  thread records a path without opening it. Imported files remain external
  sources in M0; changed/missing files produce an error instead of a reload loop.
- `moth-render`: instanced quads and a GPU resource registry, shared by the window
  and headless checks. Texture pages use size classes, full mip pyramids and
  page-level LRU eviction. Original bytes do not depend on a GPU texture format.
- `moth-ui`: theme tokens, bundled licensed font, localized string lookup and
  responsive wrapped controls. Pan/select, fit, zoom and paste have visible
  controls for touch input.

The native input adapter is the only app code consuming raw mouse events.
Normalized pointers carry identity, kind and pressure; two-contact pan/pinch is
implemented by a separate gesture layer. Pen identity is represented by the API;
actual pen-device classification is limited by winit 0.30 and still needs device
validation.

## Budgets and lifetimes

| Resource | Bound |
| --- | --- |
| Board metadata / import backlog | 10,000 items |
| Texture arrays, including every layer and mip | 96 MiB |
| Instance buffer | 10,000 instances, 320,000 bytes |
| Decode requests / completed images | 8 / 2 |
| Concurrent decode consumers | min(cores − 1, 4), at least 1 |
| Rayon pool threads | cores − 1, at least 1 |
| Compressed source / raw scratch input | 64 MiB |
| Header dimensions / pixel count | 32,768 per side / 32 megapixels |
| PNG decoder internal allocations | 8 MiB |
| Clipboard requests / results | 1 / 1 |
| Clipboard staging directory | 512 MiB and 10,000 entries |

The decode concurrency cap bounds temporary pixels independently of CPU count.
Texture lookup entries exist only for resident slots. Texture pages and lookup
entries are evicted together; debug teardown checks allocation accounting returns
to zero. There is no retained CPU pixel cache.

The clipboard worker stages original RGBA pixels before reporting success and
passes an already-built preview with the result, avoiding a second read. Recovery
files are private to the user on Unix and never overwrite existing files. The
staging budget rejects new pastes instead of evicting originals. Cleanup and a
recovery browser are deferred to M1; users can reopen a scratch `.rgba` record as
an image via a CLI argument. These records do **not** preserve board layout.

The event loop waits between input, worker completions and requested UI animation
frames. Zero-sized windows do not render. Surface loss, suspend/resume and device
loss replace GPU/UI resources while retaining scene metadata, camera and source
handles. Adapter creation retries a fallback adapter; `--software` forces it.
RGBA8 is the baseline portable texture format; compressed formats remain M1.

## M0 limits

M0 is a volatile canvas preview, not the reliability release. Closing a nonempty
board requires explicit discard confirmation. Saving, write-ahead scene journaling,
undo/redo UI, crash recovery and schema migrations remain M1. Original source
files must stay available while the canvas is open. Large-image visible tiles and
full-resolution inspection beyond the preview limit are not implemented.

winit 0.30 does not deliver native Wayland file drops. Use an X11/XWayland session
for file drag-and-drop; CLI image arguments and the paste button are alternatives.
Clipboard interoperability and pen pressure need manual testing on each desktop.
The Wayland decoration backend omits its title font to avoid an unmaintained font
parser; the application title is shown in the toolbar.

## M1 persistence

The storage worker has 32 request and 32 result slots. Scene mutations are
validated, journaled and synced before the UI receives their command. Originals
are copied into private recovery storage before acknowledgment, so external source
changes cannot destroy an acknowledged image. Scene metadata is duplicated in the
worker to validate commands and provide checkpoints without blocking frames.

Each explicit save appends missing original blobs and one atomic checkpoint.
Three immutable backups per destination survive session discard. Compaction uses
a sibling temporary object and atomic replacement. Recovery, recent-file discovery
and source extraction run on the worker. The path field, recent/recovery buttons,
Save/Open and undo/redo controls provide pointer paths for all new operations.
Undo/redo retains 256 commands per session; it is not restored from disk.

The M0 limits above describe the original preview. M1 adds persistence but retains
the 10,000-item UI/render limit and RGBA8 preview uploads; runtime compressed
texture support remains an open release gate. Board I/O never runs on the event
thread. See [M1 validation](M1-VALIDATION.md) for executed checks and remaining gates.
