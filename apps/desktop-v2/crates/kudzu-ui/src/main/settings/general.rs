//! General preferences and the main-thread shortcut recorder.
use super::{group, setting, update};
use crate::{
    shell::MainState,
    theme::Theme,
    widgets::kit_ext::{
        button::{Button, Size},
        switch::Switch,
    },
};
use kudzu_app::{ports::ShortcutSlot, settings::Theme as Preference};
use rune::{col, row, text, El, Key, KeyEvent};
pub fn view(s: &MainState, t: &Theme) -> El {
    let prefs = s.svc.settings.watch().get();
    let status = s.svc.expansion.status().get();
    let ready = kudzu_app::permissions::ready(&status);
    let title = if ready {
        "Permissions are set"
    } else {
        "Enable typing in other apps"
    };
    let hint = if ready {
        if status.can_expand {
            "Kudzu is ready to expand your snippets."
        } else {
            &status.summary
        }
    } else {
        "A guided setup, with every step beside you."
    };
    let mut result=col().w_full().gap(32.0)
 .child(group(t,"Permissions",vec![col().w_full().padding(16.0).gap(16.0)
 .child(text(title).font(t.ty.body_strong).color(t.c.foreground))
 .child(text(hint.to_owned()).font(t.ty.body).color(t.c.muted_foreground).wrap())
 .child(text(format!("{} Accessibility · {} Input Monitoring",if status.permissions.accessibility {"✓"}else{"○"},if status.permissions.input_monitoring {"✓"}else{"○"})).font(t.ty.body).color(t.c.muted_foreground))
 .child(Button::new(if ready {"Review permissions"}else{"Set up permissions"}).on_press(|s:&mut MainState|s.shell.open_permissions()).build(t).key("settings-permissions"))]))
 .child(group(t,"Quick search",vec![setting(t,"Shortcut","Opens the search tray over any app. Return inserts, Command Return copies.",
 Button::new(if s.settings.recording {"Press shortcut…".into()}else{kudzu_core::keys::accelerator_labels(&prefs.palette_hotkey).join(" ")})
 .size(Size::Sm).on_press(|s:&mut MainState|{s.settings.recording=true;s.settings.error=None;})
 .build(t).key("settings-shortcut").on_key(|s:&mut MainState,e,_| record_key(s,e)))]))
 .child(group(t,"iCloud",vec![setting(t,"Sync with iCloud","The same snippets on every Mac signed in to your iCloud. Stored in your private iCloud database; Kudzu has no servers of its own.",
 Switch::new(prefs.icloud_sync).label("Sync with iCloud").on_toggle(|s:&mut MainState| update(s,|p|p.icloud_sync=!p.icloud_sync)).build(t).key("settings-icloud")),
 col().key("settings-sync-status").w_full()]))
 .child(group(t,"App",vec![setting(t,"Appearance","",appearance(t,prefs.theme)),
 setting(t,"Open at login","Kudzu lives in the menu bar and starts with your Mac.",Switch::new(prefs.launch_at_login).label("Open at login").on_toggle(|s:&mut MainState|update(s,|p|p.launch_at_login=!p.launch_at_login)).build(t).key("settings-login")),
 setting(t,"Show in Dock while open","Otherwise Kudzu only appears in the menu bar.",Switch::new(prefs.show_in_dock).label("Show in Dock while open").on_toggle(|s:&mut MainState|update(s,|p|p.show_in_dock=!p.show_in_dock)).build(t).key("settings-dock"))]));
    if let Some(error) = &s.model.hotkey_error {
        result = result
            .child(
                text("That shortcut is taken")
                    .font(t.ty.body)
                    .color(t.c.destructive),
            )
            .child(
                text(error.to_string())
                    .font(t.ty.body)
                    .color(t.c.destructive)
                    .wrap(),
            );
    }
    if let Some(error) = &*s.svc.login_notice.get() {
        result = result.child(
            text(error.clone())
                .font(t.ty.body)
                .color(t.c.destructive)
                .wrap(),
        );
    }
    if let Some(error) = &s.settings.error {
        result = result.child(
            text(error.clone())
                .font(t.ty.body)
                .color(t.c.destructive)
                .wrap(),
        );
    }
    result
}
fn appearance(t: &Theme, current: Preference) -> El {
    let mut options = row().gap(4.0).role("radiogroup").label("Appearance");
    for (value, label, id) in [
        (Preference::Dark, "Dark", "dark"),
        (Preference::Light, "Light", "light"),
        (Preference::System, "System", "system"),
    ] {
        options = options.child(
            Button::new(label)
                .size(Size::Sm)
                .on_press(move |s: &mut MainState| update(s, |p| p.theme = value))
                .build(t)
                .key(format!("settings-theme-{id}"))
                .role("radio")
                .selected(value == current),
        );
    }
    options
}
pub fn set_shortcut(s: &mut MainState, chord: &str) {
    s.settings.recording = false;
    s.model.hotkey_error = s
        .svc
        .hotkeys
        .set(ShortcutSlot::QuickSearch, chord, s.shell.registrar())
        .err();
    if s.model.hotkey_error.is_none() {
        s.settings.captured_until =
            Some(s.svc.clock.monotonic() + std::time::Duration::from_millis(300));
    }
}
pub fn record_key(s: &mut MainState, e: &KeyEvent) -> bool {
    if s.settings.is_composing() || !s.settings.recording {
        return false;
    }
    if e.key == Key::Escape {
        s.settings.recording = false;
        return true;
    }
    let code = match &e.key {
        Key::Text(v) if v == " " => "Space".into(),
        Key::Text(v) if v.chars().count() == 1 => {
            let c = v.chars().next().unwrap();
            if c.is_ascii_alphabetic() {
                format!("Key{}", c.to_ascii_uppercase())
            } else if c.is_ascii_digit() {
                format!("Digit{c}")
            } else {
                String::new()
            }
        }
        Key::Command(c) => format!("Key{}", c.to_ascii_uppercase()),
        Key::Enter => "Enter".into(),
        Key::Tab => "Tab".into(),
        Key::Backspace => "Backspace".into(),
        _ => String::new(),
    };
    let event = kudzu_core::keys::KeyEvent {
        key: code.clone(),
        code,
        ctrl: e.mods.ctrl,
        alt: e.mods.alt,
        shift: e.mods.shift,
        meta: e.mods.cmd,
    };
    if let Some(chord) = kudzu_core::keys::accelerator_from_event(&event) {
        set_shortcut(s, &chord);
    }
    true
}
