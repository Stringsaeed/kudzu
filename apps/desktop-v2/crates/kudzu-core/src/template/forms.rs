//! Fill-in forms: the fields a template asks for, and the values remembered per snippet.
//!
//! [`FormMemory`] is persisted as `form-memory.json`; its shape is pinned by the golden tests.
//! [`FormFieldSpec`] also crosses the agent protocol, so both keep their serde derives.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Form control kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormFieldKind {
    Text,
    Multiline,
    Choice,
}

/// A form control requested by a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFieldSpec {
    pub name: String,
    pub kind: FormFieldKind,
    pub options: Vec<String>,
    pub default_value: Option<String>,
}

/// Last submitted form values per snippet (bounded to 200 snippets).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FormMemory {
    pub values: HashMap<Uuid, HashMap<String, String>>,
}

impl FormMemory {
    /// Valid remembered values, then defaults, then the first choice, else empty text.
    pub fn initial_values(
        &self,
        snippet_id: Uuid,
        fields: &[FormFieldSpec],
    ) -> HashMap<String, String> {
        let remembered = self.values.get(&snippet_id);
        fields
            .iter()
            .map(|field| {
                let is_choice = field.kind == FormFieldKind::Choice;
                let candidates = [
                    remembered.and_then(|r| r.get(&field.name)),
                    field.default_value.as_ref(),
                ];
                let value = candidates
                    .into_iter()
                    .flatten()
                    .find(|v| !is_choice || field.options.contains(v))
                    .or(if is_choice {
                        field.options.first()
                    } else {
                        None
                    })
                    .cloned()
                    .unwrap_or_default();
                (field.name.clone(), value)
            })
            .collect()
    }

    /// Records a submission.
    pub fn record(&mut self, submitted: HashMap<String, String>, snippet_id: Uuid) {
        if !self.values.contains_key(&snippet_id) && self.values.len() >= Self::CAPACITY {
            // Deterministic eviction, as in the Swift app: the smallest UUID goes.
            if let Some(victim) = self.values.keys().min().copied() {
                self.values.remove(&victim);
            }
        }
        self.values.insert(snippet_id, submitted);
    }

    /// Maximum number of snippets remembered.
    pub const CAPACITY: usize = 200;

    /// Drops memory for snippets that no longer exist.
    pub fn retain_snippets(&mut self, ids: &std::collections::HashSet<Uuid>) {
        self.values.retain(|k, _| ids.contains(k));
    }
}
