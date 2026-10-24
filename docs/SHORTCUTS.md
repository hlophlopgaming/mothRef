# Canvas controls and PureRef shortcut comparison

Reviewed against MothRef input code on 2026-10-19. The configurable binding registry
and navigation presets described below supersede the earlier hard-coded dispatch audit. The PureRef column transcribes
the maintainer-supplied **Actions / Controls** screenshot; its version and custom
binding state are unknown. It is a compatibility reference, not a claim about
current PureRef defaults. No PureRef executable was inspected. The screenshot is
not redistributed here.

`LMB` = left mouse button, `MMB` = middle mouse button, `RMB` = right mouse button,
`Space` = the screenshot's `Spc`. Drag combinations mean holding the listed keys
while dragging. MothRef command shortcuts use Command on macOS instead of Ctrl.
Entries marked **missing** or **partial** are requirements, not available shortcuts.
An action being present in a menu does not mean its PureRef binding is implemented.

## Available MothRef controls

| Task | Current control |
| --- | --- |
| Select / Hand tool | `V` / `H`; bottom-toolbar buttons |
| Pan | MMB-drag, Space+LMB-drag, or LMB-drag with Hand selected; two-contact touch pan |
| Zoom | Wheel at cursor, bottom-toolbar `−` / percentage / `+`; pinch / continuous trackpad zoom |
| Fit entire board | `Home`, Fit board button |
| Fit selected group | Tap Space over the selection, or select images, double-click any selected image: fit the combined rotated bounds in both viewport dimensions with padding; double-click the same image again to return |
| Focus one image | Double-click an image outside the selection |
| Previous / next image | `PageUp` / `PageDown`; cycles in layer order and wraps, ignoring notes |
| Move / duplicate selection | LMB-drag / Alt+LMB-drag; selected images move as a group |
| Add to selection | Shift+click; drag empty canvas for a selection rectangle |
| Nudge selection | Arrow keys: 1 scene unit; Shift+arrows: 10 |
| Rotate / flip | `R` / `Shift+R`: ±90°; `Shift+H` / `Shift+V`: horizontal / vertical flip |
| Reset all selected transforms | `Shift+0` |
| Cancel drag | `Esc`; cancellation does not commit a move |
| Create / edit note | Double-click empty canvas / an existing note; Insert → Add note |
| New / open / save board | `Ctrl+N` / `Ctrl+O` / `Ctrl+S` |
| Paste / undo / redo | `Ctrl+V` / `Ctrl+Z` / `Ctrl+Y` |
| Duplicate / delete selection | `Ctrl+D` / `Delete`; selection-bar and context-menu controls |
| Command palette | `Ctrl+K` or More actions → View → Command palette; arrows, Enter, Esc |
| Searchable shortcut cheat sheet | `/` or More actions → View → Shortcut cheat sheet; all actions grouped by category, with current custom bindings |
| Edit bindings | Settings → Keys and mouse → Keyboard and mouse bindings, or the button in the cheat sheet |
| Settings | `Ctrl+,` / `Cmd+,`, or the visible Settings button; palette search also finds individual settings rows |
| Zoom / 100% | `=` / `−`; bind Zoom to 100% in the editor |
| Library / close tab / overlay | `Ctrl+L` / `Ctrl+W` / `Ctrl+T` |

Keyboard actions match **exact modifiers** and are suppressed in text editors,
modals and active drags. Single-shot actions do not repeat while held; nudges and
zoom may repeat. A Space tap focuses the hovered item or the selected group;
moving at least four logical screen points while Space is held commits to pan
and suppresses focus on release. Escape and loss of window focus cancel the gesture.
Visible Hand, zoom, fit, previous/next and menu controls remain pen/touch alternatives.

## Configure and preview bindings

Open **Settings → Keys and mouse → Keyboard and mouse bindings** (also available from the `/` cheat sheet). Search by localized
name or stable action ID. Each row shows an editable proposed chord and the current
binding. Leave a chord empty to unbind it. `Cmd` is the portable Ctrl/Command
modifier; `Shift`, `Alt`, `Super` (Windows/Linux), physical key names (`R`, `ArrowLeft`, `F2`, `Space`)
and `Mouse1/2/3` (left/right/middle) compose exact chords. Examples:
`Cmd+Shift+S`, `Alt+Mouse1`, `Space+Mouse1`. Keyboard actions require keys;
drag actions require a mouse button, optionally with one held key.
The existing wheel zoom/pan/off preference lives in Settings → Canvas and navigation; touch pinch and
trackpad streams retain their normalized device behavior.

The preview lists all changes and conflicts before **Apply**. Duplicate chords
and held-key gestures that would also fire another immediate key action block
Apply; the deferred focus-tap/pan pair is intentional. Nothing silently steals an
existing binding. Resolve a conflict by editing or unbinding one of its rows.
**Defaults** also previews before applying. Cancel/Escape discards the draft.
Bindings persist through the background settings worker, separately from board data.
Invalid or conflicting saved maps are rejected as a whole.

The **BeeRef-style navigation** preset uses the familiar MothRef move/Alt-duplicate,
arrows-to-nudge and Space-pan controls. The **PureRef-style navigation** preset
uses Alt+LMB pan, Cmd+Alt+LMB duplicate, Left/Right for previous/next image,
Up/Down for front/back, and Alt+arrows for nudging. Both keep Space tap-to-focus
and Space-drag-to-pan, MMB pan, and the normal board/file shortcuts. They are
navigation presets, **not full copies of either application's defaults**. Existing
custom bindings can conflict with a preset; those conflicts must be resolved in
the same preview. No unsupported screenshot feature is silently assigned.

The palette searches the same action IDs, localized labels and current shortcuts.
Unavailable actions remain visible but disabled; Enter cannot run them. Parameterized
editing/export controls remain in Tools, which is also a palette action.
The binding editor's searchable inventory shares this registry; the roadmap's
categorized Help cheat sheet uses the same registry and includes pointer gestures.

## Actions from the screenshot

| Action | Screenshot binding | MothRef status / route |
| --- | --- | --- |
| Note | `Ctrl+N` | Insert → Add note / double-click empty canvas; **conflict:** Ctrl+N creates a board |
| Always on bottom | `Ctrl+Shift+B` | Missing; M3 window capability |
| Always on top | `Ctrl+Shift+A` | Pin button; platform-dependent, defaults to `Ctrl+Shift+T` |
| Arrange by addition | `Ctrl+Alt+A` | Tools arrange order: time added; no matching binding |
| Arrange by name | `Ctrl+Alt+N` | Tools arrange order: filename; no matching binding |
| Clear canvas | `Ctrl+K` | No dedicated action/binding; command palette reserves Ctrl+K |
| Close | `Ctrl+X` | Window close control / Ctrl+W closes a tab; do not assign over future Cut |
| Grayscale | `Ctrl+Alt+G` | Tools canvas grayscale and per-image grayscale; no matching binding |
| Decrease master opacity | `Ctrl+−` | Missing master-window action; per-image opacity is a different property |
| Increase master opacity | `Ctrl++` | Missing master-window action; do not silently replace zoom controls |
| Normalize height | `Ctrl+Alt+Left` | Tools normalize height; no matching binding |
| Align bottom | `Ctrl+Down` | Tools align bottom; no matching binding |
| Align top | `Ctrl+Up` | Tools align top; no matching binding |
| Align left | `Ctrl+Left` | Tools align left; no matching binding |
| Align right | `Ctrl+Right` | Tools align right; no matching binding |
| Load | `Ctrl+L` | Ctrl+O opens boards; **conflict:** Ctrl+L opens library |
| Load images | `Ctrl+I` | Drop files / clipboard / image arguments; no dedicated image-file picker binding |
| Load most recent | `Ctrl+Shift+L` | Library/recent-board UI; missing direct most-recent command |
| Lock canvas | `Ctrl+R` | Tools lock canvas; no matching binding |
| Lock window | `Ctrl+W` | Missing window-position lock; **conflict:** closes a tab |
| Maximize | `Ctrl+F` | Titlebar maximize; no matching binding; distinguish maximize from fullscreen |
| Minimize | `Ctrl+M` | Titlebar minimize; no matching binding |
| Pack & optimize | `Ctrl+Shift+P` | Tools Pack + fit; no matching binding; fit camera is not window resizing |
| Open source | `Ctrl+Shift+O` | Missing source-opening action |
| Arrange optimal | `Ctrl+P` | Tools optimal layout; no matching binding |
| Optimize canvas | `Ctrl+O` | Fit board / Home is the camera equivalent; **conflict:** Ctrl+O opens a board |
| Overlay selection | `Ctrl+Y` | Missing per-image floating windows; **conflict:** redo. Whole-board overlay is separate |
| Custom preset 1 | `Alt+4` | Missing saved appearance presets |
| Custom preset 2 | `Alt+5` | Missing saved appearance presets |
| Custom preset 3 | `Alt+6` | Missing saved appearance presets |
| Custom preset 4 | `Alt+7` | Missing saved appearance presets |
| Dark preset | `Alt+1` | Theme setting exists; no matching preset binding |
| Light preset | `Alt+2` | Theme setting exists; no matching preset binding |
| Glass preset | `Alt+3` | Whole-board overlay exists; no equivalent named appearance preset |
| Reset camera | `Ctrl+G` | Tools Reset camera; no matching binding |
| Reset camera zoom | `Ctrl+Shift+G` | No dedicated 100%-zoom binding; separate from fit and full camera reset |
| Reset cropping | `Ctrl+Shift+C` | Tools/context-menu Reset crop; no matching binding |
| Reset transform | `Ctrl+Shift+T` | Tools/context-menu Reset transform / Shift+0; no matching binding |
| Resize to selection | `Ctrl+Shift+R` | Missing native-window resize-to-selection; not the same as fitting the camera |
| Stack | `Ctrl+Alt+S` | Tools Stack; no matching binding |

## Pointer controls from the screenshot

| Control | Screenshot binding | MothRef status / difference |
| --- | --- | --- |
| Snapped move item | LMB + Shift | Missing snapping; Shift currently extends selection |
| Change opacity | LMB + Ctrl+Alt+Shift | Per-image slider in Tools; missing drag gesture |
| Crop selection | LMB + C | Crop controls exist; missing held-key crop gesture |
| Flip | LMB + Alt+Shift | Shift+H / Shift+V and Tools; missing drag gesture; Alt-drag currently duplicates |
| Focus image | Space | Double-click focuses image or whole selected group; Space tap now focuses; Space+LMB drag pans |
| Send to front | Up | Tools layer actions; **conflict:** Up nudges selected items |
| Send to back | Down | Tools layer actions; **conflict:** Down nudges selected items |
| Focus previous image | Left | Tools Previous image; **conflict:** Left nudges selected items |
| Focus next image | Right | Tools Next image; **conflict:** Right nudges selected items |
| Pan | LMB + Alt / MMB | MMB and Hand+LMB exist; default Alt+LMB duplicates selection; optional PureRef-style preset explicitly reassigns it to pan |
| Pan while locked | LMB | Use Hand+LMB or MMB; automatic locked-canvas LMB pan is not guaranteed |
| Pan while maximized | RMB | Use Hand+LMB or MMB; RMB opens context menu |
| Pan cropping | LMB + V | Missing gesture; V currently selects the Select tool |
| Rotate | LMB + Ctrl | R / Shift+R and Tools; missing continuous drag gesture |
| Scale | LMB + Ctrl+Alt | Tools scale controls; missing modifier-drag gesture |
| Show color code | LMB + S | Tools Sample color; missing held-key gesture; external-window sampling is not complete |
| Show image coordinate | LMB + D | Tools Coordinate picker; missing held-key gesture |
| Snapped rotate | LMB + Ctrl+Shift | R / Shift+R use fixed increments; missing snapped drag rotation |
| Zoom | LMB + Z | Wheel, toolbar and pinch; missing held-key drag zoom |
| Zoom cropping | LMB + Shift+V | Missing gesture; distinct from camera zoom and frame scaling |
| Neighbour snapped move | LMB + Shift+Space | Missing neighbour snapping/guides; requires separate approval before implementation |

## Implementation priorities

These refine the existing M3/M4 roadmap, rather than marking screenshot bindings
as shipped or authorizing all missing features now.

1. **M3: one binding registry.** Stable action IDs, current-binding tooltips,
   searchable cheat sheet and command palette must share the same source.
   Detect exact chords and modifier overlaps; Ctrl+Shift+key must not accidentally
   run a Ctrl+key action. Suppress canvas actions while editing text or a modal.
2. **M3: explicit optional PureRef-style preset.** Keep MothRef defaults. Preview
   conflicts before applying a preset; include pointer gestures, not just keys.
   In particular resolve Space focus versus planned Space+LMB pan, Alt-drag pan
   versus duplicate, arrows versus nudge, and Ctrl+N/O/L/W/Y/K/X. Do not infer
   unmatched actions from partial modifier checks. Support reset to defaults.
   Exact modifiers now prevent screenshot chords from falling through to unrelated
   commands; unbound screenshot chords do nothing. The presets above implement
   only verified existing navigation actions, not missing screenshot features.

3. **M3: navigation and batch editing first.** Preserve the selected group on
   focus, expose previous/next and fit selection through visible controls, then
   add held-key pan/zoom/scale/rotate/crop/picker gestures through the pointer
   abstraction. A click versus drag threshold must disambiguate Space focus/pan.
   Test mouse, pen and touch alternatives, cancellation and Command+undo identity.
4. **M3: window/appearance actions separately.** Native-window resize, master
   opacity, window lock and per-image overlay are not camera or item transforms.
   Use platform capabilities and show unsupported actions honestly. Destructive
   clear must be a reversible command batch; opening source is an explicit user action.
5. **M4: snapping and richer appearance.** Grid/neighbour snapping needs bounded
   spatial queries, screen-space thresholds, a bypass modifier and guide feedback;
   implementation remains subject to the maintainer's separate approval. Theme
   presets must specify which appearance properties they save.

## Code used for this comparison

- [Pointer adapter](../crates/moth-platform/src/input.rs): pointer kinds, wheel,
  pan and normalized zoom.
- [Action registry](../crates/mothref/src/app/bindings.rs), [binding editor and palette](../crates/mothref/src/app/shortcuts.rs), and [shared action runner](../crates/mothref/src/app/actions.rs).
- [Application input and intents](../crates/mothref/src/app.rs): keyboard dispatch,
  focus selection, camera fitting and command routing.
- [Canvas](../crates/mothref/src/app/canvas.rs): double-click and context menus.
- [Surfaces](../crates/mothref/src/app/surfaces.rs): toolbar, selection bar and grouped context menu.
- [Settings](../crates/mothref/src/app/settings.rs): persisted preferences and schema-driven settings window.
- [Tools](../crates/mothref/src/app/tools.rs): transient geometry, arrangement, export and picker editors.
- [UI tooltips](../crates/moth-ui/src/lib.rs): advertised bindings.

This is a code audit, not a manual qualification of every listed interaction.

## Automated validation and platform limits

Registry tests cover exact modifiers, defaults, preset and held-key conflicts,
settings round trips and rejection of malformed maps. Input tests cover Space
tap/drag/cancel, focus isolation and repeat handling. Synthetic egui tests exercise
palette arrows, Enter, unavailable actions and Escape, and current/unbound tooltip
labels. Adapter tests preserve left/right/middle identity through a full gesture.
Manual native input qualification on Linux/Windows/macOS, hardware pen/touch and
non-US keyboard layouts remains unverified; physical key positions are used,
as in MothRef's previous command shortcuts. No PureRef binary was inspected.

Validation for this change: workspace clippy and tests passed, followed by the
final changed-crate tests after focus/gesture guards were tightened. The tier-2
quick benchmark rendered 10,000 images at 4.816 ms p99; `cargo xtask soak 300 5`
completed 2,828 frames, with RSS stable at 194,324 KiB from 45 seconds onward.
Tier-3 checks were not run in the agent session.

## Arrangement and shortcut feedback

Tools → Arrange offers integer sliders with editable exact values. Horizontal uses a row count and fills each row left-to-right before moving down. Vertical uses a column count and fills each column top-to-bottom before moving right. Grid uses a column count and fills rows left-to-right. Counts are clamped to the affected selection (or all items when selection is empty). Uneven horizontal/vertical layouts put the extra items in the first rows/columns. Arrangement preserves each frame size and uses one undo step.

Shortcut feedback displays the executed action and its current binding for 3.5 seconds, then dissolves over half a second. Hover holds it; pointer leave dismisses it. Tab focus holds it too; activate Hold with Enter/Space or touch to keep it visible, and use Dismiss or Escape to close. Repeated actions refresh one toast. Settings → Show shortcut feedback disables it. Reduced motion replaces the dissolve with immediate dismissal at four seconds.

## Window controls

`Ctrl+Shift+T` (`Cmd+Shift+T` on macOS) toggles the last session pin choice.
Use the pin menu to choose all windows or click a target (Windows only); Escape
or Cancel cancels selection. Unsupported choices are disabled by platform capabilities.
`Ctrl+Shift+M` (`Cmd+Shift+M`) arms native window movement; drag the canvas, or
cancel with Escape/the Cancel button. `F11` toggles borderless fullscreen.
These actions are rebindable and available from the command palette. Pointer
approach, Tab focus and the Window controls button reveal the titlebar controls.
