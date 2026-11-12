use super::*;
use crate::{
    fixtures::{clock::FakeClock, ids::FixedIds},
    persist::{PersistError, Revision, WriteTarget, Writer},
    ports::Clock,
};
use kudzu_core::library::model::{KudzuLibrary, SnippetRecord};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

struct Memory;
impl WriteTarget<KudzuLibrary> for Memory {
    fn write(&self, _: &KudzuLibrary) -> Result<(), PersistError> {
        Ok(())
    }
}
struct Harness {
    service: Arc<LibraryService>,
    clock: Arc<FakeClock>,
    writer: Writer,
}
impl Harness {
    fn new(initial: KudzuLibrary) -> Self {
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(
            clock.clone(),
            Duration::from_secs(30),
            Duration::from_secs(1),
        )
        .unwrap();
        let persisted = writer.register(Memory, Revision(0)).unwrap();
        let service = Arc::new(
            LibraryService::new(
                initial,
                "device-a".into(),
                clock.clone(),
                Arc::new(FixedIds::new(100)),
                persisted,
                Arc::new(crate::schedule::Scheduler::new(clock.clone()).unwrap()),
            )
            .unwrap(),
        );
        Self {
            service,
            clock,
            writer,
        }
    }
}

#[test]
fn create_inserts_first_with_fresh_identity_and_clock_timestamps() {
    let h = Harness::new(KudzuLibrary::default());
    let (a, first) = h
        .service
        .create_snippet(SnippetDraft {
            name: "A".into(),
            triggers: vec![";a".into()],
            content: "body".into(),
            ..Default::default()
        })
        .unwrap();
    h.clock.advance(Duration::from_secs(1));
    let (b, second) = h
        .service
        .create_snippet(SnippetDraft {
            name: "B".into(),
            is_favorite: true,
            group_id: Some(Uuid::from_u128(3)),
            ..Default::default()
        })
        .unwrap();
    assert_eq!((a.id, b.id), (Uuid::from_u128(100), Uuid::from_u128(101)));
    assert_eq!(b.created_at.timestamp(), 1_794_398_401);
    assert_eq!(b.updated_at, b.created_at);
    assert_eq!(a.created_at.timestamp(), 1_794_398_400);
    assert_eq!(h.service.watch().get().snippets, [b.clone(), a]);
    assert!(b.is_favorite && b.is_enabled);
    assert_eq!(b.group_id, Some(Uuid::from_u128(3)));
    assert!(second > first);
    assert!(h.service.saved().get().durable < second);
    h.writer.shutdown().unwrap();
    assert_eq!(h.service.saved().get().durable, second);
}

#[test]
fn update_preserves_creation_time_updates_edit_time_and_drops_only_blank_triggers() {
    let h = Harness::new(KudzuLibrary::default());
    let (mut snippet, _) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    let created = snippet.created_at;
    h.clock.advance(Duration::from_secs(60));
    snippet.created_at = h.clock.now().into();
    snippet.triggers = vec!["".into(), " \n\t".into(), ";kept ".into()];
    snippet.content = "edited".into();
    let (saved, revision) = h.service.update_snippet(snippet).unwrap();
    assert_eq!(saved.created_at, created);
    assert_eq!(saved.updated_at.timestamp(), 1_794_398_460);
    assert_eq!(saved.triggers, [";kept "]);
    assert_eq!(saved.content, "edited");
    assert_eq!(revision, h.service.watch().revision());
    let before = h.service.snapshot();
    let missing = SnippetRecord {
        id: Uuid::nil(),
        ..saved
    };
    let error = h.service.update_snippet(missing).unwrap_err();
    assert_eq!(error.to_string(), "That snippet no longer exists.");
    assert_eq!(h.service.snapshot(), before);
}

#[test]
fn duplicate_uses_first_free_case_insensitive_suffix_and_follows_source() {
    let h = Harness::new(KudzuLibrary::default());
    let (source, _) = h
        .service
        .create_snippet(SnippetDraft {
            name: "Signature".into(),
            triggers: vec![";sig".into(), ";email".into()],
            content: "hello".into(),
            is_favorite: true,
            ..Default::default()
        })
        .unwrap();
    let (other, _) = h
        .service
        .create_snippet(SnippetDraft {
            triggers: vec![";SIG2".into(), ";sig4".into(), ";EMAIL3".into()],
            ..Default::default()
        })
        .unwrap();
    h.clock.advance(Duration::from_secs(10));
    let (copy, _) = h.service.duplicate_snippet(source.id).unwrap();
    assert_eq!(copy.name, "Signature copy");
    assert_eq!(copy.triggers, [";sig3", ";email2"]);
    assert_eq!(copy.id, Uuid::from_u128(102));
    assert_eq!(copy.created_at.timestamp(), 1_794_398_410);
    assert_eq!(copy.updated_at, copy.created_at);
    assert!(copy.is_favorite);
    assert_eq!(copy.content, "hello");
    assert_eq!(
        h.service.watch().get().snippets,
        [other.clone(), source, copy]
    );
    let (unnamed, _) = h.service.duplicate_snippet(other.id).unwrap();
    assert_eq!(unnamed.name, "");
}

#[test]
fn delete_and_restore_only_missing_ids_without_overwriting_newer_records() {
    let h = Harness::new(KudzuLibrary::default());
    let (a, _) = h
        .service
        .create_snippet(SnippetDraft {
            name: "A".into(),
            ..Default::default()
        })
        .unwrap();
    let (b, _) = h
        .service
        .create_snippet(SnippetDraft {
            name: "B".into(),
            ..Default::default()
        })
        .unwrap();
    let (deleted, _) = h
        .service
        .delete_snippets(&[a.id, b.id, Uuid::nil()])
        .unwrap();
    assert!(h.service.watch().get().snippets.is_empty());
    assert_eq!(deleted.snippets.len(), 2);
    let mut newer = b.clone();
    newer.name = "Newer B".into();
    h.service
        .transact(|lib| {
            lib.snippets.push(newer.clone());
            Ok(())
        })
        .unwrap();
    let (restored, revision) = h.service.restore(deleted.clone()).unwrap();
    assert_eq!(restored, [a.id]);
    assert_eq!(h.service.watch().get().snippets, [newer, a]);
    assert_eq!(h.service.restore(deleted).unwrap(), (vec![], revision));
}

#[test]
fn moving_snippets_changes_only_selected_groups_and_edit_times() {
    let h = Harness::new(KudzuLibrary::default());
    let (a, _) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    let (b, _) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    h.clock.advance(Duration::from_secs(5));
    let group = Uuid::from_u128(99);
    h.service
        .move_snippets(&[a.id, Uuid::nil()], Some(group))
        .unwrap();
    let library = h.service.watch().get();
    assert_eq!(library.snippets[0], b);
    assert_eq!(library.snippets[1].group_id, Some(group));
    assert_eq!(library.snippets[1].updated_at.timestamp(), 1_794_398_405);
    h.service.move_snippets(&[a.id], None).unwrap();
    assert_eq!(h.service.watch().get().snippets[1].group_id, None);
}

#[test]
fn groups_append_and_move_at_a_clamped_sort_index_and_refuse_cycles() {
    let h = Harness::new(KudzuLibrary::default());
    let (a, _) = h.service.create_group("A".into(), None).unwrap();
    let (b, _) = h.service.create_group("B".into(), None).unwrap();
    let (child, _) = h.service.create_group("Child".into(), Some(a.id)).unwrap();
    assert_eq!((a.sort_index, b.sort_index, child.sort_index), (0, 1, 0));
    h.service.move_group(b.id, Some(a.id), 0).unwrap();
    let groups = h.service.watch().get().groups.clone();
    assert_eq!(groups.iter().find(|g| g.id == b.id).unwrap().sort_index, 0);
    assert_eq!(
        groups.iter().find(|g| g.id == child.id).unwrap().sort_index,
        1
    );
    let before = h.service.snapshot();
    for parent in [a.id, b.id, child.id] {
        let error = h.service.move_group(a.id, Some(parent), 0).unwrap_err();
        assert_eq!(error.to_string(), "A group can't be moved inside itself.");
    }
    assert_eq!(h.service.snapshot(), before);
    let mut edited = child.clone();
    edited.name = "  Renamed  ".into();
    edited.is_enabled = false;
    let (edited, _) = h.service.update_group(edited).unwrap();
    assert_eq!(edited.name, "Renamed");
    assert!(!edited.is_enabled);
    let (blank, _) = h
        .service
        .update_group(kudzu_core::library::model::GroupRecord {
            name: "  ".into(),
            ..edited
        })
        .unwrap();
    assert_eq!(blank.name, "Renamed");
    let before_cycle_update = h.service.snapshot();
    assert_eq!(
        h.service
            .update_group(kudzu_core::library::model::GroupRecord {
                parent_id: Some(child.id),
                ..a.clone()
            }),
        Err(LibraryError::GroupCycle)
    );
    assert_eq!(h.service.snapshot(), before_cycle_update);
    h.service.move_group(b.id, None, i64::MAX).unwrap();
    assert_eq!(
        h.service
            .watch()
            .get()
            .groups
            .iter()
            .find(|g| g.id == b.id)
            .unwrap()
            .sort_index,
        1
    );
    h.service.move_group(b.id, None, -1).unwrap();
    assert_eq!(
        h.service
            .watch()
            .get()
            .groups
            .iter()
            .find(|g| g.id == a.id)
            .unwrap()
            .sort_index,
        1
    );
}

#[test]
fn recursive_group_deletion_keeps_or_deletes_snippets_without_touching_other_groups() {
    for remove in [false, true] {
        let h = Harness::new(KudzuLibrary::default());
        let (root, _) = h.service.create_group("Root".into(), None).unwrap();
        let (child, _) = h
            .service
            .create_group("Child".into(), Some(root.id))
            .unwrap();
        let (leaf, _) = h
            .service
            .create_group("Leaf".into(), Some(child.id))
            .unwrap();
        let (other, _) = h.service.create_group("Other".into(), None).unwrap();
        let mut snippets = Vec::new();
        for group in [root.id, child.id, leaf.id, other.id] {
            snippets.push(
                h.service
                    .create_snippet(SnippetDraft {
                        group_id: Some(group),
                        ..Default::default()
                    })
                    .unwrap()
                    .0,
            );
        }
        h.service
            .delete_group(
                root.id,
                if remove {
                    GroupDeletion::DeleteSnippets
                } else {
                    GroupDeletion::KeepSnippets
                },
            )
            .unwrap();
        let library = h.service.watch().get();
        assert_eq!(library.groups, [other]);
        if remove {
            assert_eq!(library.snippets, [snippets[3].clone()]);
        } else {
            assert_eq!(library.snippets.len(), 4);
            assert_eq!(library.snippets[0], snippets[3]);
            assert!(library.snippets[1..].iter().all(|s| s.group_id.is_none()));
            assert_eq!(library.snippets[1].updated_at, snippets[2].updated_at);
        }
    }
}

#[test]
fn expansions_accumulate_per_snippet_and_device_and_clamp_negative_savings() {
    let h = Harness::new(KudzuLibrary::default());
    let (snippet, _) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    let (other, _) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    let foreign = kudzu_core::library::model::UsageRecord {
        snippet_id: snippet.id,
        device_id: "device-b".into(),
        count: 7,
        chars_saved: 70,
        last_used_at: None,
    };
    h.service
        .transact(|lib| {
            lib.usage.push(foreign.clone());
            Ok(())
        })
        .unwrap();
    h.service.record_expansion(snippet.id, 12).unwrap();
    h.clock.advance(Duration::from_secs(2));
    h.service.record_expansion(snippet.id, -3).unwrap();
    h.service.record_expansion(other.id, 5).unwrap();
    let library = h.service.watch().get();
    assert_eq!(library.usage[0], foreign);
    assert_eq!(
        (library.usage[1].count, library.usage[1].chars_saved),
        (2, 12)
    );
    assert_eq!(library.usage[1].device_id, "device-a");
    assert_eq!(
        library.usage[1].last_used_at.unwrap().timestamp(),
        1_794_398_402
    );
    assert_eq!(
        (
            library.usage[2].snippet_id,
            library.usage[2].count,
            library.usage[2].chars_saved
        ),
        (other.id, 1, 5)
    );
    assert_eq!(library.snippets[1], snippet);
}

mod conflicts;
mod storage;
mod transactions;

#[test]
fn missing_duplicate_and_update_keep_their_distinct_reference_errors() {
    let h = Harness::new(KudzuLibrary::default());
    let before = h.service.snapshot();
    assert_eq!(
        h.service
            .duplicate_snippet(Uuid::nil())
            .unwrap_err()
            .to_string(),
        "Snippet not found"
    );
    let snippet = SnippetRecord::new("Missing", vec![], "");
    assert_eq!(
        h.service.update_snippet(snippet).unwrap_err().to_string(),
        "That snippet no longer exists."
    );
    assert_eq!(h.service.snapshot(), before);
}

/// The library list's bulk bar edits many snippets at once: one revision, the rules of
/// `update_snippet` for each, and an id another device deleted meanwhile is skipped.
#[test]
fn updating_many_snippets_is_one_revision_and_skips_missing_ids() {
    let h = Harness::new(KudzuLibrary::default());
    let (a, _) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    let (b, start) = h.service.create_snippet(SnippetDraft::default()).unwrap();
    h.clock.advance(Duration::from_secs(60));
    let mut edits: Vec<SnippetRecord> = [a.clone(), b.clone()]
        .into_iter()
        .map(|mut s| {
            s.is_favorite = true;
            s.created_at = h.clock.now().into();
            s.triggers = vec![" ".into(), ";x".into()];
            s
        })
        .collect();
    edits.push(SnippetRecord {
        id: Uuid::nil(),
        ..a.clone()
    });
    let (saved, revision) = h.service.update_snippets(edits).unwrap();
    assert_eq!(revision.0, start.0 + 1);
    assert_eq!(saved.iter().map(|s| s.id).collect::<Vec<_>>(), [a.id, b.id]);
    let lib = h.service.watch().get();
    for (id, created) in [(a.id, a.created_at), (b.id, b.created_at)] {
        let s = lib.snippets.iter().find(|s| s.id == id).unwrap();
        assert!(s.is_favorite);
        assert_eq!(s.created_at, created);
        assert_eq!(s.updated_at.timestamp(), 1_794_398_460);
        assert_eq!(s.triggers, [";x"]);
    }
    assert_eq!(lib.snippets.len(), 2);
}

/// S50 review minor 2: a bulk edit over the whole library finds each snippet by id once, not
/// by scanning the library per id (⌘A then Favorite all on a large library).
#[test]
fn updating_every_snippet_of_a_large_library_is_linear() {
    let snippets: Vec<SnippetRecord> = (0..20_000u128)
        .map(|n| SnippetRecord {
            id: Uuid::from_u128(n + 1),
            ..SnippetRecord::new(format!("s{n}"), vec![], "body")
        })
        .collect();
    let h = Harness::new(KudzuLibrary {
        snippets: snippets.clone(),
        ..KudzuLibrary::default()
    });
    let edits: Vec<SnippetRecord> = snippets
        .into_iter()
        .map(|mut s| {
            s.is_favorite = true;
            s
        })
        .collect();
    let started = std::time::Instant::now();
    let (saved, _) = h.service.update_snippets(edits).unwrap();
    let took = started.elapsed();
    assert_eq!(saved.len(), 20_000);
    // A scan per id is 200 million comparisons (seconds, even in release); one lookup per id
    // is milliseconds. The bound leaves room for a loaded machine and a debug build.
    assert!(
        took < std::time::Duration::from_millis(400),
        "took {took:?}"
    );
}
