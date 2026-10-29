// SPDX-License-Identifier: GPL-3.0-or-later
// Test-only GNOME Shell extension for the desktop stand: lists windows with their
// stacking, layer and focus, and activates a window by title. Never ship it.
import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const IFACE = `<node><interface name="test.Inspect">
  <method name="List"><arg type="s" direction="out"/></method>
  <method name="Activate"><arg type="s" direction="in"/></method>
</interface></node>`;

export default class Inspect extends Extension {
    enable() {
        this._object = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._object.export(Gio.DBus.session, '/test');
        this._name = Gio.bus_own_name_on_connection(Gio.DBus.session, 'test.Inspect', 0, null, null);
    }

    disable() {
        Gio.bus_unown_name(this._name);
        this._object.unexport();
    }

    // Bottom to top, so the last entry is the topmost window.
    List() {
        const focus = global.display.focus_window;
        const windows = global.get_window_actors().map(a => a.get_meta_window());
        return JSON.stringify(global.display.sort_windows_by_stacking(windows).map(w => ({
            title: w.get_title(), pid: w.get_pid(), above: w.is_above(),
            maximized: w.get_maximized(), layer: w.get_layer(), focus: w === focus,
        })));
    }

    Activate(title) {
        for (const actor of global.get_window_actors()) {
            const window = actor.get_meta_window();
            if (window.get_title() === title)
                window.activate(global.get_current_time());
        }
    }
}
