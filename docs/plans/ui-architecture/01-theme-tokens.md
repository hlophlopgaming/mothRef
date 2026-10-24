# Step 1 — Theme tokens

**Goal:** every color, radius and spacing value comes from one theme module, in a
dark set (today's look, pixel-identical) and a light set. No visible change.

## Today

- `crates/moth-ui/src/lib.rs` defines `BACKGROUND`, `ACCENT`, `PANEL`, `MUTED`
  constants and `theme()` (around line 549), which forces `Visuals::dark()` and
  sets panel, window, text, widget and selection colors with literals.
- Further `Color32::from_rgb` literals: `moth-ui/src/canvas.rs` (2),
  `moth-ui/src/library.rs` (1), `mothref/src/app/canvas.rs` (3, note cards),
  `mothref/src/app/modal.rs` (1, destructive button).
- `Tools::dark` already exists as a preference.

## Do

1. Add `crates/moth-ui/src/theme.rs` with a `Tokens` struct of named roles, for
   example: `canvas`, `canvas_dots`, `surface`, `surface_raised`, `border`,
   `text`, `text_muted`, `accent`, `accent_text` (accent for text and thin lines),
   `on_accent`, `selection`, `danger`, `note_fill`, `note_text`, plus `radius_s/m/l`
   and `space_xs…xl`.
2. Provide `Tokens::DARK` with today's exact values and `Tokens::LIGHT` with a
   provisional light set (the final light palette comes from docs/DESIGN.md).
3. Make `theme(ctx, &Tokens)` build `egui::Visuals` from the tokens
   (`Visuals::light()` or `dark()` as the base, chosen by the set), and store the
   active tokens in egui memory or a context extension so widgets can read them.
4. Replace every `Color32::from_rgb` / `from_rgba_unmultiplied` outside
   `theme.rs` with a token. Keep `BACKGROUND`/`ACCENT`/`PANEL`/`MUTED` only as
   deprecated aliases if removing them would grow this PR; remove them in step 4.
5. Wire the existing `dark` preference to choose the set. Default stays dark.

## Done when

- `rg "Color32::from_rgb" crates` finds matches only in `moth-ui/src/theme.rs`.
- The dark set reproduces today's screens; the software golden example
  (`xtask/examples/software_golden.rs`) still matches if it covers UI.
- Switching the preference to light recolors the whole UI without restarts.
