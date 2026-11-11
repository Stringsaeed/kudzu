//! Deterministic, human-readable JSON encoding of a [`KudzuLibrary`].

use serde_json::{Map, Value};

use super::InterchangeError;
use crate::types::KudzuLibrary;

/// Encodes a library as pretty-printed JSON with sorted keys and ISO-8601 dates.
pub fn encode(library: &KudzuLibrary) -> Result<String, InterchangeError> {
    let value =
        serde_json::to_value(library).map_err(|e| InterchangeError::Encoding(e.to_string()))?;
    serde_json::to_string_pretty(&sort_keys(value))
        .map_err(|e| InterchangeError::Encoding(e.to_string()))
}

/// Decodes a library, rejecting format versions newer than [`KudzuLibrary::CURRENT_FORMAT_VERSION`].
pub fn decode(json: &str) -> Result<KudzuLibrary, InterchangeError> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| InterchangeError::Decoding(e.to_string()))?;
    if let Some(found) = value.get("formatVersion").and_then(Value::as_i64) {
        if found > KudzuLibrary::CURRENT_FORMAT_VERSION {
            return Err(InterchangeError::UnsupportedFormatVersion {
                found,
                max_supported: KudzuLibrary::CURRENT_FORMAT_VERSION,
            });
        }
    }
    serde_json::from_value(value).map_err(|e| InterchangeError::Decoding(e.to_string()))
}

/// Rebuilds every object with keys inserted in sorted order, so output is sorted whether or not
/// serde_json's `preserve_order` feature is enabled somewhere in the dependency graph.
fn sort_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = Map::new();
            for (k, v) in entries {
                sorted.insert(k, sort_keys(v));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_keys).collect()),
        other => other,
    }
}
