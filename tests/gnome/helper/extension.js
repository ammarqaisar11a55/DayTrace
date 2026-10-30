// Test only. Never installed by DayTrace, and only ever enabled inside the
// disposable session created by session.sh.
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class TestHelper extends Extension {
    enable() {
        global.context.unsafe_mode = true;
    }

    disable() {
        global.context.unsafe_mode = false;
    }
}
