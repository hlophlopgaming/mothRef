#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Desktop stand: run the MothRef release binary under a real KWin (Plasma 5.27)
# or GNOME Shell (46) inside Xvfb, drive it with xdotool and read window state
# from the compositor itself. See SKILL.md next to this file.
#
#   stand.sh install                      apt packages, kwin copy, helpers, release build
#   stand.sh kde|gnome start              compositor + session bus (+ Xwayland)
#   stand.sh kde|gnome app [x11]          MothRef (native Wayland, or XWayland with x11)
#   stand.sh kde|gnome apps               test windows: "Krita" (+ popup) and "Firefox"
#   stand.sh kde|gnome list               windows bottom→top: title, above, focus, …
#   stand.sh kde|gnome activate TITLE     focus a window by title
#   stand.sh kde resize WIDTH [HEIGHT]    unmaximize MothRef and resize it (KWin only)
#   stand.sh kde|gnome popup on|off       show or hide Krita's transient popup
#   stand.sh kde|gnome shot FILE.png      screenshot of the whole virtual screen
#   stand.sh kde|gnome env                print exports for xdotool/dbus-send (eval it)
#   stand.sh stop                         kill everything, restore /run/systemd/seats
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
STAND=${MOTHREF_STAND:-/tmp/mothref-stand}
BIN="$REPO/target/release/mothref"
TWOWIN="$STAND/target/release/twowin"

die() { echo "stand: $*" >&2; exit 1; }

# Kill by exact process name only: `pkill -f` would match this script's own command line.
kill_name() { pkill -x "$1" 2>/dev/null || true; }

install() {
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends \
        kwin-wayland kwin-wayland-backend-virtual xwayland qdbus-qt5 \
        gnome-shell gnome-shell-common gsettings-desktop-schemas dconf-gsettings-backend \
        at-spi2-core dbus-x11 mesa-vulkan-drivers x11-apps xdotool xterm imagemagick \
        libdbus-1-dev xvfb >/dev/null
    mkdir -p "$STAND"
    # The packaged binary carries file capabilities the container refuses to exec.
    cp /usr/bin/kwin_wayland "$STAND/kwin_wayland"
    cargo build -q --release --manifest-path "$HERE/twowin/Cargo.toml" --target-dir "$STAND/target"
    (cd "$REPO" && cargo build -q -p mothref --release)
    echo "installed into $STAND"
}

setup() {
    DESKTOP=$1
    case $DESKTOP in
        kde) DISPLAY_NUM=91; SOCKET=wayland-kde ;;
        gnome) DISPLAY_NUM=92; SOCKET=wayland-gnome ;;
        *) die "unknown desktop $DESKTOP (kde or gnome)" ;;
    esac
    DIR="$STAND/$DESKTOP"
    mkdir -p "$DIR/run" "$DIR/home"
    chmod 700 "$DIR/run"
    export XDG_RUNTIME_DIR="$DIR/run" DISPLAY=":$DISPLAY_NUM" WAYLAND_DISPLAY=$SOCKET
    export XDG_SESSION_TYPE=wayland HOME="$DIR/home" LIBGL_ALWAYS_SOFTWARE=1
    export XDG_DATA_HOME="$DIR/home/.local/share" XDG_CONFIG_HOME="$DIR/home/.config"
    if [ "$DESKTOP" = kde ]; then export XDG_CURRENT_DESKTOP=KDE QT_QPA_PLATFORM=xcb; else export XDG_CURRENT_DESKTOP=GNOME; fi
    # Always the stand's own session bus, never one inherited from the caller.
    unset DBUS_SESSION_BUS_ADDRESS
    if [ -f "$DIR/bus" ]; then export DBUS_SESSION_BUS_ADDRESS=$(cat "$DIR/bus"); fi
}

alive_bus() {
    [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] &&
        dbus-send --session --print-reply --dest=org.freedesktop.DBus / org.freedesktop.DBus.GetId >/dev/null 2>&1
}

xvfb() {
    if ! pgrep -f "Xvfb :$DISPLAY_NUM " >/dev/null; then
        rm -f "/tmp/.X$DISPLAY_NUM-lock" "/tmp/.X11-unix/X$DISPLAY_NUM"
        (setsid nohup Xvfb ":$DISPLAY_NUM" -screen 0 1400x900x24 +extension GLX >"$DIR/xvfb.log" 2>&1 &)
        sleep 2
    fi
}

session_bus() {
    if ! alive_bus; then
        # Started after the environment is set, so activated services (dconf) see it.
        dbus-daemon --session --fork --print-address=1 >"$DIR/bus"
        export DBUS_SESSION_BUS_ADDRESS=$(cat "$DIR/bus")
    fi
}

xwayland_display() {
    if [ "$DESKTOP" = kde ]; then echo ":0"; else grep -o "public X11 display :[0-9]*" "$DIR/shell.log" | grep -o ":[0-9]*" | tail -1; fi
}
xauthority() { ls -t "$DIR"/run/.mutter-Xwaylandauth* 2>/dev/null | head -1 || true; }

start() {
    xvfb
    session_bus
    if [ "$DESKTOP" = kde ]; then
        [ -x "$STAND/kwin_wayland" ] || die "run: stand.sh install"
        kill_name kwin_wayland; sleep 1
        rm -f "$DIR"/run/*.js
        (setsid "$STAND/kwin_wayland" --xwayland --x11-display "$DISPLAY" --width 1380 --height 880 \
            --socket "$WAYLAND_DISPLAY" --no-lockscreen >"$DIR/kwin.log" 2>&1 &)
        sleep 6
    else
        # GNOME Shell needs a live system bus, and treats /run/systemd/seats as
        # proof that logind runs (it does not here).
        mkdir -p /run/dbus
        if ! dbus-send --system --print-reply --dest=org.freedesktop.DBus / org.freedesktop.DBus.GetId >/dev/null 2>&1; then
            rm -f /run/dbus/system_bus_socket /run/dbus/pid
            dbus-daemon --system --fork
        fi
        [ -d /run/systemd/seats ] && mv /run/systemd/seats /run/systemd/seats.mothref-stand-hidden
        local ext="$XDG_DATA_HOME/gnome-shell/extensions"
        mkdir -p "$ext/inspect@mothref.test" "$ext/mothref-windows@hlophlopgaming.github.io"
        cp "$HERE"/gnome-inspect/* "$ext/inspect@mothref.test/"
        cp "$REPO"/crates/moth-platform/gnome-shell/* "$ext/mothref-windows@hlophlopgaming.github.io/"
        gsettings set org.gnome.shell enabled-extensions \
            "['inspect@mothref.test', 'mothref-windows@hlophlopgaming.github.io']"
        kill_name gnome-shell; sleep 2
        rm -f "$DIR/run/gnome-shell-disable-extensions" "$DIR"/run/.mutter-Xwaylandauth*
        (MUTTER_DEBUG_DUMMY_MODE_SPECS=1380x860 setsid gnome-shell --nested --wayland \
            --wayland-display "$WAYLAND_DISPLAY" >"$DIR/shell.log" 2>&1 &)
        sleep 12
        # The shell opens in the overview, which takes keyboard input.
        xdotool mousemove 700 500 click 1 || true
    fi
    echo "$DESKTOP started on $DISPLAY / $WAYLAND_DISPLAY, Xwayland $(xwayland_display)"
}

app() {
    [ -x "$BIN" ] || die "build first: cargo build -p mothref --release"
    kill_name mothref; sleep 1
    if [ "${1:-}" = x11 ]; then
        (setsid env -u WAYLAND_DISPLAY DISPLAY="$(xwayland_display)" XAUTHORITY="$(xauthority)" \
            RUST_LOG=mothref=info,moth_platform=info "$BIN" --software >"$DIR/app.log" 2>&1 &)
    else
        (setsid env RUST_LOG=mothref=info,moth_platform=info "$BIN" --software >"$DIR/app.log" 2>&1 &)
    fi
    sleep 8
    grep -o "Compositor integration probed.*" "$DIR/app.log" || true
}

apps() {
    [ -x "$TWOWIN" ] || die "run: stand.sh install"
    kill_name twowin; kill_name xterm; rm -f "$DIR/popup"
    local display auth
    display=$(xwayland_display); auth=$(xauthority)
    (cd "$DIR" && XAUTHORITY="$auth" DISPLAY="$display" setsid "$TWOWIN" >"$DIR/twowin.log" 2>&1 &)
    (XAUTHORITY="$auth" DISPLAY="$display" setsid xterm -T Firefox -geometry 80x24+300+300 >/dev/null 2>&1 &)
    sleep 3
}

# KWin has no way to read window state from outside, so a one-shot script
# prints it to KWin's log (Plasma 5 and 6 APIs).
kwin_run() {
    local n=$RANDOM file="$DIR/run/stand-$RANDOM.js"
    printf '%s\n' "$1" >"$file"
    local before
    before=$(grep -c "MOTHSTAND" "$DIR/kwin.log" || true)
    dbus-send --session --print-reply --dest=org.kde.KWin /Scripting \
        org.kde.kwin.Scripting.loadScript "string:$file" "string:stand$n" >/dev/null
    # `start` runs every loaded script; running by id is unreliable on Plasma 5.
    dbus-send --session --print-reply --dest=org.kde.KWin /Scripting org.kde.kwin.Scripting.start >/dev/null
    sleep 1
    dbus-send --session --print-reply --dest=org.kde.KWin /Scripting \
        org.kde.kwin.Scripting.unloadScript "string:stand$n" >/dev/null
    rm -f "$file"
    grep "MOTHSTAND" "$DIR/kwin.log" | tail -n +"$((before + 1))" | sed 's/.*MOTHSTAND //'
}
KWIN_LIST='const list = workspace.windowList ? workspace.windowList() : workspace.clientList();'

list() {
    if [ "$DESKTOP" = kde ]; then
        kwin_run "$KWIN_LIST
const active = workspace.activeWindow || workspace.activeClient;
const rows = [];
for (let i = 0; i < list.length; i++) { const w = list[i]; if (w.normalWindow || w.dialog) rows.push(w); }
rows.sort(function (a, b) { return a.stackingOrder - b.stackingOrder; });
rows.forEach(function (w) { print('MOTHSTAND ' + JSON.stringify({title: w.caption, pid: w.pid, above: w.keepAbove, focus: w === active, dialog: w.dialog})); });"
    else
        dbus-send --session --print-reply --dest=test.Inspect /test test.Inspect.List |
            sed -n 's/^ *string "//; s/"$//p' | sed 's/\\"/"/g' |
            python3 -I -c 'import json, sys; [print(json.dumps(w)) for w in json.load(sys.stdin)]'
    fi
}

activate() {
    if [ "$DESKTOP" = kde ]; then
        kwin_run "$KWIN_LIST
for (let i = 0; i < list.length; i++) if (list[i].caption === $(printf '%s' "$1" | python3 -I -c 'import json,sys; print(json.dumps(sys.stdin.read()))')) {
  if (workspace.activeClient !== undefined) workspace.activeClient = list[i]; else workspace.activeWindow = list[i]; }" >/dev/null
    else
        dbus-send --session --print-reply --dest=test.Inspect /test test.Inspect.Activate "string:$1" >/dev/null
    fi
    sleep 1
}

resize() {
    [ "$DESKTOP" = kde ] || die "resize is KWin-only"
    kwin_run "$KWIN_LIST
for (let i = 0; i < list.length; i++) if (list[i].caption === 'MothRef') { const w = list[i];
  w.setMaximize(false, false); w.frameGeometry = {x: 10, y: 10, width: $1, height: ${2:-700}}; }" >/dev/null
    sleep 1
}

stop() {
    for name in mothref twowin xterm kwin_wayland gnome-shell Xvfb; do kill_name "$name"; done
    [ -d /run/systemd/seats.mothref-stand-hidden ] && mv /run/systemd/seats.mothref-stand-hidden /run/systemd/seats
    echo stopped
}

case ${1:-} in
    install) install ;;
    stop) stop ;;
    kde|gnome)
        setup "$1"
        command=${2:-}
        shift 2 || true
        case $command in
            start) start ;;
            app) app "$@" ;;
            apps) apps ;;
            list) list ;;
            activate) activate "$1" ;;
            resize) resize "$@" ;;
            popup) if [ "${1:-}" = on ]; then touch "$DIR/popup"; else rm -f "$DIR/popup"; fi; sleep 2 ;;
            shot) import -window root "$1" ;;
            env) env | grep -E '^(XDG_|DISPLAY|WAYLAND_DISPLAY|DBUS_SESSION_BUS_ADDRESS|HOME|LIBGL)' |
                while IFS= read -r line; do printf 'export %s=%q\n' "${line%%=*}" "${line#*=}"; done ;;
            *) die "unknown command '$command'" ;;
        esac ;;
    *) sed -n '2,20p' "$0"; exit 1 ;;
esac
