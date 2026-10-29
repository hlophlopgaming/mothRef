// SPDX-License-Identifier: GPL-3.0-or-later
// GNOME Shell exposes no protocol for keeping a Wayland window above others.
// This extension offers MothRef that over the session bus: keeping its own
// windows above all others, and keeping its board above one chosen window.
// It only ever changes windows owned by the calling process, plus the chosen
// app's active popup while it is lifted above the board.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.mothref.Shell';
const OBJECT_PATH = '/org/mothref/Shell';
const CALLBACK_PATH = '/org/mothref/Compositor';
const CALLBACK_INTERFACE = 'org.mothref.Compositor1';
const INTERFACE = `<node>
  <interface name="org.mothref.Shell1">
    <method name="SetKeepAbove">
      <arg type="s" direction="in" name="title"/>
      <arg type="b" direction="in" name="above"/>
      <arg type="u" direction="out" name="windows"/>
    </method>
    <method name="Pick"/>
    <method name="CancelPick"/>
    <method name="Attach">
      <arg type="s" direction="in" name="title"/>
      <arg type="s" direction="in" name="target"/>
    </method>
    <method name="Detach"/>
    <property name="Version" type="u" access="read"/>
  </interface>
</node>`;

// Mutter 49 dropped the flags argument.
function unmaximize(window) {
    try {
        window.unmaximize();
    } catch {
        window.unmaximize(Meta.MaximizeFlags.BOTH);
    }
}

// A window that is not shown yet gets stacked below the focused window when
// Mutter maps it without focus, which would hide an overlay controller under
// its board. `shown` is emitted after that, so raising there wins.
function raiseWhenShown(window) {
    const handler = window.connect('shown', () => window.raise());
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 2000, () => {
        try {
            window.disconnect(handler);
        } catch {
            // The window is already gone.
        }
        return GLib.SOURCE_REMOVE;
    });
}

function keepAbove(window, above) {
    if (above) {
        // Mutter keeps maximized windows in the normal layer even when marked
        // above, so the request would do nothing.
        if (window.get_maximized())
            unmaximize(window);
        window.make_above();
        // Raise within the top layer so the latest request wins.
        window.raise();
    } else {
        window.unmake_above();
    }
}

function windows() {
    return global.get_window_actors().map(actor => actor.get_meta_window()).filter(w => w);
}

function regular(window) {
    const type = window.get_window_type();
    return type === Meta.WindowType.NORMAL || type === Meta.WindowType.DIALOG ||
        type === Meta.WindowType.MODAL_DIALOG;
}

// The caller is identified by the bus, never by an argument, so another
// process cannot change windows that are not its own.
function callerPid(invocation, done) {
    Gio.DBus.session.call(
        'org.freedesktop.DBus',
        '/org/freedesktop/DBus',
        'org.freedesktop.DBus',
        'GetConnectionUnixProcessID',
        new GLib.Variant('(s)', [invocation.get_sender()]),
        new GLib.VariantType('(u)'),
        Gio.DBusCallFlags.NONE,
        -1,
        null,
        (connection, result) => {
            try {
                const [pid] = connection.call_finish(result).deepUnpack();
                done(pid);
            } catch (error) {
                invocation.return_dbus_error('org.mothref.Shell1.Error', String(error));
            }
        });
}

function report(sender, method, id) {
    Gio.DBus.session.call(
        sender, CALLBACK_PATH, CALLBACK_INTERFACE, method,
        new GLib.Variant('(s)', [id]), null, Gio.DBusCallFlags.NO_AUTO_START,
        -1, null, null);
}

// One caller's window selection and attachment. Ends when MothRef leaves the bus.
class Session {
    constructor(sender, pid, onGone) {
        this.sender = sender;
        this.pid = pid;
        this.signals = [];
        this.watch = Gio.bus_watch_name_on_connection(
            Gio.DBus.session, sender, Gio.BusNameWatcherFlags.NONE, null,
            () => onGone(sender));
    }

    connect(object, signal, handler) {
        this.signals.push([object, object.connect(signal, handler)]);
    }

    stop() {
        for (const [object, id] of this.signals) {
            try {
                object.disconnect(id);
            } catch {
                // The window is already gone.
            }
        }
        this.signals = [];
        this.drop();
        if (this.boardTitle) {
            for (const board of this.boards())
                board.unmake_above();
        }
        this.boardTitle = null;
        this.target = null;
    }

    close() {
        this.stop();
        Gio.bus_unwatch_name(this.watch);
    }

    mine(window) {
        return window.get_pid() === this.pid;
    }

    boards() {
        return windows().filter(w => this.mine(w) && w.get_title() === this.boardTitle);
    }

    pick() {
        this.stop();
        this.connect(global.display, 'notify::focus-window', () => {
            const window = global.display.focus_window;
            if (!window || this.mine(window) || !regular(window))
                return;
            this.stop();
            report(this.sender, 'Picked', String(window.get_id()));
        });
    }

    related(window) {
        if (window === this.target)
            return true;
        if (this.target.get_pid() > 0 && window.get_pid() === this.target.get_pid())
            return true;
        for (let parent = window.get_transient_for(), depth = 0; parent && depth < 8;
            parent = parent.get_transient_for(), depth++) {
            if (parent === this.target)
                return true;
        }
        return false;
    }

    drop() {
        if (this.lifted) {
            try {
                this.lifted.unmake_above();
            } catch {
                // The window is already gone.
            }
            this.lifted = null;
        }
    }

    // The board stays above the chosen app while the app is active. The app's
    // active popup or dialog is lifted above the board until focus moves on.
    follow() {
        const window = global.display.focus_window;
        if (window !== this.lifted)
            this.drop();
        if (!window)
            return;
        if (this.related(window)) {
            for (const board of this.boards())
                keepAbove(board, true);
            if (window !== this.target && !window.is_above()) {
                window.make_above();
                window.raise();
                this.lifted = window;
            }
        } else if (!this.mine(window) && regular(window)) {
            for (const board of this.boards())
                board.unmake_above();
            // Leaving the top layer puts the board on top of the normal one;
            // the window the user just activated belongs above it.
            window.raise();
        }
    }

    attach(title, id) {
        this.stop();
        this.target = windows().find(w => String(w.get_id()) === id) ?? null;
        if (!this.target)
            return false;
        this.boardTitle = title;
        this.connect(global.display, 'notify::focus-window', () => this.follow());
        this.connect(this.target, 'unmanaged', () => {
            this.stop();
            report(this.sender, 'TargetClosed', id);
        });
        this.follow();
        return true;
    }
}

class Service {
    constructor() {
        this.sessions = new Map();
    }

    get Version() {
        return 5;
    }

    destroy() {
        for (const session of this.sessions.values())
            session.close();
        this.sessions.clear();
    }

    session(invocation, done) {
        const sender = invocation.get_sender();
        const existing = this.sessions.get(sender);
        if (existing) {
            done(existing);
            return;
        }
        callerPid(invocation, pid => {
            const session = new Session(sender, pid, gone => {
                this.sessions.get(gone)?.close();
                this.sessions.delete(gone);
            });
            this.sessions.set(sender, session);
            done(session);
        });
    }

    SetKeepAboveAsync([title, above], invocation) {
        callerPid(invocation, pid => {
            let count = 0;
            for (const window of windows()) {
                if (window.get_pid() !== pid || window.get_title() !== title)
                    continue;
                keepAbove(window, above);
                if (above)
                    raiseWhenShown(window);
                count++;
            }
            invocation.return_value(new GLib.Variant('(u)', [count]));
        });
    }

    PickAsync(_, invocation) {
        this.session(invocation, session => {
            session.pick();
            invocation.return_value(null);
        });
    }

    CancelPickAsync(_, invocation) {
        this.sessions.get(invocation.get_sender())?.stop();
        invocation.return_value(null);
    }

    AttachAsync([title, target], invocation) {
        this.session(invocation, session => {
            if (!session.attach(title, target))
                report(session.sender, 'TargetClosed', target);
            invocation.return_value(null);
        });
    }

    DetachAsync(_, invocation) {
        this.sessions.get(invocation.get_sender())?.stop();
        invocation.return_value(null);
    }
}

export default class MothRefWindows extends Extension {
    enable() {
        this._service = new Service();
        this._object = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this._service);
        this._object.export(Gio.DBus.session, OBJECT_PATH);
        this._name = Gio.bus_own_name_on_connection(
            Gio.DBus.session, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        if (this._name)
            Gio.bus_unown_name(this._name);
        this._object?.unexport();
        this._service?.destroy();
        this._name = 0;
        this._object = null;
        this._service = null;
    }
}
