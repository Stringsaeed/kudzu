use crate::ports::{EngineEvents, ExpansionEngine};
use crate::{
    fixtures::{
        apps::FakeApps, clock::FakeClock, engine::FakeEngine, ids::FixedIds,
        pasteboard::FakePasteboard,
    },
    library::LibraryService,
    persist::{PersistError, Revision, WriteTarget, Writer},
    schedule::Scheduler,
    settings::{Settings, SettingsService},
    tasks::Task,
    watch::Watch,
};
use kudzu_core::library::model::KudzuLibrary;
use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

pub(crate) struct Memory<T> {
    pub values: Arc<Mutex<Vec<T>>>,
    pub failure: Arc<Mutex<Option<PersistError>>>,
}
impl<T> Default for Memory<T> {
    fn default() -> Self {
        Self {
            values: Arc::default(),
            failure: Arc::default(),
        }
    }
}
impl<T> Clone for Memory<T> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            failure: self.failure.clone(),
        }
    }
}
impl<T: Clone + Send> WriteTarget<T> for Memory<T> {
    fn write(&self, value: &T) -> Result<(), PersistError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values.lock().unwrap().push(value.clone());
        Ok(())
    }
}
pub(crate) struct Harness {
    pub library: Arc<LibraryService>,
    pub settings: Arc<SettingsService>,
    pub engine: Arc<FakeEngine>,
    pub apps: Arc<FakeApps>,
    pub pasteboard: Arc<FakePasteboard>,
    pub clock: Arc<FakeClock>,
    pub writer: Writer,
}
impl Harness {
    pub fn wire_outcomes(&self, expansion: &Arc<super::ExpansionService>) {
        self.engine
            .start(Arc::new(OutcomeRouter(Arc::downgrade(expansion))))
            .unwrap();
        let generation = expansion.resync();
        until(&expansion.feed(), |f| f.generation == generation);
    }
    pub fn expansion(&self) -> Arc<super::ExpansionService> {
        Arc::new(self.expansion_service())
    }
    pub fn expansion_service(&self) -> super::ExpansionService {
        {
            super::ExpansionService::new(
                self.library.watch(),
                self.settings.clone(),
                self.engine.clone(),
                self.apps.clone(),
                super::RenderEnvironment {
                    clock: self.clock.clone(),
                    pasteboard: self.pasteboard.clone(),
                    locale: "en_US".into(),
                    local_time: |instant| {
                        chrono::DateTime::<chrono::Utc>::from(instant).fixed_offset()
                    },
                    history: Arc::new(|index, current| {
                        assert_eq!(current, Some("current copy"));
                        (index == 1).then(|| "earlier copy".into())
                    }),
                },
            )
            .unwrap()
        }
    }
    pub fn new(initial: KudzuLibrary) -> Self {
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(
            clock.clone(),
            Duration::from_secs(30),
            Duration::from_secs(1),
        )
        .unwrap();
        let library = Arc::new(
            LibraryService::new(
                initial,
                "device-s27".into(),
                clock.clone(),
                Arc::new(FixedIds::new(1000)),
                writer.register(Memory::default(), Revision(0)).unwrap(),
                Arc::new(Scheduler::new(clock.clone()).unwrap()),
            )
            .unwrap(),
        );
        let settings = Arc::new(
            SettingsService::new(
                Settings::default(),
                writer.register(Memory::default(), Revision(0)).unwrap(),
            )
            .unwrap(),
        );
        let engine = Arc::new(FakeEngine::default());
        engine
            .start(Arc::new(
                crate::fixtures::engine::FakeEngineEvents::default(),
            ))
            .unwrap();
        Self {
            library,
            settings,
            engine,
            apps: Arc::default(),
            pasteboard: Arc::default(),
            clock,
            writer,
        }
    }
}
pub(crate) fn until<T>(watch: &Watch<T>, ready: impl Fn(&T) -> bool) {
    let (tx, rx) = mpsc::channel();
    let _subscription = watch.subscribe(move || {
        let _ = tx.send(());
    });
    while !ready(&watch.get()) {
        rx.recv_timeout(Duration::from_secs(10))
            .expect("service did not publish");
    }
}
pub(crate) fn finish<T: Send + 'static>(task: Task<T>) -> T {
    until(&task.ready(), |ready| *ready);
    task.take().unwrap().unwrap()
}

struct OutcomeRouter(std::sync::Weak<super::ExpansionService>);
impl EngineEvents for OutcomeRouter {
    fn status_changed(&self, status: kudzu_core::system::EngineStatus) {
        if let Some(s) = self.0.upgrade() {
            s.status_changed(status);
        }
    }
    fn expanded(&self, _: crate::ports::Expansion) {}
    fn form_requested(&self, _: crate::ports::FormRequest) {}
    fn activity(&self, event: kudzu_core::activity::ActivityEvent) {
        if let Some(s) = self.0.upgrade() {
            s.activity_outcome(&event);
        }
    }
    fn trigger_lost(&self, lost: crate::ports::TriggerLost) {
        if let Some(s) = self.0.upgrade() {
            self.activity(s.trigger_lost(lost));
        }
    }
    fn clipboard_history(&self, _: usize, _: Option<&str>) -> Option<String> {
        None
    }
}

/// Tests with one fake Engine use its first namespace; restart tests take tokens from the fake.
pub(crate) fn form_token(native: u64) -> crate::ports::FormToken {
    crate::ports::EngineInstances::default()
        .next_engine()
        .form_token(native)
}
/// A submission by an event stamped `stamp` (no key code), after no focus report.
pub(crate) fn submitted_at(stamp: u64) -> crate::ports::FormSubmission {
    crate::ports::FormSubmission {
        event: Some(crate::ports::InputEvent {
            stamp: crate::ports::EventStamp::from_cg_event_timestamp(stamp),
            key_code: None,
        }),
        focus_seq: 0,
    }
}
/// Key-status report number `seq`, with no current event.
pub(crate) fn focus(key: bool, seq: u64) -> crate::ports::PanelFocus {
    crate::ports::PanelFocus {
        key,
        seq,
        after: None,
    }
}
