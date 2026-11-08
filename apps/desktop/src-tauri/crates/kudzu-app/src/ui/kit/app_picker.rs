//! `components/app/app-picker.tsx`: chosen apps as removable chips, then an "Add app" button whose
//! popover searches installed apps (cmdk, wrapping) and toggles them; a query shaped like a
//! bundle ID (`com.example.app`) can be added as typed.
//!
//! Installed apps come from `backend::commands::list_apps`, which is blocking: they load once per
//! process on a worker the first time a picker lays out, and land through the window's handle.
//!
//! ```ignore
//! // state: excluded: AppPickerState
//! AppPicker::new("excluded-apps", &settings.excluded_apps, &s.settings_page.excluded, |s: &mut MainUi| &mut s.settings_page.excluded)
//!     .on_change(|s: &mut MainUi, ids, _| s.update_settings(json!({ "excludedApps": ids })))
//!     .build(cx, &mut layer)
//! ```

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use rune::{Align, Cx, El, Shadow, Transition, Tx, el, icon, row, text};

use super::app_icon::app_icon;
use super::button::{Button, Size, Variant};
use super::command::{Command, CommandItem, CommandState};
use super::popover::Popover;
use super::toast::KitHost;
use crate::ui::icons;
use crate::ui::theme::{radius, text as ty, Theme, Utilities};
use crate::ui::theme_for;

type Access<S> = Rc<dyn for<'a> Fn(&'a mut S) -> &'a mut AppPickerState>;
type Change<S> = Rc<dyn Fn(&mut S, Vec<String>, &mut Tx)>;

/// An installed app as the picker shows it.
#[derive(Clone, Debug)]
pub struct App {
    pub bundle_id: String,
    pub name: String,
    pub path: Option<String>,
}

/// The picker's popover and search, owned by the screen.
#[derive(Clone, Debug, Default)]
pub struct AppPickerState {
    pub open: bool,
    pub cmd: CommandState,
}

enum Apps {
    Idle,
    Loading,
    Ready(Arc<Vec<App>>),
}

fn apps() -> &'static Mutex<Apps> {
    static APPS: OnceLock<Mutex<Apps>> = OnceLock::new();
    APPS.get_or_init(|| Mutex::new(Apps::Idle))
}

/// The installed apps, if they've loaded (`useInstalledApps`).
pub fn installed() -> Arc<Vec<App>> {
    match &*apps().lock().unwrap_or_else(|e| e.into_inner()) {
        Apps::Ready(a) => a.clone(),
        _ => Arc::new(Vec::new()),
    }
}

/// Starts loading the installed apps on a worker, once; `land` re-runs the window's view.
fn ensure_loaded<S: KitHost>(s: &S) {
    {
        let mut a = apps().lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(*a, Apps::Idle) {
            return;
        }
        *a = Apps::Loading;
    }
    let handle = s.kit_handle();
    std::thread::spawn(move || {
        let list: Vec<App> = crate::backend::commands::list_apps()
            .into_iter()
            .map(|a| App { bundle_id: a.bundle_id, name: a.name, path: a.path })
            .collect();
        *apps().lock().unwrap_or_else(|e| e.into_inner()) = Apps::Ready(Arc::new(list));
        if let Some(h) = handle {
            h.update(|_, _| {});
        }
    });
}

/// `appName`: the app's name, else its bundle ID.
pub fn app_name(apps: &[App], bundle_id: &str) -> String {
    apps.iter().find(|a| a.bundle_id == bundle_id).map_or_else(|| bundle_id.to_string(), |a| a.name.clone())
}

/// `/^[\w-]+(\.[\w-]+)+$/`: a query that looks like a bundle ID.
pub fn looks_like_bundle_id(q: &str) -> bool {
    let parts: Vec<&str> = q.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-'))
}

pub struct AppPicker<S> {
    key: String,
    value: Vec<String>,
    state: AppPickerState,
    access: Access<S>,
    add_label: String,
    change: Option<Change<S>>,
}

impl<S: KitHost> AppPicker<S> {
    pub fn new(key: impl Into<String>, value: &[String], state: &AppPickerState, access: impl for<'a> Fn(&'a mut S) -> &'a mut AppPickerState + 'static) -> Self {
        AppPicker { key: key.into(), value: value.to_vec(), state: state.clone(), access: Rc::new(access), add_label: "Add app".into(), change: None }
    }
    /// `addLabel` (default "Add app").
    pub fn add_label(mut self, l: impl Into<String>) -> Self {
        self.add_label = l.into();
        self
    }
    /// `onChange(bundleIds)`.
    pub fn on_change(mut self, f: impl Fn(&mut S, Vec<String>, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }

    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
        let th = theme_for(cx);
        let t = &th.t;
        let list = installed();
        let value = Rc::new(self.value);
        let chosen: HashSet<String> = value.iter().cloned().collect();
        let toggle = {
            let (value, change) = (value.clone(), self.change.clone());
            Rc::new(move |s: &mut S, id: &str, tx: &mut Tx| {
                let next: Vec<String> = if value.iter().any(|v| v == id) {
                    value.iter().filter(|v| *v != id).cloned().collect()
                } else {
                    value.iter().cloned().chain(std::iter::once(id.to_string())).collect()
                };
                if let Some(f) = &change {
                    f(s, next, tx);
                }
            })
        };
        let mut root = row().key(self.key.clone()).flex_wrap().center_y().gap(6.0).on_layout_tx(|s: &mut S, _, _| ensure_loaded(s));
        for id in value.iter() {
            let name = app_name(&list, id);
            let path = list.iter().find(|a| &a.bundle_id == id).and_then(|a| a.path.clone());
            let remove = {
                let (toggle, id) = (toggle.clone(), id.clone());
                row()
                    .key("remove")
                    .role("button")
                    .label(format!("Remove {name}"))
                    .focusable(true)
                    .size(20.0, 20.0)
                    .center()
                    .radius(4.0)
                    .hover(Theme::tint(t.foreground, 0.1))
                    .child(icon(&icons::X, 12.0).color(t.muted_foreground))
                    .on_press_tx(move |s: &mut S, tx| toggle(s, &id, tx))
            };
            // `inline-flex h-7 items-center gap-1.5 rounded-md bg-surface-raised pr-1 pl-1.5
            // text-xs shadow-raised animate-pop`.
            let chip = row()
                .key(format!("{}-chip-{id}", self.key))
                .h(28.0)
                .pl(6.0)
                .pr(4.0)
                .gap(6.0)
                .center_y()
                .radius(radius::MD)
                .fill(t.surface_raised)
                .elevate(&t.elev_raised)
                .transition(Transition::scale(0.94).springs(rune::SpringPair::new(crate::ui::theme::motion::pop(), crate::ui::theme::motion::moderate())))
                .child(app_icon(th, path.as_deref(), &name, 16.0))
                .child(text(name.clone()).font(ty::XS).color(t.foreground).truncate().max_w(160.0))
                .child(remove);
            root = root.child(chip);
        }

        // The search list.
        let query = self.state.cmd.query().to_string();
        let items: Vec<CommandItem<S>> = list
            .iter()
            .map(|a| {
                let (toggle, id) = (toggle.clone(), a.bundle_id.clone());
                let mut item = CommandItem::new(format!("{} {}", a.name, a.bundle_id))
                    .child(app_icon(th, a.path.as_deref(), &a.name, 18.0))
                    .text(a.name.clone())
                    .on_select(move |s: &mut S, tx| toggle(s, &id, tx));
                if chosen.contains(&a.bundle_id) {
                    item = item.child(el().key("selected").grow(1.0).justify_end().child(icon(&icons::CHECK, 14.0).color(t.iris)));
                }
                item
            })
            .collect();
        let empty = if looks_like_bundle_id(&query) {
            let (toggle, access, q) = (toggle.clone(), self.access.clone(), query.clone());
            text(format!("Add “{query}”")).font(ty::SM).color(t.foreground).cursor(rune::Cursor::Pointer).on_press_tx(move |s: &mut S, tx| {
                toggle(s, &q, tx);
                access(s).cmd.reset();
            })
        } else {
            text("No apps found.").font(ty::SM).color(t.popover_foreground)
        };
        let cmd = Command::new(format!("{}-cmd", self.key), &self.state.cmd, {
            let a = self.access.clone();
            move |s: &mut S| &mut a(s).cmd
        })
        .placeholder("Search apps or type a bundle ID…")
        .wrap(true)
        .max_height(288.0)
        .group(None, items)
        .empty(empty)
        .build(cx);

        // `Button variant="ghost" size="sm" className="h-7 text-muted-foreground ring-1 ring-dashed ring-border"`.
        let add = Button::new(self.add_label.clone())
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .icon(&icons::PLUS)
            .color(t.muted_foreground)
            .id(format!("{}-add", self.key))
            .build(th)
            .box_shadow(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: t.border, inset: false });
        let access = self.access.clone();
        let picker = Popover::new(format!("{}-popover", self.key))
            .open(self.state.open)
            .on_open_change(move |s: &mut S, open, _| {
                let st = access(s);
                st.open = open;
                if !open {
                    st.cmd.reset();
                }
            })
            .trigger(add)
            .align(Align::Start)
            .width(288.0)
            .padding(0.0)
            .gap(0.0)
            .ring(false)
            .label("Add app")
            .child(cmd)
            .build(cx, layer);
        root.child(picker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_ids_are_recognised_like_the_web_regex() {
        assert!(looks_like_bundle_id("com.apple.Safari"));
        assert!(looks_like_bundle_id("org.my-app.x_y"));
        assert!(!looks_like_bundle_id("Safari"));
        assert!(!looks_like_bundle_id("com..apple"));
        assert!(!looks_like_bundle_id("com.apple."));
    }

    #[test]
    fn names_fall_back_to_the_bundle_id() {
        let apps = vec![App { bundle_id: "com.apple.Safari".into(), name: "Safari".into(), path: None }];
        assert_eq!(app_name(&apps, "com.apple.Safari"), "Safari");
        assert_eq!(app_name(&apps, "com.x.y"), "com.x.y");
    }
}
