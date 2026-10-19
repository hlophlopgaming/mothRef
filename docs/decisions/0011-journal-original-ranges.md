# 0011 — Read originals from journal ranges

Date: 2026-10-19

## Context

Opening a `.moth` board copied live originals into separate `.source` files, read
and hashed those files again, and appended their bytes to a new recovery journal.
Foreign import likewise staged and reread originals before journaling them.
The journal already owns durability; the extra files only served decoding/export.

## Decision

Register bounded, read-only byte ranges in the session's own recovery journal as
ordinary opaque storage handles. Decode, export and save keep using `Storage::open`.
Each open has an independent cursor, seeks use range-relative offsets, reads stop
at the range boundary, and writable storage operations reject range handles.

Opening `.moth` verifies originals again while copying them directly into the new
journal, in source-offset order. Foreign import appends each unique original
directly while parsing. Individual inserts use the same append path. A completion
still follows the existing command/checkpoint sync. Checksums, format versions,
budgets, undo grouping and decoder isolation do not change.

## Consequences

No `.source` copy is needed for new sessions. Existing journals remain readable.
Originals are independent of later changes to the user-opened file. Board save and
compaction never replace the session journal, so registered offsets remain valid.
Any future journal replacement must explicitly preserve or remap these handles.
Clipboard staging keeps its existing crash-safe boundary.

This decision removes working copies. Progressive scene delivery and footer-chain
reading follow in decision 0012; persisted previews and zero-copy opening of the
user's board file remain outside these changes.
