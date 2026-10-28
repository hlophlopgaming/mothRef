// SPDX-License-Identifier: GPL-3.0-or-later
// GNOME Shell exposes no protocol for keeping a Wayland window above others.
// This extension offers MothRef exactly that one operation over the session bus,
// limited to windows owned by the calling process.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.mothref.Shell';
const OBJECT_PATH = '/org/mothref/Shell';
const INTERFACE = `<node>
  <interface name="org.mothref.Shell1">
    <method name="SetKeepAbove">
      <arg type="s" direction="in" name="title"/>
      <arg type="b" direction="in" name="above"/>
      <arg type="u" direction="out" name="windows"/>
    </method>
    <property name="Version" type="u" access="read"/>
  </interface>
</node>`;

class Service {
    get Version() {
        return 1;
    }

    // The caller is identified by the bus, never by an argument, so another
    // process cannot change windows that are not its own.
    SetKeepAboveAsync([title, above], invocation) {
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
                    let count = 0;
                    for (const actor of global.get_window_actors()) {
                        const window = actor.get_meta_window();
                        if (!window || window.get_pid() !== pid || window.get_title() !== title)
                            continue;
                        if (above)
                            window.make_above();
                        else
                            window.unmake_above();
                        count++;
                    }
                    invocation.return_value(new GLib.Variant('(u)', [count]));
                } catch (error) {
                    invocation.return_dbus_error('org.mothref.Shell1.Error', String(error));
                }
            });
    }
}

export default class MothRefWindows extends Extension {
    enable() {
        this._object = Gio.DBusExportedObject.wrapJSObject(INTERFACE, new Service());
        this._object.export(Gio.DBus.session, OBJECT_PATH);
        this._name = Gio.bus_own_name_on_connection(
            Gio.DBus.session, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        if (this._name)
            Gio.bus_unown_name(this._name);
        this._object?.unexport();
        this._name = 0;
        this._object = null;
    }
}
