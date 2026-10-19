# Canvas controls and PureRef shortcut comparison

Reviewed against MothRef input code on 2026-10-19. The PureRef column transcribes
the maintainer-supplied **Actions / Controls** screenshot; its version and custom
binding state are unknown. It is a compatibility reference, not a claim about
current PureRef defaults. No PureRef executable was inspected. The screenshot is
not redistributed here.

`LMB` = left mouse button, `MMB` = middle mouse button, `RMB` = right mouse button,
`Space` = the screenshot's `Spc`. Drag combinations mean holding the listed keys
while dragging. MothRef command shortcuts use Command on macOS instead of Ctrl.
Entries marked **missing** or **partial** are requirements, not available shortcuts.
An action being present in Tools does not mean its PureRef binding is implemented.

## Available MothRef controls

| Task | Current control |
| --- | --- |
| Select / Hand tool | `V` / `H`; visible tool-rail buttons |
| Pan | MMB-drag, or LMB-drag with Hand selected; two-contact touch pan |
| Zoom | Wheel at cursor, toolbar `+` / `−`; pinch / continuous trackpad zoom |
| Fit entire board | `Home`, Fit board button |
| Fit selected group | Select images, double-click any selected image: fit the combined rotated bounds in both viewport dimensions with padding; double-click the same image again to return |
| Focus one image | Double-click an image outside the selection |
| Move / duplicate selection | LMB-drag / Alt+LMB-drag; selected images move as a group |
| Add to selection | Shift+click; drag empty canvas for a selection rectangle |
| Nudge selection | Arrow keys: 1 scene unit; Shift+arrows: 10 |
| Rotate / flip | `R` / `Shift+R`: ±90°; `Shift+H` / `Shift+V`: horizontal / vertical flip |
| Reset all selected transforms | `Shift+0` |
| Cancel drag | `Esc`; cancellation does not commit a move |
| Create / edit note | Double-click empty canvas / an existing note; Add note in Tools |
| New / open / save board | `Ctrl+N` / `Ctrl+O` / `Ctrl+S` |
| Paste / undo / redo | `Ctrl+V` / `Ctrl+Z` / `Ctrl+Y` |
| Library / close tab / overlay | `Ctrl+L` / `Ctrl+W` / `Ctrl+T` |

Keyboard actions are subject to canvas focus, text-editor/modal focus and pending
storage work. Window features depend on platform capabilities. The visible Hand,
zoom, fit and Tools controls are important alternatives for pen/touch and mice
without a working wheel or MMB.

## Actions from the screenshot

| Action | Screenshot binding | MothRef status / route |
| --- | --- | --- |
| Note | `Ctrl+N` | Add note in Tools / double-click empty canvas; **conflict:** Ctrl+N creates a board |
| Always on bottom | `Ctrl+Shift+B` | Missing; M3 window capability |
| Always on top | `Ctrl+Shift+A` | Pin button; platform-dependent, no matching binding |
| Arrange by addition | `Ctrl+Alt+A` | Tools arrange order: time added; no matching binding |
| Arrange by name | `Ctrl+Alt+N` | Tools arrange order: filename; no matching binding |
| Clear canvas | `Ctrl+K` | No dedicated action/binding; future command palette also reserves Ctrl+K |
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
| Focus image | Space | Double-click focuses image or whole selected group; no Space focus binding |
| Send to front | Up | Tools layer actions; **conflict:** Up nudges selected items |
| Send to back | Down | Tools layer actions; **conflict:** Down nudges selected items |
| Focus previous image | Left | Tools Previous image; **conflict:** Left nudges selected items |
| Focus next image | Right | Tools Next image; **conflict:** Right nudges selected items |
| Pan | LMB + Alt / MMB | MMB and Hand+LMB exist; **conflict:** Alt+LMB duplicates selection |
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
   Current native keyboard dispatch checks the command modifier without exact
   Shift/Alt matching for several actions; screenshot chords such as Ctrl+Alt+N
   can therefore fall through to New board today. They are not safe aliases to try.
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
- [Application input and intents](../crates/mothref/src/app.rs): keyboard dispatch,
  focus selection, camera fitting and command routing.
- [Canvas](../crates/mothref/src/app/canvas.rs): double-click and context menus.
- [Tools](../crates/mothref/src/app/tools.rs): arrangement, transforms and pickers.
- [UI tooltips](../crates/moth-ui/src/lib.rs): advertised bindings.

This is a code audit, not a manual qualification of every listed interaction.
