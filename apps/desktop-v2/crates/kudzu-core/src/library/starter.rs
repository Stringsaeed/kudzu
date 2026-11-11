//! The Getting Started group and its starter snippets, added on first launch and after an
//! import leaves the library without the sample trigger.

use chrono::{DateTime, Utc};

use super::model::{GroupRecord, KudzuLibrary, SnippetRecord};

/// Starter trigger used in onboarding.
pub const SAMPLE_TRIGGER: &str = ";kudzu";
/// Starter expansion used in onboarding.
pub const SAMPLE_EXPANSION: &str = "Kudzu is working!";
/// Name of the group that holds the starter snippets.
pub const GETTING_STARTED_GROUP: &str = "Getting Started";

/// Adds the "Getting Started" group and starter snippets (`;kudzu`, `;date`, `;sig`) unless a
/// snippet already uses `;kudzu`. Returns whether anything was added.
pub fn ensure_getting_started_snippets(library: &mut KudzuLibrary, now: DateTime<Utc>) -> bool {
    let has_sample = library.snippets.iter().any(|s| {
        s.triggers
            .iter()
            .any(|t| t.to_lowercase() == SAMPLE_TRIGGER)
    });
    if has_sample {
        return false;
    }
    let existing = library.groups.iter().find(|g| {
        g.parent_id.is_none() && g.name.to_lowercase() == GETTING_STARTED_GROUP.to_lowercase()
    });
    let group_id = match existing {
        Some(g) => g.id,
        None => {
            let next = library
                .groups
                .iter()
                .filter(|g| g.parent_id.is_none())
                .map(|g| g.sort_index)
                .max()
                .map_or(0, |m| m + 1);
            let group = GroupRecord {
                sort_index: next,
                ..GroupRecord::new(GETTING_STARTED_GROUP)
            };
            let id = group.id;
            library.groups.push(group);
            id
        }
    };
    let starters = [
        ("Kudzu Test", SAMPLE_TRIGGER, SAMPLE_EXPANSION),
        (
            "Today's Date",
            ";date",
            r#"{{date format="EEEE, d MMMM yyyy"}}"#,
        ),
        ("Signature", ";sig", "Best regards,\nYour Name"),
    ];
    for (name, trigger, content) in starters {
        library.snippets.push(SnippetRecord {
            group_id: Some(group_id),
            created_at: now,
            updated_at: now,
            ..SnippetRecord::new(name, vec![trigger.to_string()], content)
        });
    }
    true
}
