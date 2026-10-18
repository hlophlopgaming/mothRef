# M1: runtime texture-format selection (BC7 / RGBA8)

## Context

The M1 portability milestone requires the compressed texture format to be chosen
at runtime from adapter capabilities, and LOD caches in `.moth` to stay
rebuildable for any format. The full matrix is BC7 (desktop), ASTC (mobile),
ETC2 (legacy mobile) and uncompressed RGBA8 (universal fallback).

Rust's BC7 encoder options are thin: `texpresso` implements only BC1–BC5, and
the state-of-the-art encoder `intel_tex_2` ships ISPC kernels that build only for
x86/x86_64. No portable, battle-tested pure-Rust BC7 encoder exists. The mobile
build is M8+, so there is no ARM adapter or ASTC encoder to exercise today.

## Decision

Pick the image format once at device creation from `TEXTURE_COMPRESSION_BC`:
BC7 when the adapter reports it, RGBA8 otherwise. Encode BC7 on the decode pool
in `moth-io` via `intel_tex_2`, declared as a target-gated dependency so the
`aarch64-linux-android` core check keeps building (there, only RGBA8 is ever
produced). ASTC is deferred to M8 with a portable ARM `astcenc` encoder; ETC2 is
dropped. The `.moth` format remains GPU-format independent: it persists originals
and hashes, never a GPU-encoded LOD cache, so any format can be rebuilt.

## Consequences

Desktop texture residency drops 4× (1 byte/texel vs 4). The x86 gate keeps the
Android core build clean without scattering `#[cfg(target_os)]` across crates.
BC7 is lossy, so golden images use a wider tolerance and gradients can band;
encode runs off the main thread on the decode pool. Adding ASTC later is one
enum variant, one encoder dependency and one capability probe — the selection
and upload paths are already format-agnostic.
