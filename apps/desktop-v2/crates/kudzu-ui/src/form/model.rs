//! The form's fields, values and preview. One control per first-definition name.
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use kudzu_app::forms::FormRequest;
use kudzu_core::template::{FormFieldKind, FormFieldSpec, RenderResult};
use rune::{Latest, TextEdit};

/// Controls and values for the request on screen. Field values are user content: never logged.
pub struct Panel {
    pub unique: Vec<FormFieldSpec>,
    pub edits: HashMap<String, TextEdit>,
    pub values: HashMap<String, String>,
    pub preview: Latest<Option<RenderResult>>,
    pub preview_result: Option<RenderResult>,
    pub open_choice: Option<String>,
    pub height: f64,
}

impl Panel {
    pub fn from_request(request: &FormRequest) -> Self {
        let unique = unique_fields(&request.fields);
        let mut edits = HashMap::new();
        let mut values = HashMap::new();
        for (index, field) in unique.iter().enumerate() {
            let value = request
                .initial_values
                .get(&field.name)
                .cloned()
                .unwrap_or_default();
            let mut edit = TextEdit::new(&value);
            if index == 0 {
                edit.focused = field.kind != FormFieldKind::Choice;
            }
            edits.insert(field.name.clone(), edit);
            values.insert(field.name.clone(), value);
        }
        Self {
            unique,
            edits,
            values,
            preview: Latest::default().debounce(Duration::from_millis(50)),
            preview_result: None,
            open_choice: None,
            height: super::place::FORM_H_MIN,
        }
    }

    pub fn pull(&mut self) {
        for (name, edit) in &self.edits {
            self.values.insert(name.clone(), edit.text.clone());
        }
    }

    pub fn focused_multiline(&self) -> bool {
        self.unique.iter().any(|field| {
            field.kind == FormFieldKind::Multiline
                && self.edits.get(&field.name).is_some_and(|edit| edit.focused)
        })
    }
}

/// First definition of each name, in template order. Repeated names share one value.
pub fn unique_fields(fields: &[FormFieldSpec]) -> Vec<FormFieldSpec> {
    let mut seen = HashSet::new();
    fields
        .iter()
        .filter(|field| seen.insert(field.name.clone()))
        .cloned()
        .collect()
}
