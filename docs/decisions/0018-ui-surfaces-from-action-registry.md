# 0018 — UI surfaces are built from the action registry

## Context

The current UI works, but its layout is written by hand in each surface:

- `crates/mothref/src/app/tools.rs` (the Board tools window) builds `Command`s
  inline for rotate, flip, delete and arrange instead of running the registered
  `Action`. The same action therefore has several implementations, and moving a
  button means moving code.
- About 25 user settings (performance mode, zoom, arrange gap and threshold,
  theme, undo limit…) are fields of the `Tools` panel struct. The Board tools
  window cannot be removed without first moving the settings out of it.
- Colors are literals: about 20 `Color32::from_rgb` calls across
  `moth-ui/src/lib.rs`, `moth-ui/src/canvas.rs`, `moth-ui/src/library.rs`,
  `mothref/src/app/canvas.rs` and `mothref/src/app/modal.rs`, and `theme()`
  forces `Visuals::dark()`. A light theme or a new identity touches every file.
- Toolbars are hand-laid in `moth-ui/src/chrome.rs` and `tools.rs`.

The foundations are already right and stay: board logic lives in core crates and
changes only through `Command`; the UI emits `Intent`s applied in one place
(`app/intent.rs`); `app/bindings.rs` is a single action inventory (75 actions with
label and default chord) already used by keyboard dispatch, the command palette
and the cheat sheet; `App::action_enabled` and `App::run_action` exist.

The UX redesign (see `docs/DESIGN.md` once merged) replaces the Board tools
window with a bottom toolbar, a selection bar, a context menu, a settings window
and a layers drawer. Without this decision that redesign would be a rewrite, and
the next one would be a rewrite again.

## Decision

1. **A button is a reference to an action.** Every clickable command in any
   surface runs a registered `Action` through `Intent::Action`. An action owns
   its label, icon, default chord, availability (`action_enabled`) and effect
   (`run_action`). Surfaces never build `Command`s themselves.
2. **Surfaces are lists.** The bottom toolbar, the selection bar (per item type),
   the context menu and the palette are ordered lists of `Action`s plus
   separators. Moving a button is editing a list.
3. **Settings are a model, not panel fields.** One `Settings` struct with a
   schema entry per field (section, label, kind, default, range). Persistence,
   the settings window and settings search are generated from the schema.
4. **Visual values come from theme tokens.** Colors, radii and spacing live in one
   `moth-ui` module with a light and a dark token set. No `Color32` literal
   outside that module.
5. **Surface modules stay thin.** Each surface lays out actions and settings; it
   holds no board logic.

## Consequences

- The redesign becomes five small, behavior-preserving steps (ROADMAP M2.6),
  each one PR, each shippable.
- Adding a feature means: register an action, put it in the lists where it
  belongs. The "where does it live in the UI" question has to be answered in
  every feature plan.
- Text-only and icon-only presentations of the same action stay consistent,
  including tooltips and shortcut labels.
- Touch paths (AGENTS.md, Portability) are easier to keep: every action reachable
  from a context menu is also reachable from a list that a touch surface can show.
