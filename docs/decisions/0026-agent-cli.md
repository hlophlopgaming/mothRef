# 0026 — Native subcommands and JSON-lines in the GUI binary

Date: 2026-11-17

## Context

The maintainer moved the agent-ready CLI into M2 and requested documented board
operations with continued coverage as the app grows. The final interface must use
ordinary commands and JSON directly, without a Python wrapper. An installed GUI
binary must include its own CLI.

## Decision

Use clap subcommands and `mothref --agent` in the existing executable, dispatching
before window/GPU initialization. No arguments or ordinary file arguments retain
GUI behavior. Run all CLI/protocol I/O on a dedicated agent thread. Both frontends
construct the same typed `Operation` enum; clap derives flags from that enum and
serde validates JSON. Reuse GUI geometry/group command builders and the native
storage worker for journaled edits, undo/redo, imports, checkpoints and exports.
Validate whole command lists against scratch metadata before journaling them.

Ordinary mutations/batches auto-save after successful completion. Raw JSON lines
save only on explicit `save` and retain bounded undo/redo for the process lifetime.
Read-only conversion/inspection uses private staging without modifying the input.
Dry-run suppresses saves and refuses exports, while still permitting private
journal I/O. Errors stop before final autosave; unsaved work remains recoverable.
Successful saved/query sessions and dry-runs close workers before discarding their
own staging. Scripts are sequential, not transactions.

Default insertion places images/notes beside board bounds; default duplication
preserves relative positions/Y and moves copies beyond the right edge. Explicit
coordinates retain their meaning. Metadata/mutation replies expose board bounds,
rotated item bounds and centers. Static native PNG preview bundles carry transforms
between actual pixels and board coordinates, including aspect-ratio padding.
Explicit HTTP URLs use the existing bounded native clipboard-download path.

Share destination locks between GUI and CLI storage workers, using stable
zero-byte sentinels in the recovery locks folder keyed by canonical board path.
Compaction does not invalidate locks; process exit releases them. Persistent
sentinels must not be deleted while processes can hold/acquire locks.

Embed api.json, actions.json and the canonical agent guide in the executable.
Native integration tests compare the compiled GUI/CLI inventories with documented
coverage. New board features ship their operation/flags, docs and tests together;
GUI-only interactions have explicit coverage reasons. No Python runtime, package
or test harness is part of the final interface.

## Consequences

One installed binary provides GUI, normal commands and a stateful JSON transport.
The scene, journal and file-format implementation stay shared. Clap adds a
GPL-compatible MIT/Apache-2.0 dependency outside core crates. Native tests run with
the actual executable in desktop CI and do not require a display.

Undo history ends with the process. Live GUI attachment and automated recovery
selection remain outside this interface. Scene export and sampling retain native
preview-resolution limits; exact originals can be exported separately. Source-path
sorting waits for source metadata. Older app versions and different recovery roots
do not share writer-lock protection. Windows/macOS launch/console integration and
socket-dependent download coverage require their respective CI/manual evidence.
