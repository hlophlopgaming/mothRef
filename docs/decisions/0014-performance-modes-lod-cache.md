# 0014 — Performance modes and a rebuildable LOD cache

Date: 2026-10-20

## Context

Every zoom step that crossed a preview size class re-read the original from the
journal, re-hashed it, decoded it, rebuilt the mip pyramid and re-encoded every
mip to BC7. Nothing from earlier decodes was kept ("there is no retained CPU pixel
cache"), so zooming into a board repeated the most expensive work in the app.

PureRef exposes this trade-off as a user setting ("Performance mode": No cache,
Cache on disk, In memory, In memory with low zoom quality). The maintainer asked for
the same choice in MothRef, with **In memory** as the default, so zoom speed can
beat PureRef while users with little RAM can still pick a lean mode.

## Decision

Add `moth_io::LodCache`, shared by every board tab's decode pool, and a persisted
`performance_mode` preference with four modes:

| Mode | Kept between zoom steps | Detail cap |
| --- | --- | --- |
| In memory (default) | GPU-ready pyramids in RAM | 2048 px |
| In memory, low zoom quality | GPU-ready pyramids in RAM | 1024 px |
| Cache on disk | GPU-ready pyramids in the OS cache folder | 2048 px |
| No cache | nothing (previous behavior) | 2048 px |

- Preview requests carry the item's content hash, so a cache hit never opens the
  original. Imports still learn the hash from the bytes and keep their 256-pixel
  decode, so bulk import and board opening do not slow down.
- On a miss, caching modes decode at `max(4 × requested side, 256)` (capped by the
  mode) and serve later zoom steps by slicing that pyramid. Across the whole zoom
  range an image is decoded at most a few times instead of once per size class.
- The slice chosen for a request always satisfies the renderer's residency rule
  (size class ≥ requested side, or the original size), so a hit can never cause a
  repeated request. The quality modes also require the chosen mip to carry the
  requested detail; the low-quality mode only requires the class.
- The RAM part has an explicit budget with LRU eviction: 25% of available RAM plus
  its own bytes, at most 4 GiB, 512 MiB when RAM is unknown, and zero in the other
  modes (switching mode releases it immediately).
- The disk part lives behind the `LodStore` trait; `moth-platform::lod_store`
  stores one file per image and GPU format in the OS cache folder, writes through a
  temporary file and rename, and prunes least recently used files to 90% of a
  2 GiB budget on the decode workers. Files are untrusted on read: sizes are
  validated before allocation, each mip carries a blake3 digest, and damaged files
  are misses. Entries are disposable and never part of a `.moth` file.
- `pick_image_format` now selects BC7 only on x86/x86_64 builds, where the encoder
  exists. Apple silicon and Windows on Arm adapters can report BC support; there the
  decoded RGBA8 previews would never match the registry format and never upload.

## Consequences

- Zooming into images that were already shown no longer decodes in the default
  mode; cache hits cost one memory copy (RAM) or one bounded file read (disk).
- RAM use in the default mode is higher than before, by design and within the
  budget above. "No cache" keeps the previous memory profile.
- The GPU registry is unchanged: zooming out still keeps larger resident textures
  until LRU eviction. Downgrading on zoom-out, request priorities/cancellation and
  tiled pyramids are follow-ups in M2.5 of the roadmap.
- No `.moth` format change, no new dependency.

Validation: per maintainer instruction, nothing was built or run in the authoring
session; formatting was applied with rustfmt and the workspace checks run in CI.
Native measurements (zoom latency with a warm cache, RSS per mode) remain open.
