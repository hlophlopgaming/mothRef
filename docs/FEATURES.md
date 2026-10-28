# MothRef feature inventory

What MothRef must eventually do, at three levels of detail: a headline feature matrix with priorities, the full PureRef 2 + BeeRef command checklist, and the Milanote checklist. [`ROADMAP.md`](../ROADMAP.md) says *when*; this file says *what*. A milestone is not done until every item tagged with it here is ticked.

Moved out of `ROADMAP.md` on 2026-10-26. Ticks were reconciled with `CHANGELOG.md` up to 0.1.0-alpha.10 on the same day; items marked "check" still need a hands-on look.

Priorities: **P0** = no release without it, **P1** = required for 1.0, **P2** = after 1.0. Milestone tags such as (M4) refer to `ROADMAP.md`.

## Feature matrix

| Feature | Source | Priority | Milestone |
| --- | --- | --- | --- |
| Infinite canvas: pan, zoom, rubber-band selection | BeeRef, PureRef | P0 | M0 |
| Drag-and-drop from files and browsers, paste from clipboard | BeeRef, PureRef | P0 | M0 |
| Works without a dedicated GPU (software fallback) | MothRef | P0 | M0 |
| Autosave + crash recovery | MothRef | P0 | M1 |
| Save prompt on exit | MothRef, BeeRef 0.3.4-dev | P0 | M1 |
| `.moth` format, recent files | MothRef, PureRef 2.1 | P0 | M1 |
| Undo/redo for everything | BeeRef, PureRef | P0 | M1 |
| Move, scale, rotate, flip | BeeRef, PureRef | P0 | M2 |
| Crop | BeeRef, PureRef | P0 | M2 |
| Transform content inside crop frame (#140) | MothRef | P1 | M2 |
| Mass scale to same width/height/area | BeeRef | P0 | M2 |
| Arrange: vertical, horizontal, optimal, by filename, gap | BeeRef | P0 | M2 |
| Text notes | BeeRef, PureRef | P0 | M2 |
| Opacity and grayscale | BeeRef | P1 | M2 |
| Color sampler with hex copy | BeeRef, PureRef | P1 | M2 |
| Export scene to PNG/SVG, export all images | BeeRef | P1 | M2 |
| Export a region (#143), image to file/clipboard (#132) | MothRef | P1 | M2 |
| Hotkey to jump between images (#116) | MothRef | P1 | M3 |
| Configurable hotkeys and mouse | BeeRef, PureRef | P1 | M3 |
| Navigation without middle-click or wheel input | MothRef | P1 | M3 |
| Consistent batch editing of selected items | BeeRef, PureRef | P1 | M3 |
| Open `.bee` / `.pur`, offer conversion and save only as `.moth` | MothRef | P1 | M3 |
| `.bee` import | MothRef | P0 | M2 |
| `.pur` import, verified 1.x and 2.x samples | MothRef | P1 | M2 |
| Always-on-top, frameless, move window by key | BeeRef, PureRef | P0 | M3 |
| Board tabs + tear-off windows | MothRef | P1 | M3 |
| Transparent overlay, click-through (#100) | MothRef, PureRef | P1 | M3 |
| Attach to another app's window | PureRef 2.0 | P2 | M3 |
| Browser extension "Send to MothRef" | MothRef | P1 | M3 |
| Groups and hierarchy tree | PureRef 2.0 | P1 | M4 |
| Freehand drawing | PureRef 2.0 | P1 | M4 |
| Shapes | PureRef 2.1 | P1 | M4 |
| Rich text: links, checklists, formatting | PureRef 2.0 | P1 | M4 |
| Configurable heading font size (#130) | MothRef | P1 | M4 |
| GIFs with frame stepping | PureRef 2.0 | P1 | M4 |
| Grid with snapping | PureRef 2.1 | P1 | M4 |
| Alignment guides and snapping (#129) | MothRef | P1 | M4 |
| Searchable in-app cheat sheet of all current hotkeys | MothRef, Milanote | P1 | M3 |
| Command Palette / Omnibar: search and run actions, show shortcuts | MothRef, PureRef 2.0 | P1 | M3 |
| Slideshow incl. random order | PureRef 2.0 | P2 | M4 |
| ICC color profiles | PureRef 2.0 | P1 | M4 |
| Trackpad gestures | PureRef 2.0 | P1 | M3 |
| Start screen with board previews | MothRef, BeeRef | P0 | M5 |
| Stacks (folders) and tags, smart collections | MothRef | P1 | M5 |
| File manager thumbnails (#152) | MothRef | P1 | M5 |
| Milanote-style elements (see the Milanote checklist) | Milanote | P1 | M4–M6 |
| Web build | MothRef | P2 | M8 |
| Real-time collaboration | MothRef | P2 | M9 |
| Save-state chip (replaced the status bar), empty-board state, zoom field, selection badge | MothRef | P1 | M2 |
| Watch folder import | MothRef | P1 | M3 |
| Overlay HUD and grab handle | MothRef | P1 | M3 |
| Timed drawing sessions | MothRef | P1 | M4 |
| Minimap / navigator | MothRef | P1 | M4 |
| Board preview chunk | MothRef | P0 | M5 |
| Library sorting, views, board stats, library health | MothRef | P1 | M5 |
| Palette export/import (`.gpl`, `.ase`) | MothRef | P1 | M6 |
| SpaceMouse | PureRef 2.0 | P2 | after 1.0 |
| Composition overlays, ruler with calibration (target 1.5) | MothRef, This Ruler Rules | P2 | M10 |
| Mirror view | MothRef | P2 | M10 |
| Value check, perspective helper (target 2.0) | MothRef | P2 | M10 |
| Video references | FrameRef | P2 | M10 |
| Open in external editor | MothRef | P2 | M10 |
| Redlines, review statuses, image comparison | MothRef | P2 | M11 |
| Client presentation and pitch sheet export | MothRef | P2 | M11 |
| JSON Canvas export/import | Obsidian | P2 | M12 |
| Deep links, wiki-links, linked `.md` notes, preview sidecar | MothRef | P2 | M12 |
| CLI / headless automation (`clap` subcommands, `--json`) | MothRef | P2 | M13 |
| Fetch images from HTTP(S) URLs into a board | MothRef | P2 | M13 |
| Local drop API, ShareX / ComfyUI / file-manager integrations (target 1.5) | RefDrop, PureDrop, MothRef | P2 | M14 |
| PDF import with page picker (release to be confirmed) | MothRef | P2 | M15 |
| Excalidraw, draw.io and UML/XML import (target 2.0) | MothRef | P2 | M16 |
| Blender plugin (target 2.0) | MothRef | P2 | M16 |
| Video-frame capture from the browser (target 2.0) | RefDrop, MothRef | P2 | M16 |

`#NNN` refers to issues in the [BeeRef repository](https://github.com/rbreu/beeref/issues).

## Parity checklist (PureRef 2.1 + BeeRef 0.3.x)

The matrix above lists features at headline level. This checklist is the full command-level inventory from the [PureRef handbook](https://www.pureref.com/handbook/) and BeeRef's [actions](https://github.com/rbreu/beeref/blob/main/beeref/actions/actions.py) and [CHANGELOG](https://github.com/rbreu/beeref/blob/main/CHANGELOG.rst). A milestone is not done until every item tagged with it is ticked. Source: **PR** = PureRef, **BR** = BeeRef.

As in PureRef: when nothing is selected, arrange/align/normalize actions apply to all items; otherwise only to the selection. Most of them also work on notes.

### Normalize (M2)
- [x] Normalize height (PR, BR)
- [x] Normalize width (PR, BR)
- [x] Normalize size (PR, BR)
- [x] Normalize area (PR)
- [x] Normalize scale (PR)

### Arrange (M2 unless noted)
- [x] Optimal — tightly pack all selected displayed bounds into a compact square, with no gap or resizing; independent of window size and zoom (maintainer clarification)
- [x] By name / by filename (PR, BR)
- [x] By time added (PR)
- [ ] By file path (PR)
- [x] Randomly (PR 2.1)
- [ ] By hierarchy order (PR) — M4, needs the hierarchy
- [x] Horizontal, vertical and square layouts by filename (BR)
- [x] Running the same arrange command again reverses the order (PR)
- [x] Configurable gap between items for horizontal, vertical and grid layouts; Optimal always uses zero gap
- [x] Auto-arrange newly added images, with a chosen default method (PR "select and pack new images", BR "default arrange method")
  - [x] Ask how to arrange when one drop/paste adds more than `arrange_prompt_threshold` (default 5) images; result lands at the cursor in one undo step
  - [x] Make the threshold and the default method real settings instead of a constant
- [x] Pack all + optimize canvas in one command (PR)

### Align and distribute (M2)
- [x] Align left / right / top / bottom (PR)
- [x] Distribute horizontally / vertically (PR)
- [x] Stack — pile selected images on top of each other in layer order (PR)

### Transform (M2 unless noted)
- [x] Flip horizontally / vertically (PR, BR)
- [x] Reset scale, rotation, flip, crop, or all transforms (PR, BR)
- [x] Raise to top / lower to bottom (BR)
- [ ] Setting "bring selected images to front" (PR)
- [x] Per-image sampling: bilinear vs nearest neighbour, plus a default for new images (PR); automatic nearest-neighbour at large zoom for pixel art (BR)
- [ ] Lock item so it cannot be selected on the canvas, e.g. as a backdrop (PR) — M4

### Canvas and view (M0–M2)
- [x] Fit scene / focus canvas (PR, BR) — M0
- [x] Fit selection / focus image; double-click a selected image to fit the entire group's width and height, again on the same image to go back; an unselected image focuses alone (PR, BR) — M0
- [x] Reset camera; reset zoom to 100% (PR) — M0
- [x] Focus next / previous image (PR; BeeRef #116)
- [x] Optimize canvas — fit the canvas snugly around all items (PR)
- [x] Lock canvas — nothing on it can be changed (PR)
- [x] Grayscale for the whole canvas (PR) and per image (PR, BR)
- [x] Configurable scroll wheel: pan, zoom or nothing; Shift / Shift+Ctrl + wheel to pan (PR, BR)
- [ ] Optional scrollbars and menu bar (BR)
- [x] Scale the scene with the window on resize, as a setting (PR)

### Window modes (M3)
- [x] Always on top (PR, BR) — also on KDE Plasma and GNOME Wayland (decision 0019; GNOME needs the bundled extension)
- [ ] Always on top of a chosen application (PR) — Windows, X11 and KDE Plasma Wayland implemented (pin above a selected window); native qualification open; macOS/GNOME open
- [x] Always on bottom — use the board as an interactive wallpaper (PR); capability-gated on Wayland, native qualification pending
- [x] Lock window position; option to stay locked while maximized (PR); requires native window coordinates (unavailable on Wayland)
- [ ] Transparent to mouse (click-through), with a system-global shortcut and "disable on activate" (PR; BeeRef #100) — click-through works on X11; opt-in global shortcuts and "disable on activate" implemented; Windows/macOS native qualification and Wayland global shortcuts open
- [x] Overlay selection — every selected image gets its own borderless floating window (PR); shared texture budget, up to 32 windows; repeat the action to close the selected floating views
- [x] Master window opacity (PR) — window opacity in the View popover and Over panel (0.1.0-alpha.10); Ctrl/⌘+Alt+Up/Down steps added
- [x] Resize window to selection or to all items (PR); rotated bounds, clamped to the current monitor
- [x] Toggle title bar, window border, fullscreen (PR, BR); move window by hotkey (PR, BR) — modifier+drag with its own cursor still open
- [x] Appearance: background, canvas and selection colors; "glass" theme with a transparent canvas (PR) — custom RGB colors and per-color reset in Settings › Appearance; native blur qualification remains separate

### Pickers and info (M2)
- [ ] Color picker: RGB / HSV / HEX, copy value, keep picking outside the window while the button is held, ignores image opacity (PR, BR) — all done except picking outside the window
- [ ] Coordinate picker: pixel X/Y, image name, source, size, size of the selection (PR) — picker exists; check every field before ticking
- [ ] Show color gamut of an image (BR)
- [ ] Open image source — the URL or path the image came from (PR); filled automatically by the browser extension

### Images, files and export
- [ ] Embed images or link to files on disk (PR) — M5
- [ ] Relink missing linked images (PR) — M5, with Library health
- [ ] Optional auto-downscale of very large images on import (PR) — M2
- [x] Export selected images, all images, scene (PR, BR) — M2
- [ ] Load most recent scene; option to reopen the last scene on start (PR) — M1
- [ ] Open image files from the command line; first argument `.moth`/`.bee`/`.pur` opens a board (BR) — M1
- [ ] Notify when pasting from an empty or unusable clipboard (BR) — M0
- [ ] Comment on an image, shown as an icon and tooltip (PR) — M6, with comment pins

### Notes (M2 plain, M4 rich)
- [x] Notes auto-size to text; manual size wraps text (PR)
- [ ] Creating a note with an image or group selected parents it to that item (PR) — M4
- [ ] Copy / paste text formatting (PR) — M4
- [ ] Links to local files; clicking a link to a `.moth` / `.pur` / `.bee` file opens that board (PR) — M4

### GIFs (M4)
- [ ] Play / pause, timeline scrubbing, playback speed, frame step (PR)
- [ ] Copy a single frame, or drag it out as a new image item (PR)

### Drawing (M4)
- [ ] Line color, thickness and style for all tools (PR)
- [ ] Shift snaps lines to 0/45/90°, makes perfect circles and squares (PR)
- [ ] Double-click a drawing (or its hierarchy entry) to edit it again (PR)

### Groups and hierarchy (M4)
- [ ] Groups with a customizable background, auto-resize to content, empty groups removed (PR)
- [ ] Locked groups: click selects the group, double-click enters it (PR)
- [ ] Slideshow and pack operate on a group's content when only the group is selected (PR)
- [ ] Hierarchy: drag to reorder and re-parent, rename with F2, status icons (linked, locked, comment), tooltip with source and size (PR)
- [ ] Parent selected items to the first selected item (PR)

### Slideshow (M4)
- [ ] Interval setting; order: ordered / shuffle (each once) / random (PR)
- [ ] Toolbar play / pause / step; manual navigation pauses it; UI fade options (PR)

### Input, shortcuts, localization
- [ ] Every action rebindable; system-global shortcuts for chosen actions (PR, BR) — M3; in-app rebinding done, system-global shortcuts open
- [x] Settings show which values differ from defaults (BR) — M2
- [x] Settings store and panel: persist preferences; first values `arrange_prompt_threshold` and the arrange gap
- [x] Undo history size slider: how many steps to keep in memory (default 256), shown with an estimate of the memory the history consumes (bytes per step × kept steps), so a user can trade undo depth for RAM
- [ ] Touch input and SpaceMouse (PR) — touch via the input abstraction (M0), SpaceMouse after 1.0
- [x] Interface translations; at least English and Russian at 0.3, more later (PR 2.1 ships 7 languages) — M2
- [x] Extensible translation catalogs with automatic language registration and English fallback
- [ ] Debug log viewer in the app (BR) — M1

## Milanote checklist

Inventory of [Milanote](https://milanote.com) features from its [help center](https://help.milanote.com/llms.txt), [keyboard shortcuts](https://help.milanote.com/getting-started/shortcuts) and [release notes](https://milanote.com/releases), with a decision for each. MothRef takes what helps collect and arrange references offline; it skips what only makes sense for a cloud SaaS. Milestone in brackets; **skip** = not planned, **after 1.0** = P2.

### Cards and elements
- [ ] **Note card**: rich text with large/normal heading, bold, italic, underline, strikethrough, highlight, bullet/numbered/check lists, quote, code block, indent, center alignment (M4)
- [ ] Markdown shortcuts at line start: `#`, `.`/`-`, `1.`, `[ ]`, `>` (M4)
- [ ] Optional **source line** at the bottom of a note; Ctrl/⌘+Enter copies it to the next note (M6)
- [ ] **Card colors**: one of 9 colors for notes, columns and boards; a color strip on image cards (M6)
- [ ] **Heading** card — big text without a card background (M4)
- [ ] **To-do card** as its own element, besides checklists inside notes (M6)
- [ ] **Image caption** — select an image and start typing; toggle caption visibility (M6)
- [x] Image actions: download original, open in the system viewer (M2)
- [ ] **Link card** with three toggleable parts: preview image, URL, description; preview fetched only on user action (M6)
- [ ] Custom link thumbnail — drop an image onto the link card's preview (M6)
- [ ] Pasting a board link (`.moth` path or `mothref://` link) creates a **board shortcut card** (M6)
- [ ] **Column**: title, drag to reorder with an insertion line, resize, collapse/expand, item count, "Group into column" from a selection; columns can hold boards but not other columns (M6)
- [ ] **Nested board**: card with preview, double-click to enter, drop a board on a board to nest it, drag onto a breadcrumb to move it up (M6)
- [ ] **Lines**: drag from a card's corner handle, connect to another card or anchor to a point, bend by the middle handle (double-click to straighten), Shift for straight, color and weight, label by typing, pasted text on a line becomes its label (M6)
- [ ] **Color swatch**: new swatches auto-pick a color that matches the board's images; HEX / RGB / HSL display or no label; paste a HEX; eyedropper from board images; resizable (M6)
- [ ] Palette extraction from a selected image (k-means, 5–8 swatches) — MothRef addition (M6)
- [ ] **Sketch card** with a white background (M4)
- [ ] Drawing shortcuts: D pen, V select, E eraser, X previous color, 1–4 pen size, Esc to finish; pressure "ink effect" with an off switch; export a drawing as SVG (M4)
- [ ] **Comment pins** on items, shown on hover; hidden in presentation mode (M6)
- [ ] Table card with formulas (after 1.0)
- [ ] Document card — long-form text opened full-screen (after 1.0)
- [ ] PDF documents (M15); other file cards and audio cards (after 1.0)
- [ ] Video card and video link embeds (YouTube, Vimeo) (after 1.0, together with video on the canvas)
- Map card — **skip** (needs online map tiles)
- Built-in stock photo search (Pexels) — **skip** (network service; the browser extension covers collecting)

### Creating and editing
- [ ] Double-click empty canvas creates a note (setting, off by default to keep PureRef behaviour) (M4)
- [ ] Ctrl/⌘+Enter creates the next note/board/column below the selected one, matching its width and pushing items below down (M6)
- [x] Duplicate with Alt/⌥+drag (M2)
- [x] Nudge with arrow keys, farther with Shift+arrows (M2)
- [x] Send backward / bring forward one step, besides to-top / to-bottom (M2)
- [ ] Lock item position from the context menu (M4, same as PureRef "lock item")
- [ ] Cards snap to the grid; optional "crop images to fit the grid" (M4)
- [ ] Remove image background — local model, no cloud (after 1.0)

### Organizing
- [ ] **Unsorted tray** per board: imported and clipped items land there first, then get dragged onto the canvas; the browser extension can send to it (M6)
- [ ] **Item labels**: colored labels on any item, rename updates everywhere, usage count, find all items with a label. Same tag system as board tags in the library (M6)
- [ ] **Synced notes**: duplicate a note as a linked copy that stays in sync, within one board file (M6); across board files (after 1.0)
- [ ] Item **trash**: deleted items stay restorable per board until emptied, on top of undo (M5)
- [ ] **Templates**: "convert board to template", start a new board from a template (M5)
- [ ] Search across boards and inside them: note text, captions, labels, link titles; jump to the item (M5)
- [ ] Navigation: back / forward history, go to parent board, recent boards (M6)

### Viewing and presenting
- [ ] **Presentation mode**: full-screen, editing locked, comments hidden, click an item to focus/zoom it, settings remembered (M4, alongside slideshow)
- [x] Dark / light theme toggle (M2)
- [x] Interface scale smaller / larger (M2)
- [x] Searchable in-app shortcut cheat sheet on `/` and in Help: all actions grouped by category, showing current custom bindings (M3, next to the command palette)

### Export
- [x] PDF export of a board or a selection (M2)
- [ ] Markdown and plain-text export of notes, headings, to-dos and link cards (M6)
- [x] ZIP of all original images and files (M2, with "export all images")
- Word export — **skip** (Markdown covers it)

### Web clipper (M3, in the MothRef browser extension)
- [ ] "Save" button on hover over any image
- [ ] Save selected text as a note: right-click menu or a button next to the selection (setting)
- [ ] Choose the target: active board, one of the 10 most recent boards, or the board's unsorted tray
- [ ] Clipped items keep the page URL as their source

### Collaboration and cloud (M9 or skip)
- [ ] Real-time co-editing, read-only share links, per-board editor list (M9)
- [ ] Reactions and @mentions on comments (M9)
- Password-protected boards — **skip** as a server feature; optional encrypted `.moth` files (after 1.0)
- Accounts, plans, billing, team admin, notifications by email — **skip**

### Mobile and tablet (future Android port)
- [ ] Share-sheet target ("Share to MothRef") instead of the browser extension
- [ ] Touch gestures: double-tap canvas for a note, long-press to drag, pinch to zoom, three-finger swipe undo/redo, long-press + drag to multi-select
