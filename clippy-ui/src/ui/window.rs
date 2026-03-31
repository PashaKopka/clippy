use crate::ui::entry_row::build_entry_row;
use crate::ui::search_bar::build_search_bar;
use crate::ui::EntryAction;
use adw::ViewStack;
use clippy_db::models::ClipboardEntry;
use gtk4::prelude::*;
use gtk4::{
    Box as GBox, Entry, ListBox, Orientation, PolicyType, ScrolledWindow, SelectionMode, Widget,
};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
pub struct ClipboardWindow;

impl ClipboardWindow {
    fn find_listbox(w: &Widget) -> Option<ListBox> {
        if let Some(lb) = w.downcast_ref::<ListBox>() {
            return Some(lb.clone());
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            if let Some(lb) = Self::find_listbox(&c) {
                return Some(lb);
            }
            child = c.next_sibling();
        }
        None
    }

    pub fn build(
        history: Rc<RefCell<Vec<ClipboardEntry>>>,
        dbus: Rc<RefCell<crate::dbus_client::DbusClient>>,
    ) -> (Widget, Sender<EntryAction>, Receiver<EntryAction>) {
        let (action_tx, action_rx) = channel();

        // The root widget is a ToolbarView
        let toolbar_view = adw::ToolbarView::new();
        let header_bar = adw::HeaderBar::new();
        header_bar.set_show_end_title_buttons(false);

        // A ViewSwitcher for tabs
        let switcher = adw::ViewSwitcher::new();

        switcher.set_policy(adw::ViewSwitcherPolicy::Narrow);
        header_bar.set_title_widget(Some(&switcher));
        toolbar_view.add_top_bar(&header_bar);

        let stack = ViewStack::new();

        // Default tab: all
        let page_all =
            Self::build_list_page(&history.borrow(), false, action_tx.clone(), dbus.clone());
        let p_all = stack.add_titled_with_icon(&page_all, Some("all"), "All", "view-list-symbolic");
        p_all.set_icon_name(Some("view-list-symbolic"));

        // Text tab
        let text_history: Vec<ClipboardEntry> = history
            .borrow()
            .iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::Text { .. }))
            .cloned()
            .collect();
        let page_text =
            Self::build_list_page(&text_history, false, action_tx.clone(), dbus.clone());
        let p_text =
            stack.add_titled_with_icon(&page_text, Some("text"), "Text", "view-list-symbolic");
        p_text.set_icon_name(Some("text-x-generic"));

        // Images tab
        let images_history: Vec<ClipboardEntry> = history
            .borrow()
            .iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::Image { .. }))
            .cloned()
            .collect();
        let page_images =
            Self::build_list_page(&images_history, false, action_tx.clone(), dbus.clone());
        let p_images = stack.add_titled_with_icon(
            &page_images,
            Some("images"),
            "Images",
            "view-list-symbolic",
        );
        p_images.set_icon_name(Some("image-x-generic-symbolic"));

        // Links tab
        let links_history: Vec<ClipboardEntry> = history
            .borrow()
            .iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::Link { .. }))
            .cloned()
            .collect();
        let page_links =
            Self::build_list_page(&links_history, false, action_tx.clone(), dbus.clone());
        let p_links =
            stack.add_titled_with_icon(&page_links, Some("links"), "Links", "view-list-symbolic");
        p_links.set_icon_name(Some("network-wireless-hotspot-symbolic"));

        // Files tab
        let files_history: Vec<ClipboardEntry> = history
            .borrow()
            .iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::FilePath { .. }))
            .cloned()
            .collect();
        let page_files =
            Self::build_list_page(&files_history, false, action_tx.clone(), dbus.clone());
        let p_files =
            stack.add_titled_with_icon(&page_files, Some("files"), "Files", "folder-symbolic");
        p_files.set_icon_name(Some("network-wireless-hotspot-symbolic"));

        // Pinned tab
        let page_pinned =
            Self::build_list_page(&history.borrow(), true, action_tx.clone(), dbus.clone());
        let p_pinned =
            stack.add_titled_with_icon(&page_pinned, Some("pinned"), "Pinned", "starred-symbolic");
        p_pinned.set_icon_name(Some("starred-symbolic"));

        switcher.set_stack(Some(&stack));

        let content = GBox::new(Orientation::Vertical, 0);
        let (search_widget, search_entry) = build_search_bar();
        content.append(&search_widget);

        let stack_for_search = stack.clone();
        search_entry.connect_changed(move |_entry| {
            for name in ["all", "text", "images", "links", "files", "pinned"] {
                if let Some(page_widget) = stack_for_search.child_by_name(name) {
                    if let Some(lb) = Self::find_listbox(&page_widget) {
                        lb.invalidate_filter();
                    }
                }
            }
        });

        Self::wire_search(&stack, search_entry);

        content.append(&stack);

        toolbar_view.set_content(Some(&content));

        (toolbar_view.upcast(), action_tx, action_rx)
    }

    pub fn rebuild(
        toolbar_view: &Widget,
        history: Rc<RefCell<Vec<ClipboardEntry>>>,
        action_tx: Sender<EntryAction>,
        dbus: Rc<RefCell<crate::dbus_client::DbusClient>>,
    ) {
        let Some(tv) = toolbar_view.downcast_ref::<adw::ToolbarView>() else {
            return;
        };

        let Some(content) = tv.content().and_then(|w| w.downcast::<GBox>().ok()) else {
            return;
        };

        let mut child = content.first_child();
        child = child.and_then(|w| w.next_sibling());

        let Some(stack) = child.and_then(|w| w.downcast::<adw::ViewStack>().ok()) else {
            return;
        };

        let entries = history.borrow();

        let text_entries: Vec<_> = entries.iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::Text { .. }))
            .cloned()
            .collect();

        let image_entries: Vec<_> = entries.iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::Image { .. }))
            .cloned()
            .collect();

        let link_entries: Vec<_> = entries.iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::Link { .. }))
            .cloned()
            .collect();

        let files_entries: Vec<_> = entries.iter()
            .filter(|e| matches!(e.kind, clippy_db::EntryKind::FilePath { .. }))
            .cloned()
            .collect();

        let tabs = vec![
            ("all", entries.clone(), false),
            ("text", text_entries, false),
            ("images", image_entries, false),
            ("links", link_entries, false),
            ("files", files_entries, false),
            ("pinned", entries.clone(), true),
        ];

        for (name, tab_entries, pinned_only) in tabs {
            let Some(page_widget) = stack.child_by_name(name) else {
                continue;
            };

            let Some(list) = Self::find_listbox(&page_widget) else {
                continue;
            };

            while let Some(row) = list.first_child() {
                list.remove(&row);
            }

            for entry in tab_entries.iter().filter(|e| !pinned_only || e.pinned) {
                let row = build_entry_row(entry, action_tx.clone(), dbus.clone());

                row.set_widget_name(&format!(
                    "{}|{}",
                    entry.id,
                    match &entry.kind {
                        clippy_db::EntryKind::Text(t)
                        | clippy_db::EntryKind::Link(t)
                        | clippy_db::EntryKind::FilePath(t) => t,
                        _ => "",
                    }
                ));

                row.set_tooltip_text(Some(&entry.preview()));
                list.append(&row);
            }

            list.invalidate_filter();
        }
    }

    fn wire_search(stack: &ViewStack, search_entry: Entry) {
        let stack_clone = stack.clone();

        for name in ["all", "text", "images", "links", "files", "pinned"] {
            if let Some(page_widget) = stack_clone.child_by_name(name) {
                if let Some(lb) = Self::find_listbox(&page_widget) {
                    let search_entry_clone = search_entry.clone();
                    lb.set_filter_func(move |row| {
                        let q = search_entry_clone.text().to_lowercase();
                        if q.is_empty() { return true; }

                        let text = row
                            .widget_name()
                            .split_once('|')
                            .map(|(_, text)| text.to_lowercase())
                            .unwrap_or_default();

                        !text.is_empty() && text.contains(&q)
                    });
                }
            }
        }
    }

    fn build_list_page(
        entries: &[ClipboardEntry],
        pinned_only: bool,
        action_tx: Sender<EntryAction>,
        dbus: Rc<RefCell<crate::dbus_client::DbusClient>>,
    ) -> ScrolledWindow {
        let sw = ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .build();

        sw.add_css_class("list-scroll");

        sw.set_child(Some(&Self::build_list_content(
            entries,
            pinned_only,
            action_tx,
            dbus,
        )));

        sw
    }

    fn build_list_content(
        entries: &[ClipboardEntry],
        pinned_only: bool,
        action_tx: Sender<EntryAction>,
        dbus: Rc<RefCell<crate::dbus_client::DbusClient>>,
    ) -> adw::Clamp {
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(600);
        clamp.set_tightening_threshold(400);

        let filtered: Vec<&ClipboardEntry> = entries
            .iter()
            .filter(|e| !pinned_only || e.pinned)
            .collect();

        if filtered.is_empty() {
            clamp.set_child(Some(&Self::build_empty_state(pinned_only)));
        } else {
            let list = ListBox::new();

            list.set_selection_mode(SelectionMode::Single);
            list.add_css_class("clipboard-list");
            list.add_css_class("boxed-list");

            for entry in &filtered {
                let row = build_entry_row(entry, action_tx.clone(), dbus.clone());

                row.set_widget_name(&format!(
                    "{}|{}",
                    entry.id,
                    match &entry.kind {
                        clippy_db::EntryKind::Text(t)
                        | clippy_db::EntryKind::Link(t)
                        | clippy_db::EntryKind::FilePath(t) => t,
                        _ => "",
                    }
                ));

                row.set_tooltip_text(Some(&entry.preview()));
                list.append(&row);
            }
            list.connect_row_activated({
                let tx = action_tx.clone();
                move |_, row| {
                    if let Some((id_str, _)) = row.widget_name().split_once('|') {
                        if let Ok(id) = id_str.parse::<i64>() {
                            let _ = tx.send(EntryAction::Paste(id));
                        }
                    }
                }
            });
            clamp.set_child(Some(&list));
        }
        clamp
    }

    fn build_empty_state(pinned_only: bool) -> adw::StatusPage {
        let page = adw::StatusPage::new();
        if pinned_only {
            page.set_icon_name(Some("starred-symbolic"));
            page.set_title("No Pinned Items");
            page.set_description(Some("Star items to pin them here"));
        } else {
            page.set_icon_name(Some("edit-paste-symbolic"));
            page.set_title("Nothing Copied Yet");
            page.set_description(Some("Copy something to get started"));
        }
        page
    }
}
