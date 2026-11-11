//! RUNE-GAP(#141, #561, #562, #563, #146, #574): S71's opt-in native integration hooks.
use crate::{Panel, State, WINDOW};
use kudzu_appkit::{
    app_menu::AppMenu,
    appearance::{self, Preference},
    geometry::{Point, Rect},
    guide_panel::{Generation, GuidePanel, Lifecycle, Observation, Step},
    screens, settings_window,
    sidebar_material::SidebarMaterial,
    status_item::{Entry, Glyph, GlyphKind, Item, ItemId, Shortcut, StatusItem},
    window::{self, WindowRef},
};
use objc2::rc::Retained;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWorkspace};
use objc2_foundation::MainThreadMarker;
use rune::{raw_window_handle::RawWindowHandle, AppHandle, DisplayTarget, OverlayPlacement};
use std::{cell::RefCell, rc::Rc, time::Duration};

struct Native {
    status: StatusItem,
    menu: AppMenu,
    sidebar: SidebarMaterial,
    sidebar_frame: Rect,
    guide: Option<GuidePanel>,
    lifecycle: Lifecycle,
    generation: Option<Generation>,
    last_paused: Option<bool>,
}
thread_local! { static NATIVE: RefCell<Option<Native>> = const { RefCell::new(None) }; }

fn mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect("demo must run on the main thread")
}
pub fn rect([x, y, width, height]: [f32; 4]) -> Rect {
    Rect {
        x: x as f64,
        y: y as f64,
        width: width as f64,
        height: height as f64,
    }
}

fn with_window<R>(run: impl for<'a> FnOnce(WindowRef<'a>) -> R) -> Option<R> {
    let handle = WINDOW.with(|slot| slot.borrow().clone())?;
    handle
        .with_raw_handle(|raw| {
            let RawWindowHandle::AppKit(handle) = raw.as_raw() else {
                return None;
            };
            // SAFETY: Rune's callback keeps this NSView alive; the adapter's borrow cannot escape.
            unsafe { window::with_raw_view(mtm(), handle.ns_view, run) }
        })
        .ok()
        .flatten()
}

fn item(id: u32, title: &str, shortcut: Shortcut, action: impl Fn() + 'static) -> Entry {
    Entry::Item(Item {
        id: ItemId(id),
        title: title.into(),
        enabled: id != 0,
        tooltip: None,
        shortcut,
        action: Rc::new(action),
    })
}

pub fn install(main: AppHandle<State>, panel: AppHandle<Panel>) {
    let close = {
        let main = main.clone();
        Rc::new(move || main.update(|s, _| crate::close(s)))
    };
    let menu =
        AppMenu::install(mtm(), "Kudzu Next", close).expect("first update has standard menus");
    let status = StatusItem::new(
        mtm(),
        "Dev",
        Some("Kudzu Next S38 demo"),
        Glyph {
            kind: GlyphKind::Normal,
            update: true,
        },
        vec![
            item(0, "Running", Shortcut::default(), || {}),
            Entry::Separator,
            item(1, "Open Kudzu Next…", Shortcut::command("o"), {
                let main = main.clone();
                move || main.update(|s, _| crate::promote(s))
            }),
            item(
                2,
                "Launcher",
                Shortcut {
                    shift: true,
                    ..Shortcut::command("p")
                },
                move || panel.open(),
            ),
            item(3, "Pause demo", Shortcut::default(), {
                let main = main.clone();
                move || main.update(|s, _| s.paused = !s.paused)
            }),
            item(4, "Appearance", Shortcut::command(","), {
                let main = main.clone();
                move || {
                    main.update(|s, _| {
                        crate::set_preference(s, Preference::System);
                    })
                }
            }),
            item(5, "Close Windows", Shortcut::default(), {
                let main = main.clone();
                move || main.update(|s, _| crate::close(s))
            }),
            item(6, "Quit Kudzu Next", Shortcut::default(), move || {
                main.quit()
            }),
        ],
    )
    .expect("status item");
    appearance(Preference::Dark);
    NATIVE.with(|slot| {
        *slot.borrow_mut() = Some(Native {
            status,
            menu,
            sidebar: SidebarMaterial::new(mtm()),
            sidebar_frame: Rect::default(),
            guide: None,
            lifecycle: Lifecycle::default(),
            generation: None,
            last_paused: None,
        })
    });
}

pub fn patch() {
    NATIVE.with(|slot| {
        if let Some(native) = slot.borrow().as_ref() {
            eprintln!("app-menu patch: {:?}", native.menu.patch());
        }
    });
}
pub fn appearance(preference: Preference) {
    appearance::apply(mtm(), preference).expect("native appearance");
}
pub fn clear() {
    NATIVE.with(|slot| {
        slot.borrow_mut().take();
    });
    WINDOW.with(|slot| {
        slot.borrow_mut().take();
    });
}

pub fn sync_sidebar(drawn: [f32; 4]) {
    NATIVE.with(|slot| {
        if let Some(native) = slot.borrow_mut().as_mut() {
            native.sidebar_frame = rect(drawn);
            with_window(|window| {
                native
                    .sidebar
                    .sync(window, native.sidebar_frame)
                    .expect("sidebar sync")
            });
        }
    });
}

pub fn rewrap_content() {
    WINDOW.with(|slot| {
        if let Some(handle) = slot.borrow().as_ref() {
            let _ = handle.with_raw_handle(|raw| {
                let RawWindowHandle::AppKit(handle) = raw.as_raw() else {
                    return;
                };
                // SAFETY: scoped Rune handle; only our demo content hierarchy is changed.
                let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
                if let Some(window) = view.window() {
                    if let Some(old) = window.contentView() {
                        let wrapper: Retained<NSView> =
                            NSView::initWithFrame(mtm().alloc(), old.frame());
                        window.setContentView(Some(&wrapper));
                        old.setFrame(wrapper.bounds());
                        old.setAutoresizingMask(
                            NSAutoresizingMaskOptions::ViewWidthSizable
                                | NSAutoresizingMaskOptions::ViewHeightSizable,
                        );
                        wrapper.addSubview(&old);
                    }
                }
            });
        }
    });
    NATIVE.with(|slot| {
        if let Some(native) = slot.borrow().as_ref() {
            with_window(|window| {
                native
                    .sidebar
                    .sync(window, native.sidebar_frame)
                    .expect("sidebar reparent")
            });
        }
    });
}

pub fn inspect() {
    // Typed geometry only. No raw dictionaries, titles, owner names or captured pixels.
    eprintln!(
        "screens={:?} pointer_screen={:?} settings={:?} recording_preflight={}",
        screens::snapshot(mtm()),
        screens::under_pointer(mtm()),
        settings_window::find(mtm()),
        settings_window::screen_recording_access(mtm())
    );
}

pub fn begin_guide(state: &State) {
    let Some(bundle) = &state.bundle else {
        eprintln!("guide requires --bundle pointing at the Dev .app");
        return;
    };
    NATIVE.with(|slot| {
        if let Some(native) = slot.borrow_mut().as_mut() {
            native.lifecycle.stop();
            if let Some(panel) = &native.guide {
                panel.hide();
            }
            native.guide = Some(GuidePanel::new(mtm(), bundle).expect("Dev bundle guide"));
            native.generation = Some(
                native
                    .lifecycle
                    .begin(state.clock.elapsed().as_millis() as u64),
            );
        }
    });
}

pub fn stop_guide() {
    NATIVE.with(|slot| {
        if let Some(native) = slot.borrow_mut().as_mut() {
            native.lifecycle.stop();
            native.generation = None;
            if let Some(guide) = &native.guide {
                guide.hide();
            }
        }
    });
}

pub fn poll(state: &State) -> Option<Duration> {
    NATIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let native = slot.as_mut()?;
        if native.last_paused != Some(state.paused) {
            native.last_paused = Some(state.paused);
            native
                .status
                .set_title(ItemId(0), if state.paused { "Paused" } else { "Running" });
            native.status.set_title(
                ItemId(3),
                if state.paused {
                    "Resume demo"
                } else {
                    "Pause demo"
                },
            );
            native
                .status
                .set_tooltip(ItemId(3), Some("Runtime menu update"));
            native.status.set_enabled(ItemId(2), !state.paused);
            native
                .status
                .set_shortcut(ItemId(1), &Shortcut::command("o"));
            native
                .status
                .set_glyph(Glyph {
                    kind: if state.paused {
                        GlyphKind::Paused
                    } else {
                        GlyphKind::Normal
                    },
                    update: true,
                })
                .expect("updated glyph");
        }
        let generation = native.generation?;
        let companion = with_window(|window| window.frame())?;
        let settings = settings_window::find(mtm());
        let front = NSWorkspace::sharedWorkspace().frontmostApplication();
        let relevant = front.as_ref().is_some_and(|app| {
            app.processIdentifier() == std::process::id() as i32
                || app
                    .bundleIdentifier()
                    .is_some_and(|id| id.to_string() == "com.apple.systempreferences")
        });
        let now = state.clock.elapsed().as_millis() as u64;
        match native.lifecycle.poll(
            generation,
            now,
            Observation {
                visible: state.main.is_visible(),
                relevant_front_app: relevant,
                dragging: false,
                granted: false,
                reduce_motion: settings_window::reduce_motion(mtm()),
                settings,
                companion,
            },
        ) {
            Step::WaitUntil(due) => Some(Duration::from_millis(due.saturating_sub(now))),
            Step::Ready(destination) => {
                let screen = with_window(|window| {
                    screens::for_settings(mtm(), destination, window.screen())
                })
                .flatten()?;
                let frame = settings_window::companion_frame(
                    destination,
                    screen.visible,
                    (companion.width, companion.height),
                );
                let top = screens::snapshot(mtm()).first()?.frame.max_y();
                // The poll is outside Shared::drain: queue an update that requests placement,
                // then a nested update for the native correction, just as for Dock promotion.
                state.main.update(move |s, tx| {
                    let current = NATIVE.with(|slot| {
                        slot.borrow()
                            .as_ref()
                            .is_some_and(|n| n.lifecycle.is_current(generation))
                    });
                    if !current {
                        return;
                    }
                    s.main.place(
                        DisplayTarget::Primary,
                        OverlayPlacement::Rect {
                            x: frame.x as f32,
                            y: (top - frame.max_y()) as f32,
                            width: frame.width as f32,
                            height: frame.height as f32,
                        },
                    );
                    s.main.update(move |s, tx| {
                        let current = NATIVE.with(|slot| {
                            slot.borrow()
                                .as_ref()
                                .is_some_and(|n| n.lifecycle.is_current(generation))
                        });
                        if !current {
                            return;
                        }
                        let origin_set = with_window(|window| {
                            window.set_origin(Point {
                                x: frame.x,
                                y: frame.y,
                            })
                        });
                        if !matches!(origin_set, Some(Ok(()))) {
                            stop_guide();
                            return;
                        }
                        let actual =
                            with_window(|window| window.frame()).expect("companion still exists");
                        NATIVE.with(|slot| {
                            if let Some(native) = slot.borrow_mut().as_mut() {
                                if native.lifecycle.started(
                                    generation,
                                    s.clock.elapsed().as_millis() as u64,
                                    destination,
                                    actual,
                                ) {
                                    tx.wake_after(Duration::from_millis(33));
                                }
                            }
                        });
                    });
                    tx.wake_after(Duration::from_millis(Lifecycle::PLACEMENT_TIMEOUT_MS));
                });
                Some(Duration::from_millis(Lifecycle::PLACEMENT_TIMEOUT_MS))
            }
            Step::Draw(phase) => {
                let source = with_window(|window| window.rect_to_screen(state.icon)).flatten()?;
                let screen =
                    with_window(|window| screens::for_settings(mtm(), settings?, window.screen()))
                        .flatten()?;
                native
                    .guide
                    .as_ref()?
                    .show(source, companion, settings?, screen.frame, phase)
                    .expect("guide frame");
                Some(Duration::from_millis(33))
            }
            Step::Stop => {
                native.generation = None;
                if let Some(guide) = &native.guide {
                    guide.hide();
                }
                None
            }
            Step::Placing | Step::Stale => None,
        }
    })
}
