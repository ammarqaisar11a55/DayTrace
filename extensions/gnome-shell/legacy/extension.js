// GNOME Shell 42 to 44, which predate ES modules for extensions.
/* exported init */
const {Gio, Shell} = imports.gi;

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

class DayTraceExtension {
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
        const title = includeTitle ? window.get_title() || '' : '';
        return [
            true,
            app ? app.get_id() || '' : '',
            app ? app.get_name() || '' : '',
            window.get_wm_class() || '',
            window.get_pid() > 0 ? window.get_pid() : 0,
            title,
        ];
    }
}

function init() {
    return new DayTraceExtension();
}
