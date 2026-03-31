import St from 'gi://St';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

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

        this._buildTrayIcon();

        console.log('[Clippy] extension enabled');
    }

    disable() {
        if (this._timer) {
            GLib.source_remove(this._timer);
            this._timer = null;
        }
        if (this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }
        this._clipboard = null;
        this._last = '';
        console.log('[Clippy] extension disabled');
    }

    _buildTrayIcon() {
        this._indicator = new PanelMenu.Button(0.0, 'ClippyIndicator', false);

        // Icon
        let icon = new St.Icon({
            icon_name: 'edit-paste-symbolic',
            style_class: 'system-status-icon'
        });
        this._indicator.add_child(icon);

        // Max Entries Option
        let maxEntriesMenu = new PopupMenu.PopupSubMenuMenuItem('Max entries');
        let entryCounts = [10, 30, 50, 100];
        for (let count of entryCounts) {
            maxEntriesMenu.menu.addAction(`${count}`, () => {
                this._setSetting('max_entries', `${count}`);
            });
        }
        this._indicator.menu.addMenuItem(maxEntriesMenu);

        // Prune Old Time Option
        let pruneTimeMenu = new PopupMenu.PopupSubMenuMenuItem('Prune old time');
        let timeOptions = [
            { label: '1 day', secs: 1 * 24 * 60 * 60 },
            { label: '3 days', secs: 3 * 24 * 60 * 60 },
            { label: '7 days', secs: 7 * 24 * 60 * 60 },
            { label: '30 days', secs: 30 * 24 * 60 * 60 }
        ];
        for (let opt of timeOptions) {
            pruneTimeMenu.menu.addAction(opt.label, () => {
                this._setSetting('prune_time_secs', `${opt.secs}`);
                this._setSetting('is_prune_active', 'true');
            });
        }
        pruneTimeMenu.menu.addAction('Never', () => {
            this._setSetting('is_prune_active', 'false');
        });
        this._indicator.menu.addMenuItem(pruneTimeMenu);

        this._indicator.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        // Quit Option
        this._indicator.menu.addAction('Quit Clippy', () => {
            this._quitDaemon();
            // Disabling the extension kills this part automatically from user point of view,
            // or the user will toggle it off in App. We stop the daemon.
            this.disable();
        });

        Main.panel.addToStatusArea('ClippyIndicator', this._indicator);
    }

    _setSetting(key, value) {
        try {
            Gio.DBus.session.call(
                'com.example.clippy',
                 '/com/example/clippy',
                 'com.example.clippy.Daemon',
                 'SetSetting',
                 new GLib.Variant('(ss)', [key, value]),
                 null,
                 Gio.DBusCallFlags.NO_AUTO_START,
                 1000,
                 null,
                 null
            );
        } catch (e) {
            console.error(`[Clippy] Failed to set setting: ${e}`);
        }
    }

    _quitDaemon() {
        try {
            Gio.DBus.session.call(
                'com.example.clippy',
                 '/com/example/clippy',
                 'com.example.clippy.Daemon',
                 'Quit',
                 null,
                 null,
                 Gio.DBusCallFlags.NO_AUTO_START,
                 1000,
                 null,
                 null
            );
        } catch (e) {
            console.error(`[Clippy] Failed to quit daemon: ${e}`);
        }
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