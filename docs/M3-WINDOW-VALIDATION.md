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
| Wayland | Unsupported pin choices disabled; frameless drag and reveal remain usable | Pending |
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
