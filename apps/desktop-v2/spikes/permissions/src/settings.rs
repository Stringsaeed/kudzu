//! Filter for the System Settings window. Matches `macos.m` `settingsFrame`: owner pid, layer 0,
//! at least 400×300. Bounds are CoreGraphics (origin top-left of the primary display).

use crate::place::{Rect, MIN_SETTINGS_H, MIN_SETTINGS_W};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRecord {
    pub pid: i32,
    pub layer: i32,
    pub bounds: Rect,
}

pub fn pick(windows: &[WindowRecord], settings_pid: i32) -> Option<WindowRecord> {
    windows.iter().copied().find(|window| {
        window.pid == settings_pid
            && window.layer == 0
            && window.bounds.w >= MIN_SETTINGS_W
            && window.bounds.h >= MIN_SETTINGS_H
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(pid: i32, layer: i32, w: f64, h: f64) -> WindowRecord {
        WindowRecord {
            pid,
            layer,
            bounds: Rect {
                x: 10.0,
                y: 20.0,
                w,
                h,
            },
        }
    }

    #[test]
    fn picks_the_first_on_screen_settings_window_of_layer_zero() {
        let settings = 42;
        let windows = [
            rec(1, 0, 800.0, 600.0),
            rec(settings, 25, 800.0, 600.0),
            rec(settings, 0, 200.0, 200.0),
            rec(settings, 0, 900.0, 700.0),
            rec(settings, 0, 1000.0, 800.0),
        ];
        let hit = pick(&windows, settings).expect("a Settings window");
        assert_eq!(hit.bounds.w, 900.0);
        assert_eq!(hit.bounds.h, 700.0);
    }

    #[test]
    fn ignores_other_pids_and_undersized_frames() {
        let windows = [rec(7, 0, 900.0, 700.0), rec(9, 0, 100.0, 100.0)];
        assert!(pick(&windows, 9).is_none());
        assert!(pick(&[], 1).is_none());
    }
}
