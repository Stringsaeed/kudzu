//! Searchable app chips shared by group rules and snippet rules.
//!
//! The caller owns the chosen ids, the query and whether the list is open. Checking an app
//! does not close the list. A query that is a bundle id and matches nothing can be added as-is.

use kudzu_core::system::AppInfo;
use rune::{col, el, icon, row, text, Cx, El, TextEdit, TextStyle};

use super::app_icon::{AppIcon, IconCache};
use crate::theme::{icons, radius, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::input::Input;

const CHIP_ICON: u32 = 16;
const ROW_ICON: u32 = 18;
const LIST_W: f32 = 288.0;

/// `[\w-]+(\.[\w-]+)+` — a dotted bundle id, the same test the Tauri picker uses.
pub fn is_bundle_id(query: &str) -> bool {
    let mut parts = query.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    if !is_ident(first) {
        return false;
    }
    let mut more = false;
    for part in parts {
        if !is_ident(part) {
            return false;
        }
        more = true;
    }
    more
}

fn is_ident(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Name or bundle id contains `query`. An empty query matches every app.
pub fn matches(query: &str, app: &AppInfo) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    let q = q.to_lowercase();
    app.name.to_lowercase().contains(&q) || app.bundle_id.to_lowercase().contains(&q)
}

/// The custom id offered when the query is a bundle id and no installed app matches.
pub fn custom_id<'a>(query: &'a str, apps: &[AppInfo]) -> Option<&'a str> {
    let query = query.trim();
    if !is_bundle_id(query) || apps.iter().any(|app| matches(query, app)) {
        return None;
    }
    Some(query)
}

fn display_name(apps: &[AppInfo], bundle_id: &str) -> String {
    apps.iter()
        .find(|app| app.bundle_id == bundle_id)
        .map(|app| app.name.clone())
        .unwrap_or_else(|| bundle_id.to_owned())
}

/// Icon fetches for the chips, and for the open list's rows.
pub fn icons_needed(
    value: &[String],
    apps: &[AppInfo],
    query: &str,
    open: bool,
) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for id in value {
        out.push((id.clone(), CHIP_ICON));
    }
    if open {
        for app in apps.iter().filter(|app| matches(query, app)) {
            out.push((app.bundle_id.clone(), ROW_ICON));
        }
    }
    out
}

/// Chosen apps as chips, plus an "Add app" list that stays open across checks.
pub struct AppPicker<'a> {
    pub value: &'a [String],
    pub apps: &'a [AppInfo],
    pub query: &'a TextEdit,
    pub open: bool,
    pub icons: &'a IconCache,
    pub add_label: &'a str,
    /// Window origin of the element the list is parented to. Subtracted from `cx.layout`
    /// so a caller can put the list inside a dialog instead of on the window.
    pub origin: [f32; 2],
}

impl<'a> AppPicker<'a> {
    pub fn new(
        value: &'a [String],
        apps: &'a [AppInfo],
        query: &'a TextEdit,
        icons: &'a IconCache,
    ) -> Self {
        Self {
            value,
            apps,
            query,
            open: false,
            icons,
            add_label: "Add app",
            origin: [0.0, 0.0],
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn origin(mut self, origin: [f32; 2]) -> Self {
        self.origin = origin;
        self
    }

    pub fn build<S: 'static>(
        self,
        theme: &Theme,
        cx: &Cx<S>,
        edit: impl Fn(&mut S) -> &mut TextEdit + Clone + 'static,
        toggle: impl Fn(&mut S, String) + Clone + 'static,
        set_open: impl Fn(&mut S, bool) + Clone + 'static,
        overlays: &mut Vec<El>,
    ) -> El {
        let c = &theme.c;
        let mut chips = row()
            .key("app-picker")
            .w_full()
            .gap(6.0)
            .items_center()
            .flex_wrap();
        for id in self.value {
            let name = display_name(self.apps, id);
            let remove = format!("Remove {name}");
            let pick = id.clone();
            let on_remove = toggle.clone();
            let chip = row()
                .key(format!("app-chip-{id}"))
                .h(28.0)
                .pl(6.0)
                .pr(4.0)
                .gap(6.0)
                .items_center()
                .radius(radius::MD)
                .fill(c.surface_raised)
                .child(
                    AppIcon::new(name.clone())
                        .size(CHIP_ICON as f32)
                        .image(self.icons.image(id, CHIP_ICON))
                        .build(theme),
                )
                .child(
                    text(name.clone())
                        .font(TextStyle::new(12.0, 400.0).lh(16.0))
                        .color(c.foreground),
                )
                .child(
                    Button::new(remove)
                        .variant(Variant::Ghost)
                        .size(Size::IconXs)
                        .icon(&icons::X)
                        .on_press(move |s: &mut S| on_remove(s, pick.clone()))
                        .build(theme),
                );
            chips = chips.child(chip);
        }
        let open_now = self.open;
        let (open_list, shut) = (set_open.clone(), set_open.clone());
        chips = chips.child(
            Button::new(self.add_label)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .icon(&icons::PLUS)
                .on_press(move |s: &mut S| open_list(s, !open_now))
                .build(theme)
                .key("app-picker-add"),
        );
        if self.open {
            if let Some(anchor) = cx.layout("app-picker-add") {
                push_scrim(overlays, cx.layout("app-picker"), self.origin, shut.clone());
                overlays.push(self.list(theme, anchor, edit, toggle, shut));
            }
        }
        chips
    }

    fn list<S: 'static>(
        self,
        theme: &Theme,
        anchor: [f32; 4],
        edit: impl Fn(&mut S) -> &mut TextEdit + 'static,
        toggle: impl Fn(&mut S, String) + Clone + 'static,
        set_open: impl Fn(&mut S, bool) + Clone + 'static,
    ) -> El {
        let c = &theme.c;
        let query = self.query.text.clone();
        let shown: Vec<_> = self
            .apps
            .iter()
            .filter(|app| matches(&query, app))
            .cloned()
            .collect();
        let custom = custom_id(&query, self.apps).map(str::to_owned);
        let close = set_open.clone();
        let mut list = col()
            .key("app-picker-list")
            .absolute(
                anchor[0] - self.origin[0],
                anchor[1] - self.origin[1] + anchor[3] + 4.0,
            )
            .w(LIST_W)
            .max_h(288.0)
            .radius(radius::MD)
            .fill(c.popover)
            .clip_rounded(radius::MD)
            .child(
                Input::new(self.query)
                    .with(move |field| {
                        field
                            .label("Search apps or type a bundle ID…")
                            .placeholder("Search apps or type a bundle ID…")
                            .width(Some(LIST_W))
                            .on_edit(edit)
                            .on_key(move |s: &mut S, ev, _| {
                                if ev.key == rune::Key::Escape {
                                    close(s, false);
                                    true
                                } else {
                                    false
                                }
                            })
                    })
                    .build(theme)
                    .key("app-picker-search"),
            );
        for shadow in &theme.depth.float {
            list = list.box_shadow(*shadow);
        }
        if shown.is_empty() {
            let empty = if let Some(id) = custom {
                let add = toggle.clone();
                let typed = id.clone();
                let shut_query = set_open;
                el().key("app-picker-custom")
                    .w_full()
                    .padding(8.0)
                    .on_press(move |s: &mut S| {
                        add(s, typed.clone());
                        shut_query(s, true);
                    })
                    .child(
                        text(format!("Add “{id}”"))
                            .font(theme.ty.input)
                            .color(c.foreground),
                    )
            } else {
                el().padding(8.0).child(
                    text("No apps found.")
                        .font(theme.ty.input)
                        .color(c.muted_foreground),
                )
            };
            list = list.child(empty);
        }
        for app in shown {
            let chosen = self.value.iter().any(|id| id == &app.bundle_id);
            let id = app.bundle_id.clone();
            let on = toggle.clone();
            let mut item = row()
                .key(format!("app-row-{id}"))
                .w_full()
                .h(32.0)
                .px(8.0)
                .gap(8.0)
                .items_center()
                .on_press(move |s: &mut S| on(s, id.clone()))
                .child(
                    AppIcon::new(app.name.clone())
                        .size(ROW_ICON as f32)
                        .image(self.icons.image(&app.bundle_id, ROW_ICON))
                        .build(theme),
                )
                .child(
                    text(app.name.clone())
                        .font(theme.ty.input)
                        .color(c.foreground),
                );
            if chosen {
                item = item.child(icon(&icons::CHECK, 14.0).key("app-check").color(c.iris));
            }
            list = list.child(item);
        }
        list
    }
}

/// Close the list on a press outside it, leaving the chip row uncovered so Remove still hits.
fn push_scrim<S: 'static>(
    overlays: &mut Vec<El>,
    row: Option<[f32; 4]>,
    origin: [f32; 2],
    set_open: impl Fn(&mut S, bool) + Clone + 'static,
) {
    let Some(row) = row else {
        let close = set_open;
        overlays.push(
            el().key("app-picker-scrim")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .on_press(move |s: &mut S| close(s, false)),
        );
        return;
    };
    let top = row[1] - origin[1];
    let bottom = top + row[3];
    if top > 0.0 {
        let close = set_open.clone();
        overlays.push(
            el().key("app-picker-scrim")
                .absolute(0.0, 0.0)
                .w_full()
                .h(top)
                .on_press(move |s: &mut S| close(s, false)),
        );
    }
    overlays.push(
        el().key("app-picker-scrim")
            .pin(Some(0.0), Some(bottom), Some(0.0), Some(0.0))
            .on_press(move |s: &mut S| set_open(s, false)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::app_icon::IconCache;
    use rune::{Cx, Env, Runtime};
    use std::sync::Arc;

    fn app(name: &str, bundle_id: &str) -> AppInfo {
        AppInfo {
            bundle_id: bundle_id.into(),
            name: name.into(),
            pid: None,
            path: None,
        }
    }

    #[test]
    fn search_matches_names_and_bundle_ids_and_offers_a_custom_id() {
        let apps = vec![
            app("Mail", "com.apple.mail"),
            app("Safari", "com.apple.Safari"),
        ];
        assert!(matches("mai", &apps[0]));
        assert!(matches("SAFARI", &apps[1]));
        assert!(matches("com.apple.mail", &apps[0]));
        assert!(!matches("notes", &apps[0]));
        assert!(matches("  ", &apps[0]));
        assert_eq!(custom_id("com.example.app", &apps), Some("com.example.app"));
        assert_eq!(custom_id("com.apple.mail", &apps), None);
        assert_eq!(custom_id("not an id", &apps), None);
        assert!(!is_bundle_id("com."));
        assert!(!is_bundle_id(".com.apple"));
        assert!(is_bundle_id("a.b"));
    }

    struct PickerState {
        value: Vec<String>,
        apps: Vec<AppInfo>,
        query: TextEdit,
        open: bool,
        icons: IconCache,
    }

    fn picker_view(s: &PickerState, cx: &mut Cx<PickerState>) -> El {
        let mut overlays = Vec::new();
        let row = AppPicker::new(&s.value, &s.apps, &s.query, &s.icons)
            .open(s.open)
            .build(
                Theme::light(),
                cx,
                |s: &mut PickerState| &mut s.query,
                |s: &mut PickerState, id| {
                    if let Some(i) = s.value.iter().position(|v| v == &id) {
                        s.value.remove(i);
                    } else {
                        s.value.push(id);
                    }
                },
                |s: &mut PickerState, open| s.open = open,
                &mut overlays,
            );
        rune_kit::root(row, overlays)
    }

    #[test]
    fn checking_an_app_keeps_the_list_open() {
        let mut rt = Runtime::new(
            PickerState {
                value: Vec::new(),
                apps: vec![
                    app("Mail", "com.apple.mail"),
                    app("Notes", "com.apple.Notes"),
                ],
                query: TextEdit::new(""),
                open: true,
                icons: IconCache::new(Arc::new(|| {})),
            },
            picker_view,
            (400.0, 400.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.frame(0.0);
        let mail = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("app-row-com.apple.mail"))
            .unwrap();
        let (x, y) = (mail.rect[0] + mail.rect[2] / 2.0, mail.rect[1] + 4.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rt.state.open);
        assert_eq!(rt.state.value, ["com.apple.mail"]);
        assert!(rt
            .elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("app-picker-list")));
    }
}
