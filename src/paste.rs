use gtk4::gdk;
use gtk4::prelude::*;

#[derive(Debug)]
pub enum PasteError {
    NoDisplay,
    AutoPasteFailed,
}

/// Step 1 + optional Step 2.
/// Call AFTER hiding/closing the Clippy window so focus has returned.
pub fn paste_text(text: &str) {
    // ── Step 1: Write to clipboard via GDK ───────────────────────────────
    //
    // This uses GNOME's own wl_data_device_manager — no wlroots protocols
    // needed. GDK handles the Wayland clipboard ownership model internally,
    // including keeping the data alive after our window closes.
    //
    // IMPORTANT: GDK clipboard ownership is tied to the GdkDisplay connection.
    // As long as the process is alive, the clipboard data is served correctly.
    let display = gdk::Display::default().unwrap();
    let clipboard = display.clipboard();

    // set_text() on GdkClipboard works on GNOME Wayland, X11, and XWayland.
    clipboard.set_text(text);
}
