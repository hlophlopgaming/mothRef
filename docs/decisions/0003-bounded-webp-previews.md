# 0003 — Bounded full-frame WebP previews

Date: 2026-10-16

## Context

TD-07 calls for WebP import. The available pure-Rust `image-webp` decoder exposes
full-frame output and no reduced/row-output API. AGENTS.md normally forbids
full-resolution preview decoding. The maintainer explicitly permits a documented,
limited WebP exception with rejection of oversized files.

## Decision

Permit static WebP only with both of these admission limits checked before
allocating a frame:

- Encoded input at most 8 MiB.
- Source width × height × 4 at most 8 MiB (2,097,152 pixels), including opaque RGB.

The ordinary 32,768 per-axis limit also applies. Validate embedded VP8/VP8L
frame dimensions against the outer canvas before decoding, so an extended header
cannot disguise a larger frame. Reject animation. Decode on the existing bounded
worker pool under `catch_unwind`, area-average into the requested preview, apply
EXIF orientation, and release the source. Do not cache full frames.

These are hard input/frame admission limits, not an allocator-level cap on all
private codec allocations. The dependency's `set_limits` does not enforce every
internal allocation; do not claim that its `max_alloc` guarantees a process RSS
ceiling. The representative largest admitted fixture is checked with dhat for
less than 64 MiB additional peak allocations; image-reader fuzzing also exercises
malformed data. A strict global allocation cap would require upstream decoder
support or a separately designed isolation boundary.

## Consequences

Small/medium lossy and lossless WebP, alpha and EXIF orientation work without a
native dependency, including Android core checks. A 4K WebP is rejected with a
budget error even if the encoded file is tiny. GIF/BMP retain row-based decoding;
this exception does not permit a generic full-resolution fallback or change PNG,
JPEG, clipboard staging, persistence, or GPU budgets.
