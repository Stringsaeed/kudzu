//! Three launcher panels and one ordinary host window.

use rune::{
    col, text, text_input, App, AppHandle, AppSpec, Color, Cx, El, SystemMaterial, TextEdit,
    TextStyle, Tx, WindowOptions, WindowSpec,
};

use crate::bus::{self, FocusNote, FrameNote};
use crate::native;
use crate::place;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Palette,
    Picker,
    Form,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Palette => "palette",
            Kind::Picker => "picker",
            Kind::Form => "form",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Kind::Palette => "kudzu-spike-palette",
            Kind::Picker => "kudzu-spike-picker",
            Kind::Form => "kudzu-spike-form",
        }
    }

    pub fn size(self) -> (f32, f32) {
        match self {
            Kind::Palette => (place::PALETTE_W as f32, place::PALETTE_H as f32),
            Kind::Picker => (place::PICKER_W as f32, place::PICKER_H as f32),
            Kind::Form => (place::FORM_W as f32, place::FORM_H_INITIAL as f32),
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            Kind::Form => 16.0,
            _ => 18.0,
        }
    }

    pub fn dismiss(self) -> bool {
        self != Kind::Form
    }
}

pub struct Panel {
    pub kind: Kind,
    pub query: TextEdit,
    pub token: u8,
    pub material: bool,
    pub dismiss_on_blur: bool,
    pub me: AppHandle<Panel>,
}

pub struct Host;

pub const HOST_TITLE: &str = "kudzu-spike-host";

pub fn panel(kind: Kind) -> (AppHandle<Panel>, WindowSpec<Panel>) {
    let app = App::new(AppSpec::new(
        kind.title(),
        kind.size(),
        || unreachable!(),
        view,
    ))
    .windowless()
    .on_focus(on_focus);
    let handle = app.handle();
    let app = app.init({
        let me = handle.clone();
        move || {
            let mut query = TextEdit::new("");
            query.focused = true;
            Panel {
                kind,
                query,
                token: 0,
                material: false,
                dismiss_on_blur: kind.dismiss(),
                me,
            }
        }
    });
    let spec = WindowSpec::from_app(kind.name(), app)
        .configured_options(
            WindowOptions::launcher_panel().macos(|n| n.corner_radius(kind.radius()).shadow(true)),
        )
        .on_ready(move |window| bus::bus().mark_ready(kind.name(), window.window_number()))
        .on_frame_encoded(move |frame| {
            let outcome = match frame.outcome() {
                rune::FrameOutcome::Drawn => "drawn",
                rune::FrameOutcome::Unchanged => "unchanged",
                rune::FrameOutcome::Skipped => "skipped",
                _ => "other",
            };
            bus::bus().push_frame(FrameNote {
                name: kind.name(),
                at: frame.at(),
                outcome,
                seq: frame.seq(),
                token: bus::bus().paint(kind.name()),
                alpha: native::fact(kind.title()).alpha,
                encode_ms: frame.encode_ms(),
            });
        });
    (handle, spec)
}

pub fn host() -> (AppHandle<Host>, WindowSpec<Host>) {
    let app = App::new(AppSpec::new(HOST_TITLE, (840.0, 560.0), || Host, host_view)).windowless();
    let handle = app.handle();
    let spec = WindowSpec::from_app("host", app)
        .on_ready(|window| bus::bus().mark_ready("host", window.window_number()));
    (handle, spec)
}

fn on_focus(panel: &mut Panel, focused: bool, tx: &mut Tx) {
    bus::bus().push_focus(FocusNote {
        name: panel.kind.name(),
        at: std::time::Instant::now(),
        focused,
        snap: native::snap(panel.kind.title()),
    });
    if !focused && panel.dismiss_on_blur {
        tx.close_window();
    }
}

fn view(panel: &Panel, _: &mut Cx<Panel>) -> El {
    bus::bus().set_paint(panel.kind.name(), panel.token);
    let ink = if panel.material {
        Color::BLACK
    } else {
        Color::WHITE
    };
    let field = text_input(&panel.query, TextStyle::new(16.0, 400.0))
        .key("query")
        .single_line()
        .color(ink)
        .placeholder(
            "Search",
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.45,
            },
        )
        .on_edit(|panel: &mut Panel| &mut panel.query);
    let fill = if panel.token == 0 {
        Color {
            r: 0.86,
            g: 0.16,
            b: 0.14,
            a: 1.0,
        }
    } else {
        Color {
            r: 0.12,
            g: 0.72,
            b: 0.28,
            a: 1.0,
        }
    };
    let mut body = col()
        .w_full()
        .h_full()
        .padding(18.0)
        .on_press(|_: &mut Panel| bus::bus().note_click());
    body = if panel.material {
        body.system_material(SystemMaterial::Popover)
    } else {
        body.fill(fill)
    };
    body.child(
        text(format!("{} token {}", panel.kind.name(), panel.token))
            .font(TextStyle::new(20.0, 650.0))
            .color(ink),
    )
    .child(field)
}

fn host_view(_: &Host, _: &mut Cx<Host>) -> El {
    col()
        .w_full()
        .h_full()
        .fill(Color {
            r: 0.12,
            g: 0.18,
            b: 0.42,
            a: 1.0,
        })
        .child(
            text("spike host")
                .font(TextStyle::new(28.0, 650.0))
                .color(Color::WHITE),
        )
}
