use gtk4::prelude::*;
use gtk4::{Align, Box as GBox, Button, Image, Label, ListBoxRow, Orientation, Widget};
use std::sync::mpsc::Sender;
// ── Data model ────────────────────────────────────────────────────────────────

/// What kind of content this clipboard entry holds.
#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    Text(String),
    Image {
        /// Raw PNG/JPEG bytes. We store them here and render a thumbnail.
        bytes: Vec<u8>,
        width: i32,
        height: i32,
    },
    FilePath(String),
    Link(String),
}

/// A single item stored in clipboard history.
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub id: i64,
    pub kind: EntryKind,
    pub timestamp: i64, // Unix seconds
    pub pinned: bool,
}

impl ClipboardEntry {
    pub fn preview(&self) -> String {
        match &self.kind {
            EntryKind::Text(t) => {
                let s: String = t.chars().take(120).collect();
                if t.chars().count() > 120 {
                    format!("{}…", s)
                } else {
                    s
                }
            }
            EntryKind::Image { width, height, .. } => {
                format!("Image {}×{}", width, height) // TODO change
            }
            EntryKind::FilePath(p) => p.clone(), // TODO path too large
            EntryKind::Link(u) => u.clone(),     // TODO link too large
        }
    }

    /// Icon name from the Adwaita / hicolor icon theme.
    pub fn icon_name(&self) -> &'static str {
        match &self.kind {
            EntryKind::Text(_) => "text-x-generic-symbolic",
            EntryKind::Image { .. } => "image-x-generic-symbolic",
            EntryKind::FilePath(_) => "folder-symbolic",
            EntryKind::Link(_) => "web-browser-symbolic",
        }
    }

    /// CSS class added to the type badge.
    pub fn badge_css_class(&self) -> &'static str {
        match &self.kind {
            EntryKind::Text(_) => "entry-badge-text",
            EntryKind::Image { .. } => "entry-badge-image",
            EntryKind::FilePath(_) => "entry-badge-file",
            EntryKind::Link(_) => "entry-badge-link",
        }
    }

    pub fn type_label(&self) -> &'static str {
        match &self.kind {
            EntryKind::Text(_) => "Text",
            EntryKind::Image { .. } => "Image",
            EntryKind::FilePath(_) => "File",
            EntryKind::Link(_) => "Link",
        }
    }
}

#[derive(Debug, Clone)]
pub enum EntryAction {
    Paste(i64),
    Copy(i64),
    TogglePin(i64),
    Delete(i64),
}
pub fn build_entry_row(entry: &ClipboardEntry, action_tx: Sender<EntryAction>) -> ListBoxRow {
    // Outer row
    let row = ListBoxRow::new();
    row.set_activatable(true);
    row.set_selectable(true);
    row.add_css_class("entry-row");
    row.set_widget_name(&entry.id.to_string());

    // Root layout: thumbnail | body | actions
    let root = GBox::new(Orientation::Horizontal, 0); // TODO when we click this it should paste
    root.add_css_class("entry-root");

    // Left: thumbnail / icon
    let thumb = build_thumbnail(entry);
    thumb.add_css_class("entry-thumb");
    root.append(&thumb);

    // Center text content
    let body = GBox::new(Orientation::Vertical, 2);
    body.set_hexpand(true);
    body.set_valign(Align::Center);
    body.add_css_class("entry-body");

    // type badge + timestamp row
    let meta_row = GBox::new(Orientation::Horizontal, 6);
    meta_row.add_css_class("entry-meta");

    let badge = Label::new(Some(entry.type_label()));
    badge.add_css_class("entry-badge");
    badge.add_css_class(entry.badge_css_class());
    meta_row.append(&badge);

    let ts = Label::new(Some(&format_timestamp(entry.timestamp)));
    ts.add_css_class("entry-timestamp");
    ts.set_hexpand(true);
    ts.set_halign(Align::Start);
    meta_row.append(&ts);

    if entry.pinned {
        let pin_indicator = Label::new(Some(""));
        pin_indicator.add_css_class("entry-pin-indicator");
        meta_row.append(&pin_indicator);
    }

    body.append(&meta_row);

    // preview label
    let preview = Label::new(Some(&entry.preview()));
    preview.set_halign(Align::Start);
    preview.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    preview.set_lines(2);
    preview.set_wrap(true);
    preview.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    preview.add_css_class("entry-preview");
    body.append(&preview);

    root.append(&body);

    // Activate the row = paste
    // let tx = action_tx.clone();
    // row.connect_activate(move |_| {
    //     println!("Activated");
    //     let _ = tx.send(EntryAction::Paste(1));
    // });

    // Right: action buttons (revealed on hover)
    let actions_box = build_action_buttons(entry, action_tx);
    root.append(&actions_box);

    row.set_child(Some(&root));

    row
}

// ── Thumbnail helper ──────────────────────────────────────────────────────────

fn build_thumbnail(entry: &ClipboardEntry) -> Widget {
    match &entry.kind {
        // EntryKind::Image { bytes, .. } => {  TODO
        //     // PixbufLoader::new() is infallible (returns the loader directly).
        //     let loader = gdk_pixbuf::PixbufLoader::new();
        //     let _ = loader.write(bytes);
        //     let _ = loader.close();
        //     if let Some(pb) = loader.pixbuf() {
        //         if let Some(scaled) = pb.scale_simple(
        //             48,
        //             48,
        //             gdk_pixbuf::InterpType::Bilinear,
        //         ) {
        //             // gtk4::Image::from_pixbuf is deprecated since 4.12.
        //             // Use a Texture instead.
        //             let texture = gtk4::gdk::Texture::for_pixbuf(&scaled);
        //             let img = Image::from_paintable(Some(&texture));
        //             img.add_css_class("entry-thumb-image");
        //             return img.upcast();
        //         }
        //     }
        //     // Fallback
        //     icon_widget("image-x-generic-symbolic", 48)
        // }
        _ => icon_widget(entry.icon_name(), 32),
    }
}

fn icon_widget(name: &str, size: i32) -> Widget {
    let img = Image::from_icon_name(name);
    img.set_pixel_size(size);
    img.upcast()
}

// ── Action buttons ─────────────────────────────────────────────────────────────

fn build_action_buttons(entry: &ClipboardEntry, action_tx: Sender<EntryAction>) -> Widget {
    let id = entry.id;
    let pinned = entry.pinned;

    let bx = GBox::new(Orientation::Horizontal, 4);
    bx.set_valign(Align::Center);
    bx.add_css_class("entry-actions");

    // Copy (re-copy without pasting)
    let btn_copy = icon_button("edit-copy-symbolic", "Copy to clipboard");
    btn_copy.add_css_class("flat");
    {
        let tx = action_tx.clone();
        btn_copy.connect_clicked(move |_| {
            let _ = tx.send(EntryAction::Copy(id));
        });
    }
    bx.append(&btn_copy);

    // Pin / unpin
    let pin_icon = if pinned {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    };
    let btn_pin = icon_button(pin_icon, if pinned { "Unpin" } else { "Pin" });
    btn_pin.add_css_class("flat");
    if pinned {
        btn_pin.add_css_class("entry-btn-pinned");
    }
    {
        let tx = action_tx.clone();
        btn_pin.connect_clicked(move |_| {
            let _ = tx.send(EntryAction::TogglePin(id));
        });
    }
    bx.append(&btn_pin);

    // Delete
    let btn_del = icon_button("edit-delete-symbolic", "Delete");
    btn_del.add_css_class("flat");
    btn_del.add_css_class("entry-btn-delete");
    {
        let tx = action_tx.clone();
        btn_del.connect_clicked(move |_| {
            let _ = tx.send(EntryAction::Delete(id));
        });
    }
    bx.append(&btn_del);

    // TODO remove
    let btn_paste = Button::with_label("Paste");
    btn_paste.add_css_class("suggested-action");
    btn_paste.add_css_class("entry-btn-paste");
    {
        let tx = action_tx.clone();
        btn_paste.connect_clicked(move |_| {
            let _ = tx.send(EntryAction::Paste(id));
        });
    }
    bx.append(&btn_paste);

    bx.upcast()
}

fn icon_button(icon: &str, tooltip: &str) -> Button {
    let btn = Button::from_icon_name(icon);
    btn.set_tooltip_text(Some(tooltip));
    btn
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn format_timestamp(ts: i64) -> String {
    // Very lightweight relative formatter — replace with chrono if you add it.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let delta = now - ts;
    match delta {
        d if d < 60 => "Just now".into(),
        d if d < 3600 => format!("{}m ago", d / 60),
        d if d < 86400 => format!("{}h ago", d / 3600),
        d if d < 604800 => format!("{}d ago", d / 86400),
        _ => {
            // Fallback: just show the raw epoch (replace with a proper date lib)
            format!("{}d ago", delta / 86400)
        }
    }
}
