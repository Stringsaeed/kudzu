//! RUNE-GAP(#141, #146, #562): remove the relevant fixtures with each adapter.
use crate::{
    appearance::Preference,
    geometry::{Point, Rect},
    screens::{screen_at, Screen},
    settings_window::{companion_frame, pick, WindowRecord},
    sidebar_material::material_frame,
    status_item::{Glyph, GlyphKind, Shortcut},
};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn only_system_appearance_reads_the_system_source() {
    assert!(!Preference::Light.resolve_dark(|| panic!("forced light read system")));
    assert!(Preference::Dark.resolve_dark(|| panic!("forced dark read system")));
    assert!(Preference::System.resolve_dark(|| true));
    assert!(!Preference::System.resolve_dark(|| false));
}

#[test]
fn hints_match_the_mac_menu_order_and_launcher_quit_is_exact() {
    let hint = Shortcut {
        key: "p".into(),
        command: true,
        shift: true,
        ..Default::default()
    };
    assert_eq!(hint.text(), "⇧⌘P");
    assert_eq!(Shortcut::command("o").text(), "⌘O");
    assert_eq!(Shortcut::command(",").text(), "⌘,");
    assert!(Shortcut::command("Q").closes_windows());
    assert!(!hint.closes_windows());
    assert!(!Shortcut {
        shift: true,
        ..Shortcut::command("q")
    }
    .closes_windows());
    assert!(!Shortcut {
        key: "q".into(),
        ..Default::default()
    }
    .closes_windows());
}

#[test]
fn update_dot_has_opaque_core_clear_halo_and_preserves_other_pixels() {
    for scale in 1..=3 {
        for kind in [
            GlyphKind::Normal,
            GlyphKind::Paused,
            GlyphKind::SecureInput,
            GlyphKind::Permissions,
        ] {
            let plain = Glyph {
                kind,
                update: false,
            }
            .pixels(scale)
            .unwrap();
            let dotted = Glyph { kind, update: true }.pixels(scale).unwrap();
            assert_eq!(dotted.dimensions(), (18 * scale, 18 * scale));
            assert_eq!(dotted.get_pixel(14 * scale, 14 * scale).0, [0, 0, 0, 255]);
            assert_eq!(dotted.get_pixel(11 * scale, 14 * scale).0, [0, 0, 0, 0]);
            for y in 0..9 * scale {
                for x in 0..18 * scale {
                    assert_eq!(dotted.get_pixel(x, y), plain.get_pixel(x, y));
                }
            }
            assert!(plain.pixels().any(|pixel| pixel.0[3] > 0));
        }
    }
}

#[test]
fn settings_metadata_rejects_other_processes_layers_and_small_windows() {
    let records = [
        WindowRecord {
            pid: 7,
            layer: 0,
            bounds: rect(0., 0., 900., 700.),
        },
        WindowRecord {
            pid: 42,
            layer: 1,
            bounds: rect(0., 0., 900., 700.),
        },
        WindowRecord {
            pid: 42,
            layer: 0,
            bounds: rect(0., 0., 399., 700.),
        },
        WindowRecord {
            pid: 42,
            layer: 0,
            bounds: rect(0., 0., 900., 299.),
        },
        WindowRecord {
            pid: 42,
            layer: 0,
            bounds: rect(830., 186., 740., 852.),
        },
    ];
    assert_eq!(
        pick(&records, 42, 1200.),
        Some(rect(830., 162., 740., 852.))
    );
    assert_eq!(pick(&records, 9, 1200.), None);
}

#[test]
fn companion_prefers_left_flips_right_and_clamps_to_twelve_points() {
    let screen = rect(0., 65., 1800., 1065.);
    assert_eq!(
        companion_frame(rect(830., 162., 740., 852.), screen, (420., 660.)),
        rect(392., 354., 420., 660.)
    );
    assert_eq!(
        companion_frame(rect(40., 100., 800., 600.), screen, (420., 660.)),
        rect(858., 77., 420., 660.)
    );
    assert_eq!(
        companion_frame(
            rect(0., 0., 500., 400.),
            rect(0., 0., 700., 500.),
            (420., 660.)
        ),
        rect(268., 12., 420., 660.)
    );
    assert_eq!(
        companion_frame(
            rect(-1800., 100., 800., 700.),
            rect(-1920., 0., 1920., 1080.),
            (420., 660.)
        ),
        rect(-982., 140., 420., 660.)
    );
    assert_eq!(
        companion_frame(
            rect(1820., -800., 800., 600.),
            rect(1800., -900., 1000., 800.),
            (420., 660.)
        ),
        rect(2368., -860., 420., 660.)
    );
}

#[test]
fn sidebar_tracks_animation_resize_fullscreen_and_flipped_content() {
    let bounds = rect(0., 0., 1180., 760.);
    assert_eq!(
        material_frame(rect(0., 0., 224., 760.), bounds, false),
        rect(0., 0., 224., 760.)
    );
    assert_eq!(
        material_frame(rect(0., 48., 112., 700.), bounds, false),
        rect(0., 12., 112., 700.)
    );
    assert_eq!(
        material_frame(rect(0., 0., 0., 760.), bounds, false),
        rect(0., 0., 0., 760.)
    );
    assert_eq!(
        material_frame(rect(0., 0., 224., 900.), rect(0., 0., 1440., 900.), false),
        rect(0., 0., 224., 900.)
    );
    assert_eq!(
        material_frame(rect(0., 48., 224., 500.), rect(10., 20., 900., 560.), true),
        rect(10., 68., 224., 500.)
    );
    assert_eq!(
        material_frame(rect(-20., -10., 224., 500.), bounds, false),
        rect(0., 270., 204., 490.)
    );
}

#[test]
fn fresh_screen_snapshots_support_negative_origins_and_mixed_scales() {
    let screens = [
        Screen {
            frame: rect(0., 0., 1800., 1169.),
            visible: rect(0., 65., 1800., 1065.),
            scale: 2.,
        },
        Screen {
            frame: rect(-1920., 0., 1920., 1080.),
            visible: rect(-1920., 0., 1920., 1055.),
            scale: 1.,
        },
    ];
    assert_eq!(
        screen_at(&screens, Point { x: -300., y: 400. }),
        Some(screens[1])
    );
    assert_eq!(
        screen_at(&screens[..1], Point { x: -300., y: 400. }),
        Some(screens[0])
    );
    assert_eq!(screen_at(&[], Point { x: 0., y: 0. }), None);
    assert_eq!(
        crate::screens::union_frames(&screens),
        Some(rect(-1920., 0., 3720., 1169.))
    );
    assert_eq!(
        crate::screens::settings_screen(&screens, rect(-500., 100., 400., 400.), Some(screens[0])),
        Some(screens[1])
    );
    assert_eq!(
        crate::screens::settings_screen(&screens, rect(5000., 100., 400., 400.), Some(screens[1])),
        Some(screens[1])
    );
}

#[test]
fn both_privacy_panes_are_available() {
    use crate::settings_window::Pane;
    assert_eq!(
        Pane::InputMonitoring.url(),
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
    );
    assert_eq!(
        Pane::Accessibility.url(),
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    );
}

#[test]
fn metadata_adapter_never_grows_capture_or_window_title_queries() {
    // Assemble the forbidden tokens so this guard scans itself too.
    let forbidden = [
        ["CGWindow", "ListCreateImage"].concat(),
        ["Screen", "CaptureKit"].concat(),
        ["kCGWindow", "Name"].concat(),
        ["kCGWindowOwner", "Name"].concat(),
    ];
    let mut folders = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let source = std::fs::read_to_string(&path).unwrap();
                for token in &forbidden {
                    assert!(
                        !source.contains(token),
                        "{} contains a prohibited metadata query",
                        path.display()
                    );
                }
            }
        }
    }
}
