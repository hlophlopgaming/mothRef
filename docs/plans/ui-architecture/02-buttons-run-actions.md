# Step 2 — Buttons run actions

**Goal:** every clickable command in the UI runs a registered `Action` through
`Intent::Action`. No surface builds a `Command`. No visible change.

## Today

- `crates/mothref/src/app/bindings.rs`: the `actions!` macro defines `Action`
  (75 entries: id, `Text` label, default chord). Keyboard dispatch, the palette
  and the cheat sheet already use it.
- `crates/mothref/src/app/actions.rs`: `App::action_enabled` and
  `App::run_action` turn an action into an `Intent` or a side effect.
- `crates/mothref/src/app/intent.rs`: `Intent::Action(Action)` exists.
- `crates/mothref/src/app/tools.rs` (`Tools::show`, from around line 364) builds
  commands inline: rotate and flip via `transform_selection`, delete via
  `Command::Remove`, arrange, normalize, align, scale, z-order, crop reset, export.
  Similar inline handling may exist in `moth-ui/src/chrome.rs` (`toolbar`,
  returns `moth_ui::Actions`) and in the canvas context menu
  (`app/canvas.rs`, `response.context_menu`).

## Do

1. Inventory every button, menu item and checkbox that triggers an effect in
   `tools.rs`, `chrome.rs`, `app/chrome.rs`, `app/canvas.rs` and `app/modal.rs`.
   Write the inventory as a table at the top of the PR description.
2. For each effect without an action, add one to `actions!` (label in `Text`,
   default chord `""` when none) and implement it in `run_action`, reusing the
   existing inline code. Parameterised effects (arrange method, normalize kind,
   align edge, z-order step) become one action per variant, matching how
   `AlignLeft`… already work.
3. Replace each inline handler with `intents.push(Intent::Action(Action::X))`.
   Disabled state comes from `action_enabled`, so the button and the shortcut
   agree.
4. Add an `icon` column to `actions!` (an `moth_ui::icons::Icon` or `None`) so
   later surfaces can show icon, label and chord from one place. Fill it for
   actions that already have an icon in `moth-ui/icons`.
5. Replace `moth_ui::Actions` (the struct of booleans returned by `toolbar`) with
   a list of triggered `Action`s if that keeps `moth-ui` free of `mothref` types;
   otherwise keep it and map it to actions in `app/chrome.rs`.

## Done when

- `rg "Command::" crates/mothref/src/app/tools.rs crates/mothref/src/app/chrome.rs crates/moth-ui/src` finds no command construction in UI code.
- Every former button also appears in the command palette with its shortcut.
- Undo still records one step per button press.
