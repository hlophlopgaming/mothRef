# 0027 — Image sources in boards

## Context

The maintainer wants "Open image source" (PureRef) and "Arrange by file path"
(PureRef, called a killer feature) for M2 parity (2026-11-18). Both need to know
where each image came from after the board is saved and reopened. Until now a board
kept only the file name, and FORMAT.md said native paths are never serialized.

## Decision

- Schema 9 stores an optional source per image: an absolute file path, a ZIP
  archive path plus entry name, or an http(s) URL (FORMAT.md › Migration 8 → 9).
  It is metadata the user sees and opens again, not a way to reach the image: the
  original bytes stay in the board, and core crates never open a source. Storage
  still goes through handles; the desktop backend turns a registered handle into a
  source (`DesktopStorage::source_of`), so the "no paths in core crates" rule for
  reading and writing data is unchanged.
- Images get a source when added from a file, a ZIP entry, the command line, a file
  list on the clipboard or a download; a reduced copy keeps the file it was made
  from. Pasted pixels, images inside imported `.bee`/`.pur` boards and images added
  before schema 9 have none, and "Open source" says so.
- A path that is not valid UTF-8 or longer than 4096 bytes is not stored. A URL
  carrying a password is not stored.
- "Open source" runs on the storage worker, because checking a path on a
  disconnected drive can block: it shows the file (or the archive) selected in the
  file manager, opens a URL in the browser only for plain http/https links, and
  reports a missing file with the path it was at.
- "Arrange by file path" sorts by the source: folders together, numbers by value,
  case ignored, images without a source last, by name.

## Consequences

A shared board reveals the folder layout of the computer it was made on. A way to
save a copy without sources can follow if the maintainer wants it; until then the
release notes mention it. Relinking a moved source (as the library does for boards)
is not part of this change.
