//! Picks and pastes: one copy-through-paste at a time, and never into Kudzu itself.
use super::*;
use crate::{
    fixtures::{
        apps::{FakeApps, Latch},
        clock::FakeClock,
        pasteboard::FakePasteboard,
        thumbnails::FakeThumbnailer,
    },
    persist::Writer,
    settings::{Settings, Variant},
    watch::Publisher,
};
use kudzu_core::clipboard::uti;
use std::sync::Arc;

struct Rig {
    dir: PathBuf,
    board: Arc<FakePasteboard>,
    apps: Arc<FakeApps>,
    service: Arc<ClipboardService>,
    writer: Writer,
    ids: Vec<Uuid>,
}

fn app(name: &str, pid: i32) -> AppInfo {
    AppInfo {
        bundle_id: format!("com.test.{name}"),
        name: name.into(),
        path: None,
        pid: Some(pid),
    }
}

impl Rig {
    /// A service with `texts` in its history (oldest first) and Notes in front.
    fn new(texts: &[&str]) -> Self {
        let dir = std::env::temp_dir().join(format!("kudzu-clip-pick-{}", Uuid::new_v4()));
        let board = Arc::new(FakePasteboard::default());
        let apps = Arc::new(FakeApps::default());
        *apps.frontmost.lock().unwrap() = Some(app("Notes", 4242));
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(
            clock.clone(),
            Duration::from_millis(900),
            Duration::from_secs(1),
        )
        .unwrap();
        let settings = Publisher::new(Settings::for_variant(Variant::Dev));
        let service = Arc::new(
            ClipboardService::open(
                dir.clone(),
                settings.watch(),
                ClipboardPorts {
                    board: board.clone(),
                    thumbnails: Arc::new(FakeThumbnailer::default()),
                    target: apps.clone(),
                    icons: apps.clone(),
                    clock,
                },
                &writer,
            )
            .unwrap(),
        );
        let mut ids = Vec::new();
        for t in texts {
            board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), t.as_bytes().to_vec())]]);
            service.poll().unwrap();
            ids.push(service.watch().get().entries[0].id);
        }
        *board.paste_allowed.lock().unwrap() = true;
        Self {
            dir,
            board,
            apps,
            service,
            writer,
            ids,
        }
    }

    fn pasted(&self) -> Vec<Option<String>> {
        self.board.pasted_text.lock().unwrap().clone()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.service.shutdown();
        let _ = self.writer.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_second_pick_cannot_change_what_a_pending_paste_inserts() {
    let rig = Rig::new(&["alpha", "beta"]);
    let (alpha, beta) = (rig.ids[0], rig.ids[1]);
    let hold = Arc::new(Latch::default());
    *rig.apps.activation_hold.lock().unwrap() = Some(hold.clone());
    let first = rig.service.begin_pick().unwrap();
    let ticket = rig.service.copy_for_paste(alpha, Flavor::Original).unwrap();
    let pasting = std::thread::spawn(move || first.paste_copied(ticket));
    // The paste is now inside the target's activation, which is held.
    rig.apps.activation_entered.wait();

    // A second pick, as a reopened picker makes it, is refused and leaves the pasteboard alone.
    let second = rig.service.begin_pick().unwrap();
    assert_eq!(
        rig.service.copy_for_paste(beta, Flavor::Original).err(),
        Some(ClipboardError::PasteBusy)
    );
    // A plain copy waits its turn instead of overwriting the pasteboard under the paste.
    let (started, started_rx) = std::sync::mpsc::channel();
    let service = rig.service.clone();
    let copying = std::thread::spawn(move || {
        started.send(()).unwrap();
        service.copy(beta, Flavor::Original)
    });
    started_rx.recv().unwrap();

    hold.set();
    assert_eq!(pasting.join().unwrap().unwrap(), PasteOutcome::Pasted);
    copying.join().unwrap().unwrap();
    assert_eq!(
        rig.pasted(),
        vec![Some("alpha".to_owned())],
        "the first paste inserted its own clip"
    );

    // Once the first finished, the second pick goes through with its own content.
    let ticket = rig.service.copy_for_paste(beta, Flavor::Original).unwrap();
    assert_eq!(second.paste_copied(ticket).unwrap(), PasteOutcome::Pasted);
    assert_eq!(
        rig.pasted(),
        vec![Some("alpha".to_owned()), Some("beta".to_owned())]
    );
}

#[test]
fn dropping_a_ticket_frees_the_next_pick() {
    let rig = Rig::new(&["alpha", "beta"]);
    let ticket = rig
        .service
        .copy_for_paste(rig.ids[0], Flavor::Original)
        .unwrap();
    assert_eq!(
        rig.service
            .copy_for_paste(rig.ids[1], Flavor::Original)
            .err(),
        Some(ClipboardError::PasteBusy)
    );
    drop(ticket);
    assert!(rig
        .service
        .copy_for_paste(rig.ids[1], Flavor::Original)
        .is_ok());
}

#[test]
fn giving_focus_back_waits_for_a_pending_paste() {
    let rig = Rig::new(&["alpha"]);
    let session = rig.service.begin_pick().unwrap();
    let ticket = rig
        .service
        .copy_for_paste(rig.ids[0], Flavor::Original)
        .unwrap();
    let restoring = rig.service.begin_pick().unwrap().restore_target().unwrap();
    // The ticket is held, so the restore has not activated anything.
    assert!(rig.apps.activations.lock().unwrap().is_empty());
    assert_eq!(session.paste_copied(ticket).unwrap(), PasteOutcome::Pasted);
    let done = restoring.ready();
    while !*done.get() {
        std::thread::yield_now();
    }
    assert_eq!(rig.apps.activations.lock().unwrap().len(), 2);
}

#[test]
fn this_process_is_never_the_paste_target() {
    let rig = Rig::new(&["alpha"]);
    *rig.apps.frontmost.lock().unwrap() = Some(app("Kudzu", std::process::id() as i32));
    let session = rig.service.begin_pick().unwrap();
    assert!(session.target().is_none(), "Kudzu is not a target");
    assert!(
        !session.can_paste,
        "with no external target the picker only copies"
    );
    let ticket = rig
        .service
        .copy_for_paste(rig.ids[0], Flavor::Original)
        .unwrap();
    assert_eq!(session.paste_copied(ticket).unwrap(), PasteOutcome::Copied);
    assert!(rig.pasted().is_empty(), "nothing was posted into Kudzu");
    assert!(rig.apps.activations.lock().unwrap().is_empty());
}

#[test]
fn no_frontmost_app_means_copy_only() {
    let rig = Rig::new(&["alpha"]);
    *rig.apps.frontmost.lock().unwrap() = None;
    let session = rig.service.begin_pick().unwrap();
    assert!(session.target().is_none() && !session.can_paste);
    let ticket = rig
        .service
        .copy_for_paste(rig.ids[0], Flavor::Original)
        .unwrap();
    assert_eq!(session.paste_copied(ticket).unwrap(), PasteOutcome::Copied);
    assert!(rig.pasted().is_empty());
}

#[test]
fn the_destination_is_checked_again_just_before_the_keystroke() {
    let rig = Rig::new(&["alpha"]);
    let session = rig.service.begin_pick().unwrap();
    let ticket = rig
        .service
        .copy_for_paste(rig.ids[0], Flavor::Original)
        .unwrap();
    // The wait for the activation sees Notes in front; the last look sees Kudzu.
    {
        let mut script = rig.apps.frontmost_script.lock().unwrap();
        script.push_back(Some(app("Notes", 4242)));
        script.push_back(Some(app("Kudzu", std::process::id() as i32)));
    }
    assert_eq!(
        session.paste_copied(ticket).err(),
        Some(ClipboardError::TargetGone)
    );
    assert!(
        rig.pasted().is_empty(),
        "no keystroke went to the wrong app"
    );
}
