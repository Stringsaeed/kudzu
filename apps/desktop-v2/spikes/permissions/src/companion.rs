//! Borderless always-on-top companion: 420×660, radius 18, floating level, all Spaces.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use rune::{
    col, el, text, App, AppHandle, AppSpec, Color, Cx, DragOutcome, El, MacWindowOptions,
    OverlayHandle, OverlayPlacement, TextStyle, Tx, WindowOptions, WindowSpec,
};

use crate::bundle;
use crate::drag::{self, ICON, ICON_X, ICON_Y};
use crate::guide::{self, Guide};
use crate::native;
use crate::place::{self, Rect};

pub const TITLE: &str = native::COMPANION_TITLE;
pub static READY: AtomicBool = AtomicBool::new(false);

static LAST_END: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub struct Companion {
    pub overlay: OverlayHandle<Guide>,
    pub bundle: Option<PathBuf>,
    pub presses: usize,
    pub drag_ends: Vec<DragOutcome>,
}

pub fn install() -> (
    AppHandle<Companion>,
    WindowSpec<Companion>,
    OverlayHandle<Guide>,
) {
    let mut app = App::new(AppSpec::new(
        TITLE,
        (place::COMPANION_W as f32, place::COMPANION_H as f32),
        || unreachable!(),
        view,
    ))
    .windowless()
    .keep_running();
    let overlay = app.overlay(guide::spec(), Guide::default());
    let handle = app.handle();
    let overlay_for_state = overlay.clone();
    let app = app.init(move || Companion {
        overlay: overlay_for_state,
        bundle: bundle::running_app(),
        presses: 0,
        drag_ends: Vec::new(),
    });
    let spec = WindowSpec::from_app("permissions", app)
        .configured_options(
            WindowOptions {
                transparent: true,
                borderless: true,
                always_on_top: true,
                visible: true,
                resizable: false,
                min_size: Some((0.0, 0.0)),
                ..WindowOptions::default()
            }
            .macos(
                MacWindowOptions::default()
                    .corner_radius(18.0)
                    .level(3)
                    .collection_behavior(native::COLLECTION_BITS as usize)
                    .shadow(true),
            ),
        )
        .on_created(|_| {
            READY.store(true, Ordering::SeqCst);
        })
        .on_ready(|_| {
            READY.store(true, Ordering::SeqCst);
        });
    (handle, spec, overlay)
}

fn view(_state: &Companion, _: &mut Cx<Companion>) -> El {
    let ink = Color {
        r: 0.11,
        g: 0.11,
        b: 0.12,
        a: 1.0,
    };
    let paper = Color {
        r: 0.98,
        g: 0.97,
        b: 0.95,
        a: 1.0,
    };
    col()
        .w_full()
        .h_full()
        .fill(paper)
        .padding(24.0)
        .child(
            text("Allow Kudzu to expand")
                .font(TextStyle::new(22.0, 650.0))
                .color(ink),
        )
        .child(
            text("Drag the app into the list in System Settings. This spike never grants access.")
                .font(TextStyle::new(13.0, 400.0))
                .color(ink),
        )
        .child(
            el().key(drag::DRAG_KEY)
                .size(ICON, ICON)
                .radius(16.0)
                .fill(rune::oklch(0.546, 0.245, 262.881))
                .pin(Some(ICON_X), Some(ICON_Y), None, None)
                .child(text("app").color(Color::WHITE))
                .on_press(|state: &mut Companion| {
                    state.presses += 1;
                    if let Some(bundle) = &state.bundle {
                        native::reveal_app(bundle);
                    }
                })
                .drag_out(|state: &Companion| drag::drag_item(state.bundle.as_deref()))
                .on_drag_end(|state: &mut Companion, outcome, _| {
                    state.drag_ends.push(outcome);
                    if let Ok(mut log) = LAST_END.lock() {
                        log.push(format!("{outcome:?}"));
                    }
                }),
        )
}

pub fn on_ui<T: Send + 'static>(
    handle: &AppHandle<Companion>,
    f: impl FnOnce(&mut Companion, &mut Tx) -> T + Send + 'static,
) -> Option<T> {
    on_ui_for(handle, std::time::Duration::from_secs(8), f)
}

pub fn on_ui_for<T: Send + 'static>(
    handle: &AppHandle<Companion>,
    timeout: std::time::Duration,
    f: impl FnOnce(&mut Companion, &mut Tx) -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    handle.update(move |state, ui| {
        let _ = tx.send(f(state, ui));
    });
    rx.recv_timeout(timeout).ok()
}

pub fn place_overlay(overlay: &OverlayHandle<Guide>, canvas: Rect, desktop_top: f64) {
    let (x, y, w, h) = place::canvas_to_overlay_placement(canvas, desktop_top);
    overlay.set_placement(Some(OverlayPlacement::Rect {
        x,
        y,
        width: w,
        height: h,
    }));
}

pub fn last_ends() -> Vec<String> {
    LAST_END.lock().map(|g| g.clone()).unwrap_or_default()
}
