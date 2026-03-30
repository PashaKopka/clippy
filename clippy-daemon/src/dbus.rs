use std::sync::{Arc, Mutex};
use zbus::interface;
use clippy_db::{ClipboardEntry, EntryKind};
pub struct ClippyDaemon {
    pub conn: Arc<Mutex<rusqlite::Connection>>,
}
#[interface(name = "com.example.clippy.Daemon")]
impl ClippyDaemon {
    async fn new_entry(&self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        // We shouldn't duplicate the text, so lets check last entry
        // No, actually UI logic was checking that, but Daemon should too!
        let conn = self.conn.lock().unwrap();
        // Check last entry
        if let Ok(entries) = clippy_db::load_all(&conn) {
            if let Some(first) = entries.first() {
                if let EntryKind::Text(ref t) = first.kind {
                    if *t == text {
                        return; // duplicate
                    }
                }
            }
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let entry = ClipboardEntry {
            id: 0,
            kind: EntryKind::Text(text),
            timestamp: now,
            pinned: false,
        };
        let _ = clippy_db::insert(&conn, &entry);
    }
    async fn get_history(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        if let Ok(entries) = clippy_db::load_all(&conn) {
            entries.into_iter().filter_map(|e| serde_json::to_string(&e).ok()).collect()
        } else {
            vec![]
        }
    }
    async fn delete(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        let _ = clippy_db::delete(&conn, id);
    }
    async fn set_pinned(&self, id: i64, pinned: bool) {
        let conn = self.conn.lock().unwrap();
        let _ = clippy_db::set_pinned(&conn, id, pinned);
    }

    async fn toggle_pin(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        let _ = clippy_db::toggle_pin(&conn, id);
    }
}
