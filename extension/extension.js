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
        let mimetypes = this._clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD);
        if (!mimetypes) return;

        if (mimetypes.includes('image/png')) {
            this._clipboard.get_content(
                St.ClipboardType.CLIPBOARD,
                'image/png',
                (_, bytes) => {
                    if (!bytes) return;

                    let key = "img:" + bytes.get_size();
                    if (this._last === key) return;
                    this._last = key;

                    let data = bytes.get_data();
                    if (!data) return;

                    console.log(`[Clippy] image (${bytes.get_size()} bytes)`);

                    this._sendImageToDbus(data);
                }
            );

        } else if (mimetypes.includes('x-special/gnome-copied-files')) {
            this._clipboard.get_content(
                St.ClipboardType.CLIPBOARD,
                'x-special/gnome-copied-files',
                (_, bytes) => {
                    if (!bytes) return;

                    let text = new TextDecoder().decode(bytes.get_data());
                    if (!text || text === this._last) return;

                    this._last = text;
                    this._sendFileToDbus(text);
                }
            );

        } else if (mimetypes.includes('text/uri-list')) {
            this._clipboard.get_content(
                St.ClipboardType.CLIPBOARD,
                'text/uri-list',
                (_, bytes) => {
                    if (!bytes) return;

                    let text = new TextDecoder().decode(bytes.get_data());
                    if (!text || text === this._last) return;

                    this._last = text;
                    this._sendFileToDbus(text);
                }
            );

        } else if (
            mimetypes.includes('text/plain;charset=utf-8') ||
            mimetypes.includes('UTF8_STRING') ||
            mimetypes.includes('text/plain')
        ) {
            this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_, text) => {
                if (!text || text === this._last) return;

                this._last = text;
                this._sendToDbus(text);
            });
        }
    }

    _sendImageToDbus(data) {
        try {
            Gio.DBus.session.call(
                'com.example.clippy',
                '/com/example/clippy',
                'com.example.clippy.Daemon',
                'NewImage',
                new GLib.Variant('(ay)', [data]),
                null,
                Gio.DBusCallFlags.NO_AUTO_START,
                1000,
                null,
                (conn, res) => {
                    try {
                        conn.call_finish(res);
                        this._failCount = 0;
                    } catch (e) {
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

    _sendFileToDbus(uris) {
        try {
            Gio.DBus.session.call(
                'com.example.clippy',
                '/com/example/clippy',
                'com.example.clippy.Daemon',
                'NewFile',
                new GLib.Variant('(s)', [uris]),
                null,
                Gio.DBusCallFlags.NO_AUTO_START,
                1000,
                null,
                (conn, res) => {
                    try {
                        conn.call_finish(res);
                        this._failCount = 0;
                    } catch (e) {
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