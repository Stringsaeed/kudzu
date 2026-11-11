//! Tasks the shell started and nobody waits on: kept until done, then a failure is logged.
//! The log line names the operation only, never its content (rule 9).
use kudzu_app::tasks::Task;
use std::sync::{Mutex, PoisonError};

type Check = Box<dyn FnMut() -> bool + Send>;

#[derive(Default)]
pub(crate) struct Settle {
    pending: Mutex<Vec<Check>>,
}

impl Settle {
    pub(crate) fn add<E: std::fmt::Display + Send + 'static>(
        &self,
        what: &'static str,
        task: Task<Result<(), E>>,
    ) {
        let check = move || match task.take() {
            None => false,
            Some(Ok(Ok(()))) => true,
            Some(Ok(Err(error))) => {
                log::warn!("{what} failed: {error}");
                true
            }
            Some(Err(error)) => {
                log::warn!("{what} failed: {error}");
                true
            }
        };
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Box::new(check));
    }

    /// A task whose value nobody reads: only a failure to run is logged.
    pub(crate) fn add_value<T: Send + 'static>(&self, what: &'static str, task: Task<T>) {
        let check = move || match task.take() {
            None => false,
            Some(Ok(_)) => true,
            Some(Err(error)) => {
                log::warn!("{what} failed: {error}");
                true
            }
        };
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Box::new(check));
    }

    /// Drops every finished task. Cheap: no task blocks here.
    pub(crate) fn drain(&self) {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain_mut(|check| !check());
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::Settle;
    use kudzu_app::tasks::Task;

    #[test]
    fn finished_tasks_leave_and_unfinished_ones_stay() {
        let settle = Settle::default();
        settle.add("done", Task::<Result<(), String>>::done(Ok(())));
        settle.add("failed", Task::<Result<(), String>>::done(Err("x".into())));
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        let slow = Task::spawn("slow", move || {
            let _ = receiver.recv();
            Ok::<(), String>(())
        })
        .unwrap();
        settle.add("slow", slow);
        settle.drain();
        assert_eq!(settle.len(), 1);
        sender.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while settle.len() > 0 {
            assert!(std::time::Instant::now() < deadline);
            settle.drain();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
