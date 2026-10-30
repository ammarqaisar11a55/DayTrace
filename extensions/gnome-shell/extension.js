// GNOME Shell 45 and later. See legacy/extension.js for older versions.
import Gio from 'gi://Gio';
import Shell from 'gi://Shell';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const OBJECT_PATH = '/io/github/ammarqaisar11a55/DayTrace';
const INTERFACE = `
<node>
  <interface name="io.github.ammarqaisar11a55.DayTrace.Focus">
    <method name="GetFocus">
      <arg type="b" name="includeTitle" direction="in"/>
      <arg type="b" name="focused" direction="out"/>
      <arg type="s" name="appId" direction="out"/>
      <arg type="s" name="appName" direction="out"/>
      <arg type="s" name="wmClass" direction="out"/>
      <arg type="u" name="pid" direction="out"/>
      <arg type="s" name="title" direction="out"/>
    </method>
    <signal name="FocusChanged"/>
  </interface>
</node>`;

export default class DayTraceExtension extends Extension {
    enable() {
        this._exported = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this);
        this._exported.export(Gio.DBus.session, OBJECT_PATH);
        this._focusSignal = global.display.connect('notify::focus-window', () => {
            this._exported.emit_signal('FocusChanged', null);
        });
    }

    disable() {
        global.display.disconnect(this._focusSignal);
        this._focusSignal = null;
        this._exported.unexport();
        this._exported = null;
    }

    GetFocus(includeTitle) {
        const window = global.display.focus_window;
        if (!window)
            return [false, '', '', '', 0, ''];

        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const title = includeTitle ? window.get_title() ?? '' : '';
        return [
            true,
            app?.get_id() ?? '',
            app?.get_name() ?? '',
            window.get_wm_class() ?? '',
            window.get_pid() > 0 ? window.get_pid() : 0,
            title,
        ];
    }
}
