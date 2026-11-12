//! The open app list: search, scrolling rows, and the scrim around the chip row.
//!
//! RUNE-GAP(none; plan gap S55-escape): Escape in the search field is consumed here. A focused
//! field never reaches an ancestor `on_key`, and the editor treats Escape as blur.
//! RUNE-GAP(none; plan gap S55-scrim): one scrim cannot leave the chip row clickable, so the
//! press catcher is split above and below that row.

use kudzu_core::system::AppInfo;
use rune::{col, el, icon, text, Cx, El, Key, KeyEvent, TextEdit, Tx};

use super::{custom_id, matches, AppPicker, ICON_PX, LIST_W, ROW_ICON};
use crate::theme::{icons, radius, Theme};
use crate::widgets::app_icon::AppIcon;
use crate::widgets::kit_ext::input::Input;

/// Closures the list uses to edit the query, toggle an id, and move the active row.
pub(super) struct Bind<Edit, Toggle, Open, Active> {
    edit: Edit,
    toggle: Toggle,
    set_open: Open,
    set_active: Active,
}

impl<Edit, Toggle, Open, Active> Bind<Edit, Toggle, Open, Active> {
    pub(super) fn new(edit: Edit, toggle: Toggle, set_open: Open, set_active: Active) -> Self {
        Self {
            edit,
            toggle,
            set_open,
            set_active,
        }
    }
}

pub(super) fn mount<S, Edit, Toggle, Open, Active>(
    picker: &AppPicker<'_>,
    theme: &Theme,
    cx: &Cx<S>,
    bind: Bind<Edit, Toggle, Open, Active>,
    overlays: &mut Vec<El>,
) where
    S: 'static,
    Edit: Fn(&mut S) -> &mut TextEdit + Clone + 'static,
    Toggle: Fn(&mut S, String) + Clone + 'static,
    Open: Fn(&mut S, bool) + Clone + 'static,
    Active: Fn(&mut S, usize) + Clone + 'static,
{
    let origin = picker.key("origin");
    overlays.push(
        el().key(origin.clone())
            .absolute(0.0, 0.0)
            .size(0.0, 0.0)
            .a11y_hidden(),
    );
    if !picker.open {
        return;
    }
    let set_open = bind.set_open.clone();
    push_scrim(
        overlays,
        picker,
        cx.window_layout(&origin),
        cx.window_layout(&picker.key("")),
        set_open,
    );
    overlays.push(list(picker, theme, bind));
}

fn list<S, Edit, Toggle, Open, Active>(
    picker: &AppPicker<'_>,
    theme: &Theme,
    bind: Bind<Edit, Toggle, Open, Active>,
) -> El
where
    S: 'static,
    Edit: Fn(&mut S) -> &mut TextEdit + Clone + 'static,
    Toggle: Fn(&mut S, String) + Clone + 'static,
    Open: Fn(&mut S, bool) + Clone + 'static,
    Active: Fn(&mut S, usize) + Clone + 'static,
{
    let c = &theme.c;
    let query = picker.query.text.clone();
    let shown: Vec<AppInfo> = picker
        .apps
        .iter()
        .filter(|app| matches(&query, app))
        .cloned()
        .collect();
    let custom = custom_id(&query, picker.apps).map(str::to_owned);
    let mut row_keys = Vec::new();
    let mut row_ids: Vec<Option<String>> = Vec::new();
    let mut rows = col().w_full();
    if shown.is_empty() {
        if let Some(id) = &custom {
            row_keys.push(picker.key("custom"));
            row_ids.push(Some(id.clone()));
            rows = rows.child(custom_row(
                picker,
                theme,
                id,
                bind.edit.clone(),
                bind.toggle.clone(),
                bind.set_open.clone(),
            ));
        } else {
            rows = rows.child(
                el().padding(8.0).child(
                    text("No apps found.")
                        .font(theme.ty.input)
                        .color(c.muted_foreground),
                ),
            );
        }
    }
    for app in &shown {
        row_keys.push(picker.key(&format!("row-{}", app.bundle_id)));
        row_ids.push(Some(app.bundle_id.clone()));
        rows = rows.child(app_row(picker, theme, app, bind.toggle.clone()));
    }
    let n = row_keys.len();
    let active = if n == 0 { 0 } else { picker.active.min(n - 1) };
    let descendant = row_keys.get(active).cloned();
    let Bind {
        edit,
        toggle,
        set_open,
        set_active,
    } = bind;
    let edit_for_key = edit.clone();
    let active_for_change = set_active.clone();
    let custom_row_only = shown.is_empty() && custom.is_some();
    let already = custom
        .as_ref()
        .is_some_and(|id| picker.value.iter().any(|v| v == id));
    let field = Input::new(picker.query)
        .with(move |input| {
            let edit_for_field = edit.clone();
            let keys = row_keys.clone();
            let ids = row_ids.clone();
            let toggle = toggle.clone();
            let set_open = set_open.clone();
            let set_active = set_active.clone();
            let edit_for_choose = edit_for_key.clone();
            input
                .label("Search apps or type a bundle ID…")
                .placeholder("Search apps or type a bundle ID…")
                .width(Some(LIST_W))
                .active_descendant(descendant.clone())
                .on_edit(edit_for_field)
                .on_change(move |s: &mut S, _| active_for_change(s, 0))
                .on_key(move |s: &mut S, ev, tx| {
                    on_search_key(
                        s,
                        ev,
                        tx,
                        &Search {
                            edit: &edit_for_choose,
                            toggle: &toggle,
                            set_open: &set_open,
                            set_active: &set_active,
                            row_keys: &keys,
                            row_ids: &ids,
                            custom_only: custom_row_only,
                            already,
                            active,
                        },
                    )
                })
        })
        .build(theme)
        .key(picker.key("search"));
    let mut surface = col()
        .key(picker.key("list"))
        .role("listbox")
        .label("Apps")
        .anchored_fit(
            picker.key("add"),
            rune::Side::Bottom,
            rune::Align::Start,
            4.0,
        )
        .w(LIST_W)
        // `anchored_fit` flips to the side that fits. A search field plus a 288 pt row
        // viewport fits above the add button on the group dialog and covers the name.
        // 256 pt stays under that button; a shorter window still shrinks or flips it
        // inside the viewport.
        .max_h(256.0)
        .min_h(0.0)
        .shrink()
        .radius(radius::MD)
        .fill(c.popover)
        .child(field)
        .child(
            rune_kit::popover::fit_scroll(rows)
                .key(picker.key("rows"))
                .max_h(288.0),
        );
    for shadow in &theme.depth.float {
        surface = surface.box_shadow(*shadow);
    }
    surface
}

struct Search<'a, Edit, Toggle, Open, Active> {
    edit: &'a Edit,
    toggle: &'a Toggle,
    set_open: &'a Open,
    set_active: &'a Active,
    row_keys: &'a [String],
    row_ids: &'a [Option<String>],
    custom_only: bool,
    already: bool,
    active: usize,
}

fn on_search_key<S, Edit, Toggle, Open, Active>(
    s: &mut S,
    ev: &KeyEvent,
    tx: &mut Tx,
    search: &Search<'_, Edit, Toggle, Open, Active>,
) -> bool
where
    Edit: Fn(&mut S) -> &mut TextEdit,
    Toggle: Fn(&mut S, String),
    Open: Fn(&mut S, bool),
    Active: Fn(&mut S, usize),
{
    let field = (search.edit)(s);
    if field.preedit().is_some() || field.is_composing() {
        return false;
    }
    if bare(ev) && ev.key == Key::Escape {
        (search.set_open)(s, false);
        return true;
    }
    let n = search.row_keys.len();
    if ev.key == Key::Enter {
        if n > 0 {
            choose(
                s,
                search.edit,
                search.toggle,
                search.active.min(n - 1),
                search.row_ids,
                search.custom_only,
                search.already,
            );
        }
        return true;
    }
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || n == 0 {
        return false;
    }
    let index = search.active.min(n - 1);
    let next = match ev.key {
        Key::Home => Some(0),
        Key::End => Some(n - 1),
        Key::Down if !ev.mods.shift => Some((index as isize + 1).rem_euclid(n as isize) as usize),
        Key::Up if !ev.mods.shift => Some((index as isize - 1).rem_euclid(n as isize) as usize),
        _ => None,
    };
    if let Some(next) = next {
        (search.set_active)(s, next);
        tx.scroll_into_view(search.row_keys[next].clone());
        return true;
    }
    false
}

fn choose<S>(
    s: &mut S,
    edit: &impl Fn(&mut S) -> &mut TextEdit,
    toggle: &impl Fn(&mut S, String),
    index: usize,
    row_ids: &[Option<String>],
    custom_only: bool,
    already: bool,
) {
    let Some(id) = row_ids.get(index).cloned().flatten() else {
        return;
    };
    if custom_only {
        if !already {
            toggle(s, id);
        }
        edit(s).clear();
        return;
    }
    toggle(s, id);
}

fn custom_row<S: 'static>(
    picker: &AppPicker<'_>,
    theme: &Theme,
    id: &str,
    edit: impl Fn(&mut S) -> &mut TextEdit + 'static,
    toggle: impl Fn(&mut S, String) + 'static,
    set_open: impl Fn(&mut S, bool) + 'static,
) -> El {
    let typed = id.to_owned();
    let already = picker.value.iter().any(|v| v == id);
    let label = format!("Add “{id}”");
    el().key(picker.key("custom"))
        .w_full()
        .padding(8.0)
        .on_press(move |s: &mut S| {
            if !already {
                toggle(s, typed.clone());
            }
            edit(s).clear();
            set_open(s, true);
        })
        .role("button")
        .label(label.clone())
        .child(text(label).font(theme.ty.input).color(theme.c.foreground))
}

fn app_row<S: 'static>(
    picker: &AppPicker<'_>,
    theme: &Theme,
    app: &AppInfo,
    toggle: impl Fn(&mut S, String) + 'static,
) -> El {
    let c = &theme.c;
    let chosen = picker.value.iter().any(|id| id == &app.bundle_id);
    let id = app.bundle_id.clone();
    let active = picker
        .apps
        .iter()
        .filter(|app| matches(&picker.query.text, app))
        .nth(picker.active)
        .is_some_and(|hit| hit.bundle_id == app.bundle_id);
    let mut item = rune::row()
        .key(picker.key(&format!("row-{id}")))
        .w_full()
        .h(32.0)
        .px(8.0)
        .gap(8.0)
        .items_center()
        .hover(c.muted)
        .on_press(move |s: &mut S| toggle(s, id.clone()))
        .role("option")
        .label(app.name.clone())
        .checked(chosen)
        .child(
            AppIcon::new(app.name.clone())
                .size(ROW_ICON)
                .image(picker.icons.image(&app.bundle_id, ICON_PX))
                .build(theme),
        )
        .child(
            text(app.name.clone())
                .font(theme.ty.input)
                .color(c.foreground)
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .truncate(),
        );
    if active {
        item = item.fill(c.muted);
    }
    if chosen {
        item = item.child(
            icon(&icons::CHECK, 14.0)
                .key(picker.key(&format!("check-{}", app.bundle_id)))
                .color(c.iris),
        );
    }
    item
}

/// Close the list on a press outside it, leaving the chip row uncovered so Remove still hits.
fn push_scrim<S: 'static>(
    overlays: &mut Vec<El>,
    picker: &AppPicker<'_>,
    origin: Option<[f32; 4]>,
    row: Option<[f32; 4]>,
    set_open: impl Fn(&mut S, bool) + Clone + 'static,
) {
    let base = origin.map(|r| [r[0], r[1]]).unwrap_or([0.0, 0.0]);
    let Some(row) = row else {
        let close = set_open;
        overlays.push(
            el().key(picker.key("scrim-top"))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .on_press(move |s: &mut S| close(s, false)),
        );
        return;
    };
    let top = row[1] - base[1];
    let bottom = top + row[3];
    if top > 0.0 {
        let close = set_open.clone();
        overlays.push(
            el().key(picker.key("scrim-top"))
                .absolute(0.0, 0.0)
                .w_full()
                .h(top)
                .on_press(move |s: &mut S| close(s, false)),
        );
    }
    overlays.push(
        el().key(picker.key("scrim-bottom"))
            .pin(Some(0.0), Some(bottom), Some(0.0), Some(0.0))
            .on_press(move |s: &mut S| set_open(s, false)),
    );
}

fn bare(ev: &KeyEvent) -> bool {
    !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift
}
