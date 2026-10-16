# M1: footer-committed records and durable acknowledgments

## Context

The M0 canvas has reversible scene commands and staged clipboard originals but
no persistent layout. M1 must recover acknowledged commands without coupling core
crates to paths, windowing or GPU formats.

## Decision

Use the version-one byte-stream format in [FORMAT.md](../FORMAT.md): checksummed
originals, command records and atomic scene checkpoints, each committed by a
checksummed footer. Append rather than rewrite; tolerate torn tails and abandoned
writes. One bounded storage worker journals and syncs before the UI applies a
command. Explicit saves append checkpoints; compaction uses a synced sibling
temporary object and atomic replacement. Rotating immutable backups are separate
from per-session discardable recovery. Originals remain unmodified source bytes.

Keep 256 undo/redo commands in memory per session. Persistent replay is not a
persistent undo history. New boards are opened only into an empty canvas; tabs
remain M3. The initial file chooser is an editable native path plus recent-file
and recovery buttons, avoiding another UI/runtime dependency.

## Consequences

Acknowledged commands survive process death, and shutdown needs no pending batch
flush. Full-file checksum validation is streaming and bounded but currently scans
all record bytes on open. Compaction is serialized with writes and may delay
subsequent acknowledgments, without blocking frames. Disk-full journal errors
stop further mutations until recovery, preventing ambiguous durability.

Schema 1 contains no LOD cache and makes no compressed GPU format assumption.
Compressed texture upload selection and the complete eight-hour reliability soak
remain release gates until independently validated. The platform shutdown helper
uses ctrlc for SIGTERM/SIGINT and native console termination; native macOS app
termination and Windows window-session messages still require manual validation.
