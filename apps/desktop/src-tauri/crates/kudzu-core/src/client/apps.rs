//! Installed apps (`lib/apps.ts`): naming a bundle id for app pickers and rules.

use serde::{Deserialize, Serialize};

/// An installed or running app, as the backend lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub bundle_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The first app with `bundle_id`'s name, else the bundle id itself.
pub fn app_name<'a>(apps: &'a [AppInfo], bundle_id: &'a str) -> &'a str {
    apps.iter().find(|a| a.bundle_id == bundle_id).map_or(bundle_id, |a| a.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_apps() {
        let apps = vec![
            AppInfo { bundle_id: "com.apple.Safari".into(), name: "Safari".into(), pid: None, path: None },
            AppInfo { bundle_id: "com.apple.Safari".into(), name: "Other".into(), pid: Some(1), path: None },
        ];
        assert_eq!(app_name(&apps, "com.apple.Safari"), "Safari");
        assert_eq!(app_name(&apps, "com.unknown"), "com.unknown");
        let json: AppInfo = serde_json::from_str(r#"{"bundleId":"a","name":"A","pid":null}"#).unwrap();
        assert_eq!(json.pid, None);
    }
}
