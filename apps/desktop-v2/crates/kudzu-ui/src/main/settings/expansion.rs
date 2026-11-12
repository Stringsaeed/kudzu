//! Expansion preferences. Pause flips the service source of truth atomically.
use super::{group, setting, update};
use crate::{
    shell::MainState,
    theme::Theme,
    widgets::kit_ext::{
        button::{Button, Size, Variant},
        input::Input,
        switch::Switch,
    },
};
use kudzu_core::library::model::InsertionMethod;
use rune::{col, row, text, El};
pub fn limited_prefix(value: &str) -> String {
    let mut units = 0;
    value
        .chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= 4
        })
        .collect()
}
pub fn view(s: &MainState, t: &Theme) -> El {
    let prefs = s.svc.settings.watch().get();
    col().w_full().gap(32.0).child(group(t,"Expanding",vec![
 setting(t,"Pause all snippets","Handy while gaming or presenting. Command Shift P also works.",Switch::new(prefs.paused).label("Pause all snippets").on_toggle(|s:&mut MainState|{if let Err(e)=s.svc.expansion.toggle_paused(){s.settings.error=Some(e.to_string());}}).build(t).key("settings-pause")),
 setting(t,"Trigger prefix","Suggested at the start of new triggers so they don't fire mid-sentence.",Input::new(&s.settings.prefix).with(|i|i.label("Trigger prefix").width(Some(80.0)).on_edit(|s:&mut MainState|&mut s.settings.prefix)).build(t).key("settings-prefix")),
 setting(t,"Insert by","Default for every snippet. Typing is most compatible; pasting is fastest for long text.",methods(t,"settings-insertion",prefs.default_insertion,None))]))
 .child(group(t,"Good to know",vec![col().w_full().py(12.0).gap(8.0)
 .child(info(t,"Undo an expansion — press Backspace right after it to get your trigger back."))
 .child(info(t,"Clipboard is safe — when Kudzu pastes, it restores whatever you had copied, and marks its own text so clipboard managers ignore it."))
 .child(info(t,"Secure Input — while a password field is focused, macOS blocks all text expanders. Kudzu shows this in the menu bar."))]))
}
fn info(t: &Theme, value: &str) -> El {
    text(value.to_owned())
        .font(rune::TextStyle::new(12.0, 400.0).lh(19.5))
        .color(t.c.muted_foreground)
        .wrap()
}
pub fn methods(t: &Theme, id: &str, current: InsertionMethod, app: Option<String>) -> El {
    let mut options = row().gap(4.0).role("radiogroup").label("Insertion method");
    for (value, label, key) in [
        (
            InsertionMethod::Automatic,
            if app.is_some() { "Auto" } else { "Automatic" },
            "automatic",
        ),
        (InsertionMethod::Type, "Type", "type"),
        (InsertionMethod::Paste, "Paste", "paste"),
    ] {
        let app = app.clone();
        options = options.child(
            Button::new(label)
                .variant(if value == current {
                    Variant::Secondary
                } else {
                    Variant::Ghost
                })
                .size(Size::Sm)
                .on_press(move |s: &mut MainState| {
                    update(s, |p| {
                        if let Some(id) = &app {
                            p.app_insertions.insert(id.clone(), value);
                        } else {
                            p.default_insertion = value;
                        }
                    })
                })
                .build(t)
                .key(format!("{id}-{key}"))
                .role("radio")
                .checked(current == value),
        );
    }
    options
}
