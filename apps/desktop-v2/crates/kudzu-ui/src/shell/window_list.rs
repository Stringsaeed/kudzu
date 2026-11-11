//! Which windows are Kudzu's, for the engine's typed-form click check (FRM-4).
//! A destroyed window reports no number and drops out at the next push, because window
//! numbers get reused. The list is pushed again for every window open, close or change.
use kudzu_app::ports::{KudzuWindow, KudzuWindows};
use std::sync::{Arc, Mutex, PoisonError};

pub(crate) struct WindowList {
    port: Arc<dyn KudzuWindows>,
    handles: Mutex<Vec<rune::WindowHandle>>,
}

impl WindowList {
    pub(crate) fn new(port: Arc<dyn KudzuWindows>) -> Self {
        Self {
            port,
            handles: Mutex::new(Vec::new()),
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
                // None of the five windows passes clicks through; S66's guide overlay does.
                passes_clicks_through: false,
            })
            .collect();
        windows.sort_by_key(|window| window.number);
        windows.dedup();
        windows
    }
}

#[cfg(test)]
mod tests {
    use super::WindowList;
    use kudzu_app::fixtures::engine::FakeEngine;
    use std::sync::Arc;

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
}
