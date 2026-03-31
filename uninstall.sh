#!/usr/bin/env bash
set -e
echo "Uninstalling Clippy..."

# Stop running processes
pkill clippy-daemon || true
pkill clippy-ui || true

echo "Removing binaries..."
rm -f ~/.local/bin/clippy-daemon
rm -f ~/.local/bin/clippy-ui

echo "Removing autostart daemon..."
rm -f ~/.config/autostart/clippy-daemon.desktop

echo "Removing desktop file and icons..."
rm -f ~/.local/share/applications/com.example.clippy.ui.desktop
rm -f ~/.local/share/icons/hicolor/512x512/apps/clippy-icon.png

update-desktop-database ~/.local/share/applications/ || true

gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor/ || true

echo "Removing GNOME extension..."

gnome-extensions disable clippy@example.com || true

rm -rf ~/.local/share/gnome-shell/extensions/clippy@example.com
echo "Uninstall complete!"
