//! Native permission companion placement and click-through curved guide overlay (`S66`).
//!
//! Headless runs (`native == false`) keep this coordinator completely inert so tests and scene
//! captures never query `MainThreadMarker`, `NSWorkspace`, `NSScreen`, or raw AppKit handles.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use kudzu_appkit::app_bundle;
use kudzu_appkit::geometry::{Point, Rect};
use kudzu_appkit::guide_panel::{Generation, GuidePanel, Lifecycle, Observation, Step};
use kudzu_appkit::window::{self, WindowRef};
use kudzu_appkit::{screens, settings_window};
use objc2_foundation::MainThreadMarker;
use rune::raw_window_handle::RawWindowHandle;
use rune::{DisplayTarget, OverlayPlacement, WindowHandle};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const FIRST_DELAY: Duration = Duration::from_millis(450);

/// Pending placement request returned by `Guide::poll` when System Settings is found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacementPlan {
    pub generation: Generation,
    pub destination: Rect,
    pub target: Rect,
    pub rune_rect: [f32; 4],
}

/// Result of advancing the guide state machine on a UI poll turn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GuideOutcome {
    pub wake_after: Option<Duration>,
    pub guide_window: Option<u32>,
    pub placement: Option<PlacementPlan>,
}

/// Main-thread coordinator owning `Lifecycle`, the drawn app-icon rect, and `GuidePanel`.
pub struct Guide {
    native: bool,
    bundle: Option<PathBuf>,
    clock: Instant,
    lifecycle: Lifecycle,
    generation: Option<Generation>,
    panel: Option<GuidePanel>,
    icon_rect: Option<Rect>,
    dragging: bool,
}

impl Default for Guide {
    fn default() -> Self {
        Self::new(false, None)
    }
}

impl Guide {
    pub fn new(native: bool, bundle: Option<PathBuf>) -> Self {
        Self {
            native,
            bundle,
            clock: Instant::now(),
            lifecycle: Lifecycle::default(),
            generation: None,
            panel: None,
            icon_rect: None,
            dragging: false,
        }
    }

    pub fn is_native(&self) -> bool {
        self.native
    }

    pub fn is_active(&self) -> bool {
        self.generation
            .is_some_and(|gen| self.lifecycle.is_current(gen))
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Records the 80×80 app icon's drawn layout rect in top-down Rune window coordinates.
    pub fn record_icon_rect(&mut self, [x, y, width, height]: [f32; 4]) {
        if [x, y, width, height].iter().all(|v| v.is_finite()) && width > 0.0 && height > 0.0 {
            self.icon_rect = Some(Rect {
                x: f64::from(x),
                y: f64::from(y),
                width: f64::from(width),
                height: f64::from(height),
            });
        }
    }

    /// Starts a fresh Settings placement + guide cycle after `Permissions::begin` succeeds.
    pub fn begin(&mut self) -> Option<Duration> {
        if !self.native {
            return None;
        }
        self.stop();
        let now = self.now_ms();
        self.generation = Some(self.lifecycle.begin(now));
        Some(FIRST_DELAY)
    }

    /// Stops any active placement retry or overlay animation immediately.
    pub fn stop(&mut self) {
        self.lifecycle.stop();
        self.generation = None;
        self.hide_panel();
    }

    pub fn on_drag_start(&mut self) {
        self.dragging = true;
        self.stop();
    }

    pub fn on_drag_end(&mut self) {
        self.dragging = false;
    }

    /// Advances placement and overlay rendering on the main thread.
    pub fn poll(
        &mut self,
        open: bool,
        granted: bool,
        companion_handle: Option<&WindowHandle>,
    ) -> GuideOutcome {
        let Some(generation) = self.generation else {
            return GuideOutcome::default();
        };
        if !self.native || !open || granted || self.dragging {
            self.stop();
            return GuideOutcome::default();
        }
        let Some(mtm) = MainThreadMarker::new() else {
            self.stop();
            return GuideOutcome::default();
        };
        let Some(handle) = companion_handle else {
            return GuideOutcome {
                wake_after: Some(Duration::from_millis(200)),
                guide_window: None,
                placement: None,
            };
        };
        let Some(companion) = with_window(mtm, handle, |w| w.frame()) else {
            self.stop();
            return GuideOutcome::default();
        };

        let settings = settings_window::find(mtm);
        let relevant = app_bundle::is_relevant_front_app(mtm);
        let reduce_motion = settings_window::reduce_motion(mtm);
        let now = self.now_ms();

        match self.lifecycle.poll(
            generation,
            now,
            Observation {
                visible: open,
                relevant_front_app: relevant,
                dragging: self.dragging,
                granted,
                reduce_motion,
                settings,
                companion,
            },
        ) {
            Step::WaitUntil(due) => GuideOutcome {
                wake_after: Some(Duration::from_millis(due.saturating_sub(now))),
                guide_window: self.panel_window_number(),
                placement: None,
            },
            Step::Ready(destination) => {
                let Some(screen) = with_window(mtm, handle, |w| {
                    screens::for_settings(mtm, destination, w.screen())
                })
                .flatten() else {
                    self.stop();
                    return GuideOutcome::default();
                };
                let Some(primary) = screens::snapshot(mtm).first().copied() else {
                    self.stop();
                    return GuideOutcome::default();
                };
                let target = settings_window::companion_frame(
                    destination,
                    screen.visible,
                    (companion.width, companion.height),
                );
                let top = primary.frame.max_y();
                let rune_rect = [
                    target.x as f32,
                    (top - target.max_y()) as f32,
                    target.width as f32,
                    target.height as f32,
                ];
                GuideOutcome {
                    wake_after: Some(Duration::from_millis(Lifecycle::PLACEMENT_TIMEOUT_MS)),
                    guide_window: None,
                    placement: Some(PlacementPlan {
                        generation,
                        destination,
                        target,
                        rune_rect,
                    }),
                }
            }
            Step::Draw(phase) => {
                let (Some(bundle), Some(icon_local), Some(settings_rect)) =
                    (self.bundle.as_deref(), self.icon_rect, settings)
                else {
                    self.stop();
                    return GuideOutcome::default();
                };
                let Some(source) =
                    with_window(mtm, handle, |w| w.rect_to_screen(icon_local)).flatten()
                else {
                    self.stop();
                    return GuideOutcome::default();
                };
                let Some(screen) = with_window(mtm, handle, |w| {
                    screens::for_settings(mtm, settings_rect, w.screen())
                })
                .flatten() else {
                    self.stop();
                    return GuideOutcome::default();
                };
                if self.panel.is_none() {
                    match GuidePanel::new(mtm, bundle) {
                        Ok(panel) => self.panel = Some(panel),
                        Err(err) => {
                            log::warn!("permission guide panel could not initialize: {err}");
                            self.stop();
                            return GuideOutcome::default();
                        }
                    }
                }
                if let Some(panel) = &self.panel {
                    if let Err(err) =
                        panel.show(source, companion, settings_rect, screen.frame, phase)
                    {
                        log::debug!("permission guide frame skipped: {err}");
                        self.stop();
                        return GuideOutcome::default();
                    }
                }
                GuideOutcome {
                    wake_after: Some(FRAME_INTERVAL),
                    guide_window: self.panel_window_number(),
                    placement: None,
                }
            }
            Step::Stop => {
                self.stop();
                GuideOutcome::default()
            }
            Step::Placing | Step::Stale => GuideOutcome {
                wake_after: None,
                guide_window: self.panel_window_number(),
                placement: None,
            },
        }
    }

    /// Requests Rune placement in top-down primary-screen coordinates.
    pub fn request_rune_place<S: 'static>(
        app_handle: &rune::AppHandle<S>,
        [x, y, width, height]: [f32; 4],
    ) {
        app_handle.place(
            DisplayTarget::Primary,
            OverlayPlacement::Rect {
                x,
                y,
                width,
                height,
            },
        );
    }

    /// Completes native AppKit origin adjustment on the nested update turn after Rune placement.
    pub fn finish_placement(
        &mut self,
        plan: PlacementPlan,
        companion_handle: Option<&WindowHandle>,
    ) -> Option<Duration> {
        if !self.native || !self.lifecycle.is_current(plan.generation) {
            return None;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            self.stop();
            return None;
        };
        let Some(handle) = companion_handle else {
            self.stop();
            return None;
        };
        let origin_set = with_window(mtm, handle, |w| {
            w.set_origin(Point {
                x: plan.target.x,
                y: plan.target.y,
            })
        });
        if !matches!(origin_set, Some(Ok(()))) {
            self.stop();
            return None;
        }
        let Some(actual) = with_window(mtm, handle, |w| w.frame()) else {
            self.stop();
            return None;
        };
        let started =
            self.lifecycle
                .started(plan.generation, self.now_ms(), plan.destination, actual);
        if started && self.bundle.is_some() {
            Some(FRAME_INTERVAL)
        } else {
            if !started {
                self.hide_panel();
            }
            None
        }
    }

    fn now_ms(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }

    fn hide_panel(&mut self) {
        if let Some(panel) = &self.panel {
            panel.hide();
        }
    }

    fn panel_window_number(&self) -> Option<u32> {
        self.panel.as_ref().and_then(GuidePanel::window_number)
    }
}

fn with_window<R>(
    mtm: MainThreadMarker,
    handle: &WindowHandle,
    run: impl for<'a> FnOnce(WindowRef<'a>) -> R,
) -> Option<R> {
    handle
        .with_raw_handle(|raw| {
            let RawWindowHandle::AppKit(appkit) = raw.as_raw() else {
                return None;
            };
            // SAFETY: Rune's scoped borrow keeps `ns_view` alive for the duration of `run`.
            unsafe { window::with_raw_view(mtm, appkit.ns_view, run) }
        })
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::{Guide, GuideOutcome};

    #[test]
    fn headless_guide_never_activates_or_schedules_wakes() {
        let mut guide = Guide::new(false, None);
        guide.record_icon_rect([170.0, 77.0, 80.0, 80.0]);
        assert_eq!(guide.begin(), None);
        assert!(!guide.is_active());
        assert_eq!(guide.poll(true, false, None), GuideOutcome::default());
    }

    #[test]
    fn drag_start_stops_active_native_guide_immediately() {
        let mut guide = Guide::new(true, None);
        assert!(guide.begin().is_some());
        assert!(guide.is_active());
        guide.on_drag_start();
        assert!(guide.is_dragging());
        assert!(!guide.is_active());
        guide.on_drag_end();
        assert!(!guide.is_dragging());
    }
}
