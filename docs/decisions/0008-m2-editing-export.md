# 0008 — M2 appearance, source names and bounded export

## Context

M2 needs undoable opacity/grayscale, filename arrangement, crop controls and export.
The scene already represents frame/content geometry; appearance must survive journals,
checkpoints, undo and old-board migration. Export must not block the UI or overwrite work.

## Decision

Schema 4 extends `Transform` with opacity and grayscale and gives items a bounded
source filename (metadata, never a native path). New insert/transform tags retain
all schema 1–3 decoders. Migration defaults to opaque color and an empty filename.
Saving an old board first atomically compacts it to schema 4 through the storage trait.

The inspector emits existing `SetRect`/`SetTransform` commands as one undo step only
on Apply. Arrangement reuses the existing geometry routines with a configurable gap.
Source filenames are captured for newly dropped/opened images; older/imported boards
without names use stable identity as their tie breaker.

PNG and SVG scene export run on the bounded storage worker. SVG describes transforms,
clips, text and embedded PNG previews; resvg rasterizes the same document to PNG.
This avoids a second custom compositor and includes notes in scene export. The parser
receives only generated SVG, never user SVG or external links. Source decoding uses
the existing checked, panic-isolated decoder. A single export is capped at 4096 pixels
on its longest side, 64 MiB of generated SVG, and 2048-pixel source previews. Original
file exports copy the unchanged encoded bytes and retain full resolution. Destination
files use exclusive creation; original collections require a fresh directory. Failures
can leave a partial new export, but cannot replace an existing file or mutate a board.
Clipboard ownership lives on the worker for the tab's lifetime.

## Consequences

Scene/clipboard exports and color sampling currently have preview resolution; this
limit is visible in the export controls. Full-resolution tiled export is a later
extension of the existing tiled decode work, rather than an unbounded full decode.
Rasterized notes use the already attributed Noto Sans font. resvg/base64 are licensed
through cargo-deny. No renderer/export cache is introduced. Windows/macOS clipboard
and touch hardware qualification still require native manual checks.
