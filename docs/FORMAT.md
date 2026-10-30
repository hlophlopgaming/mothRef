# MothRef format, schema 6

All integers and IEEE-754 f32 coordinates are little-endian. Core readers accept
seekable byte streams; native filesystem paths are never serialized. Schema 1 is the
first persistent format; there is no schema 0 board to migrate. Schema 2 adds item
content transforms; schema 3 adds text notes; schema 4 adds appearance and source
filenames; schema 5 adds image sampling and fixed-width note metrics; schema 6 adds
nested groups. The reader keeps the schema 1, 2, 3, 4 and 5 record layouts, so every older board and journal still opens: a schema 1 insert
carries no transform and migrates to the identity transform. Future schemas must
retain both readers and provide explicit migrations.

The 16-byte header is `MOTH`, u32 schema version `6`, then eight zero bytes.
Readers accept versions 1 through 6 and reject every other version. Every record
consists of a payload followed by a 96-byte footer:

| Footer offset | Size | Meaning |
| --- | --- | --- |
| 0 | 8 | ASCII `MOTHEND1` |
| 8 | 8 | Absolute payload offset |
| 16 | 8 | Payload length |
| 24 | 1 | Kind: 1 original blob, 2 command, 3 checkpoint |
| 25 | 7 | Reserved, zero |
| 32 | 32 | BLAKE3 of payload |
| 64 | 32 | BLAKE3 of footer bytes 0–63 |

A footer commits exactly one payload. Writers append only, including after a
failed/torn write. Readers scan bounded windows for valid footers, verify lengths,
offsets and both checksums, and replay committed records in byte order. Records
cannot overlap a previous committed record. Junk gaps and trailing bytes set the
recovery flag. A corrupt payload is skipped; a semantically invalid committed
command returns an error, never a panic. Unsupported headers are rejected.

Original blobs contain encoded source bytes unchanged. Their payload checksum is
their content identity. The reader retains only offsets and lengths, never decoded
pixels. A command inserting an image must reference a previously committed blob.
No GPU texture format or disposable LOD cache is persisted in any schema.

Commands use these layouts:

- Insert (schema 1, tag `1`): UUIDv7 (16 bytes), BLAKE3 blob id (32 bytes),
  rectangle (four f32: x, y, width, height), remaining bytes are a validated
  fractional index. The rectangle starts at payload offset 49.
- Insert (schema 2, tag `4`): tag `1` layout with a transform (21 bytes) between the
  rectangle and the fractional index; the index starts at payload offset 86.
- Insert note (schema 3, tag `6`): UUIDv7 (16 bytes), u32 UTF-8 byte length, the
  note text, a rectangle (four f32), a transform (21 bytes), then a validated
  fractional index. The rectangle starts after the text.
- Remove: u8 `2`, UUID (16 bytes).
- SetRect: u8 `3`, UUID (16 bytes), rectangle (four f32).
- SetTransform: u8 `5`, UUID (16 bytes), transform (21 bytes).
- SetText (schema 3, tag `7`): u8 `7`, UUID (16 bytes), u32 UTF-8 byte length, the
  note text. The inverse carries the previous text.

A transform is f32 `rotation` (radians, around the frame centre), one u8 flag byte
(bit 0 flips content horizontally, bit 1 flips it vertically), two f32
`content_offset` values (content pan in frame widths and heights), f32
`content_scale` (1.0 fills the frame without distortion, clipped to it) and f32
`content_rotation` (radians inside the frame). A writer rejects non-finite values
and a non-positive `content_scale`. The scene rejects the same values, so a corrupt
transform never enters a board. Content stays clipped by the frame, which keeps the
item's indexed footprint the rotated rectangle.

Schema 4 uses new tags `8` (image insert), `9` (note insert), and `10`
(SetTransform). The 21-byte transform is extended by f32 opacity in `[0,1]` and
one grayscale byte (`0` or `1`). Insert tags then store a u16 UTF-8 filename
length (at most 1024 bytes), the filename, and the remaining fractional index.
Thus image insert tag `8` has its filename length at offset 91. Note tag `9`
places it immediately after the 26-byte transform. Tag `10` is 43 bytes total.
Names are metadata only, never filesystem paths. Readers migrate tags `1`, `4`,
`5`, `6` and `7` with opacity `1`, grayscale `false`, and an empty filename.
Frozen migration fixtures are `corpus/moth/v1.moth` through `v4.moth`.

### Migration 4 → 5

Schema 5 writes tags `11` (image insert), `12` (note insert) and `13`
(SetTransform). Its transform is 32 bytes: the existing 26 bytes followed by
u8 sampling (`0` inherit canvas preference, `1` smooth, `2` nearest neighbor,
`3` automatic), f32 note font size in world units, and u8 note wrapping (`0`/`1`).
The font size must be finite in `[0, 1e9]`; wrapping requires a positive font size.
Unknown sampling codes and non-boolean wrap flags are rejected. Image tag `11`
stores the filename length at offset 97; note tag `12` stores it immediately after
the transform. Tag `13` is 49 bytes total. Appearance resets preserve sampling.

Older tags migrate with sampling `0`, note font size `0` and wrapping `false`.
Zero font size retains the legacy formula: frame height divided by the explicit
line count (at least one) and by `1.3`. Creating or editing a note freezes its font
size; changing its manual width enables wrapping, while auto-sized notes expand
to their text. Multi-item proportional scaling changes both note frames and fonts.
Automatic sampling uses nearest neighbor at two screen pixels per original pixel
or above, independently of the currently resident mip. It changes neither source
bytes nor decode/cache budgets.

`corpus/moth/v5.moth` contains a sampling override and a wrapped note; regenerate
only the current fixture with `cargo xtask fixture`. Desktop saves atomically
compact an older board to schema 5 before appending a schema-5 checkpoint;
journals start with the current header. No header or committed payload is
rewritten in place. Applications that only support schemas 1–4 reject schema 5.

### Migration 5 → 6

Schema 6 adds groups: a titled backdrop that hugs its members. Every item gains an
optional parent group, written as 16 bytes where the nil UUID means "directly on the
board" (item ids are UUIDv7, never nil). New tags:

- `14` image insert: tag `11` layout with the parent between the filename and the
  fractional index.
- `15` note insert: tag `12` layout with the parent between the filename and the
  fractional index.
- `16` group insert: UUIDv7, rectangle (four f32, offset 17), u8 color code
  (offset 33, below `6`), u16 UTF-8 title length (at most 256 bytes), the title,
  the parent, then the fractional index.
- `17` SetParent: UUID, parent (33 bytes total). The inverse carries the old parent.
- `18` SetGroup: UUID, u8 color code, u16 title length, the title.

A group's rectangle is derived, not stored state: while it has members it is the
union of their rotated footprints, padded by 24 world units on the sides and bottom
and 36 on top for the title chip; while empty it shows its resting rectangle, which
is the one written in its insert and changed by SetRect. Groups keep the identity
transform; SetTransform on a group is rejected. A group can be removed only once
it is empty, groups nest at most 16 levels, and a group can never contain itself.
Color codes index theme tokens, never literal colors.

Checkpoints write parents before their members, so replaying inserts in order never
names a group that does not exist yet; the checkpoint writes a group's current
frame as its resting rectangle. Older tags migrate with no parent. `corpus/moth/v5.moth`
is now frozen; `corpus/moth/v6.moth` holds two nested groups, an image and a note.
Applications that only support schemas 1–5 reject schema 6.

A checkpoint contains zero or more u32-length-prefixed Insert commands. The
entire checkpoint is one record, so replacing the scene is atomic. Commands after
the last checkpoint replay through the same scene command API. Journal and board
streams share this encoding. The desktop worker syncs an original before its
insertion command, then syncs the command before acknowledging it to the UI.
Acknowledged mutations therefore need no deferred shutdown flush.

Limits: 256 MiB per record; 64 MiB per checkpoint; 5,235 bytes per image command
(including a 1,024-byte name, a parent and a 4,096-byte fractional index);
32 KiB of note text per insert/SetText; one million scene items and blob entries
in the reader. The current UI still limits boards to 10,000 items. The desktop
journal/file budget is 8 GiB; exhaustion rejects new work instead of evicting
originals. Undo/redo retains at most 256 commands.

Explicit saves append missing blobs and one checkpoint. Before updating a
nonempty board, the desktop worker syncs an immutable backup in app-owned storage
and keeps the latest three per destination. Compaction writes only live originals
and a checkpoint to an exclusive temporary object beside the destination, syncs
it, then atomically replaces the destination (`ReplaceFileW` on Windows, rename
and directory sync on Unix). Failed compaction cannot destroy the previous board.
Compaction currently runs on the storage worker after a save when the file is
larger than twice its estimated live original bytes and at least 1 MiB.
