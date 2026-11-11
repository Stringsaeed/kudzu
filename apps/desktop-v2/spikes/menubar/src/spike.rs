use rune::{
    col, text, text_input, App, AppHandle, AppSpec, Color, Cx, El, RunOptions, TextEdit, TextStyle,
    Tx, WindowOptions, WindowSpec,
};

use crate::native;

pub struct State {
    pub main: AppHandle<State>,
    pub panel: AppHandle<Panel>,
    pub field: TextEdit,
}

pub struct Panel {
    pub field: TextEdit,
}

pub fn run() {
    let panel_app = App::new(AppSpec::new(
        "S10 launcher",
        (500.0, 200.0),
        || Panel { field: focused() },
        panel_view,
    ))
    .windowless();
    let panel = panel_app.handle();
    let panel_spec = WindowSpec::from_app("panel", panel_app)
        .configured_options(WindowOptions::launcher_panel());
    let main_app = App::new(AppSpec::new(
        "S10 menu bar spike",
        (700.0, 360.0),
        || unreachable!(),
        main_view,
    ))
    .windowless()
    .keep_running()
    .standard_menus()
    .on_quit(on_quit)
    .on_reopen(on_reopen);
    let main = main_app.handle();
    let app = main_app.init({
        let main = main.clone();
        let panel = panel.clone();
        move || State {
            main,
            panel,
            field: focused(),
        }
    });
    let spec = WindowSpec::from_app("main", app);
    // Queue creation while no native window exists, using exactly the requested boundary.
    main.update(|s, _| native::install(s.main.clone(), s.panel.clone()));
    if std::env::args().any(|arg| arg == "--auto") {
        let driver = main.clone();
        std::thread::spawn(move || {
            let pause = || std::thread::sleep(std::time::Duration::from_millis(700));
            pause();
            driver.update(|_, _| native::auto_inspect());
            driver.set_dock_icon(true);
            pause();
            driver.update(|_, _| native::auto_patch());
            driver.open();
            pause();
            eprintln!("S10 main visible after open={}", driver.is_visible());
            driver.update(|_, _| native::auto_close());
            pause();
            eprintln!("S10 main visible after close={}", driver.is_visible());
            eprintln!("S10 requesting Rune quit with no shown windows");
            driver.quit();
        });
    }
    rune::prewarm_gpu();
    rune::run_windows_with_args(
        RunOptions { accessory: true },
        std::iter::empty::<String>(),
        move |_| vec![spec.boxed(), panel_spec.boxed()],
    );
    eprintln!("S10 event loop returned");
}

fn focused() -> TextEdit {
    let mut field = TextEdit::new("edit this text");
    field.focused = true;
    field
}

fn on_quit(_: &mut State, _: &mut Tx) {
    eprintln!("S10 on_quit");
}
fn on_reopen(s: &mut State, visible: bool, _: &mut Tx) -> bool {
    eprintln!("S10 on_reopen visible={visible}");
    s.main.open();
    true
}

fn field<S: 'static>(value: &TextEdit, edit: fn(&mut S) -> &mut TextEdit) -> El {
    text_input(value, TextStyle::new(18.0, 400.0))
        .key("field")
        .single_line()
        .color(Color::WHITE)
        .on_edit(edit)
}
fn main_view(s: &State, _: &mut Cx<State>) -> El {
    col()
        .w_full()
        .h_full()
        .padding(24.0)
        .fill(Color::BLACK)
        .child(text("S10: use the Dev menu-bar item to operate this probe").color(Color::WHITE))
        .child(field(&s.field, |s: &mut State| &mut s.field))
}
fn panel_view(s: &Panel, _: &mut Cx<Panel>) -> El {
    col()
        .w_full()
        .h_full()
        .padding(24.0)
        .fill(Color::BLACK)
        .child(text("S10 launcher panel").color(Color::WHITE))
        .child(field(&s.field, |s: &mut Panel| &mut s.field))
}
