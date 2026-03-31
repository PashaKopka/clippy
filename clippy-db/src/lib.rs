pub mod models;
use directories::ProjectDirs;
pub use models::*;
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::PathBuf;

// Maximum number of clipboard entries to keep
pub const MAX_ENTRIES_NUMBER: i64 = 30;
pub const PRUNE_OLD_TIME_SECS: i64 = 7 * 24 * 60 * 60; // 7 days
pub const IS_PRUNE_OLD_ACTIVE: bool = true;

// Images larger than this are stored on disk instead of in the DB blob.
const IMAGE_INLINE_LIMIT_BYTES: usize = 1024 * 1024; // 1 MB

pub fn data_dir() -> PathBuf {
    ProjectDirs::from("com", "example", "clippy")
        .expect("Could not determine data directory")
        .data_dir()
        .to_path_buf()
}

pub fn db_path() -> PathBuf {
    data_dir().join("history.db")
}

pub fn images_dir() -> PathBuf {
    data_dir().join("images")
}

/// Open (or create) the database and run all migrations.
/// Call once at startup and keep the returned Connection for the app lifetime.
pub fn open() -> SqlResult<Connection> {
    let dir = data_dir();

    std::fs::create_dir_all(&dir).expect("Could not create data directory");
    std::fs::create_dir_all(images_dir()).expect("Could not create images directory");

    let conn = Connection::open(db_path())?;
    // WAL mode: faster writes, readers don't block writers

    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    migrate(&conn)?;

    Ok(conn)
}

fn migrate(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS clipboard_entries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            kind        TEXT    NOT NULL,   -- 'text'  'link'  'file'  'image'
            content     BLOB    NOT NULL,   -- text/link: UTF-8 string
                                            -- file: newline-separated paths
                                            -- image (small): raw PNG/JPEG bytes
                                            -- image (large): empty, see image_path
            mime_type   TEXT,               -- e.g. 'text/plain', 'image/png'
            image_path  TEXT,               -- set only for large images stored on disk
            created_at  INTEGER NOT NULL,   -- Unix timestamp (seconds)
            pinned      INTEGER NOT NULL DEFAULT 0,
            label       TEXT                -- optional user nickname
        );
        CREATE INDEX IF NOT EXISTS idx_created_at ON clipboard_entries(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pinned     ON clipboard_entries(pinned);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ",
    )
}

pub fn get_setting(conn: &Connection, key: &str, default: &str) -> String {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .unwrap_or_else(|_| default.to_string())
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> SqlResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Insert a new entry. Returns the newly assigned id.
pub fn insert(conn: &Connection, entry: &ClipboardEntry) -> SqlResult<i64> {
    let (kind, content, mime_type, image_path) = entry_to_row(entry);
    conn.execute(
        "INSERT INTO clipboard_entries (kind, content, mime_type, image_path, created_at, pinned, label)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            kind,
            content,
            mime_type,
            image_path,
            entry.timestamp,
            entry.pinned as i64,
            Option::<String>::None, // label — user sets this later
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> SqlResult<()> {
    conn.execute(
        "UPDATE clipboard_entries SET pinned = ?1 WHERE id = ?2",
        params![pinned as i64, id],
    )?;
    Ok(())
}

pub fn toggle_pin(conn: &Connection, id: i64) -> SqlResult<()> {
    let new_pinned_value: Option<i64> = conn
        .query_row(
            "SELECT pinned FROM clipboard_entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(pinned_value) = new_pinned_value {
        let is_pinned: bool = pinned_value != 0;

        conn.execute(
            "UPDATE clipboard_entries SET pinned = ?1 WHERE id = ?2",
            params![!is_pinned, id],
        )?;
    }

    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> SqlResult<()> {
    // Check if it has an on-disk image first
    let image_path: Option<String> = conn
        .query_row(
            "SELECT image_path FROM clipboard_entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    if let Some(path) = image_path {
        let _ = std::fs::remove_file(&path);
    }
    conn.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn is_text_exists(conn: &Connection, text: &str) -> SqlResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM clipboard_entries WHERE (kind = 'text' OR kind = 'link') AND CAST(content AS TEXT) = ?1",
        params![text],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn run_cleanup(conn: &Connection) -> SqlResult<()> {
    let max_entries: i64 = get_setting(conn, "max_entries", &MAX_ENTRIES_NUMBER.to_string())
        .parse()
        .unwrap_or(MAX_ENTRIES_NUMBER);

    let is_prune_active: bool = get_setting(conn, "is_prune_active", if IS_PRUNE_OLD_ACTIVE { "true" } else { "false" })
        .parse()
        .unwrap_or(IS_PRUNE_OLD_ACTIVE);

    let prune_time_secs: i64 = get_setting(conn, "prune_time_secs", &PRUNE_OLD_TIME_SECS.to_string())
        .parse()
        .unwrap_or(PRUNE_OLD_TIME_SECS);

    // Keep only the most recent max_entries unpinned items
    let mut stmt = conn.prepare(&format!(
        "SELECT id, image_path FROM clipboard_entries
         WHERE pinned = 0
         ORDER BY created_at DESC
         LIMIT -1 OFFSET {}",
         max_entries
    ))?;

    let to_delete: Vec<(i64, Option<String>)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?
    .filter_map(Result::ok)
    .collect();

    for (id, path) in to_delete {
        if let Some(p) = path {
            let _ = std::fs::remove_file(p);
        }
        conn.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?;
    }

    if is_prune_active {
        // Then delete items older than prune_time_secs
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let older_than = now - prune_time_secs;

        prune_old(conn, older_than)?;
    }

    Ok(())
}

/// Remove all non-pinned entries older than `older_than_secs` Unix timestamp.
pub fn prune_old(conn: &Connection, older_than_secs: i64) -> SqlResult<usize> {
    // Delete files first
    let mut stmt = conn.prepare("SELECT image_path FROM clipboard_entries WHERE pinned = 0 AND created_at < ?1 AND image_path IS NOT NULL")?;
    let paths: Vec<String> = stmt.query_map(params![older_than_secs], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();
    for p in paths {
        let _ = std::fs::remove_file(p);
    }

    let deleted = conn.execute(
        "DELETE FROM clipboard_entries WHERE pinned = 0 AND created_at < ?1",
        params![older_than_secs],
    )?;
    Ok(deleted)
}

pub fn get_image_bytes(conn: &Connection, id: i64) -> SqlResult<Option<Vec<u8>>> {
    let mut stmt = conn.prepare(
        "SELECT content, image_path FROM clipboard_entries WHERE id = ?1 AND kind = 'image'",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        let content: Vec<u8> = row.get(0)?;
        let image_path: Option<String> = row.get(1)?;
        let bytes = if let Some(path) = image_path {
            std::fs::read(&path).unwrap_or_else(|e| {
                eprintln!("[db] failed to read image {path}: {e}");
                vec![]
            })
        } else {
            content
        };
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

pub fn load_all(conn: &Connection) -> SqlResult<Vec<ClipboardEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, content, mime_type, image_path, created_at, pinned
         FROM clipboard_entries
         ORDER BY created_at DESC",
    )?;
    let entries = stmt
        .query_map([], row_to_entry)?
        .filter_map(|r| r.map_err(|e| eprintln!("[db] row error: {e}")).ok())
        .flatten()
        .collect();
    Ok(entries)
}
/// Load only pinned entries, newest first.
pub fn load_pinned(conn: &Connection) -> SqlResult<Vec<ClipboardEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, content, mime_type, image_path, created_at, pinned
         FROM clipboard_entries
         WHERE pinned = 1
         ORDER BY created_at DESC",
    )?;
    let entries = stmt
        .query_map([], row_to_entry)?
        .filter_map(|r| r.map_err(|e| eprintln!("[db] row error: {e}")).ok())
        .flatten()
        .collect();
    Ok(entries)
}

pub fn search(conn: &Connection, query: &str) -> SqlResult<Vec<ClipboardEntry>> {
    let pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT id, kind, content, mime_type, image_path, created_at, pinned
         FROM clipboard_entries
         WHERE kind != 'image' AND content LIKE ?1
         ORDER BY created_at DESC
         LIMIT 100",
    )?;
    let entries = stmt
        .query_map(params![pattern], row_to_entry)?
        .filter_map(|r| r.map_err(|e| eprintln!("[db] row error: {e}")).ok())
        .flatten()
        .collect();
    Ok(entries)
}

/// Convert a `ClipboardEntry` to the 4 columns we write: (kind, content, mime_type, image_path).
/// For large images, saves the file to disk and returns the path.
fn entry_to_row(entry: &ClipboardEntry) -> (String, Vec<u8>, Option<String>, Option<String>) {
    match &entry.kind {
        EntryKind::Text(t) => (
            "text".into(),
            t.as_bytes().to_vec(),
            Some("text/plain".into()),
            None,
        ),
        EntryKind::Link(u) => (
            "link".into(),
            u.as_bytes().to_vec(),
            Some("text/uri-list".into()),
            None,
        ),
        EntryKind::FilePath(paths) => (
            // paths is a newline-separated list of file paths
            "file".into(),
            paths.as_bytes().to_vec(),
            Some("text/uri-list".into()),
            None,
        ),
        EntryKind::Image { bytes, .. } => {
            if bytes.len() <= IMAGE_INLINE_LIMIT_BYTES {
                // Small enough — store inline in the BLOB
                (
                    "image".into(),
                    bytes.clone(),
                    Some("image/png".into()),
                    None,
                )
            } else {
                // Large — save to disk, store empty blob + path
                let path = images_dir().join(format!("{}.png", entry.id));
                if let Err(e) = std::fs::write(&path, bytes) {
                    eprintln!("[db] failed to write image file: {e}");
                }
                (
                    "image".into(),
                    vec![],
                    Some("image/png".into()),
                    Some(path.to_string_lossy().into_owned()),
                )
            }
        }
    }
}

/// Convert a DB row back to a `ClipboardEntry`. Returns None on unrecognised kind.
fn row_to_entry(row: &rusqlite::Row<'_>) -> SqlResult<Option<ClipboardEntry>> {
    let id: i64 = row.get(0)?;
    let kind: String = row.get(1)?;
    let content: Vec<u8> = row.get(2)?;
    let _mime: Option<String> = row.get(3)?;
    let image_path: Option<String> = row.get(4)?;
    let created_at: i64 = row.get(5)?;
    let pinned: i64 = row.get(6)?;
    let entry_kind = match kind.as_str() {
        "text" => {
            let t = String::from_utf8_lossy(&content).into_owned();
            EntryKind::Text(t)
        }
        "link" => {
            let u = String::from_utf8_lossy(&content).into_owned();
            EntryKind::Link(u)
        }
        "file" => {
            let paths = String::from_utf8_lossy(&content).into_owned();
            EntryKind::FilePath(paths)
        }
        "image" => {
            let bytes = if let Some(path) = image_path {
                // Large image stored on disk
                std::fs::read(&path).unwrap_or_else(|e| {
                    eprintln!("[db] failed to read image {path}: {e}");
                    vec![]
                })
            } else {
                content
            };
            // Decode dimensions from the PNG header (first 24 bytes)
            let (width, height) = png_dimensions(&bytes);
            EntryKind::Image {
                bytes,
                width,
                height,
            }
        }
        other => {
            eprintln!("[db] unknown kind '{other}', skipping");
            return Ok(None);
        }
    };
    Ok(Some(ClipboardEntry {
        id,
        kind: entry_kind,
        timestamp: created_at,
        pinned: pinned != 0,
    }))
}

/// Read width/height from a PNG header without decoding the whole image.
/// Returns (0, 0) if the bytes are not a valid PNG.
pub fn png_dimensions(bytes: &[u8]) -> (i32, i32) {
    // PNG header: 8 bytes signature + 4 bytes length + 4 bytes "IHDR"
    //             + 4 bytes width + 4 bytes height
    if bytes.len() < 24 {
        return (0, 0);
    }
    let w = i32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = i32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w, h)
}
