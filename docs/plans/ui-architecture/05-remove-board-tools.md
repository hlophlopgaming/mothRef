# Step 5 — Remove the Board tools window

**Goal:** the Board tools window and its toggle are deleted. Nothing it offered is
lost.

## Do

1. List every remaining control in `Tools::show` (`crates/mothref/src/app/tools.rs`)
   and map it to its new home: settings window (step 3), toolbar, selection bar or
   context menu (step 4), palette, or a dialog (export, crop region, picker).
   Put the mapping table in the PR description.
2. Move what is left:
   - Lock canvas, grayscale canvas → View group of the context menu on empty
     canvas and the palette.
   - Previous/next image, reset camera → palette and keyboard only.
   - Arrange options (layout, order, split, gap) → the Arrange submenu and an
     "Arrange…" popover from the selection bar.
   - Colour picker and coordinate picker → actions with their own small overlay
     (format choice HEX/RGB/HSV inside it).
   - Export → an "Export…" dialog opened from the context menu and the palette.
3. Delete the window, `Intent::ToggleTools`, `Action::Tools`, the "Board tools"
   button and their `Text` keys. Keep `Tools` only for transient state that the
   new surfaces still need, or dissolve it.
4. Update `docs/SHORTCUTS.md` and the README "Getting around" table if labels or
   locations changed.

## Done when

- No code path opens a Board tools window.
- Every action that existed before is reachable from at least one surface list,
  the palette and a touch path.
- `CHANGELOG.md` "Unreleased" explains where each moved control now lives.
