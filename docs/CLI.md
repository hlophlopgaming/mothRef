# MothRef native CLI

The installed `mothref` binary contains the GUI, ordinary headless subcommands and
a stateful JSON-lines interface. No extra runtime or wrapper is required. From a
checkout build `cargo build -p mothref --release`; the executable is
`target/release/mothref` (`mothref.exe` on Windows).

```sh
mothref create references.moth
mothref add references.moth image-a.jpg image-b.png
mothref arrange references.moth --mode grid --gap 24
mothref note references.moth "Lighting references" --at 0,-100
mothref info references.moth --json
mothref export references.moth preview.png --side 1200
```

`mothref` alone or with ordinary file arguments launches the GUI. A recognized
subcommand or CLI flag selects headless mode before any window/GPU initialization.
`mothref --help` lists commands; `mothref COMMAND --help` documents flags. A file
whose name matches a command can be opened as `mothref ./info`, for example.
`--software` remains a GUI option. Standard shell quoting applies to paths/text.

## Global options

| Option | Meaning |
| --- | --- |
| `--json` | Compact machine-readable success/error reply; ordinary output is formatted JSON |
| `--dry-run` | Apply in private staging without saving boards; export/preview capture is refused |
| `-o, --output NEW.moth` | Save to a new destination, including foreign-board conversion |
| `-h, --help` | Command/flag help, exit 0 |
| `-V, --version` | Binary version, exit 0 |

Global options may precede/follow a subcommand. Paths in commands/scripts resolve
against the invoking process's working directory. CLI paths support spaces and
Unicode. Existing destinations are refused for create, Save As and exports.
Normal edits checkpoint their opened native board after successful completion.
`--dry-run` may still read originals, explicitly download URLs and use private
journal/catalog state; it does not promise zero disk I/O.

## Native commands

In the table, BOARD is the input board. With `--id`, supply one or more UUIDs
(space-separated or comma-separated). Omitted ids means all items for geometry
and export; selected groups include members. Delete/group/ungroup/order require
explicit ids. Angles are radians; points/rectangles use comma-separated numbers.

| Command | Arguments and options |
| --- | --- |
| `create NEW.moth` | Create/save an empty native board |
| `open INPUT` | Read/inspect `.moth`, `.bee` or `.pur`; `-o NEW.moth` converts/saves a native copy |
| `save BOARD [NEW.moth]` | Checkpoint BOARD, or save to a new destination |
| `add BOARD [IMAGE ...]` | Add local images; `--at x,y` is available for one image; `--group-origin` groups them by source folder |
| `add BOARD --url URL` | Explicit HTTP(S) image download; optional `--at x,y` |
| `note BOARD TEXT` | Add a note, borderless label or list with `--kind note\|label\|list`. Optional `--at x,y` |
| `stroke BOARD` | Draw a vector stroke from repeated `--point x,y`. Optional `--style pen\|marker\|arrow`, `--width N`, `--color 0xRRGGBB` |
| `info BOARD` | Inspect state; `--offset 0 --limit 1000` (page size capped at 1000) |
| `check BOARD` | Read native integrity/recovery status; recovered/torn files fail; saving the board, in place or to a new file, removes a torn tail after its originals |
| `arrange BOARD` | `--mode horizontal\|vertical\|grid\|optimal` (default grid), `--gap N`, `--order z\|name\|time\|path`, optional `--count N`, `--id ...`; alias `layout` |
| `align BOARD` | `--mode left\|right\|top\|bottom\|horizontal\|vertical\|stack`, `--id ...`; horizontal/vertical distribute |
| `normalize BOARD` | `--mode width\|height\|size\|area`, `--id ...`; match first z-sorted item |
| `scale BOARD` | `--factor N`, `--id ...`; scale sizes/positions about first selected frame |
| `move BOARD` | `--by dx,dy` (alias `--delta`), `--id ...` |
| `duplicate BOARD` | Optional `--by dx,dy`, `--id ...`; default copies sit right of the board |
| `delete BOARD` | Required `--id ...`; removes selected subtrees, children first |
| `group BOARD` | Required `--id ...`, optional `--title TEXT --color N` (defaults empty/0) |
| `snap-grid BOARD` | Optional `--id ... --step N` (default 100, range 1..10000); snap the first z-sorted displayed bounds on both axes, moving the selection together |
| `group-origin BOARD` | Optional `--id ...`; group loose images by stored origin, preserving existing groups |
| `ungroup BOARD` | Required `--id ...`; dissolve selected top groups, preserve members |
| `group-edit BOARD` | Required `--id UUID --title TEXT --color N`; replace group metadata |
| `parent BOARD` | `--id UUID --parent GROUP_UUID`; omit parent to move back onto the board |
| `order BOARD` | `--mode raise\|lower\|top\|bottom --id ...`; individual images/notes |
| `reset BOARD` | `--mode scale\|normalize\|crop\|all --id ...`; original-backed geometry |
| `edit BOARD` | Required `--id UUID`; frame/text/name and transform flags below |
| `gif-info BOARD` | `--id UUID` returns frame count and delays in milliseconds |
| `gif-frame BOARD [FRAME.png]` | `--id UUID --frame N` captures a composited frame at original resolution. Omit the PNG path to add the frame to the board. `--at x,y` only applies when adding. |
| `export BOARD DESTINATION` | `--kind scene\|original\|originals\|archive` (default scene), `--id ...`, `--side N` (default 2048), optional `--region x,y,w,h` |
| `pixel-info BOARD` | `--id UUID --point x,y`; original pixel coordinate and image metadata |
| `gamut BOARD` | `--id IMAGE [--view windows\|cells\|shares] [--sort share\|warm\|light\|hue\|saturation] [--at x,y]`; measures the visible part of the image and adds a color gamut card beside it, linked to the image by an arrow; with a gamut card's id it measures that card's image again, keeping its view and order |
| `sample BOARD` | `--id UUID --point x,y`; HEX/RGB/HSV sampled without clipboard access |
| `preview BOARD DIRECTORY` | Capture immutable native preview bundle under DIRECTORY; `--side N` defaults 1024 |
| `preview-latest DIRECTORY` | Read latest completed bundle (or pass a specific completed bundle directory) |
| `commands` | Operation catalog, actual compiled command names, all GUI-action mappings and embedded agent guide |
| `capabilities` | Protocol version and actual registered GUI action IDs |
| `batch SCRIPT.json` | Execute an array or `{ops,dry_run}` object; `-` reads stdin; autosave after success |
| `undo`, `redo` | Available as JSON operations inside a batch or live `--agent` session; standalone invocation fails because history is not loaded from files |

`edit` accepts `--rect x,y,w,h`, `--text TEXT`, `--name TEXT`, `--rotation N`,
`--opacity N`, `--grayscale [true|false]`, `--sampling 0..3`, `--note-font-size N`,
`--note-wrap [true|false]`, `--note-markup [true|false]`, `--flip true,false`, `--content-offset x,y`,
`--content-scale N`, and `--content-rotation N`. Optional booleans default to true
when the flag is present without a value. Unspecified transform fields are kept.
Use `group-edit` for group titles. `edit --name` renames images, notes and strokes.

Notes recognize `# Heading`, `- Item`, `1. Item`, `- [ ] To do` and
`- [x] Done` at the start of a line. Indent list entries by two spaces to nest them.
The editor also accepts the visible markers `•`, `□` and `▣` followed by a space.
`note --kind label` keeps literal text and omits the card backing.
`note --kind list` adds bullets to plain lines. JSON uses the optional `kind` field.
`info` returns the presentation as `transform.note_kind`. Older plain notes can opt in via `edit --note-markup`. Text, font size,
and wrapping edits reflow the card unless `--rect` sets the frame explicitly.

`stroke` takes board-space points. An arrow takes two endpoints. Pen and marker take
2–2048 points. The optional width is in board units. In JSON use
`{"op":"stroke","style":"arrow","points":[[40,40],[160,90]],"width":4,"color":14990674}`.
`info` returns the frame, normalized path points, current board-unit width and sRGB.
Use `move`, `edit`, `duplicate`, and `delete` with the stroke's UUID.

Group colors are palette codes 0..5 or tagged RGB `0x01RRGGBB` (hex accepted by
`--color`; JSON uses an integer). Layout count means rows in horizontal flow or
columns in vertical flow. Optimal ignores gap and packs tightly. `--order path`
sorts by each image's saved source: images from one folder stay together, numbers
compare by value, and images without a source (pasted, from imported boards) come
last. Item JSON carries the source as `{"file": PATH}`, `{"archive": PATH,
"entry": NAME}`, `{"url": URL}` or `null`.

GIF frames are numbered from 1. In JSON, `gif-frame` takes `id`, `frame`, and an optional `path` or `at`. A PNG export creates a new file. Adding the frame creates an independent still and leaves the GIF item on the board.

Scene export chooses PNG/SVG/PDF from the destination extension. `original`
requires one image UUID; `originals` creates a new directory; `archive` creates ZIP.
Original exports preserve source bytes. Scene rendering/sample colors use the
native bounded image previews. Requested scene side is clamped to 1..4096.

## Coordinates and avoiding accidental overlap

All edit coordinates are absolute board units, independent of GUI pan/zoom,
window dimensions and display scale. X points right, Y down; negative positions
are allowed. A new image starts at original pixel size in board units. `rect` is
the unrotated top-left frame `[x,y,width,height]`; positive rotation turns it
clockwise about its center. `bounds` encloses the rotated frame.

Default add/note placement is right of the whole board bounds, including rotated
items/group frames, with a 24-unit gap. Empty boards start at `[0,0]`. Existing
items remain in place. Default copies move beyond the board's right edge while
preserving relative positions and Y. Explicit zero at/delta permits intentional
overlap. If coordinate limits are exceeded the command fails instead of placing
content at the origin.

`move --by` is relative; `add --at` and `edit --rect` are absolute. To place B right
of A using visible bounds, calculate `dx=A.x+A.width+gap-B.x`, `dy=A.y-B.y`, then
move B's UUID (or group UUID). Grid/optimal pack displayed rotated envelopes.
Horizontal/vertical flow still uses frame dimensions, so use grid/optimal for
rotated images when overlap must be avoided.

`info` includes global `bounds` even on a paginated item response (null for an
empty board), plus coordinate conventions. Item metadata includes UUID, kind,
name, content, parent, frame rect, rotated bounds, center and all transforms.
Mutation results expose `changed` (up to 1000 surviving affected items),
`changed_total` (includes deleted UUIDs) and board bounds. Inserts also return
`id` and `item`; duplicate returns `inserted`. Use paginated info to inspect the
full result, including group backdrops indirectly changed by member edits.

## JSON scripts and stateful transport

A batch is an array of typed operations; `arrange` uses raw operation name
`layout`. Coordinates in JSON are arrays. Batch auto-saves successful changes.

```json
[
  {"op":"create","path":"references.moth"},
  {"op":"add","path":"image-a.jpg"},
  {"op":"add","url":"https://example.org/reference.png"},
  {"op":"layout","mode":"grid","gap":24},
  {"op":"note","text":"Shape study","at":[0,-100]},
  {"op":"save"},
  {"op":"export","kind":"scene","path":"preview.png","side":1024},
  {"op":"check"}
]
```

Run `mothref batch workflow.json --json`. To edit an existing board, start the
script with `{"op":"open","path":"existing.moth"}`. Native batch objects may
also be `{"ops":[...],"dry_run":true}`. Requests/scripts are limited to 4 MiB and
1..1000 operations. Unknown operations/fields and invalid values fail.

For a live process with transient undo/redo, send one JSON object per stdin line:

```sh
mothref --agent <<'JSON'
{"ops":[{"op":"open","path":"references.moth"}]}
{"ops":[{"op":"move","delta":[100,0]},{"op":"undo"},{"op":"redo"},{"op":"save"}]}
{"ops":[{"op":"info"}]}
JSON
```

Unlike ordinary subcommands/batch, raw `--agent` saves **only** on explicit save.
It accepts `{ops,dry_run}` and keeps one board per process; dry-run mode cannot
change mid-session. No window is created. Replies are one line each:
`{"protocol":1,"ok":true,"results":[...],"dry_run":false}`. Errors are
`{"protocol":1,"ok":false,"error":"...","recovery":"..."}` and end the
session with nonzero status. Normal command errors also use nonzero status;
`--json` preserves machine-readable errors, including parse failures (exit 2).

Raw operation fields are listed by `mothref commands --json`. Existing JSON
operation names/fields remain valid, including nested `edit.transform`; native
edit flags flatten those same transform fields. `info` returns items_total,
blobs_total and recovered. Pixel-info returns original pixel, size, format,
encoded byte count, name and displayed frame. Check validates native reading,
chunk/original integrity and recovery status; it does not full-decode every image.

## Previews

```sh
mothref preview references.moth ./previews --side 1024 --json
mothref preview-latest ./previews --json
cli-hub previews inspect /absolute/path/from/_bundle_dir
```

The optional cli-hub only consumes bundles. The native exporter creates PNG;
each immutable bundle has manifest.json, summary.json and artifacts/board.png.
The manifest is written last; interrupted captures are ignored by latest.
`_bundle_dir`, `_manifest_path` and `_summary_path` identify the result.

Artifact `coordinates` and summary `preview` include actual pixel_size,
board_bounds, board_to_pixel and pixel_to_board. Convert each axis as
`output=input*scale+offset`. Native SVG viewBox padding/aspect-ratio preservation
is included. Pixel [u,v] has center [u+0.5,v+0.5]. The transform applies to that
capture only; recapture after changes, and use actual pixel_size rather than the
requested side. Do not pass preview pixels directly as board positions.

Static captures are supported. No live GUI attachment, live/diff/poll preview or
automated recovery choice is implemented. Updated GUI and CLI processes refuse
concurrent access to the same board when using the same recovery root.

## Safety, networking and maintenance

All mutations use existing reversible Commands and the native storage worker;
acknowledgments follow journal sync. Checkpoints append valid commits; compaction
uses native atomic replacement. A failed script stops before its final autosave,
retaining unsaved work for GUI recovery. Earlier explicit saves/exports remain:
a script is sequential, not an all-or-nothing transaction. Successful saved/query
sessions and dry-runs close their worker before discarding private staging.

Board writer locks use persistent zero-byte sentinels in the application recovery
locks folder, keyed by canonical board path. Old app versions and different
recovery roots do not share this protection. Never bypass a busy-board error.
`MOTHREF_AGENT_DATA` selects isolated recovery/catalog storage for tests; leave it
unset in normal use. Undo history is bounded and ends when the process exits.

URL downloads are explicit actions using the same native HTTP/TLS implementation
as clipboard import: a 20-second total deadline, bounded redirects, no HTTPS
downgrade, and a 256 MiB compressed-input cap. Header/decoder limits also apply.
A save writes its checkpoint back over history and torn bytes after the last original and cuts them off; torn bytes among the originals remain until the board is compacted or saved to a new .moth. There are no background network calls from this interface.

Maintain the typed Operation enum and its clap flags, embedded api.json/action
coverage, native integration tests, this reference and the canonical agent skill
when adding features. The catalog and agent guide are embedded in the executable.
CI compares GUI Actions and native command inventory to documented coverage.
The board format remains the versioned [.moth specification](FORMAT.md).
Validation status: [CLI-VALIDATION.md](CLI-VALIDATION.md).

## Source grouping

`info` returns each item's `origin` label. New local-file additions preserve their
parent directory, including when their bytes are deduplicated. `group-origin`
uses that exact label, so folders with the same final name remain separate. It
uses the selected UUIDs or the whole board when omitted, reuses existing source
groups, and leaves already-grouped images and unknown origins alone. A source
group keeps its identity after renaming. Boards saved before schema 9, clipboard
pixels and explicit URL downloads have unknown origins; filenames cannot restore
those directories. Folder labels are saved in the board and travel with it.

`add --group-origin` applies grouping after each addition. In JSON use
`{"op":"add","path":"refs/a.png","group_origin":true}` or
`{"op":"group-origin","ids":["UUID"]}` for an explicit grouping pass.

## Point snapping on the canvas

Open “Snap to points” in the canvas context menu (or the selection More menu)
to enable or disable snapping and adjust grid spacing in the same submenu.
This works in both Collect and View; the mode and spacing persist across launches.
Point snapping displays its own dot grid. The regular grid and alignment guides
are still available in Settings. Shift bypasses snapping during a drag. The grabbed item's displayed bounds determine the grid anchor, and selected
items keep their relative positions. Clicks without movement leave items put.
The grid follows board pan/zoom while the dots keep their screen radius.

For an explicit native edit use `snap-grid BOARD --id UUID --step 100` or JSON
`{"op":"snap-grid","ids":["UUID"],"step":100}`. The first z-sorted selected
item/group supplies the anchor; omitted ids operate on the whole board. The GUI
toggle changes a local preference, so it is GUI-only in the action catalog.
