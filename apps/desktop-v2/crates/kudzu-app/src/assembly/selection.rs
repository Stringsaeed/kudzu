//! "New Snippet from Selection": read the front app's selection on a worker.
//! The captured text is user content. It goes only into the new snippet's draft and is never
//! logged, persisted elsewhere or forwarded.

use crate::ports::TargetApp;
use crate::tasks::{Task, TaskError};
use std::sync::Arc;

/// Narrow service for the tray item. Selection reads can press ⌘C and wait about 300 ms, so
/// they never run on the UI thread.
pub struct SelectionCapture {
    target: Arc<dyn TargetApp>,
}

impl SelectionCapture {
    pub fn new(target: Arc<dyn TargetApp>) -> Self {
        Self { target }
    }

    /// Resolves to the selected text, or an empty string when there is none or the read
    /// failed. The caller routes to a new snippet either way.
    pub fn capture(&self) -> Result<Task<String>, TaskError> {
        let target = self.target.clone();
        Task::spawn("kudzu-selection-capture", move || {
            target.selection().ok().flatten().unwrap_or_default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SelectionCapture;
    use crate::fixtures::apps::FakeApps;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn wait<T: Send + 'static>(task: &crate::tasks::Task<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = task.take() {
                return result.unwrap();
            }
            assert!(Instant::now() < deadline, "the capture did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn an_empty_or_failed_selection_still_resolves() {
        let apps = Arc::new(FakeApps::default());
        let capture = SelectionCapture::new(apps.clone());
        assert_eq!(wait(&capture.capture().unwrap()), "");
        *apps.selection.lock().unwrap() = Some("picked".into());
        assert_eq!(wait(&capture.capture().unwrap()), "picked");
    }
}
