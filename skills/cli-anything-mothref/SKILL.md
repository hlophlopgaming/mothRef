---
name: cli-anything-mothref
description: Create, inspect, arrange, annotate and export MothRef reference boards using the native mothref binary, ordinary subcommands and a stateful JSON-lines transport.
---

# Operate MothRef boards

Use the installed `mothref` executable. The same binary contains the GUI, native
subcommands and `--agent`. From a source checkout build `cargo build -p mothref
--release` and use `target/release/mothref` (Windows: `mothref.exe`).

Start with `mothref commands --json` for the operation catalog, GUI-action coverage
and this embedded guide. Read `mothref COMMAND --help` for exact flags. Ordinary
commands auto-save successful board edits; query commands leave board files alone.

```sh
mothref create references.moth
mothref add references.moth image-a.jpg image-b.png
mothref arrange references.moth --mode grid --gap 24
mothref note references.moth "Lighting study" --at 0,-100
mothref info references.moth --json
mothref export references.moth preview.png --side 1024
```

Coordinates are absolute board units: X right, Y down, independent of GUI pan/zoom.
`rect` is the unrotated frame, `bounds` its rotated envelope, and `center` its center.
Use visible bounds to space rotated items. New images start at original pixel size.
Default add/note placement is beside board bounds with a 24-unit gap; default
copies move beyond the right edge, preserving relative positions/Y. Explicit zero
positions/deltas permit overlap. Insertions preserve existing item positions.

Read UUIDs and geometry before edits. `info` is paginated at 1000 items; continue
with `--offset 1000 --limit 1000`. Global bounds cover every item. Use `--id` with
UUIDs to scope operations; omitted ids means the board for geometry/export.
Delete/group/ungroup/order require nonempty ids. Selected groups carry members.
Move takes a relative `--by dx,dy`; edit `--rect x,y,w,h` is absolute. Angles are
clockwise radians. To place B right of A, derive from their visible bounds:
`delta=[A.x+A.width+gap-B.x, A.y-B.y]`, then move B (or its group UUID).

```sh
mothref move references.moth --id UUID --by 100,-50
mothref edit references.moth --id UUID --rotation 0.3 --flip true,false
mothref group references.moth --id UUID_A UUID_B --title Lighting --color 2
mothref check references.moth --json
mothref preview references.moth ./previews --side 1024 --json
mothref preview-latest ./previews --json
```

Mutation replies return `changed` geometry (up to 1000 surviving affected objects),
`changed_total` and board bounds; inserts also return `id` and `item`. Layout grid
and optimal account for rotated envelopes. Capture a preview after layout before
making visual corrections. Bundle artifact `coordinates` and summary `preview`
provide actual `pixel_size`, `board_to_pixel` and `pixel_to_board`: per axis,
`output = input * scale + offset`. This includes native aspect-ratio padding and
is specific to that capture. Pixel [u,v] has center [u+0.5,v+0.5].

Use `mothref batch workflow.json` for JSON operation arrays (or `{ops,dry_run}`),
or `mothref batch -` for stdin. Batch saves after success. The raw operation name
for arrange is `layout`; JSON parameters use arrays, not comma-separated strings.
Use `mothref --agent` for multiple JSON-lines requests in one process:

```json
{"ops":[{"op":"create","path":"references.moth"},{"op":"add","path":"image.jpg"}]}
{"ops":[{"op":"layout","mode":"grid","gap":24},{"op":"save"},{"op":"info"}]}
```

Raw transport saves only on explicit `save`. Its undo/redo history lasts for that
process; batch can also undo earlier operations in the same invocation. One-shot
undo/redo is rejected. Scripts have at most 1000 operations and 4 MiB of JSON.
Check `ok` and the process exit status. A raw native error ends that session.

Use `--dry-run` to validate without saving board files; it still uses private
staging, but exports/preview captures are refused. `.bee`/`.pur` conversion requires
an explicit native destination, e.g. `mothref open legacy.bee -o converted.moth`.
HTTP(S) imports require explicit `add --url URL` or a raw add with `url`; the native
downloader enforces size, redirects and a total timeout. Source URLs/paths are
not persisted, so source-path sorting remains unavailable.

Scripts are sequential, not transactions: earlier explicit saves/exports remain
if a later operation fails; unsaved acknowledged edits remain in GUI recovery.
Updated processes share board writer locks. Older app versions/different recovery
roots do not share that protection. Never bypass a busy-board error.

Preview is a static `preview-bundle/v1` capture from the native PNG exporter.
Optional `cli-hub previews inspect BUNDLE_DIR` only reads it. No live GUI control,
live/diff/poll preview or automated recovery selection is provided. Scene export
PNG/SVG/PDF and color sampling use bounded source previews; original/originals/ZIP
exports preserve encoded source bytes. Read `docs/CLI.md` in the checkout for the
full protocol. Ordinary file arguments (`mothref board.moth`) launch the GUI.
