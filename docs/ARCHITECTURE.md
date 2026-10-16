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
| App board admission / waiting import backlog | 1,000,000 items / 1,024 handles |
| Desktop source catalog | 1,000,000 handles |
| Texture arrays, including every layer and mip | 96 MiB |
| Instance buffer | 10,000 instances, 320,000 bytes, reused between ordered batch submissions |
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

The clipboard worker stages original pixels as lossless PNG before reporting
success and passes an already-built preview with the result. The preview hash
matches the staged PNG, so eviction reloads the same content. Recovery files are
private to the user on Unix and never overwrite existing files. Staging rejects
new pastes when its budget is full; it never evicts originals. Native clipboard
file lists enter the same bounded import path as file drops. Unsupported/empty
payloads and saturated queues have localized feedback. Remote URL/HTML import is
still open; it performs no implicit network fetch.

Explicit discard removes only this board's staged originals and journal; crashes
retain them for the recovery browser. Legacy checksummed `.rgba` scratch records
remain readable, but do not preserve layout by themselves.

The event loop waits between input, worker completions and requested UI animation
frames. Zero-sized windows do not render. Surface loss, suspend/resume and device
loss replace GPU/UI resources while retaining scene metadata, camera and source
handles. Adapter creation retries a fallback adapter; `--software` forces it.
RGBA8 is the baseline portable texture format; compressed formats remain M1.

## Current limits

Large-image visible tiles and full-resolution inspection beyond the 2048-pixel
preview limit are not implemented. Rendering uses fixed-size ordered batches
instead of dropping items beyond the first 10,000. App admission, import backlog,
source catalog, input-byte, journal and snapshot budgets still apply; this does
not promise that every million-item board fits a 64 MiB checkpoint.

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

RGBA8 preview uploads remain the portable baseline; runtime compressed texture
support is an open release gate. Board I/O never runs on the event thread.
See [M0 validation](M0-VALIDATION.md), [M1 validation](M1-VALIDATION.md) and the
[debt backlog](TECH-DEBT.md) for historical measurements, executed checks and
remaining gates. M0's earlier volatile-canvas and raw-staging descriptions are
historical, not the current persistence contract.
