//! Which windows are Kudzu's, for the engine's typed-form click check (FRM-4).
//! A destroyed window reports no number and drops out at the next push, because window
//! numbers get reused. The list is pushed again for every window open, close or change.
use kudzu_app::ports::{KudzuWindow, KudzuWindows};
use std::sync::{Arc, Mutex, PoisonError};

pub(crate) struct WindowList {
    port: Arc<dyn KudzuWindows>,
    handles: Mutex<Vec<rune::WindowHandle>>,
    guide: Mutex<Option<u32>>,
}

impl WindowList {
    pub(crate) fn new(port: Arc<dyn KudzuWindows>) -> Self {
        Self {
            port,
            handles: Mutex::new(Vec::new()),
            guide: Mutex::new(None),
        }
    }

    /// A native window was created (`App::on_window`). Pushes the new list.
    pub(crate) fn track(&self, handle: &rune::WindowHandle) {
        {
            let mut handles = self.handles.lock().unwrap_or_else(PoisonError::into_inner);
            handles.retain(|known| known.key() != handle.key());
            handles.push(handle.clone());
        }
        self.push();
    }

    /// Look up a tracked Rune window handle by its window key.
    pub(crate) fn window(&self, key: &str) -> Option<rune::WindowHandle> {
        let handles = self.handles.lock().unwrap_or_else(PoisonError::into_inner);
        handles.iter().find(|h| h.key() == key).cloned()
    }

    /// Register or clear the click-through permission guide overlay's `CGWindowID`.
    pub(crate) fn set_guide(&self, number: Option<u32>) {
        let changed = {
            let mut guide = self.guide.lock().unwrap_or_else(PoisonError::into_inner);
            let next = number.filter(|&n| n > 0);
            let changed = *guide != next;
            *guide = next;
            changed
        };
        if changed {
            self.push();
        }
    }

    /// Cheap and non-blocking; safe on the UI thread.
    pub(crate) fn push(&self) {
        let windows = self.current();
        self.port.set_kudzu_windows(&windows);
    }

    /// Drops handles whose window is gone; a recreated window is tracked again by `on_window`.
    fn current(&self) -> Vec<KudzuWindow> {
        let mut handles = self.handles.lock().unwrap_or_else(PoisonError::into_inner);
        handles.retain(|handle| handle.window_number().is_some());
        let mut windows: Vec<KudzuWindow> = handles
            .iter()
            .filter_map(|handle| handle.window_number())
            .map(|number| KudzuWindow {
                number,
                passes_clicks_through: false,
            })
            .collect();
        if let Some(number) = *self.guide.lock().unwrap_or_else(PoisonError::into_inner) {
            if !windows.iter().any(|w| w.number == number) {
                windows.push(KudzuWindow {
                    number,
                    passes_clicks_through: true,
                });
            }
        }
        windows.sort_by_key(|window| window.number);
        windows.dedup();
        windows
    }
}

#[cfg(test)]
mod tests {
    use super::WindowList;
    use kudzu_app::fixtures::engine::FakeEngine;
    use kudzu_app::ports::{KudzuWindow, KudzuWindows};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Recorded(Mutex<Vec<KudzuWindow>>);

    impl KudzuWindows for Recorded {
        fn set_kudzu_windows(&self, windows: &[KudzuWindow]) {
            *self.0.lock().unwrap() = windows.to_vec();
        }
    }

    #[test]
    fn a_handle_without_a_window_number_is_dropped() {
        let engine = Arc::new(FakeEngine::default());
        let list = WindowList::new(engine.clone());
        // A handle whose window was never created (or was destroyed) has no number.
        let mut host = rune::App::new(rune::AppSpec::new(
            "host",
            (10.0, 10.0),
            || (),
            |_, _| rune::col(),
        ));
        let closed = rune::App::new(rune::AppSpec::new(
            "closed",
            (10.0, 10.0),
            || (),
            |_, _| rune::col(),
        ));
        let (_, handle) = host.window(crate::theme::window::spec("closed", closed));
        list.track(&handle);
        assert_eq!(list.handles.lock().unwrap().len(), 0);
        assert_eq!(
            engine.state.lock().unwrap().windows.last(),
            Some(&Vec::new())
        );
    }

    #[test]
    fn guide_panel_is_tracked_as_click_through_and_removed_on_hide() {
        let port = Arc::new(Recorded::default());
        let list = WindowList::new(port.clone());

        list.set_guide(Some(4242));
        assert_eq!(
            *port.0.lock().unwrap(),
            vec![KudzuWindow {
                number: 4242,
                passes_clicks_through: true,
            }]
        );

        list.set_guide(None);
        assert!(port.0.lock().unwrap().is_empty());
    }
}
