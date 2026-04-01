# Clippy

Good clipboard manager for GNOME.

## Features
- Keeps a persistent history of copied text, files, and images.
- Quickly search and filter through entries.
- Easy integration into GNOME Shell via an extension indicator in the system tray.

## Dependencies
- `rust`, `cargo` for building the source
- GTK4 / Libadwaita development libraries

## Automatic Installation

You can easily build, install, and configure everything in one go:
```bash
./install.sh
```

This script will:
1. Build the daemon and UI tools using Cargo.
2. Copy the resulting binaries (`clippy-daemon` and `clippy-ui`) to `~/.local/bin/`.
3. Set up the daemon to run automatically on login by creating an autostart `.desktop` file.
4. Install the UI `.desktop` entries to `~/.local/share/applications/` so your app launcher picks it up along with an icon.
5. Create and install the GNOME extension (you may need to allow it or restart your shell afterwards).

**Note:** After running the script, log out and log back into your GNOME session (or restart GNOME shell under X11) for the extension to appear. You can start the daemon manually to get running immediately: `~/.local/bin/clippy-daemon &`

## Manual Installation

1. Build the Rust workspace components:
   ```bash
   cargo build --release
   ```

2. Move binaries to a location in your PATH, such as:
   ```bash
   mkdir -p ~/.local/bin
   cp target/release/clippy-daemon ~/.local/bin/
   cp target/release/clippy-ui ~/.local/bin/
   ```

3. Install the application icon and desktop shortcut:
```bash
   mkdir -p ~/.local/share/icons/hicolor/512x512/apps/
   cp clippy-ui/src/ui/resources/clippy-icon.png ~/.local/share/icons/hicolor/512x512/apps/clippy-icon.png
   mkdir -p ~/.local/share/applications
   cat << EOF > ~/.local/share/applications/com.example.clippy.ui.desktop
   [Desktop Entry]
   Name=Clippy
   Comment=Clipboard Manager
   Exec=$HOME/.local/bin/clippy-ui
   Icon=clippy-icon
   Terminal=false
   Type=Application
   Categories=Utility;
   StartupNotify=true
   StartupWMClass=com.example.clippy.ui
   EOF
   update-desktop-database ~/.local/share/applications/
```

4. Add auto-start on login for the daemon:
```bash
   mkdir -p ~/.config/autostart
   cat << EOF > ~/.config/autostart/clippy-daemon.desktop
   [Desktop Entry]
   Type=Application
   Name=Clippy Daemon
   Exec=$HOME/.local/bin/clippy-daemon
   Hidden=false
   NoDisplay=false
   X-GNOME-Autostart-enabled=true
   EOF
```

5. Install the GNOME extension manually:
   ```bash
   mkdir -p ~/.local/share/gnome-shell/extensions/clippy@example.com
   cp extension/extension.js extension/metadata.json ~/.local/share/gnome-shell/extensions/clippy@example.com/
   gnome-extensions enable clippy@example.com
   ```

6. Restart the GNOME Shell (log out/in or Alt+F2 `r` on X11) and run `~/.local/bin/clippy-daemon &`.

## Setting up a Hotkey

To easily access the Clippy UI, you can bind it to a custom keyboard shortcut in GNOME:

1. Open **Settings** in GNOME.
2. Navigate to **Keyboard** -> **Keyboard Shortcuts** -> **View and Customize Shortcuts**.
3. Scroll down and click on **Custom Shortcuts**.
4. Click the **+** (Add) button.
5. Fill in the details:
   - **Name**: Clippy
   - **Command**: `~/.local/bin/clippy-ui`
   - **Shortcut**: Set your desired key combination (e.g., `Super + V`).
6. Click **Add**.

Now, pressing your chosen hotkey will instantly launch the Clippy UI!

## Automatic Uninstallation

An uninstallation script is available:
```bash
./uninstall.sh
```

## Manual Uninstallation

1. Kill services: `pkill clippy-daemon; pkill clippy-ui`
2. Remove binaries:
   `rm ~/.local/bin/clippy-daemon ~/.local/bin/clippy-ui`
3. Remove autostart configuration:
   `rm ~/.config/autostart/clippy-daemon.desktop`
4. Remove application shortcuts and icon files:
   `rm ~/.local/share/applications/com.example.clippy.ui.desktop`
   `rm ~/.local/share/icons/hicolor/512x512/apps/clippy-icon.png`
5. Disable and remove the GNOME extension:
   `gnome-extensions disable clippy@example.com`
   `rm -rf ~/.local/share/gnome-shell/extensions/clippy@example.com`
