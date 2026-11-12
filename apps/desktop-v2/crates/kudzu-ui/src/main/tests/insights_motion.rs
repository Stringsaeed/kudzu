//! Insights motion gate: count-up, staggered bars, Reduce Motion, later changes instant.

use super::Rig;
use crate::main::state::MainView;
use kudzu_app::settings::Theme;
use rune::motion_gate::{self, AllowRow, Job, Recording, Sampler};

const HZ: f64 = 120.0;
const DT: f64 = 1.0 / HZ;

fn mock() -> Rig {
    let rig = Rig::themed(true, Theme::Dark);
    let sample = rig.fixtures.sample.library.clone();
    rig.kudzu
        .library
        .transact(move |lib| {
            *lib = sample;
            Ok(())
        })
        .unwrap();
    rig
}

fn record(play_secs: f64, reduced: bool, after_land: bool) -> Recording {
    let mut rig = mock();
    if reduced {
        rig.rt.set_system_reduced_motion(true);
    }
    if after_land {
        rig.rt.update(|st, _| {
            st.navigate_to(MainView::Insights);
            st.insights.finish_arrival();
        });
        rig.rt.settle_all();
        for _ in 0..(0.25 / DT).round() as usize {
            rig.rt.frame(DT);
        }
    } else {
        for _ in 0..(0.25 / DT).round() as usize {
            rig.rt.frame(DT);
        }
    }
    let mut sampler = Sampler::new(&mut rig.rt, HZ);
    sampler.frame(&mut rig.rt, true);
    if after_land {
        sampler.input("change wpm");
        rig.rt.update(|st, _| {
            st.svc.settings.update(|s| s.typing_speed_wpm = 80).unwrap();
        });
    } else {
        sampler.input("open insights");
        rig.rt.update(|st, _| st.navigate_to(MainView::Insights));
    }
    for _ in 0..(play_secs / DT).round() as usize {
        sampler.before(&mut rig.rt);
        let awake = rig.rt.frame(DT);
        sampler.frame(&mut rig.rt, awake);
    }
    sampler.rec
}

fn arrival() -> Recording {
    record(2.0, false, false)
}

fn reduced() -> Recording {
    record(0.4, true, false)
}

fn later_wpm() -> Recording {
    record(0.4, false, true)
}

const ALLOW: &[AllowRow] = &[
    AllowRow::new(
        "text-swap",
        "page-0",
        "insights/",
        "the breadcrumb label swaps with the Insights route",
    )
    .max(2),
    AllowRow::new(
        "jump",
        "page-0",
        "insights/",
        "the breadcrumb trail resizes with the Insights route",
    )
    .max(2),
    AllowRow::new(
        "jump",
        "insights-bar-9",
        "insights/arrival",
        "the last staggered bar is still on its 480 ms spring",
    )
    .max(1),
];

#[test]
fn motion_gate() {
    motion_gate::gate_rows(
        vec![
            ("insights/arrival".into(), Box::new(arrival) as Job),
            ("insights/reduced".into(), Box::new(reduced) as Job),
            ("insights/later-wpm".into(), Box::new(later_wpm) as Job),
        ],
        ALLOW,
    );
}
