use gtk4::prelude::*;
use gtk4::{Align, Box as GBox, Button, Entry, EntryBuffer, Image, Orientation};

pub fn build_search_bar() -> (GBox, Entry) {
    let bar = GBox::new(Orientation::Horizontal, 8);
    bar.add_css_class("search-bar");

    // Search icon on the left
    let icon = Image::from_icon_name("system-search-symbolic");
    icon.set_pixel_size(16);
    icon.add_css_class("search-icon");
    bar.append(&icon);

    // Text field
    let buffer = EntryBuffer::default();
    let entry = Entry::with_buffer(&buffer);
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Search clipboard history…"));
    entry.add_css_class("search-entry");

    // Clear button (hidden while empty, shown when there is text)
    let clear_btn = Button::from_icon_name("edit-clear-symbolic");
    clear_btn.add_css_class("flat");
    clear_btn.add_css_class("search-clear");
    clear_btn.set_valign(Align::Center);
    clear_btn.set_tooltip_text(Some("Clear search"));
    clear_btn.set_opacity(if buffer.text().is_empty() { 0.0 } else { 1.0 });
    clear_btn.set_sensitive(!buffer.text().is_empty());

    // Show/hide clear button based on entry content
    entry.connect_changed({
        let clear_btn = clear_btn.clone();
        move |e| {
            let is_empty = e.text().is_empty();
            clear_btn.set_opacity(if is_empty { 0.0 } else { 1.0 });
            clear_btn.set_sensitive(!is_empty);
        }
    });

    // Clear button resets entry
    clear_btn.connect_clicked({
        let entry = entry.clone();
        move |_| {
            entry.set_text("");
            entry.grab_focus();
        }
    });

    bar.append(&entry);
    bar.append(&clear_btn);

    (bar, entry)
}
