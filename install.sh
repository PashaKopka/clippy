#!/usr/bin/env bash
set -e
echo "Building Clippy..."
cargo build --release

echo "Installing binaries..."

mkdir -p ~/.local/bin
cp target/release/clippy-daemon ~/.local/bin/
cp target/release/clippy-ui ~/.local/bin/

echo "Setting up autostart for daemon..."

mkdir -p ~/.config/autostart

cat << 'DESK' > ~/.config/autostart/clippy-daemon.desktop
[Desktop Entry]
Type=Application
Name=Clippy Daemon
Exec=$HOME/.local/bin/clippy-daemon
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
DESK
echo "Installing UI desktop file and icon..."
mkdir -p ~/.local/share/applications
cat << DESK > ~/.local/share/applications/com.example.clippy.ui.desktop
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
DESK

update-desktop-database ~/.local/share/applications/ || true

mkdir -p ~/.local/share/icons/hicolor/512x512/apps/

cp clippy-ui/src/ui/resources/clippy-icon.png ~/.local/share/icons/hicolor/512x512/apps/clippy-icon.png || true

gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor/ || true

echo "Installing GNOME extension..."

EXT_DIR="$HOME/.local/share/gnome-shell/extensions/clippy@example.com"

mkdir -p "$EXT_DIR"

cp extension/extension.js extension/metadata.json "$EXT_DIR/"

echo "Enabling GNOME extension..."

gnome-extensions enable clippy@example.com || true

echo ""
echo "Installation complete!"
echo "Please log out and log back in (or restart GNOME Shell) to apply extension changes."
echo "You can manually start the daemon right now by running: ~/.local/bin/clippy-daemon &"
