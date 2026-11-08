//! `components/ui/context-menu.tsx` (Radix ContextMenu): the dropdown menu's rows and keys
//! ([`super::dropdown_menu`]), opened by a secondary press (right- or control-click, the Menu
//! key, Shift-F10) at the pointer: `side="right" align="start" sideOffset={2}`, `min-w-36`, and a
//! bordered sub-content.
//!
//! ```ignore
//! // state: group_menu: MenuState
//! let row = ContextMenu::new(format!("group-{id}"), &s.sidebar.group_menu, |s: &mut MainUi| &mut s.sidebar.group_menu)
//!     .trigger(group_row)
//!     .width(208.0)
//!     .item(MenuItem::new("New subgroup").on_select(..))
//!     .separator()
//!     .item(MenuItem::new("Delete group").destructive().on_select(..))
//!     .build(cx, &mut layer);
//! ```
//!
//! One `MenuState` can serve many rows' menus (only one is open at a time): give each row its
//! own key and keep which row opened it in the screen's state.

use std::rc::Rc;

use rune::{Align, Cx, El, Side, el};

use super::dropdown_menu::{mount_menu, Access, Entry, Flavor, MenuItem, MenuState, SubMenu};
use crate::ui::theme_for;

pub struct ContextMenu<S> {
    key: String,
    state: MenuState,
    open: bool,
    access: Access<S>,
    trigger: Option<El>,
    entries: Vec<Entry<S>>,
    width: Option<f32>,
}

impl<S: 'static> ContextMenu<S> {
    pub fn new(key: impl Into<String>, state: &MenuState, access: impl for<'a> Fn(&'a mut S) -> &'a mut MenuState + 'static) -> Self {
        ContextMenu { key: key.into(), state: state.clone(), open: state.open, access: Rc::new(access), trigger: None, entries: Vec::new(), width: None }
    }
    /// Whether this trigger's menu is the open one, when several rows share a `MenuState`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open && self.state.open;
        self
    }
    /// `ContextMenuTrigger asChild`: the area a secondary press opens the menu over.
    pub fn trigger(mut self, t: El) -> Self {
        self.trigger = Some(t);
        self
    }
    /// `w-52` and the like; otherwise it fits its rows (`min-w-36`).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn item(mut self, i: MenuItem<S>) -> Self {
        self.entries.push(Entry::Item(i));
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.entries.push(Entry::Label(l.into(), false));
        self
    }
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
    pub fn sub(mut self, s: SubMenu<S>) -> Self {
        self.entries.push(Entry::Sub(s));
        self
    }

    /// Returns the trigger area with its secondary press; the open menu goes to `layer`.
    /// `on_open` runs when a secondary press opens it (select the row, say).
    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
        self.build_with(cx, layer, |_: &mut S| {})
    }

    pub fn build_with(self, cx: &mut Cx<S>, layer: &mut Vec<El>, on_open: impl Fn(&mut S) + 'static) -> El {
        let th = theme_for(cx);
        let access = self.access.clone();
        let anchor = format!("{}-area", self.key);
        let trigger = self.trigger.unwrap_or_else(el).key(anchor.clone()).on_secondary_press(move |s: &mut S, p, _| {
            let st = access(s);
            st.open_with(None);
            st.point = Some((p.x, p.y));
            on_open(s);
        });
        if !self.open {
            return trigger;
        }
        let Some((x, y)) = self.state.point else { return trigger };
        let point = format!("{}-point", self.key);
        layer.push(el().key(point.clone()).absolute(x, y).size(0.0, 0.0).a11y_hidden());
        let entries = Rc::new(self.entries);
        mount_menu(th, &self.key, entries, &self.state, self.access, Flavor::Context, self.width, 144.0, (&point, Side::Right, Align::Start, 2.0), None, layer);
        trigger
    }
}
