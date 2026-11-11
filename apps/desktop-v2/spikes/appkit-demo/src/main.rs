//! RUNE-GAP(#141, #561, #562, #563, #146, #574): delete hooks with their native adapters.
mod native;
use kudzu_appkit::{appearance::Preference, geometry::Rect, status_item::Shortcut};
use rune::{
    col, el, row, text, text_input, App, AppHandle, AppSpec, Color, Cx, El, Key, KeyEvent,
    RunOptions, TextEdit, TextStyle, Tx, WindowHandle, WindowOptions,
};
use rune_kit::{shell::PaneState, Kit};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

thread_local! { static WINDOW: RefCell<Option<WindowHandle>> = const { RefCell::new(None) }; }

pub struct State {
    main: AppHandle<State>,
    panel: AppHandle<Panel>,
    sidebar: PaneState,
    preference: Preference,
    field: TextEdit,
    icon: Rect,
    clock: Instant,
    paused: bool,
    icon_pixels: rune::Pixels,
    bundle: Option<std::path::PathBuf>,
}

pub struct Panel {
    main: AppHandle<State>,
    own: AppHandle<Panel>,
    field: TextEdit,
    preference: Preference,
}

fn main() {
    if !std::env::args().any(|a| a == "--live") {
        println!("AppKit demo: --live [--bundle /path/to/Kudzu Next.app]. No windows opened.");
        return;
    }
    rune::set_app_id("sh.saeed.kudzu.next");
    let panel_app = App::new(AppSpec {
        on_key: Some(panel_key),
        ..AppSpec::new(
            "Kudzu Next panel",
            (500., 220.),
            || unreachable!(),
            panel_view,
        )
    })
    .windowless();
    let panel = panel_app.handle();
    let main_app = App::new(AppSpec {
        poll: Some(poll),
        min_size: Some((420., 400.)),
        ..AppSpec::new(
            "Kudzu Next AppKit demo",
            (1180., 760.),
            || unreachable!(),
            view,
        )
    })
    .windowless()
    .keep_running()
    .standard_menus()
    .chrome(rune::Chrome {
        full_size_content: true,
        traffic_lights: Some((18., 22.)),
        drag_regions: true,
    })
    .on_window(|window| WINDOW.with(|slot| *slot.borrow_mut() = Some(window.clone())))
    .on_quit(|_, _| native::clear())
    .on_reopen(|state: &mut State, _, _| {
        promote(state);
        true
    });
    let main = main_app.handle();
    let panel_app = panel_app.init({
        let main = main.clone();
        let own = panel.clone();
        move || Panel {
            main,
            own,
            field: focused(),
            preference: Preference::Dark,
        }
    });
    let main_app = main_app.init({
        let main = main.clone();
        let panel = panel.clone();
        move || State {
            main,
            panel,
            sidebar: PaneState::new(224.),
            preference: Preference::Dark,
            field: focused(),
            icon: Rect::default(),
            clock: Instant::now(),
            paused: false,
            icon_pixels: icon_pixels(),
            bundle: std::env::args()
                .skip_while(|s| s != "--bundle")
                .nth(1)
                .map(std::path::PathBuf::from),
        }
    });
    // First update installs ⌘Q while Rune's accessory app menu is still empty.
    main.update(|s, _| native::install(s.main.clone(), s.panel.clone()));
    rune::prewarm_gpu();
    rune::run_windows_with_args(
        RunOptions { accessory: true },
        std::iter::empty::<String>(),
        move |_| {
            vec![
                kudzu_ui::theme::window::spec("main", main_app)
                    .options(WindowOptions {
                        transparent: true,
                        ..WindowOptions::default()
                    })
                    .boxed(),
                kudzu_ui::theme::window::spec("panel", panel_app)
                    .configured_options(WindowOptions::launcher_panel())
                    .boxed(),
            ]
        },
    );
    native::clear();
}

fn focused() -> TextEdit {
    let mut field = TextEdit::new("Edit shortcuts work here");
    field.focused = true;
    field
}

fn icon_pixels() -> rune::Pixels {
    let image = image::load_from_memory(include_bytes!("../../../design/icon.png"))
        .expect("committed app icon")
        .to_rgba8();
    let pixels = rune::Pixels::new();
    pixels.set(image.width(), image.height(), image.into_raw());
    pixels
}

/// Called INSIDE a Rune update; nested patch runs after this turn's Dock request.
pub fn promote(state: &mut State) {
    promotion_turn(
        || {
            state.main.set_dock_icon(true);
            state.main.open();
        },
        || state.main.update(|_, _| native::patch()),
    );
}

fn promotion_turn(request: impl FnOnce(), queue_patch: impl FnOnce()) {
    request();
    queue_patch();
}

pub fn close(state: &mut State) {
    native::stop_guide();
    state.main.hide();
    state.panel.hide();
    state.main.set_dock_icon(false);
}

pub fn set_preference(state: &mut State, preference: Preference) {
    state.preference = preference;
    native::appearance(preference);
    state.panel.update(move |panel, tx| {
        panel.preference = preference;
        tx.instant();
    });
}

fn panel_key(state: &mut Panel, event: &KeyEvent, _: &mut Tx) -> bool {
    let shortcut = Shortcut {
        key: if let Key::Command(c) = &event.key {
            c.to_string()
        } else {
            String::new()
        },
        command: event.mods.cmd,
        shift: event.mods.shift,
        option: event.mods.alt,
        control: event.mods.ctrl,
    };
    if shortcut.closes_windows() {
        state.main.update(|s, _| close(s));
        true
    } else if event.key == Key::Escape {
        state.own.hide();
        true
    } else {
        false
    }
}

fn panel_view(state: &Panel, cx: &mut Cx<Panel>) -> El {
    let dark = state.preference.resolve_dark(|| cx.system_dark());
    let background = if dark { Color::BLACK } else { Color::WHITE };
    let foreground = if dark { Color::WHITE } else { Color::BLACK };
    col()
        .w_full()
        .h_full()
        .padding(24.)
        .fill(background)
        .child(text("Launcher: ⌘Q closes all windows").color(foreground))
        .child(
            text_input(&state.field, TextStyle::new(16., 400.))
                .color(foreground)
                .on_edit(|s: &mut Panel| &mut s.field),
        )
}

fn poll(state: &mut State, _: f64, tx: &mut Tx) -> bool {
    if let Some(delay) = native::poll(state) {
        tx.wake_after(delay);
    }
    false
}

fn button(label: &str, run: impl Fn(&mut State, &mut Tx) + 'static) -> El {
    text(label.to_owned()).padding(8.).on_press_tx(run)
}

fn view(state: &State, cx: &mut Cx<State>) -> El {
    let dark = state.preference.resolve_dark(|| cx.system_dark());
    let mut kit = Kit::neutral(dark);
    kit.c.bg_deep = Color::TRANSPARENT;
    let sidebar = el()
        .key("sidebar-slot")
        .w(if state.sidebar.is_open() { 224. } else { 0. })
        .h_full()
        .motion(rune::Spring::new(0.24, 0.))
        .fill(if dark {
            rune::oklcha(0.216, 0.006, 56.043, 0.55)
        } else {
            rune::oklcha(0.985, 0.001, 106.423, 0.72)
        })
        .child(text("Native Sidebar").padding(16.))
        .on_drawn_layout_tx(|_: &mut State, rect, tx| {
            native::sync_sidebar(rect);
            tx.live_only();
        });
    let content = col()
        .grow(1.)
        .h_full()
        .padding(24.)
        .gap(6.)
        .fill(kit.c.bg_base)
        .color(kit.c.text_base)
        .child(text("Kudzu Next · AppKit gap checks"))
        .child(text(format!(
            "Toolbar inset: {} pt (observe full-screen transitions)",
            cx.titlebar_inset()
        )))
        .child(
            text_input(&state.field, TextStyle::new(16., 400.))
                .on_edit(|s: &mut State| &mut s.field),
        )
        .child(button("Light → Dark → System", |s, tx| {
            let preference = match s.preference {
                Preference::Light => Preference::Dark,
                Preference::Dark => Preference::System,
                Preference::System => Preference::Light,
            };
            set_preference(s, preference);
            tx.instant();
        }))
        .child(button("Toggle sidebar", |s, _| {
            s.sidebar.set_open(!s.sidebar.is_open())
        }))
        .child(button("Rewrap content view", |_, _| {
            native::rewrap_content()
        }))
        .child(button("Read screens + Settings metadata", |_, _| {
            native::inspect()
        }))
        .child(button(
            "Start guide (Settings must already be open)",
            |s, tx| {
                native::begin_guide(s);
                tx.wake_after(Duration::from_millis(450));
            },
        ))
        .child(button(
            "Stop guide / simulate grant or real drag",
            |_, _| native::stop_guide(),
        ))
        .child(
            el().size(64., 64.)
                .pixels(&state.icon_pixels)
                .drag_out(|s: &State| {
                    s.bundle
                        .as_ref()
                        .filter(|p| p.extension().is_some_and(|e| e == "app"))
                        .map(|p| rune::DragItem::new().file(p))
                })
                .on_drag_start(|_: &mut State, _| native::stop_guide())
                .on_drawn_layout_tx(|s: &mut State, r, tx| {
                    s.icon = native::rect(r);
                    tx.live_only();
                }),
        );
    col()
        .w_full()
        .h_full()
        .child(
            row()
                .w_full()
                .h(48.)
                .pl(cx.titlebar_inset() + 12.)
                .center_y()
                .window_drag()
                .child(text("S38 native adapter demo")),
        )
        .child(
            row()
                .w_full()
                .grow(1.)
                .min_h(0.)
                .child(sidebar)
                .child(content),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn promotion_queues_the_patch_after_the_dock_request() {
        use std::cell::Cell;
        let dock = Cell::new(false);
        let requested = Cell::new(false);
        let patched_regular_menu = Cell::new(None);
        let mut next_turn: Option<Box<dyn FnOnce()>> = None;
        promotion_turn(
            || requested.set(true),
            || {
                next_turn = Some(Box::new(|| patched_regular_menu.set(Some(dock.get()))));
            },
        );
        assert_eq!(patched_regular_menu.get(), None);
        // Rune Shared::drain applies the request before draining these nested updates.
        dock.set(requested.get());
        next_turn.take().unwrap()();
        assert_eq!(patched_regular_menu.get(), Some(true));
    }
}
