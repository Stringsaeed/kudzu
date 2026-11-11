//! Motion gate for S07's shared widgets (PLAN §1.7): the selection pill sliding between
//! options, and the rejected-input shake. Driven headless on the virtual clock with
//! `rune::motion_gate`; besides the gate's findings, each cell checks that the motion it is
//! about happened.

use kudzu_ui::motion::Shake;
use kudzu_ui::theme::Theme;
use kudzu_ui::widgets::selection_pill::{Segment, Segmented};
use rune::motion_gate::{self, AllowRow, Job, Recording, Run};
use rune::{col, el, Cx, El, Env, Runtime, Tx};

const HZ: f64 = 120.0;

#[derive(Default)]
struct Pill {
    value: String,
}

fn pill_view(s: &Pill, cx: &mut Cx<Pill>) -> El {
    let seg = ["dark", "light", "system"]
        .iter()
        .fold(
            Segmented::new("appearance", "Appearance", s.value.clone()),
            |g, v| g.option(Segment::new(*v, v.to_uppercase())),
        )
        .build(Theme::light(), cx, |s: &mut Pill, v: &str| {
            s.value = v.to_owned()
        });
    col().padding(24.0).child(seg)
}

fn pill_runtime() -> Runtime<Pill> {
    let state = Pill {
        value: "dark".into(),
    };
    Runtime::new(state, pill_view, (420.0, 120.0), Env::new(true, 1.0))
}

/// The pill's presented left edge.
fn pill_x(rt: &Runtime<Pill>) -> f32 {
    rt.presented("pill").expect("a pill is drawn").0[0]
}

fn pill_slide() -> Recording {
    let mut r = Run::new(pill_runtime(), HZ);
    r.idle(1.0);
    r.step();
    r.act("tap system", |rt| Run::tap(rt, "appearance:system"));
    r.play(1.0);
    r.rec
}

#[derive(Default)]
struct Field {
    shake: Shake,
}

fn field_view(s: &Field, _: &mut Cx<Field>) -> El {
    let field = el()
        .key("field")
        .size(200.0, 32.0)
        .fill(Theme::light().c.input);
    col().padding(24.0).child(s.shake.apply(field))
}

fn field_poll(s: &mut Field, clock: f64, tx: &mut Tx) -> bool {
    s.shake.poll(clock, tx)
}

fn field_runtime() -> Runtime<Field> {
    let mut rt = Runtime::new(
        Field::default(),
        field_view,
        (300.0, 100.0),
        Env::new(true, 1.0),
    );
    rt.poll = Some(field_poll);
    rt
}

fn shake() -> Recording {
    let mut r = Run::new(field_runtime(), HZ);
    r.idle(1.0);
    r.step();
    r.act("reject input", |rt| rt.update(|s, tx| s.shake.start(tx)));
    r.play(1.0);
    r.rec
}

/// Neither cell needs an exception.
const ALLOW: &[AllowRow] = &[];

#[test]
fn motion_gate() {
    motion_gate::gate_rows(
        vec![
            ("selection-pill/slide".into(), Box::new(pill_slide) as Job),
            ("shake/field".into(), Box::new(shake) as Job),
        ],
        ALLOW,
    );
}

#[test]
fn the_pill_slides_to_the_new_option() {
    let mut rt = pill_runtime();
    for _ in 0..120 {
        rt.frame(1.0 / HZ);
    }
    let start = pill_x(&rt);
    Run::tap(&mut rt, "appearance:system");
    rt.frame(1.0 / HZ);
    let mut xs = Vec::new();
    for _ in 0..60 {
        rt.frame(1.0 / HZ);
        xs.push(pill_x(&rt));
    }
    let end = *xs.last().unwrap();
    assert!(end > start + 40.0, "moved from {start} to {end}");
    // It travels: some frames sit strictly between where it was and where it lands.
    let between = xs
        .iter()
        .filter(|x| **x > start + 1.0 && **x < end - 1.0)
        .count();
    assert!(between >= 3, "slid through {between} frames: {xs:?}");
}

#[test]
fn the_shake_moves_the_field_both_ways_and_rests() {
    let mut rt = field_runtime();
    for _ in 0..60 {
        rt.frame(1.0 / HZ);
    }
    let rest = rt.presented("field").unwrap().0[0];
    rt.update(|s, tx| s.shake.start(tx));
    let mut xs = Vec::new();
    for _ in 0..90 {
        rt.frame(1.0 / HZ);
        xs.push(rt.presented("field").unwrap().0[0] - rest);
    }
    let (min, max) = xs
        .iter()
        .fold((0.0f32, 0.0f32), |(a, b), x| (a.min(*x), b.max(*x)));
    assert!(min < -2.0 && max > 2.0, "travel {min}..{max}: {xs:?}");
    assert!(xs.last().unwrap().abs() < 0.05, "rests at its place");
}
