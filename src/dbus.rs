use std::sync::mpsc::Sender;
use zbus::{connection, interface};

/// The D-Bus object that the GNOME extension calls into.
struct ClippyService {
    tx: Sender<String>,
}

#[interface(name = "com.example.clippy")]
impl ClippyService {
    /// Called by the GNOME extension whenever clipboard content changes.
    fn new_entry(&self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        eprintln!("[dbus] received new entry ({} chars)", text.len());
        let _ = self.tx.send(text);
    }
}

/// Spawns the D-Bus service on a background tokio thread.
/// Returns a channel receiver — poll it on the GTK main thread
/// with glib::timeout_add_local, exactly like your action_rx.
pub fn spawn_dbus_service() -> std::sync::mpsc::Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let service = ClippyService { tx };

            let _conn = connection::Builder::session()
                .expect("No session D-Bus")
                .name("com.example.clippy")
                .expect("Failed to claim D-Bus name — is another instance running?")
                .serve_at("/com/example/clippy", service)
                .expect("Failed to serve D-Bus object")
                .build()
                .await
                .expect("Failed to build D-Bus connection");

            eprintln!("[dbus] service running as com.example.clippy");

            // Keep the connection alive forever (until the process exits)
            std::future::pending::<()>().await;
        });
    });

    rx
}