mod dnd;
mod model;
mod proof;
use rune::*;
use std::cell::Cell;

pub struct State {
    ids: Vec<usize>,
    height: f32,
    version: u64,
    selected: model::Selection,
    views: Cell<usize>,
    built: Cell<usize>,
    dnd: dnd::Dnd,
    presses: usize,
}
impl Default for State {
    fn default() -> Self {
        Self {
            ids: (0..5000).collect(),
            height: 64.0,
            version: 0,
            selected: Default::default(),
            views: Cell::new(0),
            built: Cell::new(0),
            dnd: Default::default(),
            presses: 0,
        }
    }
}
fn row_key(id: usize) -> String {
    format!("item-{id}")
}
fn view(s: &State, _: &mut Cx<State>) -> El {
    s.views.set(s.views.get() + 1);
    let rows = col()
        .key("rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            s.ids.len(),
            s.height,
            |s: &State, i| row_key(s.ids[i]).into(),
            |s: &State, i| {
                s.built.set(s.built.get() + 1);
                let id = s.ids[i];
                memo(row_key(id), s.selected.ids.contains(&id), || {
                    let mut item = col()
                        .w_full()
                        .h(s.height)
                        .padding(8.0)
                        .fill(if s.selected.ids.contains(&id) {
                            oklch(0.4, 0.15, 290.0)
                        } else {
                            oklch(0.2, 0.0, 0.0)
                        })
                        .hover(oklch(0.4, 0.0, 0.0))
                        .child(text(format!("Snippet {id:04} · ;s{id}")))
                        .child(text("Preview text · 12 uses"))
                        .on_press_tx(move |s: &mut State, tx| {
                            if s.dnd.suppress_press {
                                s.dnd.suppress_press = false;
                                return;
                            }
                            s.presses += 1;
                            s.selected
                                .select(id, &s.ids, tx.mods().shift, tx.mods().cmd);
                        })
                        .on_drag(move |s: &mut State, d, tx| dnd::gesture(s, id, d, tx));
                    if id.is_multiple_of(100) {
                        item = col()
                            .h(s.height + 30.0)
                            .child(
                                text(format!("Section {}", id / 100))
                                    .h(30.0)
                                    .sticky_top(0.0),
                            )
                            .child(item);
                    }
                    item
                })
            },
        )
        .virtual_rows_version(s.version);
    row()
        .key("list-root")
        .focusable(true)
        .w_full()
        .h_full()
        .on_key(keyboard)
        .child(dnd::sidebar(s))
        .child(
            col()
                .key("scroll")
                .grow(1.0)
                .h_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(rows),
        )
        .child(dnd::ghost(s))
}
fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(State::default(), view, (900.0, 600.0), Env::new(true, 1.0));
    for _ in 0..8 {
        rt.frame(1.0 / 120.0);
    }
    rt
}
fn main() {
    if rune::is_headless() {
        rune::run(AppSpec {
            scenes: &[("clips", |s: &mut State| s.height = 58.0)],
            ..AppSpec::new("S14 lists", (900.0, 600.0), State::default, view)
        });
    } else {
        proof::run();
    }
}
impl Drop for State {
    fn drop(&mut self) {
        eprintln!(
            "S14 views={} row_builds={}",
            self.views.get(),
            self.built.get()
        );
    }
}

fn keyboard(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let at = s
        .selected
        .cursor
        .and_then(|id| s.ids.iter().position(|v| *v == id))
        .unwrap_or(0);
    let i = match ev.key {
        Key::End => s.ids.len().saturating_sub(1),
        Key::Home => 0,
        Key::Up => at.saturating_sub(1),
        Key::Down => (at + 1).min(s.ids.len().saturating_sub(1)),
        _ => return false,
    };
    if let Some(&id) = s.ids.get(i) {
        s.selected.select(id, &s.ids, ev.mods.shift, ev.mods.cmd);
        tx.scroll_into_view(row_key(id));
    }
    true
}
