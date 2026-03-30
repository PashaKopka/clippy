mod dbus_client;
mod ui;
use dbus_client::DbusClient;
use gtk4::prelude::*;
use gtk4::{gdk, glib, CssProvider};
use libadwaita as adw;
//
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use clippy_db::{ClipboardEntry, EntryKind};
use ui::{ClipboardWindow, EntryAction};
const DEBUG_MODE: bool = false; // TODO remove this stuff
fn register_resources() -> Result<(), glib::Error> {
    gio::resources_register_include! {"clippy.gresource"}
}
fn main() {
    let _ = register_resources();
    let flags = gio::ApplicationFlags::empty();
    let app = adw::Application::builder()
        .application_id("com.example.clippy.ui")
        .flags(flags)
        .build();
    app.connect_activate(|app| {
        println!("ACTIVATE CALLED");
        if let Some(existing) = app.windows().into_iter().next() {
            println!("WINDOW EXISTS");
            existing.present();
            return;
        }
        println!("SETUP");
        setup(app);
    });
    let _ = app.hold();
    app.run();
}
fn setup(app: &adw::Application) {
    let provider = CssProvider::new();
    let display = gdk::Display::default().expect("No display");
    provider.load_from_resource("/com/example/clippy/style.css");
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let dbus = Rc::new(RefCell::new(DbusClient::new().expect("Failed to connect to dbus")));
    let history: Rc<RefCell<Vec<ClipboardEntry>>> = Rc::new(RefCell::new(get_history(&dbus.borrow())));
    let (ui_root, action_tx, action_rx) = ClipboardWindow::build(history.clone());
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Clippy")
        .default_width(480)
        .default_height(560)
        .resizable(false)
        .content(&ui_root)
        .build();
    // Check for updates
    glib::timeout_add_local(std::time::Duration::from_millis(500), {
        let history = history.clone();
        let ui_root = ui_root.clone();
        let action_tx = action_tx.clone();
        let dbus = dbus.clone();
        move || {
            let entries = get_history(&dbus.borrow());
            // Rebuild if different length - simplistic update check
            if entries.len() != history.borrow().len() {
                *history.borrow_mut() = entries;
                ClipboardWindow::rebuild(&ui_root, history.clone(), action_tx.clone());
            }
            glib::ControlFlow::Continue
        }
    });
    if !DEBUG_MODE {
        window.connect_close_request(|win| {
            win.set_visible(false);
            glib::Propagation::Stop
        });
    }
    let win_clone = window.clone();
    let display_clone = display.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        handle_actions(&action_rx, &win_clone, &display_clone, &history, &dbus)
    });
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
fn get_history(dbus: &DbusClient) -> Vec<ClipboardEntry> {
    if let Ok(json_entries) = dbus.get_history() {
        let entries: Vec<ClipboardEntry> = json_entries.into_iter()
            .filter_map(|e| serde_json::from_str(&e).ok())
            .collect();
        entries
    } else {
        vec![]
    }
}
fn handle_actions(
    action_rx: &Receiver<EntryAction>,
    window: &adw::ApplicationWindow,
    display: &gdk::Display,
    history: &Rc<RefCell<Vec<ClipboardEntry>>>,
    dbus: &Rc<RefCell<DbusClient>>
) -> glib::ControlFlow {
    loop {
        match action_rx.try_recv() {
            Ok(action) => on_action(action, window, display, history, dbus),
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
    dbus: &Rc<RefCell<DbusClient>>
) {
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
                let _ = dbus.borrow().set_pinned(id, entry.pinned);
            }
        }
        EntryAction::Delete(id) => {
            history.borrow_mut().retain(|e| e.id != id);
            let _ = dbus.borrow().delete(id);
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
