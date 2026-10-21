# 0015 — Viewport scheduling and native tiled detail

## Context

Whole-image texture classes stop at 2048 px and retain square allocations for
elongated images. FIFO preview requests decode obsolete zoom steps before the
final view. Full-frame JPEG output cannot provide bounded tile pixels.

## Decision

Keep a 256-pixel whole-image fallback for large images, and use fixed 256×256
atlas layers for every finer pyramid level, including one-pixel gutters. Small
originals keep the existing whole-image preview path up to 2048 px. Only current/target viewport
coverage is resident. Use source-space pyramid coordinates and derive cache keys
from the original blob hash, level and tile coordinates. Reuse the existing GPU
registry, RAM/disk budgets, LRU and checksummed disposable cache encoding.

Fit the current/target tile working set to whole-page GPU capacity, reserving
visible preview/fallback pages. Coarsen detail under pressure and restore it when
the ceiling rises. At capacity, replace the oldest matching layer rather than
blanking every image in a texture page.

Replace the bounded viewport queue each frame, with visible-area and center-distance
priorities and deterministic ties. Imports stay non-cancellable. Request the final
camera target during interpolation; cancel obsolete work cooperatively and reject
stale completions before upload. Idle workers wait on a condition variable. Eight completion slots let a tile
batch finish between UI frames without blocking after its second result.

Use mozjpeg 0.10.13 (IJG) for native JPEG scanlines, with default features disabled
(no NASM requirement). Its safe wrapper contains C-unwind failures inside the
existing catch_unwind boundary. Validate headers before allocating and limit
progressive coefficient planes. Preserve EXIF orientation and CMYK conversion.
The Android core check installs an NDK for this C dependency; the WASM dependency
graph excludes it. Browser JPEG tiles use the Worker codec planned for M8.

## Consequences

Full-detail pixel output and residency follow visible tiles rather than original
image area; missing tiles preserve a preview without double blending alpha.
Sequential compressed sources still require scanning earlier rows. Up to eight
neighboring tiles at one level share a read and codec pass; concurrent batches
for the same source are serialized. Reduced JPEG levels use scaled IDCT, while
1:1 copies exact samples without area sums. Coarse fallback requests bypass
whole-preview decode headroom. Later cold batches can re-read the original; warm tiles reuse the RAM/disk cache. WebP retains its
existing small full-frame limit. Progressive JPEG can require full coefficient
planes, which are admitted separately from output pixels. Neither original image
bytes nor the .moth schema change.

Native builds need a C compiler; Android builds need the NDK. Tier-3 performance
and long reliability qualification remain CI/maintainer release gates.
