//! MCP argument validation.
use super::Args;
use kudzu_core::{CaseMode, FireMode};
use serde_json::Value;
use uuid::Uuid;
pub(super) fn string<'a>(args: &'a Args, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(format!("`{key}` must be a string.")),
    }
}

pub(super) fn boolean(args: &Args, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("`{key}` must be true or false.")),
    }
}

pub(super) fn count(args: &Args, key: &str, default: usize, max: usize) -> Result<usize, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| (n as usize).min(max))
            .ok_or_else(|| format!("`{key}` must be a whole number.")),
    }
}

pub(super) fn triggers(args: &Args) -> Result<Option<Vec<String>>, String> {
    match args.get("triggers") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(vec![s.clone()])),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| "`triggers` must be a list of strings.".to_string())
            })
            .collect::<Result<_, _>>()
            .map(Some),
        Some(_) => Err("`triggers` must be a list of strings.".into()),
    }
}

pub(super) fn fire_mode(args: &Args) -> Result<Option<FireMode>, String> {
    Ok(match string(args, "fireMode")? {
        None => None,
        Some("immediate") => Some(FireMode::Immediate),
        Some("wholeWord") => Some(FireMode::WholeWord),
        Some(other) => {
            return Err(format!(
                "Unknown fireMode “{other}”: use immediate or wholeWord."
            ))
        }
    })
}

pub(super) fn case_mode(args: &Args) -> Result<Option<CaseMode>, String> {
    Ok(match string(args, "caseMode")? {
        None => None,
        Some("propagate") => Some(CaseMode::Propagate),
        Some("exact") => Some(CaseMode::Exact),
        Some(other) => {
            return Err(format!(
                "Unknown caseMode “{other}”: use propagate or exact."
            ))
        }
    })
}

pub(super) fn parse_id(raw: &str, what: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw.trim())
        .map_err(|_| format!("That isn't a {what} id. Ids come from the list tools."))
}
