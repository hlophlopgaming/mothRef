# 0010 — Historical parallel app responsibility split

## Integration status — 2026-10-19

Retained from the parallel upstream refactor when merging `origin/main` after
v0.1.0-alpha.3. Renumbered from 0009 to avoid the adaptive-GPU-budget decision.
The sections below describe that branch, not the current module layout. The
subsequent refactor and motion implementation retained for the merge uses
`Document` with `View`, a collected intent queue, `Workspace::pump`,
`Shell::render`, `InputState` and `DragMotion`; it does not use separate
`Session`, `Screen` or `Gesture` modules. See the current
[architecture](../ARCHITECTURE.md) and [completed refactor plan](../TECH-DEBT.md#remaining-refactor-plan-app-god-object).

## Context

`mothref/src/app.rs` had grown into a single type mixing window/GPU/event-loop
plumbing, tab lifecycle, the library screen, modal state and pointer-input state
into ~35 fields and a ~1100-line `frame()`. Independent `Option`/`bool` fields
tracked drags, moves and duplicate-drags, and the same `self.drag = None;
self.moving = None;` reset appeared in several places. UI actions were collected
into named local flags at the top of `frame()` and applied in a hand-ordered tail,
and screen modes were three separate booleans guarded by scattered early returns.

## Decision

State that describes one pointer interaction is a single `Gesture` enum owned by
`app/gesture.rs`. `Moving` records the origin rect of every selected item in
`(z, id)` order, not just the item under the cursor, and cancellation (focus loss,
suspension, tab activation, touch takeover) resets it in one place.

Every user-visible action flows through one `Intent` enum and one `App::apply`
method, shared by the UI and keyboard shortcuts. The scene is still mutated only
through `Command`/`persist*`; `Intent` merely routes to the existing paths.

Screen/modal modes are `Screen` (Board/Library) and `Modal`
(PathDialog/ConfirmClose/EditingNote). `Modal` owns the path dialog's draft and
focus so a save-path editor can suspend and resume an underlying close
confirmation or note editor.

`App` is a thin coordinator whose fields are subsystems: `Shell`
(window/gpu/egui, device/surface-loss recreation), `Workspace`
(board, tabs, screen, recovery/recent/startup and close-flow state),
`LibraryScreen` (index worker, cards, catalog), plus `Gesture`, `Modal`, `Tools`,
`Overlay` and the input adapter. `Session` is `Document` (scene, workers, loading)
plus `View` (camera, selection, presentation), so switching tabs never exchanges
persistence identities.

## Consequences

Behavior is unchanged; the named action flags in `frame()` still apply in their
original order because the ordered tail is preserved. Collapsing those flags into
a single `Vec<Intent>` is a cosmetic follow-up that must not reorder the
close-flow (save-before-discard) applies.

Document workers live with their data: `Document` owns
`begin_batch`/`end_batch`/`pump`, `Session` owns
`persist`/`persist_batch`/`discard`/`paste` (reading `view.locked`, which already
mirrors `tools.locked`). The tab lifecycle and worker drain stay on `App` as
coordinators: they orchestrate `Workspace`, `Modal`, `LibraryScreen`, `Shell` and
`Tools`, so relocating them would turn a data-owning subsystem into a coordinator
rather than remove coordination. Native scenario validation (pen/touch) remains
manual.
