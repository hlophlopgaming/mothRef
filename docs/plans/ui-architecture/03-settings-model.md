# Step 3 — Settings model and window

**Goal:** preferences live in one `Settings` model described by a schema; a
settings window (Ctrl/⌘+,) and palette search are generated from it. The settings
part of Board tools disappears.

## Today

- `crates/mothref/src/app/tools.rs`: the `Tools` struct holds about 25
  preferences (`gap`, `threshold`, `undo_limit`, `vram_limit_mib`,
  `performance_mode`, `bake_mode`, `wheel`, `smooth_zoom`, `zoom_increment`,
  `rotation_snap`, `scale_with_window`, `auto_arrange`, `default_arrange`,
  `arrange_choice`, `arrange_split`, `dark`, `ui_scale`, `reduced_motion`,
  `show_outlines`, `drag_animation`, `drag_opacity`, `shortcut_feedback`,
  `color_format`, …) next to transient panel state.
- `Tools::preferences()` and `Tools::load()` (around lines 233 and 262) convert
  them to and from the `key=value` map that `moth-platform/src/settings.rs`
  persists in `settings.txt`.
- The language picker sits in the title bar.

## Do

1. Create `crates/mothref/src/app/settings.rs` with `Settings` (all persisted
   preferences, typed) and a static schema: `key`, `section`, `label: Text`,
   `hint: Option<Text>`, `kind` (toggle, choice with options, number with range
   and unit, custom), `default`. Keep the existing `settings.txt` keys so current
   users keep their values.
2. Move persistence to `Settings` (`load`/`save` through the existing
   `moth-platform` settings store). `Tools` keeps only transient state.
3. Build a settings window from the schema: section list on the left with
   icons, rows on the right, a "changed" marker for values that differ from the
   default, "Reset section", changes applied immediately. Custom rows: video
   memory limit, disk-cache usage and "Clear disk cache", key bindings (reuse the
   existing bindings editor).
4. Sections: General (language, autosave), Appearance (theme, interface scale,
   reduced motion), Canvas and navigation (wheel, zoom, outlines, drag),
   Adding images (arrange method, gap, many-images behaviour and threshold),
   Window and overlay, Performance and memory, Keys and mouse, Integrations
   (empty for now), For developers (adapter, journal state, budgets — moved out of
   the status bar in step 4).
5. Register `Action::Settings` (Ctrl/⌘+,) and make each schema entry searchable in
   the palette as "Settings › Section › Label", opening the window at that row.
6. Move the language picker from the title bar into General.

## Done when

- `Tools` has no persisted field; `settings.txt` from the previous release loads
  unchanged.
- Adding a preference is one schema entry plus its use site.
- Typing "settings", "memory" or "zoom" in the palette finds the right rows.
