use gtk4::prelude::*;
use gtk4::{
    Box as GBox, Entry, ListBox, Orientation,
    PolicyType, ScrolledWindow, SelectionMode, Widget,
};
use libadwaita as adw;
use libadwaita::prelude::*;
use libadwaita::ViewStack;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::ui::entry_row::{build_entry_row, ClipboardEntry, EntryAction};
use crate::ui::search_bar::build_search_bar;

pub struct ClipboardWindow;

impl ClipboardWindow {
    pub fn build(
        history: Rc<RefCell<Vec<ClipboardEntry>>>,
    ) -> (Widget, Sender<EntryAction>, Receiver<EntryAction>) {
        let (action_tx, action_rx) = channel::<EntryAction>();

        let toolbar_view = adw::ToolbarView::new();

        // Borrow the history once to build the initial list
        let entries = history.borrow();

        let stack = adw::ViewStack::new();

        let all_sw = ClipboardWindow::build_list_page(&entries, false, action_tx.clone());
        let all_page = stack.add_titled(&all_sw, Some("all"), "All");
        all_page.set_icon_name(Some("edit-paste-symbolic"));

        let pinned_sw = ClipboardWindow::build_list_page(&entries, true, action_tx.clone());
        let pinned_page = stack.add_titled(&pinned_sw, Some("pinned"), "Pinned");
        pinned_page.set_icon_name(Some("starred-symbolic"));

        // Drop the borrow before we do anything else with history
        drop(entries);

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);

        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        header.set_title_widget(Some(&switcher));

        toolbar_view.add_top_bar(&header);

        let content = GBox::new(Orientation::Vertical, 0);

        let (search_widget, search_entry) = build_search_bar();
        content.append(&search_widget);

        Self::wire_search(&stack, search_entry);

        content.append(&stack);
        toolbar_view.set_content(Some(&content));

        (toolbar_view.upcast(), action_tx, action_rx)
    }

    /// Call this from main whenever the history changes (new copy, delete, pin).
    /// It fully rebuilds both list pages from the current history state.
    pub fn rebuild(
        toolbar_view: &Widget,
        history: Rc<RefCell<Vec<ClipboardEntry>>>,
        action_tx: Sender<EntryAction>,
    ) {
        // Walk down: ToolbarView → content GBox → ViewStack (second child)
        let Some(tv) = toolbar_view.downcast_ref::<adw::ToolbarView>() else { return };
        let Some(content) = tv.content().and_then(|w| w.downcast::<GBox>().ok()) else { return };

        // The stack is the second child of content (after the search bar)
        let mut child = content.first_child();
        // skip first child (search bar)
        child = child.and_then(|w| w.next_sibling());
        let Some(stack) = child.and_then(|w| w.downcast::<adw::ViewStack>().ok()) else { return };

        let entries = history.borrow();

        for (name, pinned_only) in [("all", false), ("pinned", true)] {
            let Some(page_widget) = stack.child_by_name(name) else { continue };
            let Some(sw) = page_widget.downcast_ref::<ScrolledWindow>() else { continue };

            // Rebuild the clamp+list inside the existing ScrolledWindow
            let new_clamp = Self::build_list_content(&entries, pinned_only, action_tx.clone());
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
                            let tooltip = row
                                .tooltip_text()
                                .unwrap_or_default()
                                .to_lowercase();
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
    ) -> ScrolledWindow {
        let sw = ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .build();
        sw.add_css_class("list-scroll");
        sw.set_child(Some(&Self::build_list_content(entries, pinned_only, action_tx)));
        sw
    }

    // Separated so rebuild() can call it without recreating the ScrolledWindow
    fn build_list_content(
        entries: &[ClipboardEntry],
        pinned_only: bool,
        action_tx: Sender<EntryAction>,
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
                let row = build_entry_row(entry, action_tx.clone());
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