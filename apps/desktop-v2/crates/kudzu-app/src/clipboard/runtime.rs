use super::*;
use crate::{
    schedule::{ScheduleError, Scheduler, Timer},
    watch::Subscription,
};
use std::{
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
};

enum Wake {
    Settings,
    Prune,
    Stop,
}
pub(super) struct Runtime {
    send: Sender<Wake>,
    thread: JoinHandle<()>,
    _timer: Timer,
    _settings: Subscription,
}
impl ClipboardService {
    /// Start capture after construction; stop this producer before shutting down the shared writer.
    pub fn start(self: &Arc<Self>, scheduler: &Scheduler) -> Result<(), ClipboardError> {
        let mut runtime = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        if runtime.is_some() {
            return Ok(());
        }
        let (send, receive) = mpsc::channel();
        let timer_send = send.clone();
        let timer = scheduler.every(
            Duration::from_secs(600),
            Duration::from_secs(600),
            move || {
                let _ = timer_send.send(Wake::Prune);
            },
        )?;
        let settings_send = send.clone();
        let subscription = self.settings.subscribe(move || {
            let _ = settings_send.send(Wake::Settings);
        });
        let service = self.clone();
        let thread = std::thread::Builder::new()
            .name("kudzu-clipboard".into())
            .spawn(move || loop {
                match receive.recv_timeout(Duration::from_millis(300)) {
                    Ok(Wake::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(Wake::Prune) => {
                        let _ = service.prune();
                    }
                    Ok(Wake::Settings) | Err(mpsc::RecvTimeoutError::Timeout) => {
                        let _ = service.poll();
                    }
                }
            })
            .map_err(IoFailure::from)?;
        *runtime = Some(Runtime {
            send,
            thread,
            _timer: timer,
            _settings: subscription,
        });
        Ok(())
    }
    pub fn shutdown(&self) -> Result<(), ClipboardError> {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(runtime) = runtime {
            let _ = runtime.send.send(Wake::Stop);
            runtime
                .thread
                .join()
                .map_err(|_| ClipboardError::WorkerStopped)?;
        }
        Ok(())
    }
}
impl From<ScheduleError> for ClipboardError {
    fn from(e: ScheduleError) -> Self {
        Self::Schedule(e)
    }
}
