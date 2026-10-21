# 0017 — Full-resolution mode and video-memory limits from the adapter

Date: 2026-10-22

## Context

Decisions 0014–0016 made zoom loading fast, but it still happened: a tile that was
not resident yet appeared a moment later, first as a coarser stand-in. The maintainer
asked for a mode with no loading at all while working — every image in memory at full
resolution, as PureRef's "In memory" mode behaves — and for it to be the default. The
texture cache was also capped at 32–512 MiB (96 MiB by default) regardless of the
graphics card, far below what modern adapters have.

## Decision

- **`CacheMode::Full` ("Everything at full resolution"), the new default.** When a
  board opens, a preloader (worker threads per board, one core left for the UI)
  prepares every image: the 256-pixel preview (the whole image when smaller) and every
  detail tile of every level. Images larger than one tile are drawn from tiles in this
  mode (`CacheMode::tile_threshold` = 256), so no image wastes a square power-of-two
  texture and the GPU memory used is about 1.36× the pixel count (BC7: one byte per
  pixel). Results stream through a bounded channel; the UI uploads them a few
  milliseconds per frame.
- **Nothing is released afterwards.** `Registry::keep_all` disables tile release on
  pan and texture shrinking on zoom-out. While the board loads it is covered by a
  progress bar and no viewport requests are scheduled, so the user first sees the
  finished board. Images added later are preloaded without covering the board.
- **Overflow.** Before each upload the UI asks `Registry::fits`. The first image that
  would exceed the limit stops the pass and shows a warning; `keep_all` is turned off
  and the rest of the board uses the on-demand tiles of decision 0016.
- **Reopening is cheap.** Previews come from the disk cache and tiles from the tile
  packs of decision 0016, which the preload writes when they are missing. A second
  opening reads GPU-ready data instead of decoding originals.
- **Video-memory limits from the adapter.** `moth-platform::gpu` detects memory on a
  worker thread: sysfs (`mem_info_vram_total`, `lmem_total_bytes`) and `nvidia-smi` on
  Linux; `nvidia-smi` and the display-adapter registry class
  (`HardwareInformation.qwMemorySize`, via `reg query`) on Windows; shared RAM on macOS
  and integrated adapters. The limit is 80% of dedicated VRAM (at least 512 MiB left
  free), half of RAM for shared-memory adapters, a quarter for software rendering,
  4 GiB for a discrete adapter whose memory could not be read. Shared-memory adapters
  also stay 1 GiB below free RAM. The result is shown in the status line on start and
  in Settings, where the user can set a manual limit (256 MiB up to the adapter's
  memory). The old `cache_limit_mib` preference is ignored. `MAX_TEXTURE_BUDGET` is now
  only the default hard limit until the detected one is set.
- Preloading waits for detection, so it never runs against a placeholder limit.
- AGENTS.md records the exception to "memory bounded by the viewport" and the rule
  that video-memory limits follow the hardware.

## Consequences

- Zooming and panning load nothing once a board is open in the default mode.
- Opening a board costs a decode and BC7 encoding of every original the first time,
  in parallel; later openings mostly read files. The progress bar shows how far it is.
- Video-memory use grows with the board up to the limit. RAM caches stay bounded;
  the preload holds at most 256 prepared tiles in flight and at most 1 GiB of decoded
  frames across its workers (see the update below).
- Other performance modes are unchanged apart from the larger, hardware-based limit.
- No `.moth` format change, no new dependency; detection spawns `nvidia-smi` or `reg`
  where present and falls back silently.

## Update 2026-10-22: faster loading

First openings were slow even for 100 images. Each image was decoded twice (preview,
then tiles), every level was area-averaged from the full-resolution source (one pass
per level), and BC7 used the alpha "very fast" preset for opaque photos. Reopening
uploaded through a 64-image channel and searched texture pages from the first one,
which grows quadratically with the number of tiles.

- `tiles::bake` decodes once into an oriented RGBA frame and builds each level from
  the previous one by alpha-weighted 2×2 averaging (≈1.33 passes in total); the same
  pass yields the 256-pixel preview. Level 0 is identical to on-demand tiles; coarser
  levels differ by rounding only. Interlaced images now bake too.
- Frames are bounded per pool: workers lease their frame size from a 1 GiB budget
  (a larger frame runs alone). Worker count follows the CPU up to 32.
- BC7 uses `opaque_ultra_fast` for fully opaque surfaces and `alpha_ultra_fast`
  otherwise.
- The result channel holds 256 tiles (16 MiB in BC7); texture pages are searched from
  the newest, which is the one still filling.

Validation: per maintainer instruction, nothing was built or run in the authoring
session; formatting was applied with rustfmt and the workspace checks run in CI.
Opening time and VRAM use for large boards remain to be measured on real hardware.
