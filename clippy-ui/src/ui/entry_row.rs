use clippy_db::ClipboardEntry;
use gtk4::prelude::*;
use gtk4::{Align, Box as GBox, Button, Image, Label, ListBoxRow, Orientation, Widget};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum EntryAction {
    Paste(i64),
    TogglePin(i64),
    Delete(i64),
}

pub fn build_entry_row(
    entry: &ClipboardEntry,
    action_tx: Sender<EntryAction>,
    dbus: std::rc::Rc<std::cell::RefCell<crate::dbus_client::DbusClient>>,
) -> ListBoxRow {
    // Outer row
    let row = ListBoxRow::new();

    row.set_activatable(true);
    row.set_selectable(true);
    row.add_css_class("entry-row");
    row.set_widget_name(&entry.id.to_string());

    // Root layout: thumbnail  body  actions
    let root = GBox::new(Orientation::Horizontal, 0);
    root.add_css_class("entry-root");

    // Left: thumbnail / icon
    let thumb = build_thumbnail(entry, dbus);
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

    // Right: action buttons (revealed on hover)
    let actions_box = build_action_buttons(entry, action_tx);
    root.append(&actions_box);
    row.set_child(Some(&root));

    row
}

fn build_thumbnail(
    entry: &ClipboardEntry,
    dbus: std::rc::Rc<std::cell::RefCell<crate::dbus_client::DbusClient>>,
) -> Widget {
    match &entry.kind {
        clippy_db::EntryKind::Image { .. } => {
            let img = Image::new();
            img.set_pixel_size(48); // nice size thumbnail
            let img_weak = img.downgrade();

            let (tx, rx) = async_channel::bounded(1);
            dbus.borrow().request_image_bytes_async(entry.id, tx);

            glib::spawn_future_local(async move {
                if let Ok(bytes) = rx.recv().await {
                    if let Some(img_widget) = img_weak.upgrade() {
                        let loader = gdk_pixbuf::PixbufLoader::new();
                        if loader.write(&bytes).is_ok() && loader.close().is_ok() {
                            if let Some(pixbuf) = loader.pixbuf() {
                                let max_size = 64;
                                let w = pixbuf.width();
                                let h = pixbuf.height();
                                if w > 0 && h > 0 {
                                    let (new_w, new_h) = if w > h {
                                        let ratio = max_size as f64 / w as f64;
                                        (max_size, (h as f64 * ratio) as i32)
                                    } else {
                                        let ratio = max_size as f64 / h as f64;
                                        ((w as f64 * ratio) as i32, max_size)
                                    };
                                    if let Some(scaled) = pixbuf.scale_simple(
                                        new_w,
                                        new_h,
                                        gdk_pixbuf::InterpType::Bilinear,
                                    ) {
                                        let tex = gtk4::gdk::Texture::for_pixbuf(&scaled);
                                        img_widget.set_paintable(Some(&tex));
                                    } else {
                                        let tex = gtk4::gdk::Texture::for_pixbuf(&pixbuf);
                                        img_widget.set_paintable(Some(&tex));
                                    }
                                }
                            }
                        }
                    }
                }
            });
            img.upcast()
        }
        _ => icon_widget(entry.icon_name(), 32),
    }
}

fn icon_widget(name: &str, size: i32) -> Widget {
    let img = Image::from_icon_name(name);
    img.set_pixel_size(size);
    img.upcast()
}

fn build_action_buttons(entry: &ClipboardEntry, action_tx: Sender<EntryAction>) -> Widget {
    let id = entry.id;
    let pinned = entry.pinned;
    let bx = GBox::new(Orientation::Horizontal, 4);
    bx.set_valign(Align::Center);
    bx.add_css_class("entry-actions");

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

    bx.upcast()
}

fn icon_button(icon: &str, tooltip: &str) -> Button {
    let btn = Button::from_icon_name(icon);
    btn.set_tooltip_text(Some(tooltip));
    btn
}

fn format_timestamp(ts: i64) -> String {
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
        _ => format!("{}d ago", delta / 86400),
    }
}
