//! A CPU regression check on §1.7's engine budget, measured on the poster seam. The fakes cost
//! almost nothing and sleeps are virtual, so this measures the engine's own work.
//!
//! Only the tap callback is asserted: ≤ 0.2 ms at p99 of its thread CPU time, what it costs the
//! tap thread without the time other processes take the core away. Its wall-clock time and the
//! trigger-to-first-event time (≤ 15 ms at p95 in §1.7) are printed, not asserted: the second
//! crosses threads, and both depend on how the machine schedules them. Release-build wall
//! times with real posting are S72's to measure.

use std::time::{Duration, Instant};

use super::desk::Out;
use super::rig::{snippet, Rig};

const SAMPLES: usize = 400;

/// `CLOCK_THREAD_CPUTIME_ID` in `<time.h>` on Darwin.
const CLOCK_THREAD_CPUTIME_ID: u32 = 16;

extern "C" {
    fn clock_gettime_nsec_np(clock: u32) -> u64;
}

fn thread_cpu() -> Duration {
    Duration::from_nanos(unsafe { clock_gettime_nsec_np(CLOCK_THREAD_CPUTIME_ID) })
}

fn percentile(samples: &mut [Duration], p: f64) -> Duration {
    samples.sort_unstable();
    let rank = ((samples.len() as f64) * p).ceil() as usize;
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

#[test]
fn the_tap_callback_stays_within_its_cpu_budget() {
    let rig = Rig::new(&[
        snippet(";sig", "Best regards,\nSam"),
        snippet(";addr", "1 Infinite Loop"),
        snippet(";date", "{{date format=\"yyyy-MM-dd\"}}"),
    ]);
    let mut cpu = Vec::with_capacity(SAMPLES);
    let mut wall = Vec::with_capacity(SAMPLES);
    let mut first_event = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        rig.set_doc("");
        rig.type_(";si");
        let presses = rig.with(|s| s.press_times.len());
        let event = rig.key_event("g", true);
        let (started, cpu_started) = (Instant::now(), thread_cpu());
        let swallowed = rig.deliver_key(&event);
        cpu.push(thread_cpu().saturating_sub(cpu_started));
        wall.push(started.elapsed());
        assert!(swallowed, "the completing key is withheld");
        rig.settle();
        let first = rig.with(|s| s.press_times[presses]);
        first_event.push(first.duration_since(started));
        rig.key("g", false, true);
        rig.settle();
    }
    assert_eq!(rig.doc(), "Best regards,\nSam|");
    assert_eq!(
        rig.posted().last(),
        Some(&Out::Type("Best regards,\nSam".into()))
    );
    let cpu_p99 = percentile(&mut cpu, 0.99);
    let wall_p99 = percentile(&mut wall, 0.99);
    let first_p95 = percentile(&mut first_event, 0.95);
    println!(
        "tap callback p99: {cpu_p99:?} thread CPU, {wall_p99:?} wall clock; \
         trigger to first synthetic event p95: {first_p95:?}"
    );
    assert!(
        cpu_p99 <= Duration::from_micros(200),
        "tap callback p99 {cpu_p99:?} > 0.2 ms"
    );
}
