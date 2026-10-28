# 0019 — Window stacking on Wayland through compositor scripting

## Context

"Above all windows", "above a chosen window" and the Over overlay need the board
to stay above other applications. On X11, Windows and macOS the app asks the
window system directly. Wayland has no protocol for a client to stack its own
toplevels, so on KDE Plasma and GNOME every pin choice was disabled and the
overlay could be covered by the application the user draws in. This was the
most visible gap for Linux users, and the maintainer made it the priority
(2026-10-28).

Each compositor offers a different, non-standard way in:

- **KWin (KDE Plasma 5 and 6)** loads JavaScript window scripts over the session
  bus (`org.kde.KWin /Scripting`). A script can set `keepAbove` on any window
  and listen to activation, and can call back with `callDBus`.
- **Mutter (GNOME)** has no scripting interface for applications;
  `org.gnome.Shell.Eval` is disabled outside unsafe mode. Only a GNOME Shell
  extension can call `Meta.Window.make_above()`.

Both talk over D-Bus. `zbus` 5 is flagged unmaintained by `cargo deny`
(RUSTSEC-2026-0370), `zbus` 6, released twelve days earlier, removed the
blocking API in favour of a new async runtime, and an in-tree client would be
protocol code to maintain ourselves.

## Decision

- `moth-platform::compositor` (Linux only) owns one worker thread. The main
  thread queues requests and reads atomics; the worker probes the session bus
  for `org.kde.KWin`, `org.gnome.Shell` and the MothRef extension, and wakes
  the event loop when capabilities change. Window capabilities stay a cheap,
  per-frame read.
- **KWin:** each request writes a small generated script to
  `$XDG_RUNTIME_DIR`, loads and runs it, and replaces the previous script of
  the same slot. Windows are identified by PID and title (the window class is a
  fallback for sandboxed builds); window ids from KWin (`internalId`) identify
  attach targets. Scripts that wait for a not-yet-mapped window or follow
  activation stay loaded until replaced. Callbacks are accepted only from the
  current owner of `org.kde.KWin`.
  This enables above all, Over and floating views on top, and above a chosen
  window (activation-following `keepAbove`) on Plasma Wayland. Scripts are
  started with `Scripting.start`, not by object path: Plasma 5 reuses script
  ids after unloads.
- **GNOME:** MothRef bundles a GNOME Shell extension
  (`crates/moth-platform/gnome-shell/`, GNOME 45+) that exports
  `org.mothref.Shell1.SetKeepAbove(title, above)`. It identifies the caller by
  its bus credentials, never by an argument, so a client can only change its
  own windows. Mutter ignores "above" for maximized windows, so the
  extension restores them first. Its `Version` property lets the app offer an
  update. Settings › Window and overlay offers to install it (an explicit
  user action; the files go to the user's extension folder and are enabled
  with `gnome-extensions`). GNOME on Wayland loads it after the next login.
  Without it, pin choices stay disabled as before.
- **D-Bus:** the `dbus` crate over the system `libdbus-1` (maintainer decision
  on 2026-10-28, after weighing an in-tree client, `zbus` 6 and an advisory
  exception for `zbus` 5). The worker enables libdbus socket watching and
  sleeps in `poll` on the bus socket and a wake socket, so the app stays idle
  between requests while still answering KWin's callbacks.
- Bottom stacking is a separate capability (`WindowCapabilities::bottom`)
  and stays unavailable on Wayland.
- macOS "above all" also joins all Spaces and full-screen Spaces
  (`CanJoinAllSpaces | FullScreenAuxiliary`); Windows "above a chosen window"
  borrows the topmost band while a topmost target is active and gives it back
  when anything else is activated.

## Consequences

- Plasma and GNOME users get the same pin and Over behavior as X11, after a
  one-time extension install on GNOME.
- Linux builds need `libdbus-1-dev`, and the Linux binary links
  `libdbus-1.so.3`, which every desktop distribution ships. CI installs the
  headers; Flatpak and AppImage packaging must include or depend on it.
- The KWin scripting API and GNOME Shell extension API are not stable
  contracts. Each Plasma or GNOME major release needs a native check, and the
  extension's `shell-version` list must be extended.
- Escape outside MothRef cannot cancel target selection on Wayland; Cancel and
  Escape inside MothRef do.
- Flatpak builds need `--talk-name=org.kde.KWin` / `org.mothref.Shell` and see
  a different PID; the GNOME extension then cannot match windows by caller PID.
  Flatpak packaging must revisit this.
- Other Wayland compositors (Sway, Hyprland, COSMIC…) remain unsupported; their
  pin controls stay disabled.
