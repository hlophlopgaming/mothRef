# Step 4 — Surfaces as lists

**Goal:** the board screen gets the redesigned surfaces, each built from an ordered
list of actions: a bottom toolbar, a selection bar per item type and a grouped
context menu. Diagnostics leave the status bar.

Target layout: [docs/DESIGN.md](../../DESIGN.md), screens 1, 3, 8, 9 and 10.

## Do

1. Add `crates/mothref/src/app/surfaces.rs` with the lists as data, for example
   `const TOOLBAR: &[Slot]`, `fn selection_bar(kind: SelectionKind) -> &'static [Slot]`,
   `const CONTEXT_MENU: &[Slot]`, where `Slot` is `Action(Action)`,
   `Separator`, `Group(Text, &'static [Slot])` or `Custom(CustomId)` for the few
   widgets that are not plain buttons (zoom field, opacity slider, color swatches).
2. **Bottom toolbar** (floating, centred): tool group Select (V) · Hand (H) ·
   Draw (D, from M4) · Shapes (S, from M4) | Insert ▾ (images, paste, note; later
   M6 elements) | Undo · Redo | Fit · zoom − field + . It replaces the top toolbar
   and the left tool rail. Default tool is Select; Esc returns to it.
3. **Selection bar** above the selection bounds, with one list per
   `SelectionKind` (image, several items, note, group; GIF and PDF later) and a
   trailing "…" that opens the full context menu. One bounding box with four
   handles for multi-selection instead of handles per item.
4. **Context menu** grouped (view, transform, arrange ▸, order ▸, copy/export,
   delete last), shortcuts right-aligned from the binding registry, submenu
   arrows drawn as icons (not "▸" text).
5. **Status area:** replace the two-line status bar with a small chip
   ("Saved · 9 images" / "Not saved · recoverable"). Adapter, journal and budgets
   go to Settings › For developers.
6. **Fixes found while testing the alpha** (same PR or follow-ups):
   - "→" and "▸" render as "?": add the glyphs (font fallback) or draw icons.
   - Palette: focus the search field on open, keep its position fixed while
     filtering, full-width rows with right-aligned chords, "/" instead of "Slash".
   - Empty board text lists all supported formats, no "Journaled recovery".
   - Minimap shows image thumbnails.
   - Zoom controls in "− 100% +" order.

## Done when

- The top toolbar, the left rail and the old status bar are gone; their actions
  are in the lists above and in the palette.
- Moving an action between the toolbar, the selection bar and the context menu is
  a one-line change in `surfaces.rs`.
- Every surface works with touch: long-press opens the context menu, the "…"
  button opens it too.
