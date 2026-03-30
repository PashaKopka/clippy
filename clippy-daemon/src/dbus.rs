use clippy_db::{png_dimensions, ClipboardEntry, EntryKind};
use std::sync::{Arc, Mutex};
use zbus::interface;
use url::Url;

pub struct ClippyDaemon {
    pub conn: Arc<Mutex<rusqlite::Connection>>,
}
#[interface(name = "com.example.clippy.Daemon")]
impl ClippyDaemon {
    #[zbus(signal)]
    async fn history_changed(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    async fn new_entry(&self, text: String, #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>) {
        if text.trim().is_empty() {
            return;
        }
        {
            let conn = self.conn.lock().unwrap();
            // Check if entry already exists in database
            if clippy_db::is_text_exists(&conn, &text).expect("Could not read from database") {
                return;
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let entry = ClipboardEntry {
                id: 0,
                kind: if is_link(text.as_str()) {
                    EntryKind::Link(text)
                } else {
                    EntryKind::Text(text)
                },
                timestamp: now,
                pinned: false,
            };
            let _ = clippy_db::insert(&conn, &entry);
        }
        let _ = Self::history_changed(&ctxt).await;
    }

    async fn new_image(
        &self,
        image: Vec<u8>,
        #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
    ) {
        if image.is_empty() {
            return;
        }
        {
            let conn = self.conn.lock().unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let (width, height) = png_dimensions(&image);
            let entry = ClipboardEntry {
                id: 0,
                kind: EntryKind::Image {
                    bytes: image,
                    width,
                    height,
                },
                timestamp: now,
                pinned: false,
            };
            let _ = clippy_db::insert(&conn, &entry);
        }
        let _ = Self::history_changed(&ctxt).await;
    }

    async fn get_history(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        if let Ok(entries) = clippy_db::load_all(&conn) {
            entries
                .into_iter()
                .filter_map(|e| serde_json::to_string(&e).ok())
                .collect()
        } else {
            vec![]
        }
    }

    async fn get_image_bytes(&self, id: i64) -> zbus::fdo::Result<Vec<u8>> {
        let conn = self.conn.lock().unwrap();
        match clippy_db::get_image_bytes(&conn, id) {
            Ok(Some(bytes)) => Ok(bytes),
            _ => Err(zbus::fdo::Error::Failed("Not found".into())),
        }
    }

    async fn delete(&self, id: i64, #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>) {
        {
            let conn = self.conn.lock().unwrap();
            let _ = clippy_db::delete(&conn, id);
        }
        let _ = Self::history_changed(&ctxt).await;
    }

    async fn set_pinned(
        &self,
        id: i64,
        pinned: bool,
        #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
    ) {
        {
            let conn = self.conn.lock().unwrap();
            let _ = clippy_db::set_pinned(&conn, id, pinned);
        }
        let _ = Self::history_changed(&ctxt).await;
    }

    async fn toggle_pin(&self, id: i64, #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>) {
        {
            let conn = self.conn.lock().unwrap();
            let _ = clippy_db::toggle_pin(&conn, id);
        }
        let _ = Self::history_changed(&ctxt).await;
    }
}

fn is_link(text: &str) -> bool {
    Url::parse(text).is_ok()
}
