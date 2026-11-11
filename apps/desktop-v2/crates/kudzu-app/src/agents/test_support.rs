use super::*;
use crate::{
    clipboard::{ClipboardPorts, ClipboardService},
    fixtures::{
        apps::FakeApps, clock::FakeClock, ids::FixedIds, pasteboard::FakePasteboard,
        thumbnails::FakeThumbnailer,
    },
    library::LibraryService,
    persist::{PersistError, Revision, WriteTarget, Writer},
    schedule::Scheduler,
    settings::Settings,
    watch::Publisher,
};
use kudzu_core::KudzuLibrary;
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;
pub struct Memory;
impl<T> WriteTarget<T> for Memory {
    fn write(&self, _: &T) -> Result<(), PersistError> {
        Ok(())
    }
}
pub fn directory() -> PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target/s32-fixtures")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
pub struct Harness {
    pub service: AgentsService,
    pub settings: Publisher<Settings>,
    pub writer: Writer,
    pub dir: PathBuf,
}
impl Harness {
    pub fn new(lib: KudzuLibrary, mut clips: Vec<kudzu_core::clipboard::ClipEntry>) -> Self {
        let dir = directory();
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(
            clock.clone(),
            Duration::from_secs(30),
            Duration::from_secs(1),
        )
        .unwrap();
        let settings = Publisher::new(Settings {
            agent_access: true,
            ..Settings::default()
        });
        let ids = Arc::new(FixedIds::new(100));
        let library = Arc::new(
            LibraryService::new(
                lib,
                "test".into(),
                clock.clone(),
                ids.clone(),
                writer.register(Memory, Revision(0)).unwrap(),
                Arc::new(Scheduler::new(clock.clone()).unwrap()),
            )
            .unwrap(),
        );
        clips.sort_by_key(|e| std::cmp::Reverse(e.copied_at));
        let clip_dir = dir.join("clipboard");
        std::fs::create_dir_all(&clip_dir).unwrap();
        std::fs::write(
            clip_dir.join("history.json"),
            serde_json::to_vec(&kudzu_core::clipboard::History {
                entries: clips,
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        let clipboard = Arc::new(
            ClipboardService::open(
                clip_dir,
                settings.watch(),
                ClipboardPorts {
                    board: Arc::new(FakePasteboard::default()),
                    thumbnails: Arc::new(FakeThumbnailer::default()),
                    target: Arc::new(FakeApps::default()),
                    icons: Arc::new(FakeApps::default()),
                    clock: clock.clone(),
                },
                &writer,
            )
            .unwrap(),
        );
        let config = AgentConfig {
            socket: socket_path(&dir),
            home: dir.join("home"),
            executable: dir.join("Kudzu Next.app/it's Kudzu"),
            name: "kudzu-next".into(),
            args: vec!["--mcp".into()],
            version: "1.2.3".into(),
            locale: "en_US".into(),
            bundled_skill_dir: None,
        };
        Self {
            service: AgentsService::new(config, library, clipboard, settings.watch(), clock, ids),
            settings,
            writer,
            dir,
        }
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.service.shutdown();
        self.service.host.clipboard.shutdown().unwrap();
        self.writer.shutdown().unwrap();
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

// Unix socket paths have a short kernel limit. Resolve the absolute workspace-owned
// fixture through a short relative spelling from Cargo's per-package test directory.
pub fn socket_path(dir: &std::path::Path) -> PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    PathBuf::from("../..")
        .join(dir.strip_prefix(root).unwrap())
        .join("s")
}
