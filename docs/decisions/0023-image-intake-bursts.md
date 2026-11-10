# 0023 — Image intake owns burst completion and arrangement

## Context

Paste and drop completion relied on state split between `Document::Intake`,
notifications, `Workspace`, `App` and arrangement intents. Callers needed to know
when to end an insertion undo group, temporarily remove its notification before
queuing a layout, restore it on rejection, and wait before fitting the camera.
The modal also separately checked the native drop state.

## Decision

Keep private image-burst state in `app/intake.rs`, owned by the document. Define
the existing `Workspace` burst operations in that module, including paste/drop
preparation, batch lifetime, arrangement acceptance, queue headroom and settling.
Callers read the prompt and added-images notification without mutating them.
The modal uses the module's readiness check, including native drop completion.

Keep the existing bounded clipboard, decode and storage workers. Do not introduce
an adapter trait or alter full-resolution preloading, the board format or the
journal-first acknowledgment path. `App` retains event and window coordination
as described in decisions 0010 and 0012.

## Consequences

Tests and callers use the same arrangement operation. Rejected arrangements
preserve the prompt and previous notification instead of dismissing the prompt
in a separate intent. A failed batch-end enqueue keeps the undo group open for
retry. A complete file list still waits for all worker queues; sources without
an end marker retain the 300 ms quiet fallback. Camera fitting waits for journal
acknowledgments and does not focus a single-image paste.
