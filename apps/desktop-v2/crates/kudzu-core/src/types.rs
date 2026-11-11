//! Plain value types shared by every layer: the contract between persistence, the
//! expansion engine, import/export and the UI.
//!
//! The JSON shape matches the Swift app's `KudzuLibrary` Codable encoding exactly
//! (field names such as `groupID` / `bundleIDs`, ISO-8601 dates without fractions),
//! so a library exported by either app imports into the other. Changes must be
//! additive: new fields optional or defaulted so old JSON still decodes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How the case of a typed trigger is reflected in the expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CaseMode {
    /// `;sig` → "Best regards", `;Sig` → "Best regards", `;SIG` → "BEST REGARDS".
    #[default]
    Propagate,
    /// Trigger must match exactly as written; output is never altered.
    Exact,
}

/// When a snippet fires relative to word boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FireMode {
    /// Fires as soon as the last trigger character is typed.
    #[default]
    Immediate,
    /// Fires only when the trigger is a whole word, on the following separator (which is kept).
    WholeWord,
}

/// How expanded text is delivered to the focused app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum InsertionMethod {
    /// Short plain text as keystrokes, long text via paste.
    #[default]
    Automatic,
    Type,
    Paste,
}

/// Whether an app rule lists the only apps a snippet works in, or the apps it never works in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppRuleMode {
    /// Active only in the listed apps.
    Allow,
    /// Active everywhere except the listed apps.
    Deny,
}

/// Per-app allow/deny list attached to a snippet or group.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppRule {
    pub mode: AppRuleMode,
    #[serde(rename = "bundleIDs", default)]
    pub bundle_ids: Vec<String>,
    /// Per-app insertion override; `None` inherits from the parent scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<InsertionMethod>,
}

/// A snippet: one or more triggers expanding to a template.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnippetRecord {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    /// One or more triggers. Never empty for an enabled snippet.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Template source; see docs/TEMPLATE_SYNTAX.md.
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatting: Option<crate::formatting::Formatting>,
    #[serde(rename = "groupID", default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<Uuid>,
    #[serde(rename = "fireMode", default)]
    pub fire_mode: FireMode,
    #[serde(rename = "caseMode", default)]
    pub case_mode: CaseMode,
    /// `None` inherits from the group, then the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<InsertionMethod>,
    /// `None` inherits the group's rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<AppRule>,
    #[serde(rename = "isFavorite", default)]
    pub is_favorite: bool,
    #[serde(rename = "isEnabled", default = "default_true")]
    pub is_enabled: bool,
    #[serde(rename = "createdAt", with = "iso8601", default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt", with = "iso8601", default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl SnippetRecord {
    /// A new enabled snippet with default modes, stamped now.
    pub fn new(name: impl Into<String>, triggers: Vec<String>, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            triggers,
            content: content.into(),
            formatting: None,
            group_id: None,
            fire_mode: FireMode::Immediate,
            case_mode: CaseMode::Propagate,
            insertion: None,
            rule: None,
            is_favorite: false,
            is_enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

/// A (nestable) folder of snippets that can carry an app rule and insertion override.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupRecord {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "parentID", default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    #[serde(rename = "sortIndex", default)]
    pub sort_index: i64,
    #[serde(rename = "isEnabled", default = "default_true")]
    pub is_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<InsertionMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<AppRule>,
}

impl GroupRecord {
    /// A new enabled root group.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            parent_id: None,
            sort_index: 0,
            is_enabled: true,
            insertion: None,
            rule: None,
        }
    }
}

/// Usage of one snippet on one device. Summed across devices for display.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UsageRecord {
    #[serde(rename = "snippetID")]
    pub snippet_id: Uuid,
    #[serde(rename = "deviceID", default)]
    pub device_id: String,
    #[serde(default)]
    pub count: i64,
    #[serde(rename = "charsSaved", default)]
    pub chars_saved: i64,
    #[serde(rename = "lastUsedAt", with = "iso8601_opt", default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
}

/// A whole library: the unit of import, export and backup.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KudzuLibrary {
    #[serde(rename = "formatVersion", default = "current_format_version")]
    pub format_version: i64,
    #[serde(default)]
    pub groups: Vec<GroupRecord>,
    #[serde(default)]
    pub snippets: Vec<SnippetRecord>,
    #[serde(default)]
    pub usage: Vec<UsageRecord>,
}

impl KudzuLibrary {
    /// The library format version this build writes.
    pub const CURRENT_FORMAT_VERSION: i64 = 1;
}

impl Default for KudzuLibrary {
    fn default() -> Self {
        Self { format_version: Self::CURRENT_FORMAT_VERSION, groups: vec![], snippets: vec![], usage: vec![] }
    }
}

fn default_true() -> bool {
    true
}

fn current_format_version() -> i64 {
    KudzuLibrary::CURRENT_FORMAT_VERSION
}

/// ISO-8601 dates as the Swift app writes them: whole seconds, `Z` suffix. Parsing also
/// accepts fractional seconds and numeric offsets.
pub mod iso8601 {
    use chrono::{DateTime, SecondsFormat, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    /// Formats `date` as `yyyy-MM-ddTHH:mm:ssZ`.
    pub fn format(date: &DateTime<Utc>) -> String {
        date.to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    /// Parses an RFC 3339 / ISO-8601 timestamp.
    pub fn parse(s: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
    }

    pub fn serialize<S: Serializer>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format(date))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let raw = String::deserialize(d)?;
        parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("invalid ISO-8601 date: {raw}")))
    }
}

/// Optional variant of [`iso8601`].
pub mod iso8601_opt {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(date: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
        match date {
            Some(d) => s.serialize_str(&super::iso8601::format(d)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        let raw = Option::<String>::deserialize(d)?;
        match raw {
            None => Ok(None),
            Some(r) => super::iso8601::parse(&r)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("invalid ISO-8601 date: {r}"))),
        }
    }
}
