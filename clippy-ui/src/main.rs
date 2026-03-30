mod dbus_client;
mod ui;

use clippy_db::{ClipboardEntry, EntryKind};
use dbus_client::DbusClient;
use gdk_pixbuf::prelude::PixbufLoaderExt;
use gtk4::prelude::*;
use gtk4::{gdk, glib, CssProvider};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
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

    let dbus = Rc::new(RefCell::new(
        DbusClient::new().expect("Failed to connect to dbus"),
    ));

    let history: Rc<RefCell<Vec<ClipboardEntry>>> =
        Rc::new(RefCell::new(get_history(&dbus.borrow())));

    let (ui_root, action_tx, action_rx) = ClipboardWindow::build(history.clone(), dbus.clone());
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Clippy")
        .default_width(480)
        .default_height(560)
        .resizable(false)
        .content(&ui_root)
        .build();

    let (update_tx, update_rx) = async_channel::unbounded::<()>();

    let history_clone = history.clone();
    let dbus_clone = dbus.clone();
    let ui_root_clone = ui_root.clone();
    let action_tx_clone = action_tx.clone();

    glib::spawn_future_local(async move {
        while let Ok(_) = update_rx.recv().await {
            let entries = get_history(&dbus_clone.borrow());
            let need_rebuild = {
                let mut h = history_clone.borrow_mut();
                if *h != entries {
                    *h = entries;
                    true
                } else {
                    false
                }
            };

            if need_rebuild {
                ClipboardWindow::rebuild(
                    &ui_root_clone,
                    history_clone.clone(),
                    action_tx_clone.clone(),
                    dbus_clone.clone(),
                );
            }
        }
    });

    let _ = dbus.borrow().spawn_history_changed_listener(move || {
        let _ = update_tx.send_blocking(());
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
        let entries: Vec<ClipboardEntry> = json_entries
            .into_iter()
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
    dbus: &Rc<RefCell<DbusClient>>,
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
    dbus: &Rc<RefCell<DbusClient>>,
) {
    match action {
        EntryAction::Paste(id) => {
            set_clipboard(id, display, history, dbus);
            window.set_visible(false);
        }
        EntryAction::TogglePin(id) => {
            let _ = dbus.borrow().toggle_pin(id);
        }
        EntryAction::Delete(id) => {
            let _ = dbus.borrow().delete(id);
        }
        EntryAction::OpenInBrowser(id) => {
            if let Some(entry) = get_entry(id, history) {
                if let EntryKind::Link(url) = entry.kind {
                    let _ = gio::AppInfo::launch_default_for_uri(&url, gio::AppLaunchContext::NONE);
                    window.set_visible(false);
                }
            }
        }
        EntryAction::OpenInFiles(id) => {
            if let Some(entry) = get_entry(id, history) {
                if let EntryKind::FilePath(paths) = entry.kind {
                    let uris: Vec<String> = paths.lines()
                        .map(|l| l.trim_end_matches('\r').to_string())
                        .filter(|l| !l.is_empty())
                        .collect();

                    if !uris.is_empty() {
                        // Use std::process::Command to invoke dbus-send for FileManager1
                        let mut args = vec![
                            "--session".to_string(),
                            "--dest=org.freedesktop.FileManager1".to_string(),
                            "--type=method_call".to_string(),
                            "/org/freedesktop/FileManager1".to_string(),
                            "org.freedesktop.FileManager1.ShowItems".to_string(),
                        ];

                        // Build array of strings for dbus, using only the first file
                        let array_arg = format!(
                            "array:string:{}",
                            uris[0]
                        );
                        args.push(array_arg);
                        args.push("string:".to_string()); // StartupId

                        let _ = std::process::Command::new("dbus-send")
                            .args(args)
                            .spawn();
                    }
                    window.set_visible(false);
                }
            }
        }
    }
}

fn set_clipboard(
    id: i64,
    display: &gdk::Display,
    history: &Rc<RefCell<Vec<ClipboardEntry>>>,
    dbus: &Rc<RefCell<DbusClient>>,
) {
    if let Some(entry) = get_entry(id, history) {
        match entry.kind {
            EntryKind::Text(t) => display.clipboard().set_text(&t),
            EntryKind::Link(t) => display.clipboard().set_text(&t),
            EntryKind::FilePath(t) => {
                let clipboard = display.clipboard();

                let provider = gdk::ContentProvider::new_union(&[
                    gdk::ContentProvider::for_bytes(
                        "text/uri-list",
                        &glib::Bytes::from(t.as_bytes()),
                    ),
                    gdk::ContentProvider::for_bytes(
                        "x-special/gnome-copied-files",
                        &glib::Bytes::from(format!("copy\n{}", t).as_bytes()),
                    ),
                    gdk::ContentProvider::for_bytes(
                        "text/plain",
                        &glib::Bytes::from(t.as_bytes()),
                    ),
                ]);

                clipboard.set_content(Some(&provider)).unwrap();
            },
            EntryKind::Image { .. } => {
                let bytes = dbus.borrow().get_image_bytes(id).unwrap_or_default();
                if bytes.is_empty() {
                    return;
                }

                let loader = gdk_pixbuf::PixbufLoader::new();

                loader.write(&bytes).unwrap();
                loader.close().unwrap();

                let pixbuf = loader.pixbuf().unwrap();
                let texture = gdk::Texture::for_pixbuf(&pixbuf);

                let provider = gdk::ContentProvider::for_value(&texture.to_value());
                display
                    .clipboard()
                    .set_content(Some(&provider))
                    .expect("Failed to set image to clipboard");
            }
        }
    }
}

fn get_entry(id: i64, history: &Rc<RefCell<Vec<ClipboardEntry>>>) -> Option<ClipboardEntry> {
    history.borrow().iter().find(|e| e.id == id).cloned()
}
