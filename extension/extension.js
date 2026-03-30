import St from 'gi://St';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

export default class ClippyWatcher extends Extension {
    enable() {
        this._clipboard = St.Clipboard.get_default();
        this._last = '';
        this._failCount = 0;

        // Poll every 500ms — St.Clipboard has no push signal,
        // so every clipboard manager on GNOME uses a timer.
        this._timer = GLib.timeout_add(GLib.PRIORITY_LOW, 500, () => {
            this._check();
            return GLib.SOURCE_CONTINUE; // keep repeating
        });

        console.log('[Clippy] extension enabled');
    }

    disable() {
        if (this._timer) {
            GLib.source_remove(this._timer);
            this._timer = null;
        }
        this._clipboard = null;
        this._last = '';
        console.log('[Clippy] extension disabled');
    }

    _check() {
        this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_, text) => {
            if (!text || text === this._last) return;
            this._last = text;
            console.log(`[Clippy] clipboard changed (${text.length} chars)`);
            this._sendToDbus(text);
        });
    }

    _sendToDbus(text) {
        // Call the NewEntry method on the Clippy Rust app over D-Bus.
        // If Clippy isn't running this fails silently — that's fine.
        try {
            Gio.DBus.session.call(
                'com.example.clippy',       // bus name your Rust app registers
                '/com/example/clippy',      // object path
                'com.example.clippy.Daemon',// interface name
                'NewEntry',                 // method name
                new GLib.Variant('(s)', [text]),
                null,                       // no expected reply type
                Gio.DBusCallFlags.NO_AUTO_START, // don't launch app if not running
                1000,                       // timeout ms
                null,
                (conn, res) => {
                    try {
                        conn.call_finish(res);
                        this._failCount = 0;
                    } catch (e) {
                        // App not running is normal — only log every 10 failures
                        this._failCount++;
                        if (this._failCount % 10 === 1) {
                            console.log(`[Clippy] app not reachable over D-Bus (is it running?)`);
                        }
                    }
                }
            );
        } catch (e) {
            console.error(`[Clippy] D-Bus call error: ${e}`);
        }
    }
}