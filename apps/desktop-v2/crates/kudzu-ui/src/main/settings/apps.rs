//! App exclusions and insertion overrides. One picker is mounted at a time. Its id
//! keeps the keys off any other picker on the pane.
use super::update;
use crate::{
    shell::MainState,
    theme::Theme,
    widgets::{
        app_icon::{AppIcon, IconCache},
        app_picker::{self, AppPicker},
        kit_ext::button::{Button, Size, Variant},
    },
};
use kudzu_core::library::model::InsertionMethod;
use rune::{col, row, text, Cx, El};
use std::sync::Arc;
pub fn choose(s: &mut MainState, id: String) {
    if s.settings.picker_deny {
        update(s, |p| {
            if let Some(i) = p.denied_bundle_ids.iter().position(|x| *x == id) {
                p.denied_bundle_ids.remove(i);
            } else {
                p.denied_bundle_ids.push(id);
            }
        });
    } else {
        update(s, |p| {
            p.app_insertions.entry(id).or_insert(InsertionMethod::Paste);
        });
    }
}
pub fn view(s: &MainState, t: &Theme) -> El {
    let prefs = s.svc.settings.watch().get();
    let apps = s.svc.apps.installed().get();
    let name = |id: &str| {
        apps.iter()
            .find(|a| a.bundle_id == id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| id.into())
    };
    let mut excluded = row().w_full().gap(6.0).flex_wrap().py(12.0);
    for id in &prefs.denied_bundle_ids {
        let id = id.clone();
        excluded = excluded.child(
            Button::new(format!("{} ×", name(&id)))
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .on_press(move |s: &mut MainState| {
                    update(s, |p| p.denied_bundle_ids.retain(|v| v != &id))
                })
                .build(t),
        );
    }
    excluded = excluded.child(
        Button::new("Exclude an app")
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .on_press(|s: &mut MainState| {
                s.settings.picker_deny = true;
                s.settings.picker_open = true;
            })
            .build(t)
            .key("settings-exclude-app"),
    );
    let mut entries: Vec<_> = prefs.app_insertions.iter().collect();
    entries.sort_by_key(|(id, _)| (name(id).to_lowercase(), (*id).clone()));
    let mut rows = Vec::new();
    for (id, method) in entries {
        let remove = id.clone();
        rows.push(
            row()
                .w_full()
                .py(10.0)
                .gap(12.0)
                .items_center()
                .child(
                    AppIcon::new(name(id))
                        .size(22.0)
                        .image(
                            s.settings
                                .icons
                                .as_ref()
                                .and_then(|cache| cache.image(id, 22)),
                        )
                        .build(t),
                )
                .child(
                    text(name(id))
                        .font(rune::TextStyle::new(13.0, 400.0).lh(20.0))
                        .color(t.c.foreground)
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(super::expansion::methods(
                    t,
                    &format!("settings-app-{id}"),
                    *method,
                    Some(id.clone()),
                ))
                .child(
                    Button::new(format!("Remove {}", name(id)))
                        .size(Size::IconXs)
                        .icon(&crate::theme::icons::X)
                        .on_press(move |s: &mut MainState| {
                            update(s, |p| {
                                p.app_insertions.remove(&remove);
                            })
                        })
                        .build(t)
                        .key(format!("settings-app-remove-{id}")),
                ),
        );
    }
    rows.push(
        col().w_full().py(12.0).child(
            Button::new("Add an app")
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .on_press(|s: &mut MainState| {
                    s.settings.picker_deny = false;
                    s.settings.picker_open = true;
                    s.settings.query = rune::TextEdit::new("");
                })
                .build(t)
                .key("settings-add-app"),
        ),
    );
    col().w_full().gap(32.0)
    .child(super::described(t,"Never expand in","Password managers are excluded by default. Snippet and group rules can't override this list.",vec![excluded]))
    .child(super::described(t,"How to insert, per app","Terminals work best with paste. Snippets that choose their own method win.",rows))
}

pub fn view_with_cx(s: &MainState, cx: &Cx<MainState>, t: &Theme, overlays: &mut Vec<El>) -> El {
    let body = view(s, t);
    if !s.settings.picker_open {
        return body;
    }
    let Some(icons) = &s.settings.icons else {
        return body;
    };
    let prefs = s.svc.settings.watch().get();
    let installed = s.svc.apps.installed().get();
    let values = if s.settings.picker_deny {
        prefs.denied_bundle_ids.clone()
    } else {
        Vec::new()
    };
    let (id, label) = if s.settings.picker_deny {
        ("settings-exclude", "Exclude an app")
    } else {
        ("settings-include", "Add an app")
    };
    let picker = AppPicker::new(id, &values, &installed, &s.settings.query, icons)
        .add_label(label)
        .open(true)
        .active(s.settings.picker_active)
        .overlays(overlays)
        .build(
            t,
            cx,
            |s: &mut MainState| &mut s.settings.query,
            choose,
            |s: &mut MainState, open| {
                if !s.settings.is_composing() {
                    s.settings.picker_open = open;
                }
            },
            |s: &mut MainState, index| s.settings.picker_active = index,
        );
    col()
        .w_full()
        .gap(12.0)
        .child(body)
        .child(col().key("settings-app-picker").w_full().child(picker))
}
pub fn sync(s: &mut MainState) -> bool {
    if s.settings.icons.is_none() {
        let main = s.shell.handles().main.clone();
        s.settings.icons = Some(IconCache::new(Arc::new(move || main.wake())));
    }
    let icons = s.settings.icons.as_mut().unwrap();
    let changed = icons.poll();
    if s.frame.view == crate::main::MainView::Settings(crate::shell::SettingsPane::Apps) {
        for id in s.svc.settings.watch().get().app_insertions.keys() {
            icons.request(&s.svc.apps, id, 22);
        }
    }
    if s.settings.picker_open {
        let values = s.svc.settings.watch().get().denied_bundle_ids.clone();
        let installed = s.svc.apps.installed().get();
        for (id, size) in
            app_picker::icons_needed(&values, &installed, &s.settings.query.text, true)
        {
            icons.request(&s.svc.apps, &id, size);
        }
    }
    changed
}
