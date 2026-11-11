use crate::*;
use std::time::Instant;
fn visible(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(key) && e.visible[2] > 0.0 && e.visible[3] > 0.0)
}
pub fn run() {
    let mut rt = runtime();
    assert!(
        rt.state.built.get() < 100,
        "only a small mounted window builds"
    );
    rt.update(|s, tx| {
        s.selected.select(4999, &s.ids, false, false);
        tx.scroll_into_view(row_key(4999));
    });
    for _ in 0..20 {
        rt.frame(1.0 / 120.0);
    }
    assert!(visible(&rt, "item-4999"), "unbuilt row reveal");
    rt.update(|s, tx| {
        s.ids = (4900..5000).rev().collect();
        s.version += 1;
        tx.scroll_into_view(row_key(4950));
    });
    for _ in 0..20 {
        rt.frame(1.0 / 120.0);
    }
    assert!(visible(&rt, "item-4950"), "replacement and reorder reveal");
    rt.update(|s, tx| {
        s.ids = (3000..3100).rev().collect();
        s.version += 1;
        tx.scroll_into_view(row_key(3050));
    });
    for _ in 0..20 {
        rt.frame(1.0 / 120.0);
    }
    assert!(
        visible(&rt, "item-3050"),
        "same-count replacement must replace old keys"
    );
    assert!(!visible(&rt, "item-4950"));
    let r = rt
        .elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("item-3050"))
        .unwrap()
        .rect;
    rt.pointer_down(r[0] + 20.0, r[1] + 20.0);
    rt.frame(0.0);
    rt.pointer_up(r[0] + 20.0, r[1] + 20.0);
    rt.frame(0.0);
    assert_eq!(
        rt.state.selected.ids,
        [3050].into(),
        "retained handlers act on id after replacement"
    );
    rt.update(|s, _| {
        s.ids.clear();
        s.version += 1;
    });
    rt.frame(0.0);
    assert!(!visible(&rt, "item-3050"), "empty filter removes rows");
    println!("PASS unbuilt reveal; replace/reorder + version; bounded mount");
    keyboard_proof();
    modifier_proof();
    drag_proof();
    for run in 0..7 {
        let mut rt = runtime();
        let views = rt.state.views.get();
        let mut ms = Vec::new();
        for i in 0..600 {
            let t = Instant::now();
            rt.wheel(500.0, 300.0, if i < 300 { -160.0 } else { 160.0 });
            rt.frame(1.0 / 120.0);
            ms.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let commits = rt.state.views.get() - views;
        ms.sort_by(f64::total_cmp);
        println!(
            "TRACE run={run} frames=600 median_ms={:.4} p95_ms={:.4} view_commits={commits}",
            ms[300], ms[569]
        );
        assert_eq!(commits, 0);
    }
}
fn drag_proof() {
    let mut rt = runtime();
    rt.update(|s, _| {
        s.selected.select(1, &s.ids, false, false);
        s.selected.select(3, &s.ids, true, false);
    });
    rt.frame(0.0);
    let rect = rt
        .elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("item-1"))
        .unwrap()
        .rect;
    let (x, y) = (rect[0] + 20.0, rect[1] + 20.0);
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(50.0, 50.0);
    rt.frame(0.0);
    assert_eq!(rt.state.dnd.over, Some(0));
    assert!(rt
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some("ghost")
            && e.text.as_deref() == Some("3 selected")
            && e.opacity > 0.9));
    let views = rt.state.views.get();
    for i in 0..20 {
        rt.pointer_move(60.0 + i as f32, 50.0);
        rt.frame(0.0);
    }
    assert_eq!(
        rt.state.views.get(),
        views,
        "stable drag hover is live-only"
    );
    rt.pointer_up(80.0, 50.0);
    rt.frame(0.0);
    assert_eq!(rt.state.dnd.dropped, vec![1, 2, 3]);
    assert_eq!(rt.state.presses, 0, "drag does not press");
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(50.0, 150.0);
    rt.frame(0.0);
    assert_eq!(rt.state.dnd.over, Some(1));
    rt.state.dnd.dropped.clear();
    rt.pointer_up(50.0, 150.0);
    rt.frame(0.0);
    assert!(rt.state.dnd.dropped.is_empty());
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(50.0, 50.0);
    rt.frame(0.0);
    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.0);
    assert!(
        rt.state.dnd.dropped.is_empty(),
        "cancelled drag never moves data"
    );
    rt.pointer_up(50.0, 50.0);
    rt.frame(0.0);
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.presses, 1, "click coexists with on_drag");
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(x + 20.0, y);
    rt.frame(0.0);
    rt.pointer_up(x + 20.0, y);
    rt.frame(0.0);
    assert_eq!(
        rt.state.presses, 1,
        "a drag released inside its own source is not a click (#551)"
    );
    println!("PASS selected payload; stable live hover; accept/reject; drag vs press");
}
fn keyboard_proof() {
    let mut rt = runtime();
    rt.key(KeyEvent {
        key: Key::Tab,
        mods: Mods::default(),
    });
    rt.frame(0.0);
    rt.key(KeyEvent {
        key: Key::End,
        mods: Mods::default(),
    });
    for _ in 0..20 {
        rt.frame(1.0 / 120.0);
    }
    assert_eq!(rt.state.selected.ids, [4999].into());
    assert!(visible(&rt, "item-4999"));
    rt.key(KeyEvent {
        key: Key::Up,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    });
    rt.frame(0.0);
    assert_eq!(rt.state.selected.ids, [4998, 4999].into());
    rt.key(KeyEvent {
        key: Key::Up,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    });
    rt.frame(0.0);
    assert_eq!(rt.state.selected.ids, [4997, 4998, 4999].into());
    println!("PASS End reveals unbuilt row; repeated Shift-Up extends selection");
}

fn modifier_proof() {
    let mut rt = runtime();
    let click = |rt: &mut Runtime<State>, id| {
        let r = rt
            .elements()
            .iter()
            .find(|e| e.key.as_deref() == Some(&row_key(id)))
            .unwrap()
            .rect;
        rt.pointer_down(r[0] + 20.0, r[1] + 20.0);
        rt.frame(0.0);
        rt.pointer_up(r[0] + 20.0, r[1] + 20.0);
        rt.frame(0.0);
    };
    click(&mut rt, 1);
    rt.physical_input(PhysicalInput::Modifiers(Mods {
        cmd: true,
        logo: true,
        ..Mods::default()
    }));
    rt.frame(0.0);
    click(&mut rt, 3);
    assert_eq!(rt.state.selected.ids, [1, 3].into());
    click(&mut rt, 1);
    assert_eq!(rt.state.selected.ids, [3].into());
    rt.physical_input(PhysicalInput::Modifiers(Mods {
        shift: true,
        ..Mods::default()
    }));
    rt.frame(0.0);
    click(&mut rt, 4);
    assert_eq!(rt.state.selected.ids, [1, 2, 3, 4].into());
    println!("PASS physical command-click toggles; shift-click range");
}

#[cfg(test)]
mod tests {
    #[test]
    fn runtime_contract() {
        super::run();
    }
}
