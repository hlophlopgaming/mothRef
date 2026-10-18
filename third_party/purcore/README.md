# Local purcore safety fork

Source: https://github.com/achrllrogia45/LazyRef
Revision: `e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654`.
Copyright (c) 2026 achrllrogia45; original MIT notice is in `LICENSE`.
Vendoring and local fixes explicitly authorized by the maintainer on 2026-10-18.

Retained upstream modules: binary codecs, model, checksum helpers,
and legacy reader. The filesystem/rusqlite API and container writer are not included.
The portable SQLite reader was moved here from MothRef (GPL-3.0-or-later).
New integration code is GPL-3.0-or-later; the crate's combined license expression
is `MIT AND GPL-3.0-or-later`.

`read(Read + Seek, callback)` is the import boundary. It verifies checksums,
streams a 2.x database through a virtual splice without copying it, and emits
one image payload at a time. Primitive reads, blob decoders and the legacy parser
return typed errors; an outer unwind guard contains unexpected panics in normal builds.

All boards have a 4 GiB encoded-file limit, checked before reading payloads.
SQLite records and individual legacy originals are capped at 32 MiB, retained
metadata at 8 MiB / 8192 items, and group ancestry at 64 levels. Legacy readers
scan with a 64 KiB window, resolve blob offsets and placement metadata, then emit
one original at a time. Text nesting remains capped at 32 levels. Legacy reference resolution
moves child trees instead of cloning them into arbitrarily many duplicate rows.
Limits reject inputs explicitly; the source file is never modified or deleted.

Known qualification limits: captured 1.10 and 2.1 boards are covered, plus a
private 175-image board from PureRef 2.1.2. SQL columns are resolved by name,
including the differing 2.1.2/2.1.3 physical layouts. Only simple Qt-generated
column declarations are supported; complex SQL definitions are rejected.
The stream adapter currently recognizes the known fixed 2.0/2.1 header layout;
other layouts are rejected. SQLite UTF-16 databases are unsupported. The local
blob decoder API returns `Result`; application code uses `read` to enforce the
aggregate import budgets. No path-based API or C SQLite dependency is compiled. Keep future fixes here documented against the source revision.

Application import behavior and limitations: [specification](../../docs/PUR-IMPORT.md).
