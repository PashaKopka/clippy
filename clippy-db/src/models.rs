use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntryKind {
    Text(String),
    Image {
        #[serde(skip, default)]
        bytes: Vec<u8>,
        width: i32,
        height: i32,
    },
    FilePath(String),
    Link(String),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardEntry {
    pub id: i64,
    pub kind: EntryKind,
    pub timestamp: i64,
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
                format!("Image {}x{}", width, height)
            }
            EntryKind::FilePath(p) => p.clone(),
            EntryKind::Link(u) => u.clone(),
        }
    }
    pub fn icon_name(&self) -> &'static str {
        match &self.kind {
            EntryKind::Text(_) => "text-x-generic-symbolic",
            EntryKind::Image { .. } => "image-x-generic-symbolic",
            EntryKind::FilePath(_) => "folder-symbolic",
            EntryKind::Link(_) => "web-browser-symbolic",
        }
    }
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
