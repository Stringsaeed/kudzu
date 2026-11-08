//! `components/ui/tabs.tsx` (Radix Tabs): a `bg-muted shadow-well` segmented list (or the `line`
//! variant's underlined triggers) whose active trigger is raised (`bg-surface-raised shadow-sm`).
//! Keys follow Radix: the list is one Tab stop on the active trigger; Left and Right (Up and Down
//! when vertical) move and activate (`activationMode="automatic"`), wrapping; Home and End jump.
//!
//! ```ignore
//! let list = Tabs::new("insights-range").value(&s.range)
//!     .tab("7d", "7 days").tab("30d", "30 days")
//!     .on_value_change(|s: &mut MainUi, v, _| s.range = v.into())
//!     .build(th);
//! // then show the content for `s.range` (`TabsContent`: `flex-1 text-sm`).
//! ```

use std::rc::Rc;

use rune::{Color, El, Icon, Spring, Tx, col, el, icon, row, text};

use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};

type Change<S> = Rc<dyn Fn(&mut S, &str, &mut Tx)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TabsVariant {
    #[default]
    Default,
    Line,
}

struct Tab {
    value: String,
    label: String,
    icon: Option<&'static Icon>,
    disabled: bool,
}

pub struct Tabs<S> {
    key: String,
    value: String,
    tabs: Vec<Tab>,
    variant: TabsVariant,
    vertical: bool,
    change: Option<Change<S>>,
}

impl<S: 'static> Tabs<S> {
    pub fn new(key: impl Into<String>) -> Self {
        Tabs { key: key.into(), value: String::new(), tabs: Vec::new(), variant: TabsVariant::Default, vertical: false, change: None }
    }
    pub fn value(mut self, v: &str) -> Self {
        self.value = v.to_string();
        self
    }
    pub fn tab(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.tabs.push(Tab { value: value.into(), label: label.into(), icon: None, disabled: false });
        self
    }
    /// A trigger with a leading 16 pt icon.
    pub fn tab_icon(mut self, value: impl Into<String>, label: impl Into<String>, i: &'static Icon) -> Self {
        self.tabs.push(Tab { value: value.into(), label: label.into(), icon: Some(i), disabled: false });
        self
    }
    pub fn tab_disabled(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.tabs.push(Tab { value: value.into(), label: label.into(), icon: None, disabled: true });
        self
    }
    pub fn variant(mut self, v: TabsVariant) -> Self {
        self.variant = v;
        self
    }
    /// `orientation="vertical"`.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }
    pub fn on_value_change(mut self, f: impl Fn(&mut S, &str, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }

    /// The `TabsList` with its triggers.
    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let line = self.variant == TabsVariant::Line;
        let mut list = if self.vertical { col() } else { row().h(32.0) };
        list = list.key(self.key.clone()).role("tablist").focus_group().padding(3.0).center_y();
        list = if line { list.gap(4.0) } else { list.radius(radius::LG).fill(t.muted).elevate(&t.elev_well) };
        for tab in self.tabs {
            let active = tab.value == self.value;
            let rest = if th.dark { t.muted_foreground } else { Theme::tint(t.foreground, 0.6) };
            let ink = if active { t.foreground } else { rest };
            let mut tr = row()
                .key(format!("{}-{}", self.key, tab.value))
                .role("tab")
                .label(tab.label.clone())
                .checked(active)
                .focusable(!tab.disabled)
                .focus_selected(active)
                .focus_ring(3.0, 0.0, Theme::tint(t.ring, 0.5))
                .px(6.0)
                .py(2.0)
                .gap(6.0)
                .center()
                .radius(radius::MD)
                .motion(Spring::new(0.15, 0.0));
            tr = if self.vertical { tr.w_full().justify_start().h(25.0) } else { tr.h(25.0) };
            if active && !line {
                tr = tr.fill(t.surface_raised).elevate(&t.elev_soft);
            } else {
                tr = tr.fill(Color::TRANSPARENT).hover(Color::TRANSPARENT);
            }
            if let Some(i) = tab.icon {
                tr = tr.child(icon(i, 16.0).key("icon").color(ink).a11y_hidden());
            }
            tr = tr.child(text(tab.label.clone()).key("label").font(ty::SM.weight_of(weight::MEDIUM)).color(ink).a11y_hidden());
            if line {
                // `after:` underline, 2 pt, 5 pt below (or 4 pt beside when vertical).
                let bar = if self.vertical {
                    el().pin(None, Some(0.0), Some(-4.0), Some(0.0)).w(2.0)
                } else {
                    el().pin(Some(0.0), None, Some(0.0), Some(-5.0)).h(2.0)
                };
                tr = tr.child(bar.key("underline").fill(t.foreground).opacity(if active { 1.0 } else { 0.0 }).a11y_hidden());
            }
            if tab.disabled {
                tr = tr.layer_opacity(0.5).disabled(true);
            } else if let Some(f) = self.change.clone() {
                let v = tab.value.clone();
                tr = tr.on_press_tx(move |s: &mut S, tx| f(s, &v, tx));
            }
            list = list.child(tr);
        }
        list
    }
}
