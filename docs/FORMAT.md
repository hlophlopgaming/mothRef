# MothRef format, schema 1

All integers and IEEE-754 f32 coordinates are little-endian. Core readers accept
seekable byte streams; filesystem names are never serialized. Schema 1 is the
first persistent format. There is no schema 0 board to migrate. Future schemas
must retain the schema 1 reader and provide explicit migrations.

The 16-byte header is `MOTH`, u32 schema version `1`, then eight zero bytes.
Every record consists of a payload followed by a 96-byte footer:

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
No GPU texture format or disposable LOD cache is persisted in schema 1.

Commands use these layouts:

- Insert: u8 `1`, UUIDv7 (16 bytes), BLAKE3 blob id (32 bytes), rectangle (four f32:
  x, y, width, height), remaining bytes are a validated fractional index.
- Remove: u8 `2`, UUID (16 bytes).
- SetRect: u8 `3`, UUID (16 bytes), rectangle (four f32).

A checkpoint contains zero or more u32-length-prefixed Insert commands. The
entire checkpoint is one record, so replacing the scene is atomic. Commands after
the last checkpoint replay through the same scene command API. Journal and board
streams share this encoding. The desktop worker syncs an original before its
insertion command, then syncs the command before acknowledging it to the UI.
Acknowledged mutations therefore need no deferred shutdown flush.

Limits: 256 MiB per record; 64 MiB per checkpoint; 4,161 bytes per command; one
million scene items and blob entries in the reader. The current UI still limits
boards to 10,000 items. The desktop journal/file budget is 8 GiB; exhaustion rejects
new work instead of evicting originals. Undo/redo retains at most 256 commands.

Explicit saves append missing blobs and one checkpoint. Before updating a
nonempty board, the desktop worker syncs an immutable backup in app-owned storage
and keeps the latest three per destination. Compaction writes only live originals
and a checkpoint to an exclusive temporary object beside the destination, syncs
it, then atomically replaces the destination (`ReplaceFileW` on Windows, rename
and directory sync on Unix). Failed compaction cannot destroy the previous board.
Compaction currently runs on the storage worker after a save when the file is
larger than twice its estimated live original bytes and at least 1 MiB.
