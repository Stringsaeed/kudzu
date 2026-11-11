//! RUNE-GAP(#561): delete when Rune lets the app replace/re-key its built-in Quit item.
//! Install in the first update, BEFORE any Dock promotion. After a Dock request, queue
//! `patch` from INSIDE that update: Rune drains nested updates on the next turn, after
//! applying the Dock request. Never use a sleep. Patch again after `rune::set_labels`.
use crate::menu_action::Target;
use objc2::{rc::Retained, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSString};
use std::{collections::BTreeMap, rc::Rc};

const CLOSE_TAG: isize = 0x4b5a_0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Other,
    Services,
    Separator,
    ShowAll,
    Close,
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
enum Slot {
    Existing(usize),
    Separator,
    Close,
}

fn order(kinds: &[Kind]) -> Vec<Slot> {
    let mut slots = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        if matches!(kind, Kind::ShowAll | Kind::Close) {
            continue;
        }
        if *kind == Kind::Quit && !slots.contains(&Slot::Close) {
            slots.push(Slot::Close);
        }
        slots.push(Slot::Existing(i));
        if *kind == Kind::Services && kinds.get(i + 1) != Some(&Kind::Separator) {
            slots.push(Slot::Separator);
        }
    }
    if !slots.contains(&Slot::Close) {
        slots.push(Slot::Close);
    }
    slots
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchState {
    WaitingForPromotion,
    Patched,
}

/// One owner for the process. The closure must queue `Shell::close_windows`, never terminate.
pub struct AppMenu {
    close: Retained<NSMenuItem>,
    _target: Retained<Target>,
    quit_title: String,
    mtm: MainThreadMarker,
}

impl AppMenu {
    pub fn install(
        mtm: MainThreadMarker,
        app_name: &str,
        close_windows: Rc<dyn Fn()>,
    ) -> Result<Self, &'static str> {
        let target = Target::new(mtm, BTreeMap::from([(CLOSE_TAG, close_windows)]));
        let close = target.item(mtm, "Close Windows", CLOSE_TAG);
        close.setEnabled(true);
        let owner = Self {
            close,
            _target: target,
            quit_title: format!("Quit {app_name}"),
            mtm,
        };
        owner.patch()?;
        Ok(owner)
    }

    /// Also repairs menu replacement and repositions Close Windows immediately above Quit.
    /// With Rune's still-empty accessory menu, installs ⌘Q and reports WaitingForPromotion.
    pub fn patch(&self) -> Result<PatchState, &'static str> {
        let application = NSApplication::sharedApplication(self.mtm);
        let root = application.mainMenu().ok_or("Rune menu bar is not ready")?;
        let menu = root
            .itemAtIndex(0)
            .and_then(|i| i.submenu())
            .ok_or("Rune app menu is not ready")?;
        // SAFETY: this item and its enclosing menu are only accessed on the owning UI thread.
        if let Some(previous) = unsafe { self.close.menu() } {
            previous.removeItem(&self.close);
        }
        let entries: Vec<_> = (0..menu.numberOfItems())
            .filter_map(|i| menu.itemAtIndex(i))
            .collect();
        let services = application.servicesMenu();
        let kinds: Vec<_> = entries
            .iter()
            .map(|item| kind(item, services.as_deref()))
            .collect();
        let state = if kinds.contains(&Kind::Quit) {
            PatchState::Patched
        } else {
            PatchState::WaitingForPromotion
        };
        for (item, kind) in entries.iter().zip(&kinds) {
            if *kind == Kind::Quit {
                item.setKeyEquivalent(&NSString::new());
                item.setTitle(&NSString::from_str(&self.quit_title));
            }
        }
        menu.removeAllItems();
        for slot in order(&kinds) {
            match slot {
                Slot::Existing(i) => menu.addItem(&entries[i]),
                Slot::Separator => menu.addItem(&NSMenuItem::separatorItem(self.mtm)),
                Slot::Close => menu.addItem(&self.close),
            }
        }
        self.close.setKeyEquivalent(&NSString::from_str("q"));
        self.close
            .setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        Ok(state)
    }
}

fn kind(item: &NSMenuItem, services: Option<&NSMenu>) -> Kind {
    match item.action() {
        Some(action) if action == sel!(terminate:) => Kind::Quit,
        Some(action) if action == sel!(unhideAllApplications:) => Kind::ShowAll,
        Some(action) if action == sel!(kudzuGapAction:) && item.tag() == CLOSE_TAG => Kind::Close,
        _ if item.isSeparatorItem() => Kind::Separator,
        _ if services.is_some() && item.submenu().as_deref() == services => Kind::Services,
        _ => Kind::Other,
    }
}

impl Drop for AppMenu {
    fn drop(&mut self) {
        // SAFETY: main-thread owner, as in patch.
        if let Some(menu) = unsafe { self.close.menu() } {
            menu.removeItem(&self.close);
        }
        // SAFETY: clearing the non-owning target before dropping its owner.
        unsafe {
            self.close.setTarget(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patched(kinds: &[Kind]) -> Vec<Kind> {
        order(kinds)
            .iter()
            .map(|slot| match slot {
                Slot::Existing(i) => kinds[*i],
                Slot::Separator => Kind::Separator,
                Slot::Close => Kind::Close,
            })
            .collect()
    }

    #[test]
    fn command_q_exists_before_the_first_dock_promotion() {
        assert_eq!(patched(&[]), [Kind::Close]);
        assert_eq!(patched(&[Kind::Close]), [Kind::Close]);
    }

    #[test]
    fn promotion_reorders_preserves_standard_items_and_is_idempotent() {
        use Kind::*;
        let input = [
            Close, Other, Separator, Services, Other, Other, ShowAll, Separator, Quit,
        ];
        let expected = [
            Other, Separator, Services, Separator, Other, Other, Separator, Close, Quit,
        ];
        assert_eq!(patched(&input), expected);
        assert_eq!(patched(&expected), expected);
        assert_eq!(patched(&[Close, Quit, Close]), [Close, Quit]);
    }
}
