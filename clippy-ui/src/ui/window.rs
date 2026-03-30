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
    pub fn build(
        history: Rc<RefCell<Vec<ClipboardEntry>>>,
        dbus: Rc<RefCell<crate::dbus_client::DbusClient>>,
    ) -> (Widget, Sender<EntryAction>, Receiver<EntryAction>) {
        let (action_tx, action_rx) = channel();

        // The root widget is a ToolbarView
        let toolbar_view = adw::ToolbarView::new();
        let header_bar = adw::HeaderBar::new();
        header_bar.set_show_end_title_buttons(false);

        // A ViewSwitcher for "All" vs "Pinned"
        let switcher = adw::ViewSwitcher::new();

        switcher.set_policy(adw::ViewSwitcherPolicy::Narrow);
        header_bar.set_title_widget(Some(&switcher));
        toolbar_view.add_top_bar(&header_bar);

        let stack = adw::ViewStack::new();
        let page_all = Self::build_list_page(&history.borrow(), false, action_tx.clone(), dbus.clone());
        let p1 = stack.add_titled_with_icon(&page_all, Some("all"), "All", "view-list-symbolic");

        p1.set_icon_name(Some("view-list-symbolic"));

        let page_pinned = Self::build_list_page(&history.borrow(), true, action_tx.clone(), dbus.clone());
        let p2 =
            stack.add_titled_with_icon(&page_pinned, Some("pinned"), "Pinned", "starred-symbolic");

        p2.set_icon_name(Some("starred-symbolic"));
        switcher.set_stack(Some(&stack));

        let content = GBox::new(Orientation::Vertical, 0);
        let (search_widget, search_entry) = build_search_bar();

        content.append(&search_widget);

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
        for (name, pinned_only) in [("all", false), ("pinned", true)] {
            let Some(page_widget) = stack.child_by_name(name) else {
                continue;
            };
            let Some(sw) = page_widget.downcast_ref::<ScrolledWindow>() else {
                continue;
            };
            let new_clamp = Self::build_list_content(&entries, pinned_only, action_tx.clone(), dbus.clone());
            sw.set_child(Some(&new_clamp));
        }
    }
    fn wire_search(stack: &ViewStack, search_entry: Entry) {
        let stack_clone = stack.clone();
        search_entry.connect_changed(move |entry| {
            let query = entry.text().to_lowercase();
            for name in ["all", "pinned"] {
                if let Some(page_widget) = stack_clone.child_by_name(name) {
                    if let Some(lb) = page_widget
                        .downcast_ref::<ScrolledWindow>()
                        .and_then(|sw| sw.child())
                        .and_then(|v| v.first_child())
                        .and_then(|w| w.downcast::<ListBox>().ok())
                    {
                        let q = query.clone();
                        lb.set_filter_func(move |row| {
                            if q.is_empty() {
                                return true;
                            }
                            let tooltip = row.tooltip_text().unwrap_or_default().to_lowercase();
                            tooltip.contains(&q)
                        });
                        lb.invalidate_filter();
                    }
                }
            }
        });
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
                row.set_tooltip_text(Some(&entry.preview()));
                list.append(&row);
            }
            list.connect_row_activated({
                let tx = action_tx.clone();
                move |_, row| {
                    if let Ok(id) = row.widget_name().parse::<i64>() {
                        let _ = tx.send(EntryAction::Paste(id));
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
