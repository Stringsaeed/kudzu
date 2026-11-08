//! `Apps` in settings.tsx: apps Kudzu never expands in, and the insertion method per app.

use rune::{row, text, Cx, El};
use serde_json::json;

use kudzu_core::InsertionMethod;

use super::{group, page};
use crate::ui::icons;
use crate::ui::kit::app_icon::app_icon;
use crate::ui::kit::app_picker::{app_name, installed, AppPicker};
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::segmented::{Segmented, SegmentedSize};
use crate::ui::main::insights::whole;
use crate::ui::main::MainUi;
use crate::ui::theme::text as ty;
use crate::ui::theme_for;

fn method_value(m: InsertionMethod) -> &'static str {
    match m {
        InsertionMethod::Automatic => "automatic",
        InsertionMethod::Type => "type",
        InsertionMethod::Paste => "paste",
    }
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings;
    let apps = installed();

    let denied = AppPicker::new("denied-apps", &st.denied_bundle_ids, &s.settings_page.denied, |s: &mut MainUi| &mut page(s).denied)
        .add_label("Exclude an app")
        .on_change(|s: &mut MainUi, ids, _| s.update_settings(json!({ "deniedBundleIDs": ids })))
        .build(cx, layer);
    let never = group(
        th,
        "Never expand in",
        Some("Password managers are excluded by default. Snippet and group rules can't override this list."),
        vec![whole(rune::col()).py(12.0).child(denied)],
    );

    let mut entries: Vec<(&String, &InsertionMethod)> = st.app_insertions.iter().collect();
    entries.sort_by(|a, b| kudzu_core::client::js::locale_compare(&app_name(&apps, a.0), &app_name(&apps, b.0)));
    let mut rows: Vec<El> = Vec::new();
    for (id, method) in entries {
        let name = app_name(&apps, id);
        let path = apps.iter().find(|a| &a.bundle_id == id).and_then(|a| a.path.clone());
        let bundle = id.clone();
        let remove = id.clone();
        rows.push(
            whole(row())
                .key(format!("app-rule-{id}"))
                .center_y()
                .gap(12.0)
                .py(10.0)
                .child(app_icon(th, path.as_deref(), &name, 22.0))
                .child(text(name.clone()).font(ty::BASE).color(t.foreground).truncate().grow(1.0).min_w(0.0).shrink())
                .child(
                    Segmented::new(format!("app-rule-seg-{id}"), format!("Insertion in {name}"))
                        .size(SegmentedSize::Sm)
                        .option("automatic", "Auto")
                        .option("type", "Type")
                        .option("paste", "Paste")
                        .selected(method_value(*method))
                        .on_change(move |s: &mut MainUi, v: &str| {
                            let mut next = s.settings.app_insertions.clone();
                            if let Ok(m) = serde_json::from_value::<InsertionMethod>(json!(v)) {
                                next.insert(bundle.clone(), m);
                            }
                            s.update_settings(json!({ "appInsertions": next }));
                        })
                        .build(th, cx),
                )
                .child(
                    Button::icon_only(&icons::X, format!("Remove {name}"))
                        .variant(Variant::Ghost)
                        .size(Size::IconXs)
                        .id(format!("app-rule-x-{id}"))
                        .on_press(move |s: &mut MainUi| {
                            let mut next = s.settings.app_insertions.clone();
                            next.remove(&remove);
                            s.update_settings(json!({ "appInsertions": next }));
                        })
                        .build(th),
                ),
        );
    }
    let adding = AppPicker::new("add-app-rule", &[], &s.settings_page.adding, |s: &mut MainUi| &mut page(s).adding)
        .add_label("Add an app")
        .on_change(|s: &mut MainUi, ids, _| {
            let mut next = s.settings.app_insertions.clone();
            let mut fresh = false;
            for i in ids {
                if let std::collections::hash_map::Entry::Vacant(e) = next.entry(i) {
                    e.insert(InsertionMethod::Paste);
                    fresh = true;
                }
            }
            if fresh {
                s.update_settings(json!({ "appInsertions": next }));
            }
        })
        .build(cx, layer);
    rows.push(whole(rune::col()).py(12.0).child(adding));
    let per_app = group(th, "How to insert, per app", Some("Terminals work best with paste. Snippets that choose their own method win."), rows);
    vec![never, per_app]
}
