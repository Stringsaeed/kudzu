//! Searchable app chips shared by group rules and snippet rules.
//!
//! The caller owns the chosen ids, the query, the active row and whether the list is open.
//! Checking an app does not close the list. A query that is a bundle id and matches nothing
//! can be added as-is. Every element key is prefixed with the picker's id, so two pickers
//! can share a pane. Call `.overlays` before `.build` so the list and its scrim attach.

mod list;

use kudzu_core::system::AppInfo;
use rune::{row, text, Cx, El, TextEdit, TextStyle};

use super::app_icon::{AppIcon, IconCache};
use crate::theme::{icons, radius, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

const CHIP_ICON: f32 = 16.0;
pub(super) const ROW_ICON: f32 = 18.0;
/// One catalogue size for chips and rows. The tiles still draw at 16 and 18 points.
pub(super) const ICON_PX: u32 = 64;
pub(super) const LIST_W: f32 = 288.0;

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

/// Icon fetches for the chips and the open list. One pixel size, each id once.
pub fn icons_needed(
    value: &[String],
    apps: &[AppInfo],
    query: &str,
    open: bool,
) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut push = |id: &str| {
        if out.iter().any(|(key, _)| key == id) {
            return;
        }
        out.push((id.to_owned(), ICON_PX));
    };
    for id in value {
        push(id);
    }
    if open {
        for app in apps.iter().filter(|app| matches(query, app)) {
            push(&app.bundle_id);
        }
    }
    out
}

/// Chosen apps as chips, plus an "Add app" list that stays open across checks.
pub struct AppPicker<'a> {
    id: &'static str,
    value: &'a [String],
    apps: &'a [AppInfo],
    query: &'a TextEdit,
    open: bool,
    active: usize,
    icons: &'a IconCache,
    add_label: &'a str,
    overlays: Option<&'a mut Vec<El>>,
}

impl<'a> AppPicker<'a> {
    pub fn new(
        id: &'static str,
        value: &'a [String],
        apps: &'a [AppInfo],
        query: &'a TextEdit,
        icons: &'a IconCache,
    ) -> Self {
        Self {
            id,
            value,
            apps,
            query,
            open: false,
            active: 0,
            icons,
            add_label: "Add app",
            overlays: None,
        }
    }

    pub fn add_label(mut self, label: &'a str) -> Self {
        self.add_label = label;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn active(mut self, index: usize) -> Self {
        self.active = index;
        self
    }

    /// List and scrim destination. Without this, `build` draws chips only.
    pub fn overlays(mut self, overlays: &'a mut Vec<El>) -> Self {
        self.overlays = Some(overlays);
        self
    }

    pub fn build<S: 'static>(
        mut self,
        theme: &Theme,
        cx: &Cx<S>,
        edit: impl Fn(&mut S) -> &mut TextEdit + Clone + 'static,
        toggle: impl Fn(&mut S, String) + Clone + 'static,
        set_open: impl Fn(&mut S, bool) + Clone + 'static,
        set_active: impl Fn(&mut S, usize) + Clone + 'static,
    ) -> El {
        let c = &theme.c;
        let mut chips = row()
            .key(self.key(""))
            .w_full()
            .gap(6.0)
            .items_center()
            .flex_wrap();
        for id in self.value {
            let name = display_name(self.apps, id);
            let remove = format!("Remove {name}");
            let pick = id.clone();
            let on_remove = toggle.clone();
            let mut chip = row()
                .key(self.key(&format!("chip-{id}")))
                .h(28.0)
                .pl(6.0)
                .pr(4.0)
                .gap(6.0)
                .items_center()
                .radius(radius::MD)
                .fill(c.surface_raised)
                .child(
                    AppIcon::new(name.clone())
                        .size(CHIP_ICON)
                        .image(self.icons.image(id, ICON_PX))
                        .build(theme),
                )
                .child(
                    text(name)
                        .font(TextStyle::new(12.0, 400.0).lh(16.0))
                        .color(c.foreground)
                        .truncate()
                        .max_w(160.0),
                )
                .child(
                    Button::new(remove)
                        .variant(Variant::Ghost)
                        .size(Size::IconXs)
                        .icon(&icons::X)
                        .on_press(move |s: &mut S| on_remove(s, pick.clone()))
                        .build(theme)
                        .size(20.0, 20.0),
                );
            for shadow in &theme.depth.raised {
                chip = chip.box_shadow(*shadow);
            }
            chips = chips.child(chip);
        }
        let was_open = self.open;
        let open_list = set_open.clone();
        let focus = edit.clone();
        let mark = set_active.clone();
        chips = chips.child(
            Button::new(self.add_label)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .ink(theme.c.muted_foreground)
                .icon(&icons::PLUS)
                .on_press(move |s: &mut S| {
                    let opening = !was_open;
                    open_list(s, !was_open);
                    if opening {
                        focus(s).focused = true;
                        mark(s, 0);
                    }
                })
                .build(theme)
                .key(self.key("add"))
                .border(1.0, theme.c.border)
                .border_dash(3.0, 3.0),
        );
        let mut detached = Vec::new();
        let bind = list::Bind::new(edit, toggle, set_open, set_active);
        match self.overlays.take() {
            Some(overlays) => list::mount(&self, theme, cx, bind, overlays),
            None => list::mount(&self, theme, cx, bind, &mut detached),
        }
        chips
    }

    pub(super) fn key(&self, suffix: &str) -> String {
        if suffix.is_empty() {
            self.id.to_owned()
        } else {
            format!("{}-{suffix}", self.id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::app_icon::IconCache;
    use rune::{el, Cx, Env, Runtime, ScrollSpec};
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
        active: usize,
        icons: IconCache,
    }

    fn picker_view(s: &PickerState, cx: &mut Cx<PickerState>) -> El {
        let mut overlays = Vec::new();
        let row = AppPicker::new("app-picker", &s.value, &s.apps, &s.query, &s.icons)
            .open(s.open)
            .active(s.active)
            .overlays(&mut overlays)
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
                |s: &mut PickerState, index| s.active = index,
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
                active: 0,
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
            .find(|e| e.key.as_deref() == Some("app-picker-row-com.apple.mail"))
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

    struct Two {
        include: Vec<String>,
        exclude: Vec<String>,
        include_query: TextEdit,
        exclude_query: TextEdit,
        include_open: bool,
        exclude_open: bool,
        include_active: usize,
        exclude_active: usize,
        apps: Vec<AppInfo>,
        icons: IconCache,
    }

    fn two_view(s: &Two, cx: &mut Cx<Two>) -> El {
        let mut overlays = Vec::new();
        let include = AppPicker::new("include", &s.include, &s.apps, &s.include_query, &s.icons)
            .add_label("Add include")
            .open(s.include_open)
            .active(s.include_active)
            .overlays(&mut overlays)
            .build(
                Theme::light(),
                cx,
                |s: &mut Two| &mut s.include_query,
                |s: &mut Two, id| s.include.push(id),
                |s: &mut Two, open| s.include_open = open,
                |s: &mut Two, index| s.include_active = index,
            );
        let exclude = AppPicker::new("exclude", &s.exclude, &s.apps, &s.exclude_query, &s.icons)
            .add_label("Add exclude")
            .open(s.exclude_open)
            .active(s.exclude_active)
            .overlays(&mut overlays)
            .build(
                Theme::light(),
                cx,
                |s: &mut Two| &mut s.exclude_query,
                |s: &mut Two, id| s.exclude.push(id),
                |s: &mut Two, open| s.exclude_open = open,
                |s: &mut Two, index| s.exclude_active = index,
            );
        rune_kit::root(rune::col().child(include).child(exclude), overlays)
    }

    #[test]
    fn two_pickers_on_one_pane_do_not_share_keys() {
        let mut rt = Runtime::new(
            Two {
                include: Vec::new(),
                exclude: Vec::new(),
                include_query: TextEdit::new(""),
                exclude_query: TextEdit::new(""),
                include_open: false,
                exclude_open: false,
                include_active: 0,
                exclude_active: 0,
                apps: vec![app("Mail", "com.apple.mail")],
                icons: IconCache::new(Arc::new(|| {})),
            },
            two_view,
            (480.0, 360.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.frame(0.0);
        let add = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("include-add"))
            .unwrap();
        let (x, y) = (
            add.rect[0] + add.rect[2] / 2.0,
            add.rect[1] + add.rect[3] / 2.0,
        );
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.settle_all();
        rt.frame(0.016);
        rt.frame(0.0);
        assert!(rt
            .elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("include-list")));
        assert!(rt
            .elements()
            .iter()
            .all(|e| e.key.as_deref() != Some("exclude-list")));
        assert!(rt
            .elements()
            .iter()
            .all(|e| { e.key.as_deref().is_none_or(|key| !key.contains("#dup")) }));
        let mail = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("include-row-com.apple.mail"))
            .unwrap();
        let (x, y) = (mail.rect[0] + mail.rect[2] / 2.0, mail.rect[1] + 4.0);
        assert!(
            rt.contains("include-row-com.apple.mail", x, y),
            "row {:?} is not drawn at the click",
            mail.rect
        );
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.settle_all();
        rt.frame(0.016);
        rt.frame(0.0);
        assert_eq!(rt.state.include, ["com.apple.mail"]);
        assert!(rt.state.include_open);
        assert!(rt.state.exclude.is_empty());
    }

    fn scrolled_view(s: &PickerState, cx: &mut Cx<PickerState>) -> El {
        let mut overlays = Vec::new();
        let picker = AppPicker::new("app-picker", &s.value, &s.apps, &s.query, &s.icons)
            .open(s.open)
            .active(s.active)
            .overlays(&mut overlays)
            .build(
                Theme::light(),
                cx,
                |s: &mut PickerState| &mut s.query,
                |s: &mut PickerState, id| s.value.push(id),
                |s: &mut PickerState, open| s.open = open,
                |s: &mut PickerState, index| s.active = index,
            );
        let base = rune::col()
            .w_full()
            .h_full()
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(el().h(800.0))
            .child(picker);
        rune_kit::root(base, overlays)
    }

    #[test]
    fn a_picker_in_a_scrolled_container_stays_by_its_button() {
        let mut rt = Runtime::new(
            PickerState {
                value: Vec::new(),
                apps: vec![app("Mail", "com.apple.mail")],
                query: TextEdit::new(""),
                open: true,
                active: 0,
                icons: IconCache::new(Arc::new(|| {})),
            },
            scrolled_view,
            (400.0, 400.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.update(|_, tx| tx.scroll_into_view("app-picker-add"));
        rt.frame(0.0);
        rt.frame(0.0);
        let add = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("app-picker-add"))
            .unwrap()
            .rect;
        let list = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("app-picker-list"))
            .unwrap()
            .rect;
        let below = (list[1] - (add[1] + add[3])).abs() <= 16.0;
        let above = ((list[1] + list[3]) - add[1]).abs() <= 16.0;
        assert!(below || above, "list {list:?} is not next to {add:?}");
        assert!(
            list[0] >= -0.5
                && list[1] >= -0.5
                && list[0] + list[2] <= 400.5
                && list[1] + list[3] <= 400.5,
            "list {list:?} leaves the window"
        );
    }
}
