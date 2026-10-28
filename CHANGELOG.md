# Changelog

## Unreleased

- Rework the window chrome. View shows only images: a floating window bar slides in at the top edge (drag grip, board switcher, View, pin, overlay, window buttons) and folds rarer controls into "⋯" on narrow windows; the permanent corner button is gone, and touch uses "Window controls" in the context menu. In Collect, window controls fade in over the top bar without a separate toggle button, maximized windows show a restore icon, and tabs are chips with their close button inside.

- Keep MothRef above other windows on KDE Plasma and GNOME Wayland: above all windows, the Over overlay and floating views now stay on top. GNOME needs the MothRef window extension, which Settings › Window and overlay can install (log out and back in once).
- Pin the board above a chosen window on KDE Plasma Wayland.
- GNOME: keeping a maximized board on top restores it first, since GNOME does not keep maximized windows above others. Settings offers to update an outdated window extension.
- macOS: windows kept above all also appear on every desktop and next to full-screen apps.
- Windows: pinning above a chosen window also works when that window is itself always on top.
- Disable "always on bottom" on Wayland, where compositors cannot provide it.

## 0.1.0-alpha.14 — 2026-10-28

- Default View (Tab) window controls to the right edge; add a persistent switch in Settings › Window and overlay to place them on the left.

- Put New board, Open, Save, Library and Settings directly in the MothRef menu. Move Navigation and view into a compass menu on the bottom toolbar. Keep View window controls together, remove the Move window button, and expose both pin modes in View.
- Use Ctrl/Command+O for Settings, migrate the previous default shortcut pair, and keep Open available from the application menu. Preserve custom shortcut assignments.
- Restore multiple-selection frames in View (Tab) without revealing the editing toolbar.
- Add X11 pinning above a selected application window using native foreground events on a worker, with Escape cancellation and cleanup when the target closes.

- Fix View image/window opacity replacing the canvas background with black or transparent pixels when adjusting appearance and panning.

## 0.1.0-alpha.13 — 2026-10-27

- Add View › Window actions for always on bottom, locking window position, floating selected images, sizing the window to selected/all items, and opacity steps (Ctrl/⌘+Alt+Up/Down). Floating views share board textures and are limited to 32 windows.
- Add custom background, canvas and selection colors in Settings › Appearance, with individual reset controls.
- Add opt-in system shortcuts in Settings › Window: Ctrl+Alt+Shift+T for Over, C for click-through, and Up/Down for opacity. X11 waits for native events; Windows/macOS use native registrations. Wayland retains the Over controller. Click-through can turn off when the board is activated.

- Keep the compact titlebar drag target beside its controls instead of wrapping it onto another row.
- Release application pointer capture when the compositor takes over moving or resizing a window, so consecutive gestures work on the first attempt on Wayland.

- Fix a freeze when Tab switches to View or the compact window bar reveals under the pointer.

- Reveal the window bar at the top of Over as well as View; drag its corner handle or free area to move the board, and resize frameless windows by their edges and corners in every mode.
- Keep Over appearance controls and the exit button reachable at the default controller size; scroll controls in short windows.

## 0.1.0-alpha.12 — 2026-10-27

- Fix packing occasionally leaving rotated images unmoved when rounding exhausts the layout search; retain strict fit checks and original image sizes.

## 0.1.0-alpha.11 — 2026-10-26

- Unblock Windows CI by decoding recovery paths with fixed-size byte chunks. Check canonical save destinations in recovery tests, including symlink aliases, so macOS temporary-directory aliases do not fail a valid restore.

## 0.1.0-alpha.10 — 2026-10-26

- Switch Collect / View with Tab and Over with Ctrl/Command+T; narrow windows use View. View keeps a sliding window bar, board switcher and context-menu editing. Share image, board and window opacity plus solid/translucent/glass backgrounds between the View popover and Over controller. Remember presentation for the 64 most recently adjusted saved boards; use a translucent glass fallback where native blur is unavailable.

- Open BeeRef and PureRef boards as editable, unsaved conversions; offer Save As .moth or Continue without saving, explain retained content and conversion limitations, and preserve the original source. Restrict working-board saves to .moth and keep unsaved conversions recoverable.
- Admit board files independently of individual image limits: BeeRef/PureRef boards use a 4 GiB file limit and bounded metadata/payload streaming; MothRef boards use an 8 GiB desktop admission limit with existing record and checksum protections.

- Adjust minimap opacity and distance from the canvas center in Settings › Canvas; keep the selected corner or edge as the direction.

- Configure minimap visibility, maximum size and placement in any of nine positions in Settings › Canvas; preserve adaptive board proportions and viewport limits.

- Accept images dragged from local ZIP archives in Gwenview on Wayland; read selected entries in background workers through the normal image import and journal path.

- Choose continuous keyboard camera flight or stepped panning in Settings › Canvas; configure flight speed and step distance in screen pixels. Flight follows rebound keys, keeps diagonal speed constant, and stops during text entry or dialogs.
- Escape clears the canvas selection after active drags, menus and tool windows have been dismissed; also offer Deselect in the command palette.

- Add rebindable WASD canvas panning, blocked while typing; use trackpad scrolling to pan, Control-scroll and native pinch to zoom. Keep existing visible zoom and fit controls.
- Move the minimap to the bottom left and adapt its proportions to the board, bounded to 30% of the viewport and a compact maximum size.

- Choose smooth, sharp-pixel or automatic image sampling in Settings › Canvas, with per-image overrides in the selection/context menus and item editor. Automatic switches to sharp pixels at 200% of original pixel size.
- Auto-size notes to their text, or wrap at a manually resized width while preserving text size. The item editor controls wrapping and font size; grouped scaling scales text with its frame. Export preserves fixed note fonts and wraps text.
- Save sampling and note layout in schema 5; continue reading schemas 1–4 and upgrade older boards through atomic compaction. Older MothRef builds cannot open newly saved schema-5 boards.

- Open a selected original in the system image viewer from the context menu, export window or command palette. Viewer copies are bounded and never replace board originals.

- Remove the separate Hand/Pan canvas mode, its toolbar button and H binding. Keep Select active with Space-drag, middle-drag and two-contact touch panning.

- Fix Ctrl/Command, Shift and Alt pointer gestures being rejected after physical modifier key presses; keep modifiers separate from held-key gestures such as Space-drag.

- Add Ctrl/Command-click and rectangle selection toggling, plus additive Shift rectangles. Preserve selected-group dragging and list both gestures in the configurable shortcut cheat sheet.

- Add proportional corner resizing for single images and multi-selection, with an anchored opposite corner, live previews through journal acknowledgment, Escape cancellation and one undo step.
- Arrange completed image additions, fit after the journal acknowledges the layout, and show method switching plus Undo for the whole import. Keep the maintainer-selected default of asking above five images.
- Keep editor and export windows open after actions; show picker results and export progress, success and errors in their tool windows. Clarify export destinations and formats, and cancel an armed picker when its window closes.
- Prevent a resize-time GPU scissor validation crash by using the acquired surface texture dimensions. Stop old recovery offers appearing when undo empties an active board.
- Add transform/crop/duplicate/delete icons and dropdown indicators, skip unavailable commands during palette keyboard navigation, simplify empty-board wording and route Choose files… to image insertion. Omit zero note/image counts from the save chip.

- Keep Settings navigation and controls in separate vertical columns. Remove the decorative dark grid so the canvas follows the selected paper or Screen theme.

- Use the light paper theme by default, preserving saved theme choices. Bundle OFL-licensed Bricolage Grotesque Bold for the wordmark and headings, with Noto Sans fallback for translated text.

- Keep the selection frame and selection bar at the dropped position while the journal records a move; prevent a brief jump back to the old position.

- Keep the camera position and zoom unchanged after moving, arranging, aligning or resizing items, including undo and redo.
- Fix dragged images drifting away from the cursor when zooming or wheel-panning in the middle of a drag (or while smooth zoom is still settling): the drag is now anchored to the board, so only the board zooms around the cursor and the images stay where you grabbed them. Rubber-band selection keeps its starting corner the same way.
- Recolor the dark theme with the «Pixel» palette from docs/DESIGN.md: green-grey Screen background, Bone text, Sage accent, Wing for destructive actions. In the light «Album» theme, primary buttons now use dark Graphite text instead of near-white text on sage, idle buttons have an outline, and selected icons use Moss so they stay visible on paper.
- Put save state and separate, correctly inflected image/note counts in the window bar. Translate stored status messages when drawing, keep the selection badge on one line, stack shortcut feedback above the toolbar, and keep the selection bar inside the window.
- Give menus reserved icon columns, vector submenu arrows and named Align/Order groups. Move File and View actions to window-bar menus, keep one Settings gear, add an explicit Images… picker, and align palette rows with readable shortcut symbols.
- Center Settings with independent navigation/content scrolling and a fixed search field; clarify adding-image labels and move packing heuristics to For developers. Improve light-theme text/selection contrast and secondary buttons, and show one Open file… entry in the library.
- List recovery entries by title and modification time with Restore/Discard controls. Record the original save destination beside each journal and preserve it on restore; archive restored journal bytes and refuse to discard live journals or files outside the recovery folder. Legacy journals without destination metadata (or with damaged metadata) restore as unnamed boards and require Save As.

- Keep notes at the editor’s visible text size when created at any zoom, and preserve the text scale of existing notes when editing.

- Default to Select on new and reopened boards, and allow double-click focus with either navigation tool. Frame boards, selections and newly added images clear of the window bar and floating toolbars.

## 0.1.0-alpha.9 — 2026-10-24

- Keep wrapped toolbar buttons inside narrow viewports by measuring native button widgets directly. Verify fallback arrow rendering by comparing rasterized glyphs rather than egui's fallback-face detection.

- Replace Board tools with a floating bottom toolbar, a selection bar and grouped
  context menus. Arrangement, geometry editing, export and pickers have their own
  small windows; all editing commands also use the shared command palette.
- Open Settings with Ctrl/Command+comma or its visible button. Preferences are
  grouped and searchable from the palette, apply immediately and retain existing
  settings-file keys. Language moves to General; adapter, memory and journal
  diagnostics move to For developers.
- Add token-based paper light and existing dark themes, keep the selected theme
  across GPU recovery, and draw four handles around a combined multi-selection.
  Show a compact save-state chip, list every supported image format on empty
  boards, add missing-arrow font fallback, and order zoom controls as minus,
  percentage, plus.

## 0.1.0-alpha.8 — 2026-10-24

- Update the Windows clipboard dependency to 5.4.3, replacing the yanked 5.4.1 release.

- Skip zero-sized overlay-controller frames, refresh its surface after uncovering or display-scale changes, and retry after surface recovery. Disable click-through controls when the platform reports no support.

- Restore visible arrangement gutters when old settings contain a zero gap: use the default 24 units and require a positive gap for Grid, rows and columns. Optimal remains gapless.

- Grid now packs a compact mosaic with the same gutter on both axes, preserving image sizes and rotations. Short images no longer acquire oversized vertical padding from uniform cells. Horizontal and vertical layouts retain their row/column controls.

- Optimal arrangement now packs the selected images into a compact square without gaps, independently of the viewport or zoom, preserving sizes and proportions. Other layouts keep their configurable gap. Add distinct arrangement icons to the inspector, context menu, import prompt and default-method choices.

- Fix window-control reveal deadlocking egui and keep the canvas usable on narrow screens, including boards with several long tab titles.

- Reveal window controls on pointer approach, keyboard navigation or an explicit touch button; add rebindable pin, window-move and fullscreen actions. Windows can pin above a clicked target window, with cancellation and a visible active mode.
- Hide Windows GPU-probe console windows, retain rotating diagnostic logs without a terminal, and package macOS releases as Finder-launchable app bundles. Native launch and transition qualification remains pending.
- Redraw and reconfigure after uncovering the window or changing display scale; keep maximize/restore separate from borderless fullscreen.

- Arrangement controls offer exact row/column sliders: horizontal fills rows left-to-right and vertical fills columns top-to-bottom. Grid uses a compact mosaic with gutters. Sizes stay unchanged and each arrangement is one undo step.
- Shortcut feedback shows the action and current binding for 3.5 seconds, then fades; hover or keyboard focus holds it, pointer leave dismisses it, and Hold/Dismiss buttons support keyboard and touch. Disable it in Settings.
- Page Up/Page Down jump to the previous/next image; both bindings can be customized.
- Open the searchable, categorized shortcut cheat sheet with `/` or Help; it includes current custom bindings and pointer gestures.

- Reimplement rotation snapping and optimal arrangement independently: finite packing
  spaces, atomic rejection of out-of-range layouts, and overflow-safe angle rounding.

- Fix offset canvas selection and dragging when the interface scale differs from 100%.

- Load boards several times faster in the full-resolution mode: each image is decoded
  once (preview and all tiles from the same decode), coarser levels are built from the
  previous level instead of from the full image, opaque images use a faster BC7
  preset, and reopening uploads larger batches without rescanning video memory.

- New default performance mode "Everything at full resolution": when a board opens,
  every image is loaded into video memory at full resolution behind a progress bar,
  and zooming or panning never loads anything afterwards. If the board does not fit,
  MothRef says so and the rest loads while you zoom. Later openings read prepared data
  from the cache folder instead of decoding again.
- Detect the graphics card's memory and set the video-memory limit from it (80% of
  dedicated VRAM, half of RAM for integrated graphics) instead of the old 32–512 MiB
  cap; the detected card and limit are shown on start and in Settings, where the
  limit can be overridden.

- Bake large images into tile packs in the background the first time they are shown:
  one decode writes every detail tile, so later zooms read ready GPU tiles instead of
  re-decoding the original. While a tile loads, draw the nearest coarser tile instead
  of the blurry 256-pixel preview.
- Add a "Prepare zoom tiles" setting (English/Russian): for images on screen
  (default), for the whole board in the background, or off. Settings show how many
  images are still waiting.
- Use less RAM in the default "In memory" mode: 12.5% of available RAM (at most 1 GiB)
  instead of 25% (at most 4 GiB), 2× instead of 4× preview headroom, and a disk copy
  that serves evicted previews without decoding. The disk cache budget grows to 8 GiB.

- Build image mip levels with integer pixel averaging instead of a full-frame
  floating-point resize, reducing zoom-detail latency and temporary RAM use.
  Keep the highest-detail pixels unchanged.

- Show RAM cache usage and its budget, disk cache usage, and a background disk-cache
  clear action in Settings (English/Russian).
- Reduce zoom decoding work by sharing source reads and codec passes across up to
  eight neighboring tiles. Use scaled JPEG IDCT below 1:1, smaller accumulation
  buffers, and keep 256-pixel fallback previews from caching 1024-pixel pyramids.
  PNG/GIF/BMP previews at 1:1 copy pixels without allocating per-pixel area sums.
  A bounded eight-result queue lets a complete tile batch become ready between frames.

- Update rustls-webpki to 0.103.16 to fix TLS certificate advisory RUSTSEC-2026-0382.

- Prioritize image requests by visible area and distance to the viewport center,
  replace superseded preview work, and request the target detail during smooth zoom.
- Inspect large images beyond 2048 px through a fixed-size tile atlas. Only visible
  regions produce pixels; cached tiles reuse the existing RAM/disk budgets. Pan and
  zoom-out release unneeded tile residency, with previews filling missing regions.
  Fit detail to whole-page VRAM capacity and replace individual LRU layers without
  downgrading every neighboring image in the page.

- Release oversized image textures after zooming out by copying resident lower
  mips on the GPU, without decoding again; keep headroom for small zoom reversals.
  OpenGL uses RGBA textures because its compressed-texture copies are unsupported.

## 0.1.0-alpha.7 — 2026-10-20

- Add a performance mode to the image-cache settings, as in PureRef: In memory
  (default), In memory with low zoom quality, Cache on disk and No cache. In the
  caching modes an image is decoded once per zoom range and later zoom steps reuse
  the cached detail levels instead of decoding the original again.
- Keep in-memory detail levels within 25% of available RAM (at most 4 GiB) and the
  disk cache within 2 GiB of the OS cache folder, evicting least recently used entries.
- Fix images that never appeared on Apple silicon and Windows on Arm adapters that
  report BC texture support: BC7 is now chosen only where it can be encoded.

- Keep image residency when closing or reselecting the active board, avoiding a
  preview reload before the save/discard confirmation.
- Keep wheel and trackpad scrolling out of the canvas while board tools are open.

- Replace percentage-based image-cache preferences with one persistent 32–512 MiB
  maximum (96 MiB by default), retaining automatic eviction under memory pressure.

- Add persistent smooth/immediate zoom and 1–100% zoom-step preferences,
  shared with toolbar buttons and shortcuts; preserve pointer anchors and fractional
  trackpad input across display scales.
- Tap Space to fit the selection or, with no selection, the whole board. Space-drag
  pans without fitting on release; typing does not arm either binding.
- Pack mixed-size images into free space using their displayed rotated bounds,
  preserving sizes and aspect ratios, supporting zero gap and one-step undo.

## 0.1.0-alpha.6 — 2026-10-20

- Launch the Windows app without opening a console window.
- Fix the Windows CI unused-import error that blocked release binary publication.

## 0.1.0-alpha.5 — 2026-10-20

- Add persistent image-cache settings: available-RAM allowance (1–50%), automatic
  adapter limits or a manual 32–512 MiB ceiling, with immediate application and
  reset to defaults. Include English and Russian explanations of preview eviction.

- Grow the GPU image cache by actual texture-page bytes instead of image count,
  limiting it to one eighth of available RAM plus reclaimable image pages and
  the existing adapter ceilings. Sample RAM in the background during activity
  and evict LRU pages when memory becomes scarce.

- Specialize area-preview accumulation by pixel format and reuse already-sized
  decoded images, preserving alpha-weighted pixels, mip quality and decode budgets.

- Show journaled image batches while boards are still loading, overlapping preview
  decoding with storage. Keep editing/saving blocked until completion and preserve
  partial loads as unsaved boards on errors. Read healthy `.moth` files through
  their footer chain while retaining checksum verification and corruption recovery.

- Read image originals directly from recovery-journal ranges when opening boards,
  importing BeeRef/PureRef files and adding images. Avoid duplicate `.source`
  files and their rereads while retaining journal-before-acknowledgment durability.

- Add configurable exact keyboard and mouse chords, searchable binding settings, conflict previews, reset to defaults and optional BeeRef/PureRef-style navigation presets. Use Space to focus on tap or pan on drag; show current bindings in tooltips and menus.
- Add the searchable command palette through Ctrl/Command+K or the Commands menu, with arrow-key navigation, Enter to run and Escape to close. Canvas actions stay isolated from text editors and modals.

- Rotate and flip selected items around their shared center; expose group rotation, flip and deletion in Tools, show the selection count, and prevent empty-selection deletion or duplication. Batch edits retain one undo step.

## 0.1.0-alpha.4 — 2026-10-19

- Run portable GPU tests on the available adapter instead of requiring a software adapter that Metal does not provide; retain an explicit software-rendering check on Linux.

## 0.1.0-alpha.3 — 2026-10-19

- Sharpen newly loaded previews and higher-detail zoom images gradually over 180 ms using existing mip levels, with bounded transitions and instant updates under reduced motion.

- Double-click a selected image to fit the entire selection's width and height in the canvas; preserve the group instead of focusing only the clicked image. Restore the original wheel and toolbar zoom behavior.

- Adapt the GPU image-cache limit to image count, release pages when it shrinks, and show allocated/current-limit memory in the status bar; cap discrete GPUs at 512 MiB and shared/software adapters at 96 MiB.

- Keep budgeted low-resolution GPU fallbacks while zoom detail is replaced or evicted, avoiding blank image frames during rapid zoom; ignore late lower-quality decodes.

- Discard stale drag-animation copies when another command moves, edits or removes the real item, preventing non-interactive images at old coordinates.

- Add a persistent setting to hide item selection/hover outlines and selection handles.

- Add persistent drag-animation and fallback-opacity preferences: without drag motion, images become translucent only while following the pointer.

- Make selection and hover outlines follow image sway, lift, opacity and geometry transitions using the same animation frame as the image renderer.

- Keep dragged images and selection outlines at their released position until storage acknowledgement, without replaying the move animation.

- Add canvas cursors and mouse/pen hover outlines, animated toolbar/tab feedback and a subtle library-card lift.

- Animate item entry, removal and acknowledged arrangement/undo geometry changes using bounded presentation copies.

- Smooth discrete wheel zoom around the pointer and fit/focus transitions; keep continuous input direct and add touch-pan inertia.

- Add grab-point drag sway, a gentle lift and animated cancellation; add a localized reduced-motion preference.

- Preview the entire selection during dragging, including Alt-drag copies, without changing board data.

- Add view-only motion primitives with exact endpoints and reduced-motion support.

- Build interface languages from translation catalogs, with automatic menu registration,
  English fallback and validation; document how to add a language.

- Keep the canvas tool rail compact instead of letting its separator stretch a
  panel across the board; preserve wrapping controls in short windows.

- Add Alt-drag duplication, selection dragging, arrow-key nudges, alignment,
  distribution, stacking, layer controls and repeat-to-reverse arrangement.
- Add per-tab canvas locks, canvas grayscale, image-focus toggling and persistent
  theme, interface scale, undo-depth, wheel and arrangement preferences.
- Export boards/selections as bounded raster PDFs and unchanged originals as ZIP;
  add RGB/HSV color-copy choices and original-pixel coordinate information.
- Fix grouped undo/redo ordering for dependent commands, including arranging an
  insertion burst as one undo step.

- Add saved/journaled and recovery status, editable toolbar zoom, live selection
  measurements, shortcut tooltips, and wrapping controls for small screens.

- Speed up bulk image imports and board opening by syncing originals with their
  command/checkpoint in the recovery journal, and reusing foreign-import working
  copies. Recovery still rebuilds originals from the journal before use.

- Add Board tools with Apply-based frame scaling, arbitrary rotation, crop-content
  pan/zoom/rotation, flips, opacity and grayscale; edits are journaled and undoable.
  Arrange selections with a custom gap and source-filename ordering, or scale them
  together. Schema 4 preserves appearance and filenames while reading schemas 1–3.
- Export scenes, selections and cropped regions to PNG/SVG, copy images and sampled
  HEX colors, and export one or all original images without re-encoding. Exports
  never overwrite files. Scene/clipboard exports and sampling use up to 2048-pixel
  source previews; scene output is capped at 4096 pixels on its longest side.


- Accept `.pur` boards up to 4 GiB with separate payload and metadata budgets;
  legacy boards now stream originals instead of buffering the whole file.
  Import SQLite columns by name, fixing skipped images in PureRef 2.1.2 boards
  whose column order differs from 2.1.3. The 175-image FemaleFighterPosesPack
  board passes import, `.moth` reload and grouped undo/redo checks.

- Open PureRef `.pur` boards through file open, command-line arguments and drops.
  Import supported embedded images and plain notes on the storage worker with
  journal recovery and one-step undo. A local purcore safety fork bounds memory
  and nesting; captured 1.10/2.1 boards are tested. Unsupported items are reported
  as skipped; links, rich notes, drawings and some crops remain unsupported.

- Import BeeRef text notes alongside images, and add text notes to the board
  format (schema 3): notes are imported, saved, reopened, created (context menu or
  double-click empty canvas) and edited in place (double-click a note). Schema 1
  and 2 boards still open.
- Carry BeeRef item rotation, horizontal flip and scale through the import:
  rotated/flipped images render correctly, and notes keep their rotation and
  scale (flip is preserved in the file).

- Open images and boards through the native file picker instead of typing a
  path: the Open button (canvas, empty board and library) and Ctrl/Cmd+O now
  browse the OS file manager and add the selected files to the board.

- Choose the image texture format at runtime: BC7 on desktop adapters that
  support it, uncompressed RGBA8 otherwise (including the software adapter).
  Previews are encoded to BC7 on the decode pool, quartering texture memory for
  the same residency. ASTC for mobile is deferred to the mobile milestone; the
  `.moth` format stays GPU-format independent.

## 0.1.0-alpha.2 — 2026-10-17

- Keep Undo, Redo and Save visually steady while edits are acknowledged by the journal.

- Fix crossed selection outlines and remove the filled drag preview: both now use a simple theme-colored frame without diagonals or tinting.

- Group multi-item edits into a single undo step: arranging, normalizing, rotating,
  mirroring, resetting or deleting a selection, the transform hotkeys, a `.bee`
  import, and a drop/paste burst of many images now all undo (and redo) with one
  Ctrl+Z. The previous 32-item selection cap is removed.
- After one drop or paste adds more than five images, ask how to arrange them
  (horizontal, vertical, grid or optimal) and place the result under the cursor.
  The chosen arrangement is a separate undo step; the threshold is a constant until
  a settings store lands. Pasted and dropped images now both land at the cursor.
- Add interface localization: English and Russian, with the OS language detected on
  first launch and a language picker in the title bar. The choice persists in a
  plain-text settings file next to the recent-file catalog.
- Add item content transforms to the scene and board format (schema 2). An item now
  stores frame rotation, content flip, pan, zoom and content rotation as a reversible
  `SetTransform` command, and rotated items are indexed by their rotated bounds.
  Schema 1 boards and journals still open, with the identity transform.
- Render and edit item transforms: content is fitted into the frame without distortion and
  clipped to it, so `content_scale`, `content_offset` and `content_rotation` crop, pan and
  spin the source inside a frame, and a flipped item mirrors its source. Selection outlines
  the rotated frame, clicks hit the rotated rectangle, the context menu and the
  `R`/`Shift+R`/`Shift+H`/`Shift+V`/`Shift+0` keys rotate, mirror and reset the selection,
  and the minimap and drag ghost follow the rotated footprint.
- Reject cyclic or excessively deep BeeRef table trees instead of overflowing the importer stack.
- Bound JPEG preview memory independently of worker scheduling, fixing allocation-test failures on macOS.
- Fix first-time saving of library pins and tags on Windows when the metadata file does not yet exist.
- Import BeeRef `.bee` boards by reading their embedded SQLite database directly:
  images keep their layout and stacking order, every original is journaled before
  the import is acknowledged, and notes or transforms MothRef cannot yet store
  are reported as skipped.
- Implement the reference workspace: board library with previews, folder filters, search, persistent pins and tags; four independent board tabs; empty-board import prompt and save-on-close dialog.
- Add the floating tool rail, selection handles, minimap, image dragging, arrangement and size normalization, plus a dot-grid canvas.
- Expand the selection menu: normalize height, width, size and area, and arrange
  horizontal, vertical, grid and optimal (packed to the window width), all as
  reversible commands.
- Read image and file clipboard contents on Wayland sessions by enabling arboard's
  `wayland-data-control` backend, fixing pasting a copied PNG returning
  "no supported image" on KDE Wayland and other compositors.
- Add transparent overlay mode with master opacity, click-through and a separate control window; show platform availability for always-on-top.
- Fix idle redraw loops, fitting boards opened from the library, and saving to relative filenames; prevent two tabs from writing the same destination.

- Accept native Wayland file drops and tablet-tool input with pen identity, pressure, hover and contact cancellation; keep touch pan/pinch in the gesture layer.

- Paste a direct HTTP(S) image URL or the first HTML image; download on the clipboard worker with bounded size/time and preserve original bytes for recovery.

- Import static GIF and uncompressed/bitfield BMP previews by rows; add bounded static WebP imports (8 MiB input and RGBA frame limits), including lossless/lossy, transparency and EXIF orientation.

- Clipboard staging hashes PNG bytes during encoding and reuses that identity for previews, avoiding redundant pixel hashing and rereading the staged file.

- Speed up durable clipboard staging with larger PNG write chunks and lossless RGB
  storage for fully opaque images; preserve RGBA when any pixel is transparent.

- Render visible images in ordered batches beyond the old 10,000-item ceiling,
  with a separate bounded import backlog and explicit queue feedback.
- Index fractional z-order to avoid scanning the board on every image insertion.
- Accept large JPEGs when reduced decoding fits the memory budget, honor JPEG
  EXIF orientation, convert CMYK colors and avoid enlarging small previews.
- Average PNG and clipboard preview areas (including Adam7 and transparency)
  instead of sampling individual source pixels.
- Paste native clipboard file lists through the image import path and explain
  empty/unsupported clipboard contents in the UI.

- Keep canvas drags owned by their starting pointer, cancel selection/pan when
  pinching or losing focus, and ignore gesture starts over the toolbar.
- Normalize touch pressure to a finite 0–1 range and ignore duplicate contacts.

## 0.1.0-alpha.1 — 2026-10-16

- Add version-one `.moth` boards, append-only saves, atomic background compaction,
  synced command journals, rotating backups, startup recovery and recent files.
- Add Save/Open path controls, save-on-close and undo/redo (256 session commands),
  with visible toolbar controls and Ctrl/Cmd+S, Ctrl/Cmd+Z and Ctrl/Cmd+Y.
- Add local crash reports and native termination handling. Tagged releases build
  Windows, macOS (Intel/Apple Silicon) and Linux binary archives from M1 / 0.1.

- Store clipboard originals as lossless PNGs in application data, grouped by board,
  with an 8 GiB disk budget per board. Explicit discard cleans that board's files;
  crashes retain them for future recovery tooling.

- Add the M0 native canvas preview: image file drops, clipboard paste, pan/zoom,
  rubber-band selection, fit controls, adapter status and a custom egui theme.
- Add GPU fallback, bounded texture residency, background LOD decoding and a
  headless benchmark/validation harness.
- Preview detail is capped at 2048 pixels. Native Wayland file drops and
  the 50 ms end-to-end paste target remain open.
