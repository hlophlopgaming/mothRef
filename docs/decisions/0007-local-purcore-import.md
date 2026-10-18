# M2: local purcore safety fork and bounded import

## Context

The pinned LazyRef revision `e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654`
loads entire boards, uses native filesystem scratch paths for SQLite, and reads
legacy text children recursively without a depth limit. That conflicts with
MothRef's portable storage and bounded-memory import requirements.

## Decision

On 2026-10-18 the maintainer explicitly authorized a local fork for these fixes,
overriding the previous ban on vendoring/forking purcore for this task.
Keep the attributed subset in `third_party/purcore`, including its MIT license.
Reuse the existing pure-Rust SQLite reader there rather than adding rusqlite.
The public import boundary reads `Read + Seek`, returns typed errors from primitive,
blob and legacy decoding, verifies file checksums and enforces the budgets in
the fork README. An outer unwind guard contains unexpected panics in normal builds.

Imports execute on the existing storage worker. Supported embedded images and
plain notes become UUIDv7 inserts through the existing journal-first path, sorted
by source z-order, with one undo group. The original `.pur` is read-only; saves
produce `.moth` files. Parent transforms are flattened for 2.x images and notes.

## Consequences

MothRef owns maintenance of the local fork; upstream improvements must be merged
explicitly. Captured 1.10 and 2.1 boards provide baseline qualification, not a
claim of every PureRef version or feature. Linked images are never opened
implicitly. Drawings, group decoration, rich/fixed-size notes, non-opaque items,
unsupported flags and unrepresentable crops/transforms are skipped with the
partial-import warning. Names, source strings and other metadata absent from
MothRef's scene model are not persisted. The all-versions M2 checkbox stays open.

Limits can reject large boards or records; increasing them needs memory evidence.
No `.moth` schema or journal layout changes. Tier-3 reliability qualification
remains the CI/maintainer release gate.

## Follow-up: board-size limit and 2.1.2 qualification

The maintainer subsequently requested 4 GiB per entire `.pur` board while keeping
separate RAM limits for individual payloads and metadata. Both formats now enforce
that admission limit. Legacy parsing scans streams and resolves offsets before
loading each original, replacing the former 16 MiB whole-file buffer. SQLite rows
are mapped by column name, since captured 2.1.2 and 2.1.3 files use different
physical column orders. The private FemaleFighterPosesPack board exercises 175
images through durable import, `.moth` reload and grouped undo/redo; see
`docs/M2-IMPORT-VALIDATION.md`. The file is not redistributed.
