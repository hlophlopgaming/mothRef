# 0016 — Baked tile packs, parent-tile stand-ins and a leaner RAM cache

Date: 2026-10-21

## Context

After decision 0015, zooming into a large image still decoded the original for every
cold tile batch. A sequential JPEG cannot seek to a row, so a batch near the bottom
of a photo re-reads and re-runs IDCT on every row above it. While that ran, missing
tiles were drawn from the 256-pixel whole-image fallback, which looks like a smear of
pixels during a fast zoom. Meanwhile the default "In memory" mode let the RAM cache
grow to 25% of available memory (up to 4 GiB), and nothing came back on zoom-out.

The maintainer asked for zoom that shows images at their own resolution right away,
without the app holding much more RAM.

## Decision

- **Tile packs.** The first time a viewport shows an image the renderer draws from
  tiles (longer side > 2048 px), the decode pool offers it to a single background
  `lod-bake` thread. The bake decodes the original once and streams rows through the
  existing tile accumulators: an accumulator exists only while its tile's rows are
  being read, so memory follows one band of tiles per level (capped at 192 MiB),
  not the image area. Every tile of every tiled level is encoded to the GPU format
  and written to one pack file per image and format: fixed-size payloads, an index
  sorted by tile, and a trailer with the content hash. Each tile has a blake3 digest;
  the file is untrusted on read and any mismatch is a miss. Interlaced codecs, whose
  rows arrive out of order, are rejected and keep the on-demand path.
- **Tile reads.** A viewport tile request is answered from the pack before the RAM
  or disk caches and before the original is opened. Reads go through the OS file
  cache, so warm tiles cost a short read without growing the process's RAM.
- **Parent stand-ins.** While a tile loads, the renderer draws the matching region
  of the nearest resident coarser tile (up to `PARENT_LEVELS` = 4 levels up) through
  the continuous sampling path, instead of the 256-pixel fallback. Tile edges are
  multiples of the 254-pixel core at every level, so a child never straddles two
  parents. The viewport keeps those ancestors resident; they are the oldest layers
  and are the first replaced when the tile page budget is reached.
- **Memory mode is write-through.** "In memory" also writes each pyramid to the disk
  cache and falls back to it on a RAM miss, so eviction costs one file read rather
  than a decode. The RAM budget drops to 12.5% of available RAM, at most 1 GiB, and
  whole-image previews decode with 2× instead of 4× headroom.
- The disk budget rises from 2 GiB to 8 GiB, because packs hold every detail tile
  (about a third of the pixel count in BC7). A single entry stays under a quarter of
  the budget. Packs are ordinary `.lod` entries: LRU pruning, Settings usage and
  "Clear disk cache" cover them.
- Packs are written only in the "In memory" and "Cache on disk" modes. "No cache"
  and "low zoom quality" keep their previous behavior.
- **Bake preference** ("Prepare zoom tiles"): *For images on screen* (default) bakes
  an image the first time it is visible; *For the whole board* also queues every
  original of the open board behind on-screen work, so zoom is ready before the user
  visits an image; *Off* writes no new packs but still reads existing ones. The
  queue holds at most 4096 jobs, on-screen offers jump to the front (evicting board
  work when full), and the set of remembered offers is trimmed at the same size.
  Board passes skip images that fit a whole preview after reading their header.
  Settings show how many images are still waiting.

## Consequences

- After a large image has been on screen briefly, zooming to any level reads ready
  tiles; only the first zoom into an image that was never shown waits for decodes.
- A bake costs one full decode and BC7 encoding on one background thread per board
  tab. It is cancelled when the tab's decode pool closes and never blocks viewport
  work. "For the whole board" reads every original of the board once in the
  background; that costs disk reads and space, so it is not the default.
- Disk usage grows with large originals; the budget and LRU pruning bound it.
- ARM builds bake RGBA8 tiles, four times larger than BC7, until ASTC/ETC2 (M8).
- No `.moth` format change and no new dependency.

Validation: per maintainer instruction, nothing was built or run in the authoring
session; formatting was applied with rustfmt and the workspace checks run in CI.
Native zoom latency and RSS measurements remain the open M2.5 item.
