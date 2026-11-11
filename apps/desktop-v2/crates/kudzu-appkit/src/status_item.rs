//! RUNE-GAP(#141): delete when Rune's macOS tray supports these runtime updates.
//! Callbacks run on the main thread and must only queue work, never terminate synchronously.
mod glyph;
mod shortcut;
use crate::menu_action::Target;
pub use glyph::{Glyph, GlyphKind};
use objc2::rc::Retained;
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{MainThreadMarker, NSString};
pub use shortcut::Shortcut;
use std::{collections::BTreeMap, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ItemId(pub u32);

pub struct Item {
    pub id: ItemId,
    pub title: String,
    pub enabled: bool,
    pub tooltip: Option<String>,
    pub shortcut: Shortcut,
    pub action: Rc<dyn Fn()>,
}

pub enum Entry {
    Item(Item),
    Separator,
}

/// Own on the UI thread (e.g. in a thread-local shell slot). Drop removes the status item.
pub struct StatusItem {
    status: Retained<NSStatusItem>,
    items: BTreeMap<ItemId, Retained<NSMenuItem>>,
    _target: Retained<Target>,
    mtm: MainThreadMarker,
}

impl StatusItem {
    pub fn new(
        mtm: MainThreadMarker,
        title: &str,
        tooltip: Option<&str>,
        glyph: Glyph,
        entries: Vec<Entry>,
    ) -> Result<Self, &'static str> {
        let mut callbacks = BTreeMap::new();
        for entry in &entries {
            if let Entry::Item(item) = entry {
                if callbacks
                    .insert(item.id.0 as isize, item.action.clone())
                    .is_some()
                {
                    return Err("duplicate menu item id");
                }
            }
        }
        let image = glyph.native(mtm)?;
        let target = Target::new(mtm, callbacks);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let mut items = BTreeMap::new();
        for entry in entries {
            match entry {
                Entry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                Entry::Item(model) => {
                    let item = target.item(mtm, &model.title, model.id.0 as isize);
                    item.setEnabled(model.enabled);
                    item.setToolTip(model.tooltip.as_deref().map(NSString::from_str).as_deref());
                    model.shortcut.apply(&item);
                    menu.addItem(&item);
                    items.insert(model.id, item);
                }
            }
        }
        let status =
            NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        let Some(button) = status.button(mtm) else {
            NSStatusBar::systemStatusBar().removeStatusItem(&status);
            return Err("AppKit status button unavailable");
        };
        button.setTitle(&NSString::from_str(title));
        button.setToolTip(tooltip.map(NSString::from_str).as_deref());
        button.setImage(Some(&image));
        status.setMenu(Some(&menu));
        Ok(Self {
            status,
            items,
            _target: target,
            mtm,
        })
    }

    pub fn set_title(&self, id: ItemId, title: &str) -> bool {
        self.item(id, |i| i.setTitle(&NSString::from_str(title)))
    }
    pub fn set_enabled(&self, id: ItemId, enabled: bool) -> bool {
        self.item(id, |i| i.setEnabled(enabled))
    }
    pub fn set_tooltip(&self, id: ItemId, tooltip: Option<&str>) -> bool {
        self.item(id, |i| {
            i.setToolTip(tooltip.map(NSString::from_str).as_deref())
        })
    }
    pub fn set_shortcut(&self, id: ItemId, shortcut: &Shortcut) -> bool {
        self.item(id, |i| shortcut.apply(i))
    }
    pub fn set_status_title(&self, title: &str) {
        if let Some(button) = self.status.button(self.mtm) {
            button.setTitle(&NSString::from_str(title));
        }
    }
    pub fn set_status_tooltip(&self, tooltip: Option<&str>) {
        if let Some(button) = self.status.button(self.mtm) {
            button.setToolTip(tooltip.map(NSString::from_str).as_deref());
        }
    }
    pub fn set_glyph(&self, glyph: Glyph) -> Result<(), &'static str> {
        let image = glyph.native(self.mtm)?;
        if let Some(button) = self.status.button(self.mtm) {
            button.setImage(Some(&image));
        }
        Ok(())
    }
    fn item(&self, id: ItemId, apply: impl FnOnce(&NSMenuItem)) -> bool {
        self.items.get(&id).is_some_and(|item| {
            apply(item);
            true
        })
    }
}

impl Drop for StatusItem {
    fn drop(&mut self) {
        self.status.setMenu(None);
        for item in self.items.values() {
            // SAFETY: clearing targets before releasing our retained callback target.
            unsafe {
                item.setTarget(None);
            }
        }
        NSStatusBar::systemStatusBar().removeStatusItem(&self.status);
    }
}
