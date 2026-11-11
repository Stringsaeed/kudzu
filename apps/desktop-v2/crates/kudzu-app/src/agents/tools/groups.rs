//! Group lookup and transactional path creation.
use kudzu_core::{GroupRecord, KudzuLibrary};
use std::collections::HashSet;
use uuid::Uuid;
pub(super) fn group_path(groups: &[GroupRecord], id: Option<Uuid>) -> Option<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut cur = id;
    while let Some(g) = cur.and_then(|id| groups.iter().find(|g| g.id == id)) {
        if !seen.insert(g.id) {
            break;
        }
        names.push(g.name.as_str());
        cur = g.parent_id;
    }
    names.reverse();
    (!names.is_empty()).then(|| names.join("/"))
}

/// A group by id, path ("Work/Email") or (unambiguous) name.
pub(super) fn find_group(groups: &[GroupRecord], query: &str) -> Result<Option<Uuid>, String> {
    let q = query.trim();
    if let Ok(id) = Uuid::parse_str(q) {
        return groups
            .iter()
            .find(|g| g.id == id)
            .map(|g| Some(g.id))
            .ok_or_else(|| format!("No group has the id {q}."));
    }
    let wanted = q.trim_matches('/').to_lowercase();
    if let Some(g) = groups
        .iter()
        .find(|g| group_path(groups, Some(g.id)).is_some_and(|p| p.to_lowercase() == wanted))
    {
        return Ok(Some(g.id));
    }
    let named: Vec<&GroupRecord> = groups
        .iter()
        .filter(|g| g.name.to_lowercase() == wanted)
        .collect();
    match named.as_slice() {
        [] => Ok(None),
        [g] => Ok(Some(g.id)),
        _ => Err(format!(
            "More than one group is called “{q}”. Use its path or id from list_groups."
        )),
    }
}

/// Finds the group, creating missing path components at the end of their parent.
pub(super) fn ensure_group(
    lib: &mut KudzuLibrary,
    query: &str,
    ids: &dyn crate::ports::IdSource,
) -> Result<Uuid, String> {
    if let Some(id) = find_group(&lib.groups, query)? {
        return Ok(id);
    }
    let mut parent: Option<Uuid> = None;
    for name in query.split('/').map(str::trim).filter(|n| !n.is_empty()) {
        let existing = lib
            .groups
            .iter()
            .find(|g| g.parent_id == parent && g.name.to_lowercase() == name.to_lowercase())
            .map(|g| g.id);
        parent = Some(match existing {
            Some(id) => id,
            None => {
                let sort_index = lib
                    .groups
                    .iter()
                    .filter(|g| g.parent_id == parent)
                    .map(|g| g.sort_index + 1)
                    .max()
                    .unwrap_or(0);
                let g = GroupRecord {
                    parent_id: parent,
                    sort_index,
                    id: ids.next_id(),
                    name: name.into(),
                    is_enabled: true,
                    insertion: None,
                    rule: None,
                };
                let id = g.id;
                lib.groups.push(g);
                id
            }
        });
    }
    parent.ok_or_else(|| "The group name is empty.".to_string())
}

pub(super) fn descendants(groups: &[GroupRecord], root: Uuid) -> HashSet<Uuid> {
    let mut set = HashSet::from([root]);
    loop {
        let before = set.len();
        for g in groups {
            if g.parent_id.is_some_and(|p| set.contains(&p)) {
                set.insert(g.id);
            }
        }
        if set.len() == before {
            return set;
        }
    }
}
