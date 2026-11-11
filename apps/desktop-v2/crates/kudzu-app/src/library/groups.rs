use super::{LibraryError, LibraryService};
use crate::persist::Revision;
use kudzu_core::library::model::GroupRecord;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupDeletion {
    KeepSnippets,
    DeleteSnippets,
}

impl LibraryService {
    pub fn delete_group(&self, id: Uuid, mode: GroupDeletion) -> Result<Revision, LibraryError> {
        self.transact(|library| {
            let doomed = kudzu_core::library::groups::descendant_ids(&library.groups, id);
            library.groups.retain(|g| !doomed.contains(&g.id));
            match mode {
                GroupDeletion::DeleteSnippets => library
                    .snippets
                    .retain(|s| !s.group_id.is_some_and(|id| doomed.contains(&id))),
                GroupDeletion::KeepSnippets => {
                    for snippet in &mut library.snippets {
                        if snippet.group_id.is_some_and(|id| doomed.contains(&id)) {
                            snippet.group_id = None;
                        }
                    }
                }
            }
            Ok(())
        })
        .map(|(_, revision)| revision)
    }

    pub fn create_group(
        &self,
        name: String,
        parent_id: Option<Uuid>,
    ) -> Result<(GroupRecord, Revision), LibraryError> {
        self.transact(|library| {
            let id = self.ids.next_id();
            check_parent(&library.groups, id, parent_id)?;
            let sort_index = library
                .groups
                .iter()
                .filter(|g| g.parent_id == parent_id)
                .map(|g| g.sort_index.saturating_add(1))
                .max()
                .unwrap_or(0);
            let group = GroupRecord {
                id,
                name,
                parent_id,
                sort_index,
                is_enabled: true,
                insertion: None,
                rule: None,
            };
            library.groups.push(group.clone());
            Ok(group)
        })
    }

    pub fn update_group(
        &self,
        mut group: GroupRecord,
    ) -> Result<(GroupRecord, Revision), LibraryError> {
        self.transact(|library| {
            check_parent(&library.groups, group.id, group.parent_id)?;
            let existing = library
                .groups
                .iter_mut()
                .find(|g| g.id == group.id)
                .ok_or(LibraryError::MissingGroup)?;
            group.name = if group.name.trim().is_empty() {
                existing.name.clone()
            } else {
                group.name.trim().into()
            };
            *existing = group.clone();
            Ok(group)
        })
    }

    pub fn move_group(
        &self,
        id: Uuid,
        parent_id: Option<Uuid>,
        index: i64,
    ) -> Result<Revision, LibraryError> {
        self.transact(|library| {
            check_parent(&library.groups, id, parent_id)?;
            if !library.groups.iter().any(|g| g.id == id) {
                return Err(LibraryError::MissingGroup);
            }
            let mut siblings: Vec<_> = library
                .groups
                .iter()
                .filter(|g| g.parent_id == parent_id && g.id != id)
                .collect();
            siblings.sort_by_key(|g| g.sort_index);
            let mut siblings: Vec<_> = siblings.into_iter().map(|g| g.id).collect();
            let at = (index.max(0) as usize).min(siblings.len());
            siblings.insert(at, id);
            for group in &mut library.groups {
                if let Some(position) = siblings.iter().position(|s| *s == group.id) {
                    group.sort_index = position as i64;
                    if group.id == id {
                        group.parent_id = parent_id;
                    }
                }
            }
            Ok(())
        })
        .map(|(_, revision)| revision)
    }
}

fn check_parent(
    groups: &[GroupRecord],
    id: Uuid,
    parent: Option<Uuid>,
) -> Result<(), LibraryError> {
    let mut current = parent;
    let mut seen = HashSet::new();
    while let Some(parent) = current {
        if parent == id || !seen.insert(parent) {
            return Err(LibraryError::GroupCycle);
        }
        current = groups
            .iter()
            .find(|g| g.id == parent)
            .and_then(|g| g.parent_id);
    }
    Ok(())
}
