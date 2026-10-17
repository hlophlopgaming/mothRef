# 0004 — Bounded reference workspace

Date: 2026-10-17

## Context

The maintainer requested all five supplied UI states before full M3/M5 delivery.
Tabs, previews and transparency must preserve journal durability and explicit
memory budgets. The current scene format contains images only.

## Decision

Keep at most four independently journaled sessions in one native window. Only
the active session owns image residency in the shared renderer; drain inactive
worker completions without uploading textures. Keep native destination leases
within the process so two tabs cannot concurrently save the same file.

Use a separate bounded platform worker for the initial library: twenty recent
boards plus twenty pinned boards, four 128-pixel previews per card. Persist pins
and tags atomically in app-owned metadata. Folder filters derive from parent
folders; this does not introduce a second scene format or a filesystem watcher.

Flatten the board into a temporary texture before applying master opacity, so
overlapping references have uniform transparency. Cap this texture at 64 MiB
and account for it in the resource registry. Keep overlay controls in a separate
interactive native window, allowing exit even when the board is click-through.
Expose unsupported window capabilities in the UI.

## Consequences

The requested library, board, empty, overlay and close-dialog states are reachable
through real actions. Preview textures are bounded by 10 MiB and released when
leaving the library. The image residency budget remains 96 MiB. Very large
transparent surfaces fail with a budget error instead of allocating without limit.

This slice does not complete tab tear-off, the redb index/file watcher, folder
management, rich notes or hierarchy tools. Native behavior on Windows/macOS and
actual touch devices still needs qualification; Wayland cannot promise on-top.
