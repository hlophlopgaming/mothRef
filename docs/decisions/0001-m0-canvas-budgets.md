# 0001 — Bound M0 previews and stage clipboard originals

Date: 2026-10-16

## Context

M0 establishes rendering and interaction before M1 adds board persistence. Large
boards must not retain decoded originals or allocate one GPU texture per item.
Clipboard pixels also need a stable source after their first preview is evicted.

## Decision

Use 16–2048-pixel, paged RGBA8 texture arrays with a 96 MiB allocation budget and
LRU page eviction. Keep image bytes out of the scene. Decode behind opaque storage
handles with bounded queues and at most four simultaneous decode consumers.

Stream PNG previews from scanlines and use JPEG DCT reductions. Preserve clipboard
RGBA originals as versioned, checksummed scratch records on a worker, fsync them
before acknowledging success, and send the preview with the completion. Refuse a
paste if the scratch budget is full. Never evict an original to improve latency.

M0 advertises that layouts are not saved and confirms discarding nonempty boards.
The release reliability promise starts in M1 with a write-ahead scene journal.
This sequencing follows the existing roadmap; it is not permission to ship M1
without durability.

## Consequences

Memory and GPU allocation stay bounded, but previews cap detail at 2048 pixels.
Imported source files must remain available. Scratch records preserve clipboard
pixels, not scene layout. The 4K paste latency target remains open when durable
staging exceeds 50 ms. End-to-end OS clipboard latency needs platform measurements.

The scratch layout is `MOTHRG01` (8 bytes), little-endian u32 width and height,
a 32-byte BLAKE3 checksum of tightly packed RGBA8 bytes, then those bytes. Readers
validate length, dimensions and checksum before allocating a preview. This is a
cache/source record, not a `.moth` board schema. Future changes must retain the
ability to read this record version while recovery records may still exist.
