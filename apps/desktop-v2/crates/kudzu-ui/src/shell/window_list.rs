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

    fn current(&self) -> Vec<KudzuWindow> {
        let handles = self.handles.lock().unwrap_or_else(PoisonError::into_inner);
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
