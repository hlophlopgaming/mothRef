---
name: desktop-stand
description: Run the MothRef release binary under a real KWin (KDE Plasma 5.27) or GNOME Shell 46 inside this Linux container, drive it with xdotool and read window state (stacking, keep-above, focus, maximized) from the compositor. Use to check window stacking, pinning, Over mode, the window chrome or any UI change on a real Wayland/X11 desktop, and to take screenshots.
---

# Desktop stand: MothRef under KWin and GNOME Shell

Everything goes through `stand.sh` next to this file. It keeps its state in
`$MOTHREF_STAND` (default `/tmp/mothref-stand`): one directory per desktop with
its own runtime dir, session bus, HOME and logs.

```bash
S=.claude/skills/desktop-stand/stand.sh
$S install                 # once per container: apt packages, kwin copy, helper, release build
$S kde start               # or: $S gnome start
$S kde apps                # "Krita" (with a transient "Brush popup") and "Firefox"
$S kde app                 # MothRef, native Wayland; `$S kde app x11` runs it through XWayland
$S kde list                # windows bottom→top as JSON lines
$S kde activate Krita      # focus a window by title
$S kde popup on            # show Krita's popup (off hides it)
$S kde resize 700          # unmaximize and resize MothRef (KWin only)
$S kde shot /tmp/mothref-stand/a.png   # then Read the PNG
eval "$($S kde env)"       # DISPLAY etc. for xdotool / dbus-send in this shell
$S stop                    # kill everything and restore /run/systemd/seats
```

After changing Rust code, rebuild with `cargo build -p mothref --release` and
run `$S <desktop> app` again. After changing the GNOME extension
(`crates/moth-platform/gnome-shell/`), run `$S gnome start` again: it copies the
extension and restarts the shell (the equivalent of logging out and in).

## What you get

- **KDE:** `kwin_wayland` 5.27 with the X11 backend, drawing into Xvfb `:91`,
  Wayland socket `wayland-kde`, Xwayland `:0`. `list` runs a one-shot KWin
  script that prints `title`, `pid`, `above` (keepAbove), `focus` and `dialog`.
- **GNOME:** `gnome-shell --nested` 46 in Xvfb `:92`, 1380×860, Wayland socket
  `wayland-gnome`. `list` goes through a test-only inspector extension
  (`gnome-inspect/`, never ship it) and adds `maximized` and `layer`
  (2 normal, 4 top/above).
- Software rendering everywhere: MothRef runs with `--software` (lavapipe).
- `apps` starts `twowin/` (one X11 process: main window "Krita" plus a transient
  "Brush popup") and an xterm titled "Firefox", both through Xwayland, so
  "chosen app", "its popup" and "another app" can all be tested.

## Driving the UI

xdotool works on the Xvfb display: the compositor forwards input to the focused
Wayland client. egui needs a hover frame before a click, so always
`xdotool mousemove X Y; sleep 0.3; xdotool click 1`, never `mousemove … click`
in one go.

Useful positions (logical = screen pixels, scale 1):

- KDE starts MothRef maximized at (0, 0) and 1380 wide; GNOME at (0, 32)
  because of its top bar. Library "New board" is at about (1200, 83) on KDE.
- Collect window controls appear when the pointer is over the top bar. With the
  window's right edge at `R` and bar centre at `Y`: pin ▾ at `R − 159`, and
  "Above a chosen window" in its menu at `(R − 101, Y + 89)`.
- View: the floating bar slides in when the pointer is within 16 px of the top.
- Shortcuts: Ctrl+Shift+T pin, Ctrl+T Over, Tab Collect/View, Ctrl+O Settings.

Picking a target window: choose "Above a chosen window", then
`$S kde activate Krita` (activation is what MothRef follows).

## Pitfalls already paid for

- Never `pkill -f` a pattern that appears in your own command line: it kills
  the shell running it. The script kills by exact name.
- GNOME Shell refuses to start without a live system bus, and believes logind
  runs if `/run/systemd/seats` exists; `start` handles both and `stop`
  restores the directory. Do not leave it renamed.
- GNOME opens in the overview, which takes keyboard input; `start` clicks once
  to leave it. MothRef's 1440×900 default window is larger than the nested
  screen, so Mutter maximizes it — maximized windows ignore "above".
- KWin 5 numbers loaded scripts by list length; run scripts with
  `org.kde.kwin.Scripting.start`, not by the `/<id>` path.
- The stand always uses its own session bus; do not export another
  `DBUS_SESSION_BUS_ADDRESS` into it.
- Stale `/tmp/.X91-lock` or `.X92-lock` from a dead Xvfb block a restart;
  `start` removes them when no Xvfb runs.

## Limits

Nested compositors with software rendering: no real GPU, HiDPI, touch or tablet
input, and Plasma 6 / GNOME 47+ are not available from Ubuntu 24.04. Results
qualify behavior on these versions only; record them in
`docs/M3-WINDOW-VALIDATION.md` as nested-session evidence. Running the test
suite still follows AGENTS.md §3.
