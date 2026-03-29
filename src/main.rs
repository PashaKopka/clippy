mod db;
mod dbus;
mod paste;
mod ui;

use gtk4::prelude::*;
use gtk4::{gdk, glib, CssProvider};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use ui::{ClipboardEntry, ClipboardWindow, EntryAction, EntryKind};

const DEBUG_MODE: bool = false; // TODO remove this stuff

fn register_resources() -> Result<(), glib::Error> {
    gio::resources_register_include! {"clippy.gresource"}
}

fn main() {
    let _ = register_resources();

    // Build app
    let flags = if DEBUG_MODE {
        // Allow multiple instances so you can just `cargo run` repeatedly
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let app = adw::Application::builder()
        .application_id("com.example.clippy")
        // NON_UNIQUE would allow multiple instances — we want the opposite.
        // IS_LAUNCHER makes the primary instance handle all activate() calls.
        .flags(flags)
        .build();

    app.connect_activate(|app| {
        // If a window already exists, just show it and return
        // Application::windows() returns all windows registered with this app.
        if !DEBUG_MODE {
            if let Some(existing) = app.windows().into_iter().next() {
                existing.present();
                return;
            }
        }

        setup(app);
    });

    // hold() prevents the app from exiting when the window is hidden.
    // Without this, GApplication exits when it has no visible windows.
    // let _ = app.hold();
    if !DEBUG_MODE {
        let _ = app.hold();
    }

    app.run();
}

fn setup(app: &adw::Application) {
    let provider = CssProvider::new();
    let display = gdk::Display::default().expect("No display");

    // Apply style
    provider.load_from_resource("/com/example/clippy/style.css");
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let history: Rc<RefCell<Vec<ClipboardEntry>>> = get_history();

    let (ui_root, action_tx, action_rx) = ClipboardWindow::build(history.clone());

    // Build window
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Clippy")
        .default_width(480)
        .default_height(560)
        .resizable(false)
        .content(&ui_root)
        .build();

    // Clipboard watcher
    let clip_rx = dbus::spawn_dbus_service();
    glib::timeout_add_local(std::time::Duration::from_millis(100), {
        let history = history.clone();
        let ui_root = ui_root.clone();
        let action_tx = action_tx.clone();
        move || {
            while let Ok(text) = clip_rx.try_recv() {
                on_new_text(text, history.clone(), ui_root.clone(), action_tx.clone());
            }
            glib::ControlFlow::Continue
        }
    });

    if !DEBUG_MODE {
        // Hide instead of close when user dismisses the window
        // This keeps the process (and GDK clipboard ownership) alive.
        // The next hotkey press calls activate() again, which hits the
        // `existing.present()` branch above.
        window.connect_close_request(|win| {
            win.set_visible(false);
            // Return true = we handled it, don't actually destroy the window.
            glib::Propagation::Stop
        });
    }

    // Action handler
    let win_clone = window.clone();
    let display_clone = display.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        handle_actions(&action_rx, &win_clone, &display_clone, &history)
    });

    // Escape hides (not closes) the window
    let key_ctrl = gtk4::EventControllerKey::new();
    let win_clone2 = window.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            win_clone2.set_visible(false);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_ctrl);

    window.present();
}

fn get_history() -> Rc<RefCell<Vec<ClipboardEntry>>> {
    let conn = db::open().unwrap();
    let entries = db::load_all(&conn).unwrap();
    Rc::new(RefCell::new(entries))
}

fn on_new_text(
    text: String,
    history: Rc<RefCell<Vec<ClipboardEntry>>>,
    ui_root: gtk4::Widget,
    action_tx: Sender<EntryAction>,
) {
    if text.trim().is_empty() {
        eprintln!("[clipboard] skipping empty/whitespace text");
        return;
    }

    let mut h = history.borrow_mut();

    if let Some(last) = h.first() {
        if let EntryKind::Text(ref t) = last.kind {
            if *t == text {
                eprintln!("[clipboard] duplicate, skipping");
                return;
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut entry = ClipboardEntry {
        id: 0,
        kind: EntryKind::Text(text),
        timestamp: now,
        pinned: false,
    };

    let conn = db::open().unwrap();
    match db::insert(&conn, &entry) {
        Ok(real_id) => entry.id = real_id,
        Err(e) => eprintln!("[db] insert failed: {e}"),
    }

    h.insert(0, entry);

    const MAX_HISTORY: usize = 200;
    if h.len() > MAX_HISTORY {
        h.truncate(MAX_HISTORY);
    }

    eprintln!("[clipboard] stored, history now has {} entries", h.len());

    drop(h);

    ClipboardWindow::rebuild(&ui_root, history, action_tx);
}

fn handle_actions(
    action_rx: &Receiver<EntryAction>,
    window: &adw::ApplicationWindow,
    display: &gdk::Display,
    history: &Rc<RefCell<Vec<ClipboardEntry>>>,
) -> glib::ControlFlow {
    loop {
        match action_rx.try_recv() {
            Ok(action) => on_action(action, window, display, history),
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return glib::ControlFlow::Break;
            }
        }
    }
    glib::ControlFlow::Continue
}

fn on_action(
    action: EntryAction,
    window: &adw::ApplicationWindow,
    display: &gdk::Display,
    history: &Rc<RefCell<Vec<ClipboardEntry>>>,
) {
    let conn = db::open().unwrap();
    match action {
        EntryAction::Paste(id) => {
            let text = get_entry_text(id, history);
            display.clipboard().set_text(&text);
            window.set_visible(false);
        }
        EntryAction::TogglePin(id) => {
            let mut h = history.borrow_mut();
            if let Some(entry) = h.iter_mut().find(|e| e.id == id) {
                entry.pinned = !entry.pinned;
                let _ = db::set_pinned(&conn, id, entry.pinned);
            }
        }
        EntryAction::Delete(id) => {
            history.borrow_mut().retain(|e| e.id != id);
            let _ = db::delete(&conn, id);
        }
        EntryAction::Copy(id) => {
            let text = get_entry_text(id, history);
            display.clipboard().set_text(&text);
        }
    }
}

fn get_entry_text(id: i64, history: &Rc<RefCell<Vec<ClipboardEntry>>>) -> String {
    history
        .borrow()
        .iter()
        .find(|e| e.id == id)
        .map(|e| match &e.kind {
            EntryKind::Text(t) => t.clone(),
            EntryKind::Link(t) => t.clone(),
            EntryKind::FilePath(t) => t.clone(),
            EntryKind::Image { .. } => String::new(),
        })
        .unwrap_or_default()
}

// TODO window should appear under cursor position. Cursor should be at the center of window
// TODO should not appear another window if user hit hotkey one more time
// ~/.local/share/clippy/history.db
// CREATE TABLE IF NOT EXISTS clipboard_entries (
//     id        INTEGER PRIMARY KEY AUTOINCREMENT,
//     kind      TEXT NOT NULL,          -- 'text', 'image', 'file'
//     content   BLOB NOT NULL,          -- raw text, PNG bytes, or file path
//     mime_type TEXT,
//     created_at INTEGER NOT NULL,      -- Unix timestamp
//     pinned    INTEGER DEFAULT 0,
//     label     TEXT                    -- user-set nickname
// );
// ~/.local/share/clippy/images/
