# M3 window controls and desktop launch qualification

## Implementation

The main window is frameless. Its minimize, maximize/restore, close and pin
controls reveal on pointer approach or keyboard focus/Tab. The persistent Window
controls button toggles an explicit reveal for touch. Controls wrap on narrow
windows; pin state and its mode remain visible in the status bar.

The pin button toggles the last session choice. Its adjacent menu chooses above
all windows or above a chosen window. Target selection is currently available on
Windows only; other backends disable that choice. Wayland also disables above-all
because the compositor owns stacking. A selected native handle is never persisted
across launches.

Windows target selection temporarily releases the existing pin so another window
can be clicked. Escape (including outside MothRef) or Cancel restores the previous
mode. A successful choice installs a foreground notification; activating the target
raises MothRef without activating it. Unrelated windows can cover both. There is
no polling while pinned. Closing the target never destroys MothRef: it does not
become an owned window. The remembered target is validated before re-enabling it.
Already-topmost targets, elevated applications, multiple desktops and handle reuse
within the same target process still need native qualification.

Default rebindable shortcuts:

- `Ctrl+Shift+T` (`Cmd+Shift+T` on macOS): toggle the last pin choice.
- `Ctrl+Shift+M` (`Cmd+Shift+M`): arm window movement, then drag the canvas.
  Escape or Cancel cancels. The command palette provides a touch entry point.
- `F11`: toggle borderless fullscreen. Maximize/restore remains a separate action;
  Restore exits fullscreen when it is active.

The Windows binary already declared the GUI subsystem. The likely extra-console
source was its GPU-memory probes: `reg` and `nvidia-smi` are console programs.
Their shared launcher now uses `CREATE_NO_WINDOW`, preserving captured stdout.
Release packaging asserts the PE GUI subsystem. No PureRef binary was inspected.

macOS archives now contain `MothRef.app/Contents/Info.plist` and
`Contents/MacOS/mothref`, rather than a bare executable that Finder can treat as a
terminal program. The workflow checks the plist and executable permission. The
application is still unsigned and unnotarized.

Diagnostic tracing goes to stderr and a bounded worker queue, with two rotating
2 MiB logs under the existing recovery root's `logs/` directory. On Windows,
MothRef attaches only to an existing parent console when stderr is unavailable;
it never allocates a console. Existing redirected output and xtask CLI output
remain intact. Diagnostic bursts can drop queued lines rather than block a frame.
Crash reports retain their existing worker and location.

Sources: [Microsoft process flags](https://learn.microsoft.com/en-us/windows/desktop/ProcThread/process-creation-flags),
[Windows ownership constraints](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features),
[Apple bundle layout](https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle).

## Evidence from this Linux session

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` passed. Release YAML and its embedded packaging Python parse successfully.
- The new pin and diagnostic modules passed isolated Clippy for
  `x86_64-pc-windows-msvc`. Full platform cross-compilation stopped in blake3's
  build script because `ml64.exe` is unavailable on this Linux host.
- Dependency checks used a writable copy of the cached advisory database; all
  four checks passed, with existing duplicate-dependency warnings.
- Regression coverage added for keyboard/pointer reveal, rebindable window
  actions, bounded log rotation, and captured output from hidden Windows probes.
  On the maintainer's explicit test request, `cargo test --workspace` passed:
  254 tests, no failures or ignored tests. Local socket tests ran with sandbox
  restrictions lifted and `XDG_DATA_HOME` isolated under `/tmp`.
  Stress checks remain deferred.
- Neither Windows nor macOS runtime qualification has been performed. The rows
  below are pending, not passing results. The PureRef report is an external field
  report: maximize versus fullscreen remains unconfirmed, and no corresponding
  MothRef crash has been reproduced.

## Native qualification still required

### Transparent overlay

The existing overlay composites the board before applying 10–100% master opacity,
so overlaps do not become darker. A separate interactive control window provides
opacity, click-through, on-top, drag and exit controls. Click-through applies only
to the board window. The controller skips zero-sized frames, reconfigures after
uncovering or display-scale changes, and requests a frame after surface recovery.
Its click-through control respects the platform capability flag. No new native
overlay run has been performed in this session.
For this controller follow-up, `cargo fmt --all --check` and
`cargo clippy -p mothref --all-targets -- -D warnings` passed. Automated tests and
stress checks were not run, following the ordinary-development policy in AGENTS.md.

For Windows, macOS and Linux X11, repeat the following on hardware and software
adapters; record Wayland/Flatpak limitations separately:

- [ ] Open overlapping images and a note; sweep opacity through 10%, 55% and 100%.
  Confirm uniform composite opacity and unchanged item opacity after exit.
- [ ] Enable click-through; draw in an application beneath the board. Confirm
  the controller still accepts clicks, can disable click-through and can exit.
- [ ] Toggle on-top, exit and re-enter; confirm the prior normal-window pin mode
  returns on exit. Unsupported on-top/click-through controls must be disabled.
- [ ] Minimize/restore and cover/uncover the controller; move it between displays
  with different scaling. Confirm all controls repaint and remain usable.
- [ ] Force surface/device recovery where supported, then adjust opacity, disable
  click-through and exit. Confirm the scene and undo history remain intact.
- [ ] Resize the controller narrowly and use keyboard/touch to reach every control.

These are pending manual regression checks, not automated or passing results.
The HUD's one-time shortcut hint and a grab handle integrated into the board remain
separate open roadmap tasks; the current escape path is the independent controller.

### Launch and transition matrix

Record OS/build, GPU/driver/backend, display scale, hardware/software adapter,
archive version and diagnostic logs for every run. Use a disposable board with
images, notes, overlapping items and an undo history; save a baseline first.

| Platform / launch path | Required observation | Result |
| --- | --- | --- |
| Windows Explorer, double-click extracted `mothref.exe` | No cmd/console at startup or during discrete-GPU detection; logs appear | Pending |
| Windows existing cmd/PowerShell, direct executable and redirected stderr | Diagnostic output remains usable; no new console | Pending |
| macOS Finder, double-click extracted `MothRef.app` | App opens normally; no Terminal window; logs appear | Pending |
| macOS terminal, `MothRef.app/Contents/MacOS/mothref` and redirected stderr | Diagnostic output remains usable | Pending |
| Windows target pin | Click another app, cancel inside/outside MothRef, toggle last choice, switch targets, close target, activate unrelated app | Pending |
| Windows / macOS / X11 above-all | Toggle pin with button and a rebound shortcut; overlay exit restores the prior pin | Pending |
| Wayland | KDE Plasma / GNOME with extension: pin choices work (see "Wayland stacking" below); other compositors: pin choices disabled; frameless drag and reveal remain usable | Pending |
| All desktops, mouse/keyboard/touch | Approach/leave, Tab/Shift+Tab, explicit reveal/hide, all four controls reachable at narrow sizes | Pending |

On Windows, perform at least 30 cycles of each path independently:

1. Maximize → restore using the control and a rebound shortcut. Resize between
   cycles and move between monitors with different DPI. Minimize → restore and
   cover → uncover the window too.
2. F11 fullscreen → F11 windowed, both from normal and maximized state. Confirm
   the previous state is restored and Restore can exit fullscreen.
3. Repeat with `--software`. Force device/surface recovery with a graphics-device
   restart where the driver permits it, then continue transitions.

After each cycle, check that all items, selection, transforms and notes remain,
undo/redo still works, and the board can be saved and reopened. Camera behavior
must respect the existing scale-with-window preference. Check zero-size/minimized
surfaces do not configure or spin; uncovering and DPI changes must request a fresh
frame. Record any renderer recreation and whether image residency reloads without
changing the scene. A failed or unperformed run keeps the reliability roadmap
checkbox open. Release stress/soak and CI tier-3 evidence remain release gates.


## Window modes and appearance

The View popover in the revealed window bar and the independent Over controller
share presentation controls. There are no new Settings rows: these are per-board
view state, stored by the preference worker without changing the board format.
Saved-board identities use a hash of the local path; the preference store retains
64 recently adjusted boards. Unsaved tabs retain their state in memory and acquire
persistent presentation when first saved. Moving a file to a new path starts with
default presentation. Returning from Over restores the previous Collect/View choice.

Native blur is enabled through winit on macOS. X11, Windows and unqualified
Wayland compositors show a translucent tint and an explicit fallback message.
No platform-specific code or dependency is added to the application crate.

Pending manual checks (not performed in this agent session):

- Linux/X11 and Wayland: check Tab in empty, populated and locked boards; Tab
  inside note editors, dialogs and text fields must retain normal text behavior.
- Linux, Windows and macOS: resize across 600 logical pixels, switch tabs with
  different modes, save an unnamed board and reopen it, and check Ctrl/Command+T
  and controller Close/Escape. Enter the library from Over and reopen the board.
- Check the sliding bar with mouse, keyboard focus and its persistent corner
  button with touch. Test narrow widths, board switching, pin and window controls.
- View must keep context-menu and pointer editing. Images only must hide notes
  without deleting them; hidden notes must not intercept image clicks or rectangle
  selection. Collect must show them again. Check that undo/save content is unchanged.
- Adjust all three opacities in both surfaces. Check overlapping images, notes,
  popovers and outlines, and verify settings are independent for two open boards.
- macOS: check native Glass blur. Other backends: check the stated translucent
  fallback. Confirm solid background and returning to the library restore opacity.
- Over: enable click-through, move the controller, change appearance and exit
  through the controller without a global shortcut. Minimize and restore both windows.

Group-only viewing remains pending: the scene currently represents images and
text only, with no group identity, membership or selectable group title.
Regression tests were added for presentation decoding, narrow-window selection,
and bounded preference history. Per AGENTS.md §3 they were compiled by Clippy,
but not executed. Native GUI checks, golden-image validation, benchmarks and soak
checks remain for an explicit test request or release preparation.


### Alpha.10 release preparation — 2026-10-26

The release test run supersedes the earlier deferred-test note above: all 328
workspace tests passed, including presentation decoding/filtering and bounded
presentation-history coverage. Headless render goldens and allocation checks
also passed. Local socket/Wayland tests used isolated `/tmp` data outside the
sandbox; no tests were ignored. The native manual matrix above remains pending.
See [the alpha.10 release notes](releases/v0.1.0-alpha.10.md) for corpus, fuzz,
journal-kill and performance evidence. No completed native qualification or
completed M3 milestone is claimed.


### Alpha.11 cross-platform CI correction

Alpha.10 publication was blocked by Windows Clippy's `chunks_exact(2)` warning
and a macOS test comparing a canonical recovery destination with an aliased
`/var` path. The Windows decoder now uses `as_chunks::<2>()`, preserving rejection
of odd-length UTF-16 data. Its actual functions passed isolated Windows-target
Clippy without linking a Windows executable on this Linux host.

The recovery regression now saves through a symlink alias on Unix and compares
against the canonical destination. The aliased fixture reproduced the macOS
failure on Linux before the expectation was corrected, then passed with all
original checks for source preservation, journal archival and durable resaving
still enabled. No recovery behavior or board format changed.

### Linux window-mode follow-up — 2026-10-27

Qualification used isolated data/config directories under
`/tmp/mothref-window-qualification*`, synthetic `rows.png` and `alpha.webp`
corpus images, and the release binary built locally from the working tree.
The host runs Linux 6.18.49-2-lts, KDE Plasma Wayland with XWayland; the driven
windows used XWayland (`WAYLAND_DISPLAY` and `WAYLAND_SOCKET` unset). These
observations do not qualify a standalone X11 session or native Wayland.

Observed before and during the fix:

- On an empty board, Tab entered View. Ctrl+T created the independent Over
  controller; Escape in that controller returned the board to View.
- The default 530 × 220 controller clipped appearance controls and the exit
  button. Appearance controls now stack vertically, recovery controls come
  first, and short controller windows scroll. The rebuilt controller showed
  click-through, on-top and exit at its default size; at 360 × 360 the
  three opacity controls, background selector and images-only switch fit.
- Glass displayed the explicit translucent fallback in the initial controller.
  A native slider interaction changed the rebuilt controller's displayed
  master opacity from 1.000 to 0.11. This verifies input/display response,
  not pixel-accurate compositing or the full opacity matrix.
- A native bottom-right drag changed the Collect window geometry from
  900 × 650 to 1112 × 605 after the compositor settled. Exact drag-distance
  correspondence, all edges/corners and repeated transitions remain unqualified.

The main window now draws the existing compact window bar in Over, with top-edge
reveal, the persistent touch button, minimize/maximize and the existing actions.
Dragging the corner button or spare bar area starts native window movement;
double-clicking the spare area runs the registered maximize action. Reveal
retains the actual wrapped bar height so moving onto a second row does not hide
its controls. Mouse, pen and touch resize requests use normalized pointer events,
with theme-sized edge targets and native compositor resizing. Fullscreen and
maximized windows do not intercept edge presses. The independent controller
remains available when the board explicitly passes clicks through.

Regression coverage was added for appearance layout inside a horizontal parent
and resize hit testing (eight directions, interior, out-of-bounds and wider touch
targets). Formatting and `cargo clippy -p mothref --all-targets -- -D warnings`
passed; tests were compiled, not executed, under AGENTS.md §3.

Still pending: final top-bar hover/hide and drag qualification, minimize/restore,
click-through recovery, opacity compositing with overlapping images/notes,
per-board persistence, software adapters, mixed DPI, native Wayland, Windows
and macOS (including native macOS blur). Keep the roadmap qualification checkbox
open. No board format, scene commands or undo history were changed.

### Tab freeze regression — 2026-10-27

The maintainer reported that Tab made the application stop responding. The
follow-up reproduced a live but unresponsive process: the last Collect frame
remained visible, and further input did not produce an Over controller. The
cause was a recursive egui context lock introduced in the reveal-height change:
`context.input(...)` called `context.content_rect()`, which takes that same lock.
Reveal now reads the content rectangle from the already borrowed input state.

`cargo test -p mothref view_bar_hover_does_not_deadlock_the_frame -- --nocapture`
failed deterministically with a two-second timeout before the fix. The same
regression passes after the fix and checks hover over both rows, leaving the bar
and explicit reveal. The appearance-layout test also needed to clear its unused
egui texture deltas; its initial failure was test cleanup, not the native freeze.

On the maintainer's explicit test request:

- `cargo test -p mothref -p moth-ui`: 138 passed, no failures or ignored tests.
- `cargo test --workspace`: 332 passed, no failures or ignored tests. The first
  sandboxed run failed local-socket/recovery-root tests; the successful full run
  used lifted sandbox restrictions and isolated `/tmp` application data.
- A rebuilt release binary completed three XWayland input sequences containing
  Tab, top-edge pointer movement, Ctrl+T and controller Escape without hanging.
  Each sequence required an Over controller to appear within a timeout and then
  disappear on Escape. Screenshots and stderr are retained under
  `/tmp/mothref-window-qualification-tab-fixed` for this session.

This supersedes the deferred-test note for the changes above. Native Wayland,
Windows/macOS, mixed DPI, accurate opacity composition and the full movement /
resize qualification matrix remain pending; no complete cross-platform
qualification is claimed.

### Native Wayland gesture ownership — 2026-10-27

The maintainer reported movement/resizing that required another attempt and
non-working side edges with a mouse in View/Over. XWayland alone did not reproduce
this: consecutive moves and each side edge passed, including short gestures.
A native KDE Wayland run did reproduce alternating success/failure. The sequence
was move: pass, move: fail, left: pass, right: fail, top: pass, bottom: fail.
Temporary input tracing confirmed that the compositor consumed the release after
each successful native gesture. The next press arrived while the application
still considered the previous mouse button held and was discarded.

All main-window native movement paths and edge resizing now share the same
handoff: after a successful native request, clear the normalized mouse capture,
queued egui pointer buttons and egui's active pointer/drag. Failed requests retain
their error handling. The independent controller also clears egui capture when
its own native movement starts. No synthetic board click/release is generated.

A second native check exposed a separate compact-bar layout bug: the spare drag
area requested the full width of a wrapping row and moved to the next row. It now
uses the width available before wrapping. The layout regression failed before
that change and passed afterward.

Final native checks used temporary KWin EIS input and geometry probes limited to
a disposable process, with isolated `/tmp` app data. Both View and Over passed:

- Two consecutive corner-handle moves, starting from the first press.
- A move using the spare space beside the window-bar controls.
- Left, right, top and bottom edge resizing, without retry clicks.

For the Over run, the 800 × 600 window moved twice, moved by the spare bar, then
resized through 771 × 600, 742 × 600, 742 × 571 and 742 × 542. The View run passed
the same sequence. Input was sent in small 120 Hz steps; the earlier ten-step,
80 ms synthetic movement visibly jumped and was not evidence of application
frame pacing. Actual frame pacing has not been quantitatively qualified.

`cargo test --workspace` passed all 335 tests, including missing-release
regressions for the normalized adapter and egui, the compact drag layout and the
Tab deadlock. The full run used isolated `/tmp` data and local socket access;
there were no ignored tests. Workspace Clippy, formatting and diff checks passed.
Temporary input tracing was removed. The rebuilt release binary contains both
fixes. Windows/macOS, other compositors, pen/touch, mixed DPI and the remaining
opacity/recovery matrix still require native qualification.

## Remaining parity controls implementation

The following notes describe the new parity work, separately from earlier native
test results above.

UI placement:

- View › Window, in the normal menu and compact View appearance popover: bottom
  stacking, position lock, click-through, floating selection, resize to selection
  or all items, opacity up/down. The command palette exposes the same actions.
- Settings › Appearance: background (surface), canvas and selection RGB colors,
  each with a reset to the active light/dark palette.
- Settings › Window: retain position lock while maximized (default on), disable
  click-through on activation (default on), system shortcuts (default off).
- Local opacity chords are Ctrl/Command+Alt+Up/Down and remain rebindable.
  System chords are fixed Ctrl+Alt+Shift with T (Over), C (click-through),
  Up/Down (opacity). Registration conflicts report an error and turn the setting
  off for the current session.

Floating image windows share the main device and registry, including the texture
budget, original pixels, on-demand decode scheduling and device-loss recovery.
Opening more than 32 views is rejected with a localized message. Repeat the
action with those images selected, close the native window, or press Escape to
close them. Drag anywhere to move, or use edges/corners to resize; pointer input
uses the mouse/touch/pen adapter. Changing boards closes the transient views.
Board contents, journaling and undo history are unaffected.

Bottom stacking and position locking are disabled when the compositor cannot
provide them. Wayland cannot register the supplied system chords; the separate
Over controller remains usable. X11 shortcuts block on native events and a
shutdown descriptor, without timer polling. Windows/macOS registration stays on
the native event-loop thread, as required by their APIs.

Added regression coverage (executed for alpha.13):
opacity endpoint/reversal checks, floating-camera framing and a software-adapter
golden for shared pixels/letterboxing/opacity, color validation for
both palettes, preference round-trips/defaults and malformed X11 keymaps.

Still required before qualification/release:

- Linux X11: stack under other apps; locked move attempts; maximize/restore with
  both lock settings; conflicts and CapsLock/NumLock handling for global chords;
  release all registrations when disabling shortcuts. Repeat on a non-Latin
  keyboard layout and after changing keyboard mappings.
- Linux Wayland: unsupported operations disabled; compact popover and independent
  controller remain reachable; repeated floating-window moves/resizes.
- Windows and macOS: all native behaviors above, especially global registration
  and cleanup, focus/activation click-through recovery, monitor/DPI changes,
  surface loss and software-adapter floating views. Neither OS was run here.
- Every backend: selected/all-item framing with rotations, monitor size limits,
  transparent images and 32-window rejection; closing/deleting referenced items,
  switching boards, suspend/resume and renderer recovery.
- Alpha.13 local checks: 341 workspace tests passed with 0 failures and
  0 ignored; the floating golden also passed explicitly on SwiftShader. The
  quick benchmark passed: RX 5700 XT, 10,000 images, 100 frames, p99 6.540 ms,
  13.32 MiB textures. Five-minute soak was stopped at the maintainer's request
  and is deferred to release 1.0. Native window qualification remains open.
  Tier-3 reliability/performance qualification remains CI/maintainer-only.

Open design limits: global chords are fixed rather than individually rebindable;
Wayland portal shortcuts are not implemented. Floating windows are temporary
views rather than independent saved boards. Native system move requests are
corrected to the captured coordinates while position lock is active.

## Application menu, compact controls and X11 pinning

New board, Open, Save, Library and Settings live directly in MothRef; Navigation and view is the compass menu in the
bottom action list. View anchors its handle and non-wrapping minimize, maximize,
close and overlay group to the right by default. Settings › Window and overlay
can switch the compact bar to the left; this preference is persisted. Both Collect and View expose all-window and target-window
pin choices; the unused titlebar area remains draggable.

X11 target selection watches `_NET_ACTIVE_WINDOW`, ignores MothRef windows and
grabs Escape only during selection. A dedicated worker raises the board without
activation when the target becomes active, monitors target destruction, validates
the saved PID on re-enable, and waits on native events and a shutdown descriptor.
It never reparents or moves either window. Cancellation preserves the previous
target. Closing the board or disabling pinning drops the worker connection and grabs.

Added regression coverage for the application-menu surface, compact controls on both edges at
320/800/1440 points, and malformed Escape keymaps. Release-preparation test results are recorded in the alpha.14 release notes.

Manual qualification remains open: X11 selection/cancel/reselect, alternate apps,
minimize/restore, target close, rapid pin toggles, idle CPU, and moving/resizing both
windows. The first X11 implementation raises in the normal stacking layer; targets
already marked always-on-top or fullscreen require further qualification/handling.
Windows/macOS native checks were not run. macOS target attachment and Wayland
KDE Plasma/GNOME integrations remain unimplemented; Wayland is deferred per the
maintainer's follow-up.

Protocol reference: [EWMH stacking and active-window rules](https://specifications.freedesktop.org/wm/latest-single/).

Settings now uses Ctrl/Command+O. Loading the old standard Settings/Open pair migrates it to the new defaults; custom pairs remain intact. Multiple-selection frames draw in View independently of the editing toolbar.

## Wayland stacking, macOS Spaces and Windows topmost targets — 2026-10-28

Implementation per [decision 0019](decisions/0019-wayland-window-stacking.md):

- KDE Plasma Wayland: above all, Over (board and controller) and floating views
  use generated KWin scripts; above a chosen window uses an activation-following
  KWin script. Target selection takes the next activated non-MothRef window;
  Cancel or Escape inside MothRef cancels. Closing the target turns the pin off.
- GNOME Wayland: Settings › Window and overlay explains the missing extension
  and offers **Install GNOME window integration**. After the next login, above
  all, Over and floating views use the extension. Above a chosen window stays
  disabled on GNOME.
- Always on bottom is disabled on every Wayland compositor.
- macOS: above all also sets `CanJoinAllSpaces | FullScreenAuxiliary` and
  removes it when the pin is turned off.
- Windows: a topmost target lifts the board into the topmost band while the
  target is active; activating any other application, or turning the pin off,
  returns the board to the normal band just below the new foreground window.

Evidence from this Linux session (no Plasma, GNOME, macOS or Windows desktop
was available):

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo deny check` passed. The macOS module and the Windows pin module
  passed Clippy for `aarch64-apple-darwin` and `x86_64-pc-windows-msvc` in an
  isolated crate (the full workspace cannot cross-compile here because of C
  build scripts).
- With the maintainer's agreement, the compositor tests ran:
  `cargo test -p moth-platform --lib -- compositor::` passed 3 tests. One runs
  the worker against a real `dbus-daemon` and a stand-in for KWin's scripting
  service: probe, script load and run, a refused callback from another client
  and an accepted `Picked` callback that wakes the app. D-Bus goes through the
  `dbus` crate and the system libdbus (`libdbus-1-dev` was installed for this).
- The generated KWin scripts ran in Node against a mocked Plasma 6 and Plasma 5
  `workspace` (late-mapped windows, picking that ignores MothRef and panels,
  activation following, target removal). The GNOME extension passed
  `node --check`. Neither replaces a run inside KWin or GNOME Shell.

Native checks still required (record Plasma/GNOME version and session type):

- [ ] Plasma 6 and Plasma 5 Wayland: toggle above all with the button and the
  shortcut; Over keeps board and controller above Krita/Blender/a browser; exit
  Over restores the previous pin; floating views stay on top.
- [ ] Plasma Wayland target pin: pick Krita, switch Krita ⇄ another app ⇄ MothRef,
  cancel selection, re-pick, close the target, toggle with the shortcut.
- [ ] GNOME 45–49 Wayland: install from Settings, log out/in, check the hint
  disappears, then repeat the above-all and Over checks. Check a session with
  user extensions disabled shows a usable message.
- [ ] macOS: above all over a full-screen app and on another Space; turning the
  pin off returns normal Space behavior; F11 fullscreen still works.
- [ ] Windows: target pin with a topmost target (for example a video player set
  to "always on top"), switching away, turning the pin off, then above all.

### Native run under KWin 5.27 and GNOME Shell 46 — 2026-10-28

The release binary (`--software`, lavapipe) ran as a native Wayland client
inside this container under Ubuntu 24.04's `kwin_wayland` 5.27.11 (X11 backend
in Xvfb, with Xwayland) and `gnome-shell --nested` 46.0. Window state was read
from the compositor itself: a KWin script printing `keepAbove`, activation and
stacking order, and a test-only GNOME extension listing `is_above`, maximized
state, layer and focus. Two xterm windows titled "Krita" and "Firefox" stood
in for other applications; input came from `xdotool` on the host display.

KDE Plasma 5.27 — all passed after the fixes below:

- Ctrl+Shift+T: MothRef `keepAbove` and stays on top while "Krita" is active;
  toggling again lets "Krita" cover it.
- Window bar ▾ › Above a chosen window, then activating "Krita": the KWin pick
  script reports back, MothRef is above while "Krita" is active, drops when
  "Firefox" is activated, returns when "Krita" is; closing "Krita" turns the
  pin off and the pin button.
- Over: board and controller stay above the active "Firefox"; the controller's
  On top toggles the board; leaving Over restores the previous pin.

GNOME Shell 46 — all passed after the fixes below:

- Settings › Window and overlay shows the hint and the install button; "On
  top" is disabled. Installing writes the extension, enables it through
  `gnome-extensions`, and the hint changes to "log out and back in". After
  restarting the shell, MothRef detects it (`gnome=3`).
- An outdated installed extension is detected through its `Version` property
  and the install button returns.
- Ctrl+Shift+T on a maximized board: the board is restored, moves to the top
  layer and stays above the active "Krita"; toggling again returns it.
- Over, five times in a row: board and controller in the top layer above the
  active "Firefox", controller above the board.

Bugs found by these runs and fixed:

- KWin 5 numbers loaded scripts by list length, so after an unload a new
  script's id named a script still loaded, and `run` on `/<id>` started the
  wrong one: the attach script never ran. Scripts now start through
  `org.kde.kwin.Scripting.start`.
- The class fallback for sandboxed builds also matched windows of a second
  MothRef instance; it now applies only when no window has our PID.
- Mutter ignores "above" for maximized windows; the extension restores a
  maximized window before keeping it above.
- On GNOME, the Over controller appeared under its board: Mutter stacks a
  window that is mapped without focus below the focused window. The extension
  raises the window again when it is shown, and the app re-pins the controller
  after the board and after its first frame.

Still open: Plasma 6, GNOME 47–49, real (non-nested) sessions with hardware
GPUs, HiDPI, touch/pen paths, macOS and Windows.

### Window chrome rework — 2026-10-28

The maintainer asked for a calmer, adaptive chrome without the "window
controls" toggle. Decisions: Collect keeps hover reveal for the window
controls; View uses a floating bar at the top edge with a context-menu path
for touch (see DESIGN.md §4).

Checked with the release binary under KWin 5.27 (native Wayland, software
adapter) at 1300, 800 and 480 logical pixels, before and after: Collect with
the pointer away from and over the top bar, View with the pointer away from
and at the top edge, the narrow "⋯" menu and the canvas context menu. The
first build froze on start because `read_response` and theme tokens were read
inside egui `memory`/`input` closures (a recursive context lock, like the
earlier Tab freeze); both now read outside the closures.

Formatting and Clippy for `moth-ui` and `mothref` passed. The updated reveal
and title-shortening tests were compiled, not executed, per AGENTS.md §3.
Touch, pen, HiDPI, the dark theme, Windows and macOS were not checked here.

### Target pin and the chosen app's popups — 2026-10-28

Maintainer report: with the board pinned above Krita, a Krita popup made the
board disappear. Reproduced under KWin 5.27 with a test program that owns a
main window "Krita" and a transient "Brush popup" in one process: when the
popup became active, the attach script treated it as a foreign window, cleared
the board's keepAbove, and KWin raised Krita's main window with its dialog
over the board.

Now windows of the chosen process (and transients of the chosen window) count
as the app. While one of them is active the board stays above the app, and the
active popup itself is lifted above the board until another window is
activated, giving Krita < board < popup. Only another app's normal window or
dialog lowers the board; panels, notifications and tooltips do not.

The X11 path had a separate defect found by the same run: direct
`ConfigureWindow` restacks were ignored by KWin for an inactive client, so the
board never went above the target, and a sibling restack across frames raised
BadMatch and stopped the worker. It now uses `_NET_WM_STATE_ABOVE` requests
sent as a pager, with the same popup lifting, and releases both states when the
pin stops. Windows uses the same rule: windows of the chosen process raise the
board, and a popup places the board directly below it.

Checked with the release binary on native Wayland and through XWayland (X11
path) under KWin 5.27: pick Krita; open, close and reopen the popup; activate
another app; click MothRef while the popup is open; unpin while the popup is
open (both states released); the full earlier KDE scenario still passes. The
Windows change passed Clippy for `x86_64-pc-windows-msvc` but was not run.

### Several windows, tabs, Over hint, GNOME and macOS pinning — 2026-10-29

Implementation:

- Main windows: `app/panes.rs` keeps every main window but the current one
  as a `Pane` (tabs, mode, Over, floating views, pin, input, egui context,
  surface and texture registry) and swaps it in for that window's events.
  Worker wake-ups, repaint requests and deadlines visit every window. The GPU
  device is shared; each window has its own registry and an equal share of
  the video-memory limit. Tab moves run after the frame that asked for them.
  New windows get unique titles ("MothRef (2)") because compositor
  integrations find windows by title.
- GNOME: the Shell extension (v5) adds Pick, Attach and Detach with the same
  rules as the KWin script, reports back over D-Bus and releases everything
  when MothRef leaves the bus.
- macOS: the board follows the chosen application through NSWorkspace
  activation notifications (no polling) and is ordered in front of it
  without activating MothRef.

Checked with the desktop stand (`.claude/skills/desktop-stand`):

- KWin 5.27: tab menu "Move to new window"; pin above all in the second
  window only; dragging a tab out of the strip (ghost tab shown) opens a third
  window; "Move to window" back closes the emptied window; "Gather all tabs
  here" closes the others; closing a second window with × keeps the app
  running, closing the last one exits. No panics.
- GNOME Shell 46: pick "Krita", board above it, Krita's popup above the
  board, another app covers both, re-pinning the last choice, closing the
  target releases the pin.
- The Over hint, tab chips, "+" and italics were checked visually on KWin.

Not checked: Windows and macOS (Clippy for their targets only), drops onto
another window's tab strip (X11/Windows/macOS), device loss with several
windows, HiDPI. Tests were compiled, not run (AGENTS.md §3).

### Glass blur on Windows 11 and KDE — 2026-10-29

Glass now blurs on Windows 11 22H2+ (`DWMWA_SYSTEMBACKDROP_TYPE` acrylic),
KDE Plasma on Wayland (winit's `org_kde_kwin_blur` request) and KDE on X11
(`_KDE_NET_WM_BLUR_BEHIND_REGION`, set from the compositor worker). GNOME is
TD-12. On the desktop stand (KWin 5.27, compositing off): through XWayland the
property appears on the board window when Over turns Glass on; on native
Wayland winit reports that this nested KWin offers no blur manager, as
expected without compositing. The visual effect itself, Windows 11 and real
Plasma sessions still need a native check. Windows passed Clippy for its
target only.
