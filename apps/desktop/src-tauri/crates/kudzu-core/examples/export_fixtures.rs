//! Writes golden JSON fixtures for the native macOS port (`apps/macos`), so its Swift tests replay
//! exactly what this crate produces. See `apps/macos/docs/MIGRATION.md` §6.
//!
//! ```sh
//! cd apps/desktop/src-tauri && cargo run -p kudzu-core --example export_fixtures
//! ```
//!
//! Output goes to `apps/macos/Packages/KudzuKit/Tests/KudzuCoreTests/Fixtures/<section>/`. Each
//! section is a module with `pub fn export(dir: &Path)`; add new ones (template render cases,
//! matcher traces, import corpora, secrets, sync merges) next to `models` and call them from `main`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../macos/Packages/KudzuKit/Tests/KudzuCoreTests/Fixtures");
    models::export(&dir.join("models"));
    template::export(&dir);   // T1
    matching::export(&dir);   // T1
    secrets::export(&dir.join("secrets")); // T2
    clipboard::export(&dir.join("clipboard")); // T2
    sync::export(&dir.join("sync")); // T2
    interchange::export(&dir.join("interchange")); // T2
    println!("fixtures written to {}", dir.canonicalize().unwrap_or(dir).display());
}

/// Writes `value` as pretty JSON (with a trailing newline) to `dir/name`.
fn write<T: Serialize + ?Sized>(dir: &Path, name: &str, value: &T) {
    write_raw(dir, name, &serde_json::to_string_pretty(value).expect("serializable fixture"));
}

fn write_raw(dir: &Path, name: &str, json: &str) {
    fs::create_dir_all(dir).expect("create fixture dir");
    fs::write(dir.join(name), format!("{json}\n")).expect("write fixture");
}

/// The JSON contract of the models (`KudzuCore/Models`): every type the app persists or sends.
mod models {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use chrono::{DateTime, FixedOffset, TimeZone, Utc};
    use kudzu_core::clipboard::{ClipEntry, ClipKind, ClipSource, ImageSize};
    use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};
    use kudzu_core::interchange::backup::{BackupInfo, BackupReason};
    use kudzu_core::interchange::coordinator::{ImportSummary, WarningGroup};
    use kudzu_core::interchange::dedupe::{DedupeConflict, DedupeReport};
    use kudzu_core::interchange::espanso::ImportWarning;
    use kudzu_core::interchange::kudzu_json;
    use kudzu_core::matching::find_conflicts;
    use kudzu_core::secrets::SecretKind;
    use kudzu_core::template::{self, RenderContext};
    use kudzu_core::{
        AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary, SnippetRecord, UsageRecord,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::{write, write_raw};

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(0xa1b2c3d4_e5f6_4a7b_8c9d_000000000000 | n)
    }

    /// 2026-11-08 09:30:15 UTC plus `micros` (µs: `Date` can't hold nanoseconds exactly).
    fn at(seconds_later: i64, micros: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 8, 9, 30, 15).unwrap()
            + chrono::Duration::seconds(seconds_later)
            + chrono::Duration::microseconds(i64::from(micros))
    }

    pub fn export(dir: &Path) {
        library(dir);
        clip_entries(dir);
        template_results(dir);
        conflicts(dir);
        interchange(dir);
        tauri_shapes(dir);
    }

    // MARK: Library

    fn library(dir: &Path) {
        let work = GroupRecord { id: id(1), name: "Work".into(), parent_id: None, sort_index: 0, is_enabled: true, insertion: None, rule: None };
        let email = GroupRecord {
            id: id(2),
            name: "Email".into(),
            parent_id: Some(work.id),
            sort_index: 0,
            is_enabled: true,
            insertion: Some(InsertionMethod::Paste),
            rule: Some(AppRule { mode: AppRuleMode::Allow, bundle_ids: vec!["com.apple.mail".into()], insertion: None }),
        };
        let archived = GroupRecord {
            id: id(3),
            name: "Archive — old".into(),
            parent_id: Some(email.id),
            sort_index: 2,
            is_enabled: false,
            insertion: None,
            rule: Some(AppRule { mode: AppRuleMode::Deny, bundle_ids: vec![], insertion: Some(InsertionMethod::Type) }),
        };
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let content = "Hello **world**, see docs and {{cursor}} ￼";
        let full = SnippetRecord {
            id: id(10),
            name: "Everything set".into(),
            triggers: vec![";all".into(), ";ALL2".into()],
            content: content.into(),
            formatting: Some(Formatting {
                source: content.into(),
                spans: vec![
                    FormatSpan { start: 0, end: 5, style: FormatStyle::Bold },
                    FormatSpan { start: 6, end: 15, style: FormatStyle::Italic },
                    FormatSpan { start: 6, end: 15, style: FormatStyle::Underline },
                    FormatSpan { start: 21, end: 25, style: FormatStyle::Link { url: "https://kudzu.site/docs?a=1&b=2".into() } },
                    FormatSpan { start: 41, end: 42, style: FormatStyle::Image { data_url: png.into(), width: 240 } },
                ],
            }),
            group_id: Some(email.id),
            fire_mode: FireMode::WholeWord,
            case_mode: CaseMode::Exact,
            insertion: Some(InsertionMethod::Type),
            rule: Some(AppRule {
                mode: AppRuleMode::Deny,
                bundle_ids: vec!["com.tinyspeck.slackmacgap".into(), "com.apple.Terminal".into()],
                insertion: Some(InsertionMethod::Paste),
            }),
            is_favorite: true,
            is_enabled: false,
            created_at: at(-86_400 * 40, 0),
            updated_at: at(0, 0),
        };
        let bare = SnippetRecord {
            id: id(11),
            name: "Defaults only".into(),
            triggers: vec![";d".into()],
            content: "Unicode: café 🚀 \"quotes\" \\ / \n\ttab".into(),
            formatting: None,
            group_id: None,
            fire_mode: FireMode::Immediate,
            case_mode: CaseMode::Propagate,
            insertion: None,
            rule: None,
            is_favorite: false,
            is_enabled: true,
            created_at: at(-60, 0),
            updated_at: at(-30, 0),
        };
        let empty = SnippetRecord {
            id: id(12),
            name: String::new(),
            triggers: vec![],
            content: String::new(),
            formatting: None,
            group_id: Some(archived.id),
            fire_mode: FireMode::Immediate,
            case_mode: CaseMode::Propagate,
            insertion: Some(InsertionMethod::Automatic),
            rule: Some(AppRule { mode: AppRuleMode::Allow, bundle_ids: vec!["com.apple.Notes".into()], insertion: None }),
            is_favorite: false,
            is_enabled: true,
            created_at: at(0, 0),
            updated_at: at(0, 0),
        };
        let usage = vec![
            UsageRecord { snippet_id: full.id, device_id: "mac-studio".into(), count: 128, chars_saved: 4096, last_used_at: Some(at(-3600, 0)) },
            UsageRecord { snippet_id: full.id, device_id: "macbook".into(), count: 3, chars_saved: -2, last_used_at: None },
            UsageRecord { snippet_id: bare.id, device_id: String::new(), count: 0, chars_saved: 0, last_used_at: Some(at(-1, 0)) },
        ];
        let library = KudzuLibrary { format_version: 1, groups: vec![work, email, archived], snippets: vec![full, bare, empty], usage };
        // `library.json` exactly as the Tauri store writes it (sorted keys).
        write_raw(dir, "library.json", &kudzu_json::encode(&library).expect("encode library"));

        // What older or foreign files leave out: serde defaults fill the rest. Uppercase ids and a
        // nanosecond date with an offset must still decode.
        let minimal = r#"{"snippets":[{"id":"A1B2C3D4-E5F6-4A7B-8C9D-0000000000FF","createdAt":"2026-11-08T09:30:15.123456789+02:00","updatedAt":"2026-11-08T09:30:15Z"}],"groups":[{"id":"a1b2c3d4-e5f6-4a7b-8c9d-0000000000fe"}],"usage":[{"snippetID":"a1b2c3d4-e5f6-4a7b-8c9d-0000000000ff"}]}"#;
        write_raw(dir, "library-minimal.json", minimal);
        let decoded = kudzu_json::decode(minimal).expect("decode minimal library");
        // The same library after a decode → encode in Rust (defaults applied, dates normalized).
        write_raw(dir, "library-minimal-normalized.json", &kudzu_json::encode(&decoded).expect("encode minimal"));
    }

    // MARK: Clipboard

    fn clip_entries(dir: &Path) {
        let source = |bundle: &str, name: &str, path: Option<&str>| ClipSource {
            bundle_id: bundle.into(),
            name: name.into(),
            path: path.map(String::from),
        };
        let base = |n: u128, kind: ClipKind| ClipEntry {
            id: id(100 + n),
            kind,
            text: None,
            text_len: 0,
            line_count: 0,
            files: vec![],
            image: None,
            color: None,
            rich: false,
            types: vec![],
            size: 0,
            source: None,
            hash: format!("{:016x}", 0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(n as u64 + 1)),
            first_copied_at: at(-7200, 0),
            copied_at: at(-60, 0),
            copy_count: 1,
            timings: vec![at(-60, 0)],
            last_used_at: None,
            use_count: 0,
            pinned: false,
            secret: None,
            secret_override: None,
        };
        let entries = vec![
            ClipEntry {
                text: Some("Can we move the review to Thursday?".into()),
                text_len: 35,
                line_count: 1,
                rich: true,
                types: vec!["public.utf8-plain-text".into(), "public.html".into()],
                size: 412,
                source: Some(source("com.tinyspeck.slackmacgap", "Slack", Some("/Applications/Slack.app"))),
                copy_count: 3,
                timings: vec![at(-7200, 0), at(-3600, 123_000), at(-60, 123_456)],
                copied_at: at(-60, 123_456),
                ..base(0, ClipKind::Text)
            },
            ClipEntry {
                text: Some("export function x() {\n  return 1\n}".into()),
                text_len: 34,
                line_count: 3,
                types: vec!["public.utf8-plain-text".into()],
                size: 34,
                source: Some(source("com.microsoft.VSCode", "Visual Studio Code", None)),
                last_used_at: Some(at(-30, 500_000)),
                use_count: 7,
                ..base(1, ClipKind::Code)
            },
            ClipEntry { text: Some("https://kudzu.site/".into()), text_len: 19, line_count: 1, pinned: true, ..base(2, ClipKind::Link) },
            ClipEntry { text: Some("#5B7CFA".into()), color: Some("#5b7cfa".into()), text_len: 7, line_count: 1, ..base(3, ClipKind::Color) },
            ClipEntry {
                image: Some(ImageSize { width: 2880, height: 1800 }),
                types: vec!["public.png".into(), "public.tiff".into()],
                size: 4_665_600,
                ..base(4, ClipKind::Image)
            },
            ClipEntry {
                files: vec!["/Users/me/Desktop/hero@2x.png".into(), "/Users/me/My File.txt".into()],
                text: Some("hero@2x.png\nMy File.txt".into()),
                text_len: 23,
                line_count: 2,
                types: vec!["public.file-url".into()],
                size: 220,
                ..base(5, ClipKind::File)
            },
            ClipEntry { types: vec!["com.figma.clipboard".into()], size: 182_311, copy_count: 4_000_000_000, ..base(6, ClipKind::Other) },
            ClipEntry {
                text: Some("kezwy4-dyxbeR-mosmac".into()),
                text_len: 20,
                line_count: 1,
                secret: Some(SecretKind::Password),
                secret_override: Some(false),
                ..base(7, ClipKind::Text)
            },
            ClipEntry {
                text: Some("hide me".into()),
                text_len: 7,
                line_count: 1,
                secret_override: Some(true),
                timings: vec![],
                ..base(8, ClipKind::Text)
            },
            ClipEntry {
                text: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
                text_len: 35,
                line_count: 1,
                secret: Some(SecretKind::PrivateKey),
                ..base(9, ClipKind::Code)
            },
        ];
        write(dir, "clip-entries.json", &entries);
    }

    // MARK: Template results

    fn template_results(dir: &Path) {
        let now: DateTime<FixedOffset> = FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2026, 11, 8, 11, 30, 15).unwrap();
        let resolve = |_: Uuid| None;
        let mut form_values = HashMap::new();
        form_values.insert("Name".to_string(), "Ada".to_string());
        let context = RenderContext {
            now,
            locale: "en_US".into(),
            clipboard: Some("clip".into()),
            form_values,
            resolve_snippet: &resolve,
            resolve_formatting: None,
            root_snippet_id: None,
        };
        let formatted_source = "Hi {{field name=\"Name\"}}!";
        let formatting = Formatting {
            source: formatted_source.into(),
            spans: vec![FormatSpan { start: 0, end: 2, style: FormatStyle::Bold }],
        };
        let results = vec![
            template::render("Hello {{cursor}}world", &context),
            template::render("Broken {{date", &context),
            template::render("{{cursor}}{{cursor}} {{date format=\"yyyy\" offset=\"bogus\"}}", &context),
            template::render_formatted(formatted_source, Some(&formatting), &context),
        ];
        write(dir, "render-results.json", &results);
        let fields = template::form_fields(
            "{{field name=\"Name\"}} {{field name=\"Notes\" multiline=true default=\"none\"}} {{choice name=\"Size\" options=\"S|M|L\" default=\"M\"}}",
            &resolve,
        );
        write(dir, "form-fields.json", &fields);
    }

    // MARK: Conflicts

    fn conflicts(dir: &Path) {
        let mk = |n: u128, trigger: &str| SnippetRecord { id: id(200 + n), ..SnippetRecord::new("s", vec![trigger.into()], "x") };
        let snippets = vec![mk(1, ";a"), mk(2, ";a"), mk(3, ";ab")];
        write(dir, "conflicts.json", &find_conflicts(&snippets, &[]));
    }

    // MARK: Interchange

    fn interchange(dir: &Path) {
        let backup = BackupInfo {
            path: PathBuf::from("/Users/me/Library/Application Support/sh.saeed.kudzu/backups/kudzu-2026-11-08-preImport.json"),
            filename: "kudzu-2026-11-08-preImport.json".into(),
            created_at: at(0, 0),
            reason: BackupReason::PreImport,
        };
        let backups = vec![
            backup.clone(),
            BackupInfo { reason: BackupReason::Daily, filename: "kudzu-2026-11-08-daily.json".into(), ..backup.clone() },
            BackupInfo { reason: BackupReason::PreRestore, ..backup.clone() },
            BackupInfo { reason: BackupReason::Manual, ..backup.clone() },
            BackupInfo { reason: BackupReason::PreUpdate, ..backup.clone() },
        ];
        write(dir, "backups.json", &backups);
        let warnings = vec![
            ImportWarning { file: "match/base.yml".into(), line: Some(12), match_index: Some(3), trigger: Some(":sig".into()), message: "Script extensions aren't supported".into() },
            ImportWarning { file: "export.csv".into(), line: None, match_index: None, trigger: None, message: "Script snippet isn’t supported and was skipped.".into() },
        ];
        let summary = ImportSummary {
            imported_snippets: 6,
            imported_groups: 1,
            backup,
            dedupe_report: DedupeReport {
                merged_groups: 1,
                merged_snippets: 2,
                merged_usage_rows: 3,
                conflicts: vec![DedupeConflict { trigger: ";sig".into(), snippet_ids: vec![id(10), id(11)] }],
            },
            warnings: warnings.clone(),
            grouped_warnings: vec![WarningGroup { message: warnings[0].message.clone(), count: 1, warnings: vec![warnings[0].clone()] }],
        };
        write(dir, "import-summaries.json", &[summary]);
    }

    // MARK: Tauri-crate and engine shapes (hand-made)

    /// `Settings`, `UpdateState`, `SyncStatus`, `AgentSetup`, `ClipEvent`, `ClipboardInfo`,
    /// `FormPayload`, `EspansoDetection` and the `navigate` view live in the Tauri crate, and
    /// `EngineStatus`, `AppInfo`, `FormRequest`, `ExpansionEvent` in `kudzu-engine`; neither is a
    /// dependency of this crate. These fixtures are hand-copied from their serde attributes
    /// (`Option` without `skip_serializing_if` → `null`). Keep them in sync when those types change.
    fn tauri_shapes(dir: &Path) {
        let settings = json!({
            "typingSpeedWPM": 55, "defaultTriggerPrefix": "//", "paletteHotkey": "Control+Alt+K", "paused": true,
            "defaultInsertion": "paste", "deniedBundleIDs": ["com.apple.loginwindow"],
            "appInsertions": {"com.apple.Terminal": "paste", "com.apple.mail": "type"},
            "launchAtLogin": true, "onboardingCompleted": true, "theme": "system", "showInDock": false,
            "autoCheckUpdates": false, "updateChannel": "beta", "icloudSync": false, "lastRunVersion": "0.6.0",
            "clipboardEnabled": false, "clipboardRetentionDays": 0, "clipboardHotkey": "Control+Alt+Shift+V",
            "clipboardIgnoredApps": [], "clipboardPastePlain": true, "clipboardShowSecrets": true,
            "agentAccess": true, "agentWrite": false, "agentClipboard": false
        });
        let mut settings_null = settings.clone();
        settings_null["lastRunVersion"] = serde_json::Value::Null;
        write(dir, "settings.json", &vec![settings, settings_null]);

        write(dir, "update-states.json", &json!([
            {"phase": "idle"},
            {"phase": "checking"},
            {"phase": "upToDate", "checkedAt": "2026-11-08T09:30:15.123456+00:00"},
            {"phase": "downloading", "version": "0.7.0", "downloaded": 1024, "total": 4_194_304},
            {"phase": "downloading", "version": "0.7.0", "downloaded": 0, "total": null},
            {"phase": "ready", "version": "0.7.0", "notes": "## 0.7.0\n\n* faster", "date": "2026-11-08T09:30:15Z"},
            {"phase": "ready", "version": "0.7.0", "notes": null, "date": null},
            {"phase": "restarting", "version": "0.7.0"},
            {"phase": "error", "message": "Move Kudzu to Applications to update it.", "needsMove": true}
        ]));
        write(dir, "sync-statuses.json", &json!([
            {"state": "off"},
            {"state": "unavailable", "reason": "iCloud sync is unavailable in Kudzu Dev. Data stays on this Mac."},
            {"state": "noAccount", "reason": "Sign in to iCloud in System Settings."},
            {"state": "starting"},
            {"state": "syncing"},
            {"state": "upToDate", "lastSync": "2026-11-08T09:30:15.123+00:00"},
            {"state": "needsUpdate"},
            {"state": "error", "message": "CloudKit said no"}
        ]));
        write(dir, "agent-setups.json", &json!([
            {
                "running": true, "name": "kudzu-dev", "command": "/Applications/Kudzu Dev.app/Contents/MacOS/Kudzu Dev",
                "args": ["--mcp", "--dev"],
                "claudeCommand": "claude mcp add --scope user kudzu-dev -- '/Applications/Kudzu Dev.app/Contents/MacOS/Kudzu Dev' --mcp --dev",
                "configJson": "{\n  \"mcpServers\": {}\n}", "skillPath": "~/.claude/skills/kudzu/SKILL.md", "skill": "outdated",
                "bundledSkillDir": null
            },
            {
                "running": false, "name": "kudzu", "command": "/Applications/Kudzu.app/Contents/MacOS/Kudzu", "args": ["--mcp"],
                "claudeCommand": "claude mcp add --scope user kudzu -- /Applications/Kudzu.app/Contents/MacOS/Kudzu --mcp",
                "configJson": "{}", "skillPath": "~/.claude/skills/kudzu/SKILL.md", "skill": "missing",
                "bundledSkillDir": "/Applications/Kudzu.app/Contents/Resources/skills/kudzu"
            }
        ]));
        let app = json!({"bundleId": "com.apple.mail", "name": "Mail", "pid": 812, "path": "/System/Applications/Mail.app"});
        let app_bare = json!({"bundleId": "com.apple.Notes", "name": "Notes", "pid": null, "path": null});
        write(dir, "app-infos.json", &json!([app, app_bare]));
        write(dir, "engine-statuses.json", &json!([
            {
                "canPostEvents": true, "canExpand": false, "paused": false, "tap": "listenOnly",
                "permissions": {"accessibility": true, "inputMonitoring": false}, "secureInput": true,
                "secureInputOwner": "loginwindow", "summary": "Secure Input on — expansion blocked"
            },
            {
                "canPostEvents": false, "canExpand": false, "paused": true, "tap": "off",
                "permissions": {"accessibility": false, "inputMonitoring": false}, "secureInput": false,
                "secureInputOwner": null, "summary": "Paused"
            },
            {
                "canPostEvents": true, "canExpand": true, "paused": false, "tap": "active",
                "permissions": {"accessibility": true, "inputMonitoring": true}, "secureInput": false,
                "secureInputOwner": null, "summary": "Ready"
            }
        ]));
        let field = json!({"name": "Name", "kind": "text", "options": [], "defaultValue": null});
        let choice = json!({"name": "Size", "kind": "choice", "options": ["S", "M"], "defaultValue": "M"});
        write(dir, "form-requests.json", &json!([
            {"token": 7, "snippetId": id(10).to_string(), "snippetName": "Bug report", "fields": [field, choice],
             "caret": {"x": 120.5, "y": 300.0, "width": 1.0, "height": 18.0}, "targetApp": app},
            {"token": 18_446_744_073_709_551_615_u64, "snippetId": id(11).to_string(), "snippetName": "", "fields": [], "caret": null, "targetApp": null}
        ]));
        write(dir, "form-payloads.json", &json!([
            {"token": 1, "snippetId": id(10).to_string(), "snippetName": "Bug report", "fields": [field, choice],
             "initialValues": {"Name": "Ada", "Size": "M"}, "content": "Hi {{field name=\"Name\"}}", "targetApp": app_bare},
            {"token": 2, "snippetId": id(11).to_string(), "snippetName": "x", "fields": [], "initialValues": {}, "content": "", "targetApp": null}
        ]));
        write(dir, "expansion-events.json", &json!([{"snippetId": id(10).to_string(), "charsSaved": 42}]));
        let entry: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("clip-entries.json")).unwrap()).unwrap();
        write(dir, "clip-events.json", &json!([
            {"type": "upsert", "entry": entry[0]},
            {"type": "remove", "ids": [id(100).to_string(), id(101).to_string()]},
            {"type": "reset", "entries": [entry[1], entry[2]]},
            {"type": "reset", "entries": []}
        ]));
        write(dir, "clipboard-infos.json", &json!([{"entries": [entry[3]], "diskUsage": 38_400_000, "canPaste": false}]));
        write(dir, "espanso-detections.json", &json!([{"path": "/Users/me/Library/Application Support/espanso", "snippetCount": 12, "groupCount": 3}]));
        // `lib/store.ts` `View`, as `navigate` events and `show_main_view` carry it.
        write(dir, "routes.json", &json!([
            {"kind": "all"}, {"kind": "favorites"}, {"kind": "recent"}, {"kind": "disabled"},
            {"kind": "group", "id": id(2).to_string()}, {"kind": "insights"},
            {"kind": "clipboard"}, {"kind": "clipboard", "filter": "pinned"}, {"kind": "clipboard", "filter": "image"},
            {"kind": "settings"}, {"kind": "settings", "section": "about"}
        ]));
    }
}

// T1: fixed template inputs; expected results are always recomputed by the Rust reference.
mod template {
    use super::*;
    use std::collections::HashMap;
    use kudzu_core::{template as t, formatting::Formatting};
    use chrono::{DateTime, FixedOffset};
    use serde_json::{Value, json};
    use uuid::Uuid;
    pub fn export(dir: &Path) {
        let base:Value=serde_json::from_str(BASE).unwrap();
        let sources:Vec<String>=serde_json::from_str(SOURCES).unwrap();
        let locales:Vec<String>=serde_json::from_str(LOCALES).unwrap();
        let mut cases=Vec::new();
        for locale in locales { for source in &sources {
            let mut c=base.clone();c["source"]=json!(source);c["locale"]=json!(locale);c["formatting"]=json!({"source":source,"spans":[{"start":0,"end":source.encode_utf16().count(),"style":{"kind":"bold"}}]});cases.push(c);
        }}
        cases.extend(serde_json::from_str::<Vec<Value>>(RICH_CASES).unwrap());
        cases.extend(adversarial::render_cases(&base));
        for c in &mut cases {
            let source=c["source"].as_str().unwrap().to_string();
            let snippets: HashMap<Uuid,String>=serde_json::from_value(c["resolveMap"].clone()).unwrap();
            let rich: HashMap<Uuid,Formatting>=serde_json::from_value(c.get("resolveFormatting").cloned().unwrap_or(json!({}))).unwrap();
            let resolve=|id|snippets.get(&id).cloned();
            let resolve_rich=|id|rich.get(&id).cloned();
            let ctx=t::RenderContext {
                now:c["now"].as_str().unwrap().parse().unwrap(), locale:c["locale"].as_str().unwrap().into(),
                clipboard:c.get("clipboard").and_then(Value::as_str).map(str::to_string),
                form_values:serde_json::from_value(c["formValues"].clone()).unwrap(),
                resolve_snippet:&resolve,resolve_formatting:Some(&resolve_rich),
                root_snippet_id:c.get("rootSnippetID").and_then(Value::as_str).map(|s|s.parse().unwrap()),
            };
            let formatting: Option<Formatting>=serde_json::from_value(c.get("formatting").cloned().unwrap_or(Value::Null)).unwrap();
            c["expected"]=serde_json::to_value(t::render_formatted(&source,formatting.as_ref(),&ctx)).unwrap();
        }
        // Top-level names are shared with the Swift fixture replay tests.
        fs::write(dir.join("template.json"),serde_json::to_string_pretty(&cases).unwrap()).unwrap();
        let mut icu=Vec::new();
        let dates:Vec<String>=serde_json::from_str(DATES).unwrap();
        let locales:Vec<String>=serde_json::from_str(ICU_LOCALES).unwrap();
        let patterns:Vec<String>=serde_json::from_str(PATTERNS).unwrap();
        for date in &dates { for locale in &locales { for pattern in &patterns { icu.push(json!({"now":date,"locale":locale,"pattern":pattern})); }}}
        icu.extend(adversarial::icu_cases(&dates, &patterns));
        for c in &mut icu {
            let date: DateTime<FixedOffset>=c["now"].as_str().unwrap().parse().unwrap();
            c["expected"]=json!(t::format_icu(&date,c["pattern"].as_str().unwrap(),c["locale"].as_str().unwrap()));
        }
        fs::write(dir.join("template-icu.json"),serde_json::to_string_pretty(&icu).unwrap()).unwrap();
        let mut parse_sources=sources.clone();
        parse_sources.extend(adversarial::parse_sources());
        let resolve_map: HashMap<Uuid,String>=serde_json::from_value(base["resolveMap"].clone()).unwrap();
        let parse: Vec<Value>=parse_sources.iter().map(|source| adversarial::parse_case(source,&resolve_map)).collect();
        fs::write(dir.join("template-parse.json"),serde_json::to_string_pretty(&parse).unwrap()).unwrap();
        fs::write(dir.join("template-formatting.json"),serde_json::to_string_pretty(&adversarial::formatting()).unwrap()).unwrap();
    }

    /// Adversarial inputs added by the T1 review (MIGRATION.md §5 V1): grammar corners, escapes,
    /// offsets at chrono's range limits, exotic whitespace, locale spellings and formatting spans at
    /// every boundary. Inputs only; every expectation is computed by the Rust reference.
    mod adversarial {
        use std::collections::HashMap;
        use kudzu_core::template::{self as t, Node, TemplateTag};
        use serde_json::{json, Value};
        use uuid::Uuid;

        const STAMP: &str = "uuuu-MM-dd HH:mm:ss xxx";
        const OFFSETS: &[&str] = &[
            "+1m", "-1m", "+12m", "+1y", "-1y", "+1m1d", "+1d1m", "+1y1m", "-1y2m3w4d5h6min", "+0d", "-0d", "+01d",
            "+90min", "+1h30min", "+1w", "+25h", "+1d-1d", "+1D", "+1 d", "+1", "+d", "+", "-", "", "1d",
            "+1min30", "+1mi", "+1minute", "+1dd", "+١d", "+1d\u{301}", "+1é",
            "+260117y", "+260118y", "+260119y", "-264166y", "-264167y", "-264168y", "+3121403m", "-3170016m",
            "+3000000y", "+99999999999m", "+4294967296m", "+2147483648m", "+106751991167d", "+106751991168d",
            "+15250284452w", "+15250284453w", "+2562047788015h", "+2562047788016h", "+153722867280912min",
            "+153722867280913min", "+9223372036854775807min", "+9223372036854775808d", "+95000000d", "-95000000d",
        ];
        const NOWS: &[&str] = &[
            "2024-01-31T14:07:09+05:30", "2024-02-29T23:30:00-04:00", "2023-03-31T00:15:00Z", "1582-10-04T12:00:00Z",
            "1583-01-01T00:00:00+14:00", "0001-01-01T00:00:00Z", "9999-12-31T23:59:59-12:00",
        ];

        fn bold(source: &str) -> Value {
            json!({"source":source,"spans":[{"start":0,"end":source.encode_utf16().count(),"style":{"kind":"bold"}}]})
        }

        /// Spans at every UTF-16 boundary: (i, i+1) italic, (0, i) bold, (i, n) underline, plus a whole-text link.
        fn stress(source: &str) -> Value {
            let n = source.encode_utf16().count();
            let mut spans = vec![json!({"start":0,"end":n,"style":{"kind":"link","url":"https://kudzu.site"}})];
            for i in 0..n {
                spans.push(json!({"start":i,"end":i+1,"style":{"kind":"italic"}}));
                spans.push(json!({"start":0,"end":i+1,"style":{"kind":"bold"}}));
                spans.push(json!({"start":i,"end":n,"style":{"kind":"underline"}}));
            }
            json!({"source":source,"spans":spans})
        }

        pub fn render_cases(base: &Value) -> Vec<Value> {
            let mut cases = Vec::new();
            let mut push = |overrides: Value| {
                let mut c = base.clone();
                c["locale"] = json!("en_US");
                for (k, v) in overrides.as_object().unwrap() { c[k] = v.clone(); }
                if overrides.get("formatting").is_none() { c["formatting"] = bold(c["source"].as_str().unwrap()); }
                cases.push(c);
            };
            for now in NOWS { for offset in OFFSETS {
                let source = format!("{{{{date offset=\"{offset}\" format=\"{STAMP}\"}}}}");
                push(json!({"source":source,"now":now,"formatting":null}));
            }}
            // Bare offsets go through the bare-token path; the format default applies.
            for offset in ["+1m", "-1y2m", "+1d-1d", "+1.5d", "+1e3d"] {
                push(json!({"source":format!("{{{{date offset={offset}}}}} {{{{time offset={offset}}}}}")}));
            }
            let clip = "\u{feff}\u{200b} \u{3000}\u{a0}\u{85}x y\u{2028}\u{180e}\u{2029}\t\u{b}\u{c}\r\n";
            push(json!({"source":"[{{clipboard}}]/[{{clipboard trim=true}}]","clipboard":clip}));
            push(json!({"source":"[{{clipboard trim=true}}]","clipboard":" \u{301}x\u{301} "}));
            push(json!({"source":"[{{clipboard trim=true}}][{{clipboard}}]","clipboard":null}));
            for locale in ["", "en", "en-GB", "EN_us", "AR_eg", "zh-Hant-TW", "de@calendar=buddhist", "fr_FR.UTF-8", "iw_IL", "es-419", "pt_PT", "nl-BE"] {
                push(json!({"source":"{{date format=\"EEEE e c MMMM G a\"}}","locale":locale,"formatting":null}));
            }
            for source in [
                "{{date locale=de_DE format=\"EEEE d. MMMM\"}}", "{{date locale=\"\" format=\"EEEE e\"}}", "{{time locale=ar format=\"h:mm a\"}}",
                "{{date locale=xx format=\"MMMM\"}}", "{{date format=\"\"}}", "{{time format=\"''\"}}", "{{date format=\"'\"}}",
                "{{field name=who default=x}}{{field name=who default=y}}{{choice name=who options=\"a|b\"}}",
                "{{choice name=c options=\"\" default=\"\"}}|{{choice name=d}}|{{field name=\"\"}}",
                "{{snippet id=\"00000000-0000-0000-0000-00000000000A\"}}", "{{snippet id=\"00000000-0000-0000-0000-000000000009\"}}",
                "{{snippet id=\"{00000000-0000-0000-0000-000000000001}\"}}", "{{snippet id=00000000000000000000000000000001}}",
                "{{snippet id=\"urn:uuid:00000000-0000-0000-0000-000000000001\"}}", "{{snippet id=\"00000000-0000-0000-0000-0000000000011\"}}",
                "{{snippet id=00000000-0000-0000-0000-000000000004}}", "{{snippet id=00000000-0000-0000-0000-000000000005}}",
                "{{date format=\"MMMM\" format=\"yyyy\"}}{{cursor}}{{cursor}}", "x\\\\\\\\y\\{{z\\{x\\\\{{cursor}}",
                "{{field name=x default=\"\\\\\u{301}\\\"\"}}", "{{field name=x default=\"a\\|b\"}}", "{{date}}\u{301}", "\\\u{301}{{cursor}}",
            ] { push(json!({"source":source})); }
            let nested = "nested {{cursor}}{{field name=who}}";
            let rich = json!({"00000000-0000-0000-0000-000000000001": stress(nested)});
            for source in [
                "a\\\\b\\{{c}} {{cursor}}d", "😀\\{{x}}{{field name=who}}é\u{301}z", "{{{date}}}{{{{time}}", "\\\\\\{{\\\\{{cursor}}",
                "x{{snippet id=\"00000000-0000-0000-0000-000000000001\"}}y{{snippet id=\"00000000-0000-0000-0000-000000000002\"}}z",
                "{{nope}}-{{field name=\"a}}b\"}}-{{snippet id=\"00000000-0000-0000-0000-000000000009\"}}",
            ] { push(json!({"source":source,"formatting":stress(source),"resolveFormatting":rich})); }
            // Nested formatting is used only when the root is formatted and the nested source matches.
            push(json!({"source":"{{snippet id=\"00000000-0000-0000-0000-000000000001\"}}","formatting":null,"resolveFormatting":rich}));
            push(json!({"source":"[{{snippet id=\"00000000-0000-0000-0000-000000000001\"}}]","formatting":{"source":"[{{snippet id=\"00000000-0000-0000-0000-000000000001\"}}]","spans":[]},"resolveFormatting":{"00000000-0000-0000-0000-000000000001": stress("stale")}}));
            // Eight nested levels render; the ninth is rejected.
            let mut map = serde_json::Map::new();
            for level in 1..=10u32 {
                map.insert(format!("00000000-0000-0000-0000-0000000001{level:02}"), json!(format!("{level}<{{{{snippet id=\"00000000-0000-0000-0000-0000000001{:02}\"}}}}>", level + 1)));
            }
            push(json!({"source":"{{snippet id=\"00000000-0000-0000-0000-000000000101\"}}","resolveMap":map,"rootSnippetID":null}));
            // A diamond (two branches reach the same snippet) is not a cycle; A→B→A is.
            push(json!({"source":"{{snippet id=\"00000000-0000-0000-0000-000000000201\"}}","rootSnippetID":null,"resolveMap":{
                "00000000-0000-0000-0000-000000000201":"[{{snippet id=\"00000000-0000-0000-0000-000000000202\"}}{{snippet id=\"00000000-0000-0000-0000-000000000202\"}}]",
                "00000000-0000-0000-0000-000000000202":"({{snippet id=\"00000000-0000-0000-0000-000000000201\"}}{{cursor}})"}}));
            cases
        }

        pub fn parse_sources() -> Vec<String> {
            let mut sources: Vec<String> = [
                "{{date\tformat=\"x\"}}", "{{date\u{3000}format=\"x\"}}", "{{date\u{feff}format=\"x\"}}", "{{date\u{85}format=\"x\"}}",
                "{{date\u{a0}format=\"x\"}}", "{{date\u{200b}format=\"x\"}}", "{{\n date \n}}", "{{date\r\nformat=\"x\"}}", "{{date format=\"x\"format=\"y\"}}",
                "{{date format = \"x\" }}", "{{cursor }}", "{{ cursor}}", "{{date Format=\"x\"}}", "{{date fórmat=\"x\"}}", "{{dáte}}",
                "{{field name=a_b-c.d+e default=-0 multiline=true}}", "{{date n=-0 m=00.10 o=1. p=+ q=99999999999999999999999 r=1e5 s=0.1}}",
                "{{date n=123456789012345678901234567890.5 m=0.000000001 o=-1.0 p=+0 q=1_000 r=0x10 s=.}}", "{{date n=TRUE m=True o=false}}",
                "{{field name=x multiline=1}}", "{{field name=x multiline=TRUE}}", "{{clipboard trim=\"true\"}}", "{{clipboard trim=false trim=true}}",
                "{{field name=7}}", "{{field name=-1.5 default=1e5}}", "{{choice name=c options=a|b}}", "{{choice name=c options=\"a\\\\|b|\\|c\"}}",
                "{{date format=\"a\\nb\"}}", "{{date format=\"a\\\\\"}}", "{{date format=\"\\\"}}\\\"\"}}", "{{date format=\"}}", "{{\"}}\"}}",
                "{{date}}}", "{{date}}}}", "}}{{", "{{}}}}", "{{{{}}}}", "\\{{{{date}}", "\\\\{{date", "\\", "\\\\\\", "{", "{{", "}}",
                "{\u{301}{date}}", "{{\u{301}date}}", "{{date}\u{301}}", "{{date format=\"x\"\u{301}}}", "{{date format=\"\u{301}\"}}",
                "{{date format=\"a\u{301}\"}}", "\\\u{301}{{date}}", "\\{\u{301}{date}}", "a\u{301}\\\\b", "🇺🇸{{cursor}}👨‍👩‍👧", "\r\n{{cursor}}\r\n",
                "{{snippet id=\"00000000-0000-0000-0000-00000000000A\"}}", "{{snippet id=\"{00000000-0000-0000-0000-000000000001}\"}}",
                "{{snippet id=00000000000000000000000000000001}}", "{{snippet id=\"00000000-0000-0000-0000-0000000000011\"}}",
                "{{snippet id=\"0000000-00000-0000-0000-000000000001\"}}", "{{snippet id=\"00000000-0000-0000-0000-00000000000g\"}}",
                "{{snippet id=\"00000000-0000-0000-0000-000000000001\" id=\"x\"}}", "{{snippet id=\"x\" id=\"00000000-0000-0000-0000-000000000001\"}}",
                "{{snippet}}", "{{snippet id=5}}", "{{cursor x=1 y=\"2\" z=true}}", "{{time offset=+1d offset=nope}}", "{{time offset=nope offset=+1d}}",
                "{{field name=x name=y default=a default=\"b\"}}", "{{choice name=x options=\"\"}}", "{{choice name=x options=\"|\"}}",
                "{{date format=\"\\\u{301}\"}}", "{{date format=\"a\"\u{301}b\"}}", "{{choice name=c options=\"a|\u{301}b\\|\u{301}c\"}}",
                "{{field name=x default=\"<\u{301}&\u{301}'\u{301}\"}}",
            ].iter().map(|s| s.to_string()).collect();
            sources.extend(OFFSETS.iter().map(|o| format!("{{{{date offset=\"{o}\"}}}}")));
            sources
        }

        fn typed(tag: &t::Tag) -> Value {
            match t::typed(tag) {
                TemplateTag::Date { format, offset, locale } => json!({"kind":"date","format":format,"offset":offset.map(|o| o.parts),"locale":locale}),
                TemplateTag::Time { format, offset, locale } => json!({"kind":"time","format":format,"offset":offset.map(|o| o.parts),"locale":locale}),
                TemplateTag::Clipboard { trim } => json!({"kind":"clipboard","trim":trim}),
                TemplateTag::Cursor => json!({"kind":"cursor"}),
                TemplateTag::Snippet { id } => json!({"kind":"snippet","id":id}),
                TemplateTag::Field { name, default, multiline } => json!({"kind":"field","name":name,"default":default,"multiline":multiline}),
                TemplateTag::Choice { name, options, default } => json!({"kind":"choice","name":name,"options":options,"default":default}),
                TemplateTag::Unknown => json!({"kind":"unknown"}),
            }
        }

        pub fn parse_case(source: &str, resolve_map: &HashMap<Uuid, String>) -> Value {
            let parsed = t::parse(source);
            let typed: Vec<Value> = parsed.nodes.iter().filter_map(|n| match n { Node::Tag(tag) => Some(typed(tag)), _ => None }).collect();
            let resolve = |id| resolve_map.get(&id).cloned();
            json!({
                "source": source,
                "parsed": parsed,
                "serialized": t::serialize(&parsed.nodes),
                "typed": typed,
                "info": t::analyze(&parsed),
                "formFields": t::form_fields(source, &resolve),
                "splitOptions": t::split_options(source),
            })
        }

        /// Rendered-output helpers of `formatting.rs`: HTML, attachment text and offsets, URL safety.
        pub fn formatting() -> Value {
            use kudzu_core::formatting::{html, insertion_offset, insertion_text, safe_image, safe_link, FormatSpan};
            let png = "data:image/png;base64,iVBORw0KGgo=";
            let texts: Vec<(&str, Value)> = vec![
                ("[Logo\"\u{301} onerror=alert(1) x='\u{301}]", json!([{"start":0,"end":30,"style":{"kind":"image","dataUrl":png,"width":64}}])),
                ("<\u{301}b>&\u{301}\"\u{301}'\u{301}\n<i>", json!([{"start":0,"end":3,"style":{"kind":"bold"}},{"start":2,"end":12,"style":{"kind":"link","url":"https://x.y/?a=\u{301}"}}])),
                ("😀ab😀", json!([{"start":1,"end":3,"style":{"kind":"bold"}},{"start":2,"end":4,"style":{"kind":"italic"}},{"start":0,"end":6,"style":{"kind":"underline"}}])),
                ("x[A][B]y", json!([{"start":1,"end":4,"style":{"kind":"image","dataUrl":png,"width":1}},{"start":4,"end":7,"style":{"kind":"image","dataUrl":png,"width":512}},
                    {"start":2,"end":6,"style":{"kind":"image","dataUrl":png,"width":600}},{"start":0,"end":8,"style":{"kind":"bold"}}])),
                ("[A][B]", json!([{"start":3,"end":6,"style":{"kind":"image","dataUrl":png,"width":5}},{"start":0,"end":3,"style":{"kind":"image","dataUrl":"data:image/svg+xml;base64,AAAA","width":5}},
                    {"start":1,"end":1,"style":{"kind":"image","dataUrl":png,"width":5}},{"start":5,"end":9,"style":{"kind":"image","dataUrl":png,"width":5}}])),
            ];
            let cases: Vec<Value> = texts.into_iter().map(|(text, spans)| {
                let spans: Vec<FormatSpan> = serde_json::from_value(spans).unwrap();
                let n = text.encode_utf16().count();
                json!({"text":text,"spans":spans,"html":html(text,&spans),"insertionText":insertion_text(text,&spans),
                    "insertionOffsets":(0..=n + 2).map(|o| insertion_offset(o,&spans)).collect::<Vec<_>>()})
            }).collect();
            let links = ["https://a.b", "HTTPS://A.B", "Http://x", "mailto:a@b", "MAILTO:a@b", "ftp://x", "https://\u{301}x", "https://x\u{301}", "https://a b",
                "https://a\u{a0}b", "https://a\u{feff}b", "https://a\u{200b}b", "https://a\u{85}b", "https://a\u{9f}b", "https://a\u{7f}b", "https://a\"b", "https://a'b",
                "https://a<b", "javascript:alert(1)", " https://a", "https:/a", "httpſ://a", "https://a\u{2028}", "https://\u{130}"];
            let images = [png.to_string(), "data:image/png;base64,iVBORw0KGgo".into(), "data:image/png;base64,iVBORw0KGgoA==".into(), "data:image/jpeg;base64,/9j/AAAA".into(),
                "data:image/jpeg;base64,/9j/".into(), "data:image/png;base64,\u{301}iVBORw0KGgo=".into(), "data:image/png;base64,iVBORw0KGgo\u{301}=".into(),
                "DATA:image/png;base64,iVBORw0KGgo=".into(), "data:image/png;base64,iVBORw0KGgo= ".into(), "data:image/gif;base64,R0lGODlh".into(),
                format!("data:image/png;base64,iVBORw0KGgo={}", "A".repeat(196_608 - 34 - 1)), format!("data:image/png;base64,iVBORw0KGgo={}", "A".repeat(196_608 - 34 + 3))];
            json!({
                "cases": cases,
                "links": links.iter().map(|l| json!({"url":l,"safe":safe_link(l)})).collect::<Vec<_>>(),
                "images": images.iter().map(|i| json!({"url":i,"safe":safe_image(i)})).collect::<Vec<_>>(),
            })
        }

        pub fn icu_cases(dates: &[String], patterns: &[String]) -> Vec<Value> {
            let extra_dates = [
                "2024-12-31T23:59:59.999999999-04:00", "2020-12-31T23:30:00-01:00", "2021-01-03T12:00:00+14:00", "2024-12-30T00:00:00Z",
                "2027-01-01T00:00:00+05:45", "2024-06-15T12:00:00-09:30", "2024-06-15T00:00:00-00:30", "2024-06-15T12:00:00.5+00:00",
                "1582-10-15T00:00:00Z", "1582-10-14T23:59:59Z", "0001-01-01T00:00:00+01:00", "9999-12-31T23:59:59-12:00",
            ];
            let extra_locales = ["", "en", "en-GB", "EN_us", "pt-PT", "zh-Hant-TW", "de@calendar=buddhist", "fr.UTF-8", "ar-SA", "nl_BE", "es-419", "it-CH"];
            let extra_patterns = [
                "yyyyyyyyyy uuuuuuu YYYYYY MMMMMM LLLLLL dddd DDDDD FF", "EEEEEEE eeeeee ccccc ccccccc QQQQQQ qqqq GGGGGG",
                "hhh HHH kkk KKK mmm sss SSSSSSSSSS AAAAAAAAAA aaaa bbbbb BBBB", "ZZZZZZ XXXXXX xxxxxx zzzzz OOOOO OO vv vvvvv VVV VVVVV",
                "'unterminated", "'' '''' 'a''b' 'é' é ñ ü 😀 \u{301}", "j J C g n N p P r R t T U i I l o", "yyyy-MM-dd'T'HH:mm:ss.SSSXXX",
            ];
            let mut out = Vec::new();
            let all_patterns: Vec<&str> = patterns.iter().map(String::as_str).chain(extra_patterns).collect();
            for date in extra_dates { for locale in ["en_US", "de_DE", "ar_EG"] { for pattern in &all_patterns {
                out.push(json!({"now":date,"locale":locale,"pattern":pattern}));
            }}}
            for locale in extra_locales { for pattern in &all_patterns {
                out.push(json!({"now":dates[0],"locale":locale,"pattern":pattern}));
            }}
            for date in dates { for pattern in extra_patterns {
                out.push(json!({"now":date,"locale":"fr_FR","pattern":pattern}));
            }}
            out
        }
    }
const BASE: &str = r###"{"clipboard":" \nclipboard \n","formValues":{"empty":"","who":"<Sam & سلمى>"},"formatting":{"source":"hello","spans":[{"end":5,"start":0,"style":{"kind":"bold"}}]},"now":"2024-01-31T14:07:09.123456789+05:30","resolveMap":{"00000000-0000-0000-0000-000000000001":"nested {{cursor}}{{field name=who}}","00000000-0000-0000-0000-000000000002":"{{snippet id=\"00000000-0000-0000-0000-000000000002\"}}","00000000-0000-0000-0000-000000000003":"never"},"rootSnippetID":"00000000-0000-0000-0000-000000000003"}"###;
const SOURCES: &str = r###"["hello","😀{{cursor}}x{{cursor}}","{{clipboard}}/{{clipboard trim=true}}","{{field name=who}}/{{field name=empty default=fallback}}","{{choice name=pick options=\"a\\|b|c|\" default=c}}","{{field name=qty default=007 multiline=true custom=3.50 flag=false}}","{{date}} {{time}}","{{date offset=+1m}}","{{date offset=-1y2m}}","{{date offset=+1w2d}}","{{date offset=+30min format=\"HH:mm\"}}","{{date offset=oops}}","{{date offset=+9223372036854775807y}}","{{date offset=+999999999999999999999d}}","{{date format=\"yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSSXXX\"}}","{{date format=\"EEEE, MMMM d GGGG\"}}","{{field name=x default=\"a}}b\"}}","{{field name=x name=y}}","{{field}}","{{choice}}","{{DATE}}","{{future n=5}}","{{clipboard trim=1}}","{{field name=x multiline=\"yes\"}}","{{snippet id=nope}}","{{date format=}}","{{field name=\"oops}}","a{{date","{{ }}","{{}}","{{9}}","{{a-b}}","\\{{DATE}}","\\\\{{cursor}}","\\\\\\{{cursor}}","{{{date}}","{{{{date}}","\\{{{{date}}","\\{x\\}","é{{cursor}}","é{{field name=who}}","{́{cursor}}","{{cursor n=1e5 a=.5 b=5. c=+2.50}}","before {{snippet id=\"00000000-0000-0000-0000-000000000001\"}} {{snippet id=\"00000000-0000-0000-0000-000000000001\"}} after{{cursor}}","{{snippet id=\"00000000-0000-0000-0000-000000000002\"}}","{{snippet id=\"00000000-0000-0000-0000-000000000003\"}}"]"###;
const LOCALES: &str = r###"["en_US","de_DE","ar_EG","ja_JP","fr_FR","es_ES","it_IT","pt_BR","nl_NL"]"###;
const RICH_CASES: &str = r###"[{"formValues":{},"formatting":null,"locale":"en_US","now":"2024-03-05T14:07:09.123456+05:30","resolveFormatting":{"00000000-0000-0000-0000-000000000001":{"source":"[Logo]","spans":[{"end":6,"start":0,"style":{"dataUrl":"data:image/png;base64,iVBORw0KGgo=","kind":"image","width":120}}]}},"resolveMap":{"00000000-0000-0000-0000-000000000001":"[Logo]"},"source":"😀 \\{{literal}} {{snippet id=\"00000000-0000-0000-0000-000000000001\"}}{{cursor}}!"},{"formValues":{},"formatting":null,"locale":"en_US","now":"2024-03-05T14:07:09.123456+05:30","resolveFormatting":{"00000000-0000-0000-0000-000000000001":{"source":"[Logo]","spans":[{"end":6,"start":0,"style":{"dataUrl":"data:image/png;base64,iVBORw0KGgo=","kind":"image","width":120}}]}},"resolveMap":{"00000000-0000-0000-0000-000000000001":"[Logo]"},"source":"😀 \\{{literal}} {{snippet id=\"00000000-0000-0000-0000-000000000001\"}}{{cursor}}!"},{"formValues":{},"formatting":{"source":"😀 \\{{literal}} {{snippet id=\"00000000-0000-0000-0000-000000000001\"}}{{cursor}}!","spans":[{"end":14,"start":3,"style":{"kind":"italic"}},{"end":80,"start":0,"style":{"kind":"bold"}}]},"locale":"en_US","now":"2024-03-05T14:07:09.123456+05:30","resolveFormatting":{"00000000-0000-0000-0000-000000000001":{"source":"[Logo]","spans":[{"end":6,"start":0,"style":{"dataUrl":"data:image/png;base64,iVBORw0KGgo=","kind":"image","width":120}}]}},"resolveMap":{"00000000-0000-0000-0000-000000000001":"[Logo]"},"source":"😀 \\{{literal}} {{snippet id=\"00000000-0000-0000-0000-000000000001\"}}{{cursor}}!"},{"formValues":{},"formatting":{"source":"stale","spans":[{"end":14,"start":3,"style":{"kind":"italic"}},{"end":80,"start":0,"style":{"kind":"bold"}}]},"locale":"en_US","now":"2024-03-05T14:07:09.123456+05:30","resolveFormatting":{"00000000-0000-0000-0000-000000000001":{"source":"[Logo]","spans":[{"end":6,"start":0,"style":{"dataUrl":"data:image/png;base64,iVBORw0KGgo=","kind":"image","width":120}}]}},"resolveMap":{"00000000-0000-0000-0000-000000000001":"[Logo]"},"source":"😀 \\{{literal}} {{snippet id=\"00000000-0000-0000-0000-000000000001\"}}{{cursor}}!"}]"###;
const DATES: &str = r###"["2024-03-05T14:07:09.123456789+05:30","2021-01-01T00:00:00Z","2024-12-31T23:59:59-04:00","1500-03-05T14:07:09+05:30","0000-01-01T00:00:00Z"]"###;
const ICU_LOCALES: &str = r###"["en_US","de_DE","ar_EG","ja_JP","fr_FR","es","it_IT","pt_BR","nl_NL","xx_YY"]"###;
const PATTERNS: &str = r###"["G GGGG GGGGG y yy yyyyy u Y YY YYYY","M MM MMM MMMM MMMMM LLLL","d dd D DDD F","E EE EEE EEEE EEEEE EEEEEE e ee eeee c cc ccc","a b B h hh H HH k K m mm s ss S SS SSS SSSSSS SSSSSSSSSSS A","Q QQ QQQ QQQQ QQQQQ q w ww W","Z ZZZZ ZZZZZ z zzzz O OOOO v V VV VVVV x xx xxx xxxx xxxxx X XX XXX","'Week' w', Q'Q 'o''clock' '' jj"]"###;
}
// T1: fixed key sequences; emitted events, delete semantics, buffer and undo come from Rust.
mod matching {
    use super::*;
    use kudzu_core::{matching::*, SnippetRecord};
    use serde_json::{Value,json};
    fn event(e: Option<MatcherEvent>) -> Value {
        match e {
            None => Value::Null,
            Some(MatcherEvent::Expand(m)) => json!({"kind":"expand","snippetID":m.snippet_id,"trigger":m.trigger,"typedTrigger":m.typed_trigger,"deleteCount":m.delete_count,"deleteCountIncludingSeparator":m.delete_count_including_separator(),"separator":m.separator,"insertion":m.insertion,"caseStyle":m.case_style,"fireMode":m.fire_mode}),
            Some(MatcherEvent::Undo(u)) => json!({"kind":"undo","deleteCount":u.delete_count,"retype":u.retype}),
        }
    }
    pub fn export(dir: &Path) {
        let base:Value=serde_json::from_str(BASE).unwrap();
        let snippet:Value=serde_json::from_str(SNIPPET).unwrap();
        let inputs:Vec<Value>=serde_json::from_str(INPUTS).unwrap();
        let mut cases=Vec::new();
        for input in inputs {
            let mut c=base.clone();let mut s=snippet.clone();
            for key in ["fireMode","caseMode","createdAt","updatedAt"] {s[key]=input[key].clone();}
            c["snippets"]=json!([s]);c["steps"]=json!(input["keys"].as_array().unwrap().iter().map(|key|json!({"key":key})).collect::<Vec<_>>());cases.push(c);
        }
        for c in &mut cases {
            let snippets: Vec<SnippetRecord>=serde_json::from_value(c["snippets"].clone()).unwrap();
            let bundle=c["bundleID"].as_str().unwrap().to_string();
            let inserted=c["insertedCount"].as_u64().unwrap() as usize;
            let mut matcher=ExpansionMatcher::with_index(TriggerIndex::new(&snippets,&[],MatchingConfig::default()));
            for step in c["steps"].as_array_mut().unwrap() {
                let key=step["key"].as_str().unwrap();
                let input=match key {"RESET"=>KeyInput::Reset,"BACKSPACE"=>KeyInput::Backspace,_=>KeyInput::Character(key.into())};
                let e=matcher.handle(input,Some(&bundle));
                step["event"]=event(e.clone());
                if let Some(MatcherEvent::Expand(m))=e {matcher.did_expand(&m,inserted);}
                step["buffer"]=json!(matcher.buffer_string());
            }
        }
        fs::write(dir.join("matching.json"),serde_json::to_string_pretty(&cases).unwrap()).unwrap();
        let mut scenarios = scenarios::traces();
        for c in &mut scenarios { scenarios::run(c); }
        fs::write(dir.join("matching-scenarios.json"),serde_json::to_string_pretty(&scenarios).unwrap()).unwrap();
        let mut conflicts = scenarios::conflicts();
        for c in &mut conflicts {
            let snippets: Vec<SnippetRecord>=serde_json::from_value(c["snippets"].clone()).unwrap();
            let groups: Vec<kudzu_core::GroupRecord>=serde_json::from_value(c["groups"].clone()).unwrap();
            c["expected"]=serde_json::to_value(find_conflicts(&snippets,&groups)).unwrap();
        }
        fs::write(dir.join("matching-conflicts.json"),serde_json::to_string_pretty(&conflicts).unwrap()).unwrap();
        fs::write(dir.join("matching-case.json"),serde_json::to_string_pretty(&scenarios::case_corpus()).unwrap()).unwrap();
    }

    /// Adversarial matcher traces and conflict corpora added by the T1 review: several snippets,
    /// groups, app rules, configs, separators and Unicode edge cases. Expectations come from Rust.
    mod scenarios {
        use kudzu_core::matching::*;
        use kudzu_core::{GroupRecord, SnippetRecord};
        use serde_json::{json, Value};

        fn id(n: u32) -> String { format!("00000000-0000-0000-0000-{n:012}") }

        /// A snippet record; `extra` overrides any field (groupID, rule, insertion, isEnabled…).
        fn snip(n: u32, triggers: &[&str], fire: &str, case: &str, updated: &str, extra: Value) -> Value {
            let mut s = json!({"id":id(n),"name":format!("S{n}"),"triggers":triggers,"content":"expanded","fireMode":fire,
                "caseMode":case,"isEnabled":true,"isFavorite":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":updated});
            for (k, v) in extra.as_object().unwrap() { s[k] = v.clone(); }
            s
        }

        fn group(n: u32, parent: Option<u32>, enabled: bool, extra: Value) -> Value {
            let mut g = json!({"id":id(n),"name":format!("G{n}"),"sortIndex":0,"isEnabled":enabled});
            if let Some(p) = parent { g["parentID"] = json!(id(p)); }
            for (k, v) in extra.as_object().unwrap() { g[k] = v.clone(); }
            g
        }

        fn keys(text: &str) -> Vec<String> { text.chars().map(String::from).collect() }

        fn trace(name: &str, snippets: Vec<Value>, groups: Vec<Value>, config: Value, bundles: &[Option<&str>], keys: Vec<String>) -> Vec<Value> {
            bundles.iter().map(|bundle| json!({"name":name,"snippets":snippets,"groups":groups,"config":config,"bundleID":bundle,
                "insertedCount":3,"keys":keys})).collect()
        }

        const T0: &str = "2026-01-01T00:00:00Z";
        const T1: &str = "2026-06-01T00:00:00.000001Z";
        const NOTES: Option<&str> = Some("com.apple.Notes");

        pub fn traces() -> Vec<Value> {
            let mut out = Vec::new();
            let none = json!({});
            let k = |parts: &[&str]| parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            out.extend(trace("overlapping immediate triggers", vec![snip(1,&[";a"],"immediate","exact",T0,json!({})), snip(2,&[";ab"],"immediate","exact",T1,json!({}))], vec![], none.clone(), &[NOTES], keys(";ab;a")));
            out.extend(trace("longest suffix wins", vec![snip(1,&["ab"],"immediate","exact",T1,json!({})), snip(2,&["xab"],"immediate","propagate",T0,json!({}))], vec![], none.clone(), &[NOTES], keys("xabXAByab")));
            out.extend(trace("same trigger, newest snippet", vec![snip(1,&["zz"],"immediate","exact",T0,json!({})), snip(2,&["zz"],"immediate","propagate",T1,json!({}))], vec![], none.clone(), &[NOTES], keys("zzZZ")));
            out.extend(trace("same trigger and time, first entry", vec![snip(1,&["zz","yy"],"immediate","exact",T0,json!({})), snip(2,&["zz"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES], keys("zzyy")));
            out.extend(trace("immediate beats whole word", vec![snip(1,&["tw"],"wholeWord","exact",T1,json!({})), snip(2,&["tw"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES], keys("tw tw ")));
            out.extend(trace("whole-word boundaries", vec![snip(1,&["sig","e.g.","a b","'q"],"wholeWord","propagate",T0,json!({}))], vec![], none.clone(), &[NOTES],
                keys("xsig sig.(sig)\"sig\" E.G. e.g.. a b 'q' ''q, Sig! SIG?")));
            out.extend(trace("custom separators", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({}))], vec![], json!({"separators":"-é"}), &[NOTES],
                k(&["s","i","g",".","RESET","s","i","g","-","s","i","g","é","s","i","g","e","\u{301}","RESET","s","i","g","e\u{301}","s","i","g"," "])));
            out.extend(trace("no separators configured", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({}))], vec![], json!({"separators":""}), &[NOTES], keys("sig.sig,sig sig\n")));
            out.extend(trace("exotic whitespace", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["RESET","s","i","g","\u{3000}","s","i","g","\u{a0}","s","i","g","\u{85}","s","i","g","\u{feff}","RESET","s","i","g","\u{200b}","RESET",
                    "s","i","g","\t","s","i","g","\u{2028}","s","i","g","\u{b}","s","i","g","\u{180e}","RESET","s","i","g","\u{1680}"])));
            out.extend(trace("CR, LF and CRLF", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({})), snip(2,&["x\r\ny"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["s","i","g","\r","\n","s","i","g","\r\n","BACKSPACE","BACKSPACE","x","\r","\n","y","\r","s","i","g","\n","x","\r\n","y"])));
            out.extend(trace("canonically equivalent separators", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({})), snip(2,&[";x"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["RESET","s","i","g","\u{37e}","s","i","g",";","\u{37e}","x","RESET","s","i","g","\u{2000}","s","i","g","\u{2001}"])));
            out.extend(trace("separator plus combining mark", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["RESET","s","i","g",".\u{301}","s","i","g"," \u{301}","RESET","s","i","g",".","\u{301}"," ","\u{301}"])));
            out.extend(trace("case scripts", vec![snip(1,&["straße","ǆx","σx","i\u{307}x","привет","ꭰꭱ","ⅻx","ǈx","ﬀx"],"immediate","propagate",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["RESET","S","t","r","a","ß","e","RESET","S","T","R","A","S","S","E","RESET","ǅ","X","RESET","Ǆ","X","RESET","Σ","X","RESET","İ","X","RESET","П","Р","И","В","Е","Т",
                    "RESET","П","р","и","в","е","т","RESET","Ꭰ","Ꭱ","RESET","Ⅻ","X","RESET","ǉ","x","RESET","Ǉ","X","RESET","ﬀ","X","RESET","Ｆ","Ｆ","x"])));
            out.extend(trace("case detection", vec![snip(1,&["a1b","1ab","-ab","àb","ab","日本"],"immediate","propagate",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["RESET","A","1","B","RESET","a","1","B","RESET","1","A","B","RESET","-","A","b","RESET","A","\u{300}","B","RESET","À","b","RESET","日","本","RESET","A","B"])));
            out.extend(trace("canonical equivalence", vec![snip(1,&["café"],"immediate","propagate",T0,json!({})), snip(2,&["nai\u{308}ve"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["c","a","f","e","\u{301}","RESET","C","A","F","É","RESET","n","a","ï","v","e","RESET","n","a","i","\u{308}","v","e","RESET","N","a","ï","v","e"])));
            out.extend(trace("emoji clusters", vec![snip(1,&["👨‍👩‍👧"],"immediate","exact",T0,json!({})), snip(2,&["👨"],"immediate","exact",T1,json!({})), snip(3,&["🇺🇸"],"wholeWord","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["👨","\u{200d}","👩","\u{200d}","👧","RESET","🇺","🇸"," ","🇺","🇸","🇺"," ","RESET","👍","🏽","BACKSPACE"])));
            let rules = vec![
                snip(1,&[";allow"],"immediate","exact",T0,json!({"rule":{"mode":"allow","bundleIDs":["com.apple.Notes"],"insertion":"paste"}})),
                snip(2,&[";deny"],"immediate","exact",T0,json!({"rule":{"mode":"deny","bundleIDs":["com.apple.Notes"]}})),
                snip(3,&[";grp"],"immediate","exact",T0,json!({"groupID":id(102)})),
                snip(4,&[";own"],"immediate","exact",T0,json!({"groupID":id(102),"insertion":"type","rule":{"mode":"deny","bundleIDs":[]}})),
                snip(5,&[";off"],"immediate","exact",T0,json!({"groupID":id(103)})),
                snip(6,&[";cyc"],"immediate","exact",T0,json!({"groupID":id(104)})),
                snip(7,&[";dang"],"immediate","exact",T0,json!({"groupID":id(999)})),
                snip(8,&[";plain"],"immediate","exact",T0,json!({})),
                snip(9,&[";dis"],"immediate","exact",T0,json!({"isEnabled":false})),
                snip(10,&[";term"],"immediate","exact",T0,json!({"rule":{"mode":"deny","bundleIDs":["com.apple.Terminal"],"insertion":"type"}})),
            ];
            let groups = vec![
                group(101,None,true,json!({"insertion":"paste","rule":{"mode":"allow","bundleIDs":["com.apple.Notes","com.apple.Terminal"],"insertion":"type"}})),
                group(102,Some(101),true,json!({})),
                group(103,Some(101),false,json!({})),
                group(104,Some(105),true,json!({"insertion":"type"})),
                group(105,Some(104),true,json!({"rule":{"mode":"deny","bundleIDs":["com.apple.Notes"]}})),
                group(101,None,false,json!({"name":"duplicate id, ignored"})),
            ];
            let typed: Vec<String> = [";allow",";deny",";grp",";own",";off",";cyc",";dang",";plain",";dis",";term"].iter()
                .flat_map(|t| { let mut v = keys(t); v.push("RESET".into()); v }).collect();
            out.extend(trace("rules, groups and insertion", rules.clone(), groups.clone(), none.clone(),
                &[NOTES, Some("com.apple.Terminal"), Some("com.apple.Mail"), None, Some("com.apple.loginwindow")], typed.clone()));
            out.extend(trace("config insertion and denials", rules, groups, json!({"defaultInsertion":"type","deniedBundleIDs":["com.apple.Mail"],"appInsertions":{"com.apple.Notes":"paste","com.apple.Mail":"type"}}),
                &[NOTES, Some("com.apple.Mail"), Some("com.apple.loginwindow"), None], typed));
            let long64: String = "q".repeat(63) + "!";
            let long65: String = "r".repeat(64) + "!";
            let mut capacity = keys(&"z".repeat(70));
            capacity.extend(keys(&long64)); capacity.push("RESET".into()); capacity.extend(keys(&long65));
            out.extend(trace("buffer capacity", vec![snip(1,&[&long64],"immediate","exact",T0,json!({})), snip(2,&[&long65],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES], capacity));
            out.extend(trace("trigger dedupe and empty triggers", vec![snip(1,&["","sig","SIG","Sig"],"immediate","propagate",T0,json!({})), snip(2,&["","Ex","EX"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["S","I","G","E","X","E","x","e","x"])));
            out.extend(trace("multi-cluster inputs", vec![snip(1,&["ab"],"immediate","exact",T0,json!({})), snip(2,&["cd"],"wholeWord","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["xaby","xab","","ab","","c","d ","cd.x"," cd","!"])));
            out.extend(trace("undo flows", vec![snip(1,&["sig"],"wholeWord","exact",T0,json!({})), snip(2,&[";i"],"immediate","exact",T0,json!({}))], vec![], none.clone(), &[NOTES],
                k(&["s","i","g","\r\n","BACKSPACE","BACKSPACE","BACKSPACE"," ","BACKSPACE",";","i","RESET","BACKSPACE",";","i","x","BACKSPACE",";","i","BACKSPACE",";","i","BACKSPACE","s","i","g","👍🏽","BACKSPACE"])));
            out
        }

        /// `case_propagation::detect` and `apply` over mixed scripts, special casing and clusters.
        pub fn case_corpus() -> Value {
            let typed = ["sig", "Sig", "SIG", "sIG", "S", "SI", "Ab1", "1AB", ";A", ";AB", "ÀB", "A\u{300}B", "ß", "SSß", "ǅx", "ǄX", "ǈ", "İX", "ΣΑ", "ⅫX", "ⅻ",
                "日本", "日A", "AB日", "ꭰ", "ᎠᎡ", "Ⴀ", "ᲐᲑ", "ǰ", "ŉ", "ﬀ", "Ａｂ", "ＡＢ", "١A", "Ⓐⓑ", "ⒶⒷ", "ᾼ", "ᾳ", "ﬅ", "🇺🇸A", "A\u{200d}B", ""];
            let texts = ["hello world", " 'quoted' text", "123 abc", "١٢ abc", "ß straße", "ǆungla", "ǉ", "ﬀ ligature", "ŉ", "ǰ", "ᾳ ᾼ", "ΌΣΟΣ όσος",
                "i̇stanbul", "ı", "日本 abc", "\u{301}é", "e\u{301}cole", "ⓐbc", "ⅻ", "…ok", "👍🏽 ok", "", "   ", "\nnew", "ꭰ", "ⴀ", "ა", "ﬅ", "Ꭰ"];
            let styles = [CaseStyle::AsIs, CaseStyle::Capitalized, CaseStyle::Upper];
            json!({
                "detect": typed.iter().map(|t| json!({"typed":t,"style":case_propagation::detect(t)})).collect::<Vec<_>>(),
                "apply": texts.iter().flat_map(|t| styles.iter().map(move |s| json!({"text":t,"style":s,"result":case_propagation::apply(*s,t)}))).collect::<Vec<_>>(),
            })
        }

        /// Replays `c["keys"]` through a matcher built from the case's snippets, groups and config.
        pub fn run(c: &mut Value) {
            let snippets: Vec<SnippetRecord> = serde_json::from_value(c["snippets"].clone()).unwrap();
            let groups: Vec<GroupRecord> = serde_json::from_value(c["groups"].clone()).unwrap();
            let mut config = MatchingConfig::default();
            let cfg = &c["config"];
            if let Some(s) = cfg.get("separators").and_then(Value::as_str) { config.separators = s.chars().collect(); }
            if let Some(v) = cfg.get("deniedBundleIDs") { config.denied_bundle_ids = serde_json::from_value(v.clone()).unwrap(); }
            if let Some(v) = cfg.get("appInsertions") { config.app_insertions = serde_json::from_value(v.clone()).unwrap(); }
            if let Some(v) = cfg.get("defaultInsertion") { config.default_insertion = serde_json::from_value(v.clone()).unwrap(); }
            let bundle = c["bundleID"].as_str().map(str::to_string);
            let inserted = c["insertedCount"].as_u64().unwrap() as usize;
            let mut matcher = ExpansionMatcher::with_index(TriggerIndex::new(&snippets, &groups, config));
            let mut steps = Vec::new();
            for key in c["keys"].as_array().unwrap() {
                let key = key.as_str().unwrap();
                let input = match key { "RESET" => KeyInput::Reset, "BACKSPACE" => KeyInput::Backspace, _ => KeyInput::Character(key.into()) };
                let e = matcher.handle(input, bundle.as_deref());
                let event = super::event(e.clone());
                if let Some(MatcherEvent::Expand(m)) = e { matcher.did_expand(&m, inserted); }
                steps.push(json!({"key":key,"event":event,"buffer":matcher.buffer_string()}));
            }
            c["steps"] = json!(steps);
        }

        pub fn conflicts() -> Vec<Value> {
            let a = |n, t: &[&str], fire, case| snip(n, t, fire, case, T0, json!({}));
            let ruled = |n, t: &[&str], mode: &str, ids: &[&str]| snip(n, t, "immediate", "propagate", T0, json!({"rule":{"mode":mode,"bundleIDs":ids}}));
            let cases = vec![
                (vec![a(1,&[";a","x"],"immediate","exact"), a(2,&[";a"],"immediate","exact"), a(3,&[";A"],"immediate","exact"), a(4,&[";A"],"wholeWord","propagate")], vec![]),
                (vec![a(1,&["café"],"immediate","exact"), a(2,&["cafe\u{301}"],"immediate","exact"), a(3,&["CAFÉ"],"immediate","propagate"), a(4,&["İx"],"immediate","propagate"), a(5,&["i\u{307}x"],"immediate","exact")], vec![]),
                (vec![a(1,&["ß"],"immediate","propagate"), a(2,&["SS","ss"],"immediate","propagate"), a(3,&["ǅ"],"immediate","exact"), a(4,&["ǆ","Ǆ"],"immediate","propagate")], vec![]),
                (vec![a(1,&[";a"],"immediate","exact"), a(2,&[";ab",";Ab",";1",";a1"],"immediate","propagate"), a(3,&[";abc",";1x"],"wholeWord","exact"), a(4,&[";A"],"immediate","propagate"), a(5,&[";1"],"immediate","exact")], vec![]),
                (vec![a(1,&["e\u{301}"],"immediate","exact"), a(2,&["éx","ÉX"],"immediate","exact"), a(3,&["é"],"wholeWord","exact"), a(4,&["👨"],"immediate","exact"), a(5,&["👨‍👩‍👧x","👨x"],"immediate","exact")], vec![]),
                (vec![ruled(1,&[";r"],"allow",&["a"]), ruled(2,&[";r"],"allow",&["b"]), ruled(3,&[";r"],"allow",&["a","b"]), ruled(4,&[";rx"],"deny",&["a","b"]), ruled(5,&[";rx"],"deny",&[]),
                      ruled(6,&[";r"],"allow",&[]), ruled(7,&[";ry"],"deny",&["a"])], vec![]),
                (vec![snip(1,&[";g"],"immediate","exact",T0,json!({"groupID":id(102)})), snip(2,&[";g",";gx"],"immediate","exact",T0,json!({"groupID":id(103)})),
                      snip(3,&[";g"],"immediate","exact",T0,json!({"isEnabled":false})), snip(4,&[";gy"],"immediate","exact",T0,json!({"groupID":id(101)})),
                      snip(5,&[";gz"],"immediate","exact",T0,json!({"groupID":id(102),"rule":{"mode":"deny","bundleIDs":[]}}))],
                 vec![group(101,None,true,json!({"rule":{"mode":"allow","bundleIDs":["a"]}})), group(102,Some(101),true,json!({})), group(103,None,false,json!({}))]),
                (vec![a(1,&["","x"],"immediate","exact"), a(2,&["",""],"immediate","exact"), a(3,&["xy","x"],"immediate","exact")], vec![]),
            ];
            cases.into_iter().map(|(snippets, groups)| json!({"snippets":snippets,"groups":groups})).collect()
        }
    }
const BASE: &str = r###"{"bundleID":"com.apple.Notes","insertedCount":8}"###;
const SNIPPET: &str = r###"{"caseMode":"propagate","content":"expanded","createdAt":"2026-11-08T09:19:40Z","fireMode":"immediate","id":"00000000-0000-0000-0000-000000000001","isEnabled":true,"isFavorite":false,"name":"S","triggers":[";sig","👍🏽","café"],"updatedAt":"2026-11-08T09:19:40Z"}"###;
const INPUTS: &str = r###"[{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","I","G","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","I","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i"," ",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\r\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",".","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","x","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",",",";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["👍🏽","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["c","a","f","é","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","I","G","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","I","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i"," ",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\r\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",".","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","x","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",",",";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["👍🏽","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"immediate","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["c","a","f","é","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","I","G","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","I","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i"," ",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\r\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",".","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","x","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",",",";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["👍🏽","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"propagate","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["c","a","f","é","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","S","I","G","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","I","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["h","i"," ",";","s","i","g","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","\r\n","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",".","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g","x","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":[";","s","i","g",",",";","s","i","g"," ","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["👍🏽","BACKSPACE","BACKSPACE","RESET"]},{"fireMode":"wholeWord","caseMode":"exact","createdAt":"2026-11-08T09:19:40Z","updatedAt":"2026-11-08T09:19:40Z","keys":["c","a","f","é","BACKSPACE","BACKSPACE","RESET"]}]"###;
}

/// Secret detection corpus taken verbatim from the inline Rust tests.
mod secrets {
 use std::path::Path; use serde_json::json; use kudzu_core::secrets;
 pub fn export(dir: &Path) {
 let corpora: &[(&str, &[&str])] = &[
("ordinary_text_is_untouched", &[r########"Best regards,
Saeed"########,r########"Meeting moved to 3pm tomorrow, see https://example.com/agenda?id=42"########,r########"hunter"########,r########"getElementById"########,r########"README.md"########,r########"config.v2.json"########,r########"node_modules"########,r########"user@example.com"########,r########"/Users/saeed/Projects/App2/src/Main.tsx"########,r########"#FF00aa"########,r########"$1,000,000"########,r########"550e8400-e29b-41d4-a716-446655440000"########,r########"9fceb02d0ae598e95dc970b74767f19372d61af8"########,r########"let max_tokens = 1024;"########,r########"password: string"########,r########"const password = process.env.PASSWORD"########,r########"Password: ********"########,r########"iPhone15"########,r########"The author: Jane Doe"########,r########"Use Bearer auth"########,r########"flagged {text:?} as {:?}"########]),
("vendor_keys", &[r########"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"########,r########"github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789ABCDEF"########,r########"sk-ant-api03-abcDEF123456ghiJKL789012mnoPQR345678"########,r########"sk-proj-Abc123Def456Ghi789Jkl012Mno345"########,r########"sk_live_4eC39HqLyjWDarjtT1zdp7dc"########,r########"xoxb-123456789012-1234567890123-AbCdEfGhIjKlMnOpQrStUvWx"########,r########"AKIAIOSFODNN7EXAMPLE"########,r########"AIzaSyDdI0hCZtE6vySjMm-WEfRq3CPzqKqqsHI"########,r########"glpat-xxxxxxxxxxxxxxxx2xxx"########,r########"npm_abcdefghijklmnopqrstuvwxyz0123456789"########,r########"my key is {key} ok"########,r########"my key is [redacted: API key] ok"########,r########"{key}"########]),
("private_key_block", &[r########"id:
-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAA
-----END OPENSSH PRIVATE KEY-----
thanks"########,r########"id:
[redacted: private key]
thanks"########,r########"-----BEGIN CERTIFICATE-----
MIIB
-----END CERTIFICATE-----"########,r########"-----BEGIN RSA PRIVATE KEY-----
MIIE"########,r########"[redacted: private key]"########]),
("jwt_and_authorization", &[r########"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"########,r########"token {jwt}"########,r########"token [redacted: access token]"########,r########"curl -H 'Authorization: Bearer abc.def-123_xyz'"########,r########"curl -H 'Authorization: Bearer [redacted: access token]'"########,r########"Authorization: token deadbeefcafe"########,r########"Authorization: token [redacted: access token]"########]),
("labelled_values", &[r########"DB_PASSWORD=correct-horse"########,r########"DB_PASSWORD=[redacted: password]"########,r########"{"api_key": "abcd1234", "name": "x"}"########,r########"{"api_key": "[redacted: secret]", "name": "x"}"########,r########"export GITHUB_TOKEN=abc123xyz"########,r########"export GITHUB_TOKEN=[redacted: secret]"########,r########"Wi-Fi password: purple monkey"########,r########"Wi-Fi password: [redacted: password] monkey"########,r########"The password is: Summer2024"########,r########"The password is: [redacted: password]"########,r########"API Key: q8w7e6r5"########,r########"API Key: [redacted: secret]"########,r########"--password=hunter22 --user bob"########,r########"--password=[redacted: password] --user bob"########,r########"pin: 4821"########,r########"pin: [redacted: password]"########,r########"client_secret => 'zz9plural'"########,r########"client_secret => '[redacted: secret]'"########]),
("url_credentials_and_webhooks", &[r########"postgres://admin:s3cr3t@db.local:5432/app"########,r########"postgres://admin:[redacted: secret]@db.local:5432/app"########,r########"https://user@example.com/path"########,r########"hook https://hooks.slack.com/services/T000/B000/XXXXXXXX done"########,r########"hook https://hooks.slack.com/services/[redacted: secret] done"########]),
("lone_passwords", &[r########"P@ssw0rd!"########,r########"Tr0ub4dor&3"########,r########"kezwy4-dyxbeR-mosmac"########,r########"qT7xLm9vR2bN"########,r########"a8f3k2m9x7q1z5w4"########,r########"{pw} not flagged"########,r########"[redacted: password]"########,r########"  P@ssw0rd!
"########,r########"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"########]),
("random_runs_inside_text", &[r########"secret is wJalrXUtnFEMI/K7MDENG/bPxRfiCY3XAMPL9KEY ok"########,r########"secret is [redacted: API key] ok"########,r########"hex 7f3a9c2e5b8d1f4a6c0e9b2d5f8a1c4e"########,r########"hex [redacted: API key]"########,r########"fn handle_incoming_connection_request(stream: UnixStream) {}"########]),
("mixed_text_is_not_whole", &[r########"password=abcd1234 for the staging box"########,r########"sk_live_4eC39HqLyjWDarjtT1zdp7dc
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"########]),
// Review additions: Unicode edges (combining marks after delimiters, VT, Kelvin sign, CJK
// numerals) and overlapping-format scans, where a regex or grapheme-based port drifts from the
// byte scanners. (`"a://\u{3000}"` is left out: the Rust scan panics on it.)
("adversarial", &["-----BEGIN -----BEGIN RSA PRIVATE KEY-----\nMIIE","-----BEGIN OPENSSH-X PRIVATE KEY-----\nabc\n-----END OPENSSH-X PRIVATE KEY-----","-----BEGIN RSA\nPRIVATE KEY-----\nabc","-----BEGIN EC PRIVATE KEY-----\n-----BEGIN EC PRIVATE KEY-----\nx\n-----END EC PRIVATE KEY-----","xeyJxxxxxxxxxxxxx.eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl","eyJshort.eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl","xSG.aaaaaaaaaaaaaaaaaaaaaa.SG.bbbbbbbbbbbbbbbbbbbbbb.ccccccccccccccccccccccccccccccccccccccccccc"," \u{301}Tr0ub4dor&3","/\u{301}Tr0ub4dor&3","#\u{301}Tr0ub4dor&3","Abcdef\u{4e00}\u{4e8c}","password: abc,\u{301}def ghi","password: $\u{301}HOMEVALUE","password: env.\u{301}SECRET","https://host/\u{301}user:pw@x","https://user:pw@host\u{b}x:y@z","https://hoo\u{212a}s.slack.com/services/T0/B0/XXXX","https://u:p@a\u{85}b:c@d","password\u{3000}= hunter22","password: \"abc\\","Authorization:\u{85}token deadbeefcafe","\u{a0}P@ssw0rd!\u{a0}","\u{feff}P@ssw0rd!","Ze\u{301}rich#2024x","api_key=\u{301}\u{301}\u{301}\u{301}","p\u{301}assword: hunter22 x"]),
("handles_non_ascii", &[r########"Grüße aus Köln — schöne Woche! 🌿"########,r########"Passwort-Hinweis — password: Zürich#2024"########,r########"Passwort-Hinweis — password: [redacted: password]"########]),
 ];
 let cases: Vec<_> = corpora.iter().map(|(name, inputs)| json!({"name":name,"cases":inputs.iter().map(|text| {
 let redacted = secrets::redact(text); let findings: Vec<_> = secrets::scan(text).iter().map(|f|json!({"start":f.range.start,"end":f.range.end,"kind":f.kind})).collect();
 json!({"text":text,"findings":findings,"detect":secrets::detect(text),"redacted":redacted.text,"kinds":redacted.kinds,"whole":redacted.whole})
 }).collect::<Vec<_>>() })).collect(); super::write(dir,"corpus.json",&cases);
 }
}

mod clipboard {
 use std::path::Path; use serde_json::json; use chrono::{TimeZone,Utc,Duration}; use kudzu_core::clipboard::*;
 pub fn export(dir:&Path) {
 let texts = ["Hello there","https://kudzu.app/docs?x=1","  mailto:me@example.com ","www.example.org","#1E90FF","oklch(0.7 0.1 250)","rgb(255, 0, 0)","git commit -m \"fix\"","function add(a, b) {\n  const sum = a + b;\n  return sum;\n}","Dear team,\nThanks for the meeting today.\nSee you tomorrow.","#hashtag","https://","see https://a.com now","  \n ","let x = foo();","ééé\nend","P@ssw0rd!","token: abcd1234efgh",
 // Review additions: BOM kept, U+200B isn't whitespace, marks after delimiters, bare CR, CJK numerals, decomposed prose.
 "\u{feff}hello","\u{200b}","\u{200b}https://a.com","www.\u{301}a.b","#1e90ff\u{301}","x = 1; \u{301}\ny = 2; \u{301}\nz = 3; \u{301}","cafe\u{301} au lait;\nbonjour mon ami;\nau revoir mes amis;","git \u{301}status","a\r","a\r\nb\r","\u{2167} \u{2167}\n\u{2167};\n\u{2167};","  \u{85}x = 1;\u{85}","https://example.com\u{85}","\u{600} hello","RGB(1, 2, 3)","hsl(120 50 50)","rgb(1e 2 3)","#+12345"];
 let cases:Vec<_> = texts.iter().flat_map(|text| [None,Some("com.microsoft.VSCode")].into_iter().map(move |source| {
 let items = vec![vec![(uti::PLAIN_TEXT.to_string(),text.as_bytes().to_vec())]];
 let classified = classify(&items,source).map(|c|json!({"kind":c.kind,"text":c.text,"files":c.files,"color":c.color,"rich":c.rich,"imageType":c.image_type,"types":c.types,"size":c.size,"hash":format!("{:016x}",c.hash)}));
 json!({"text":text,"source":source,"expected":classified,"blob":encode_items(&items)})
 })).collect();
 let mut history=History::new(); let mut states=vec![]; let now=Utc.with_ymd_and_hms(2026,11,1,0,0,0).unwrap();
 for (i,text) in ["alpha","beta","alpha","P@ssw0rd!","alpha"].iter().enumerate() {
 let items=vec![vec![(uti::PLAIN_TEXT.to_string(),text.as_bytes().to_vec())]];
 history.ingest(&classify(&items,None).unwrap(),None,None,now+Duration::hours(i as i64)); states.push(json!(history));
 }
 super::write(dir,"classification.json",&cases);
 // `History::ingest` draws a random id per entry; rename them in order of appearance.
 let mut history_fixture=json!({"texts":["alpha","beta","alpha","P@ssw0rd!","alpha"],"states":states});
 super::stable::Ids::new(0xc11b_0000_0000_4000_8000_000000000000,"").apply(&mut history_fixture);
 super::write(dir,"history.json",&history_fixture);
 }
}

mod sync {
 use std::path::Path; use serde_json::json; use kudzu_core::{KudzuLibrary,SnippetRecord,GroupRecord,UsageRecord}; use kudzu_core::sync::*;
 use chrono::{Utc,TimeZone,Duration}; use uuid::Uuid;
 fn value(r:&RemoteRecord)->serde_json::Value { json!({"kind":r.kind,"name":r.name,"payload":r.payload}) }
 pub fn export(dir:&Path) {
 let now=Utc.with_ymd_and_hms(2026,11,1,0,0,0).unwrap(); let id=Uuid::from_u128(1); let gid=Uuid::from_u128(2);
 let mut snippet=SnippetRecord::new("S1",vec![";a".into()],"content 1"); snippet.id=id; snippet.created_at=now; snippet.updated_at=now;
 let group=GroupRecord{id:gid,..GroupRecord::new("Work")}; snippet.group_id=Some(gid);
 let original=KudzuLibrary{groups:vec![group],snippets:vec![snippet.clone()],usage:vec![UsageRecord{snippet_id:id,device_id:"mac-a".into(),count:3,chars_saved:30,last_used_at:None}],..Default::default()};
 let mut index=SyncIndex::default(); for name in local_changes(&original,&index,"mac-a").saves { let rec=encode(&name,&original,&index,"mac-a").unwrap();record_saved(&mut index,&rec); }
 let mut theirs=snippet.clone();theirs.content="theirs".into();theirs.updated_at+=Duration::minutes(5);
 let remote=RemoteRecord{kind:RecordKind::Snippet,name:snippet_name(id),payload:serde_json::to_string(&theirs).unwrap()};
 let mut cases=vec![];
 for n in 0..6 {
 let mut lib=original.clone(); let mut idx=index.clone();
 let mut records=vec![]; let mut deleted=vec![];
 match n {0=>records.push(remote.clone()),1=>{lib.snippets[0].content="mine".into();lib.snippets[0].updated_at+=Duration::minutes(10);records.push(remote.clone());},2=>deleted.push(group_name(gid)),3=>deleted.push(snippet_name(id)),4=>records.push(RemoteRecord{kind:RecordKind::Meta,name:META_RECORD.into(),payload:r#"{"syncVersion":2}"#.into()}),_=>{let mut v:serde_json::Value=serde_json::from_str(&remote.payload).unwrap();v["color"]=json!("green");records.push(RemoteRecord{payload:v.to_string(),..remote.clone()});}}
 let before=lib.clone();let prior=idx.clone();let applied=apply_remote(&mut lib,&mut idx,&records,&deleted,"mac-a");
 cases.push(json!({"name":n,"library":before,"index":prior,"records":records.iter().map(value).collect::<Vec<_>>(),"deleted":deleted,"expectedLibrary":lib,"expectedIndex":idx,"libraryChanged":applied.library_changed,"needsUpdate":applied.needs_update,"changes": {"saves":local_changes(&lib,&idx,"mac-a").saves,"deletes":local_changes(&lib,&idx,"mac-a").deletes}}));
 }
 super::write(dir,"merges.json",&cases);
 }
}

/// Original import corpora plus their Rust results (random identities normalized by replay).
mod interchange {
 use std::path::Path; use serde_json::json; use kudzu_core::interchange::{raycast,textexpander,espanso};
 pub fn export(dir: &Path) {
 let raycast = r########"[
      { "name": "Email sign-off", "text": "Best,\n{cursor}", "keyword": "!bye" },
      { "name": "Today", "text": "{date} / {time} / {datetime} / {day}", "keyword": "!now" },
      { "name": "Custom date", "text": "{date format=\"yyyy-MM-dd\" offset=\"+1d\"} {date offset=\"-2M\"} {time offset=\"30m\"}", "keyword": "!d" },
      { "name": "Paste", "text": "{clipboard} {clipboard | trim} {clipboard offset=1}", "keyword": "!p" },
      { "name": "Reply", "text": "Hi {argument name=\"Name\" default=\"there\"},\n{argument name=\"Tone\" options=\"formal, casual\" default=\"casual\"}\n{snippet name=\"Email sign-off\"}", "keyword": "!reply" },
      { "name": "Unsupported", "text": "{selection} {uuid} {snippet name=\"Nope\"} {argument name=\"x\" bogus}", "keyword": "!u" },
      { "name": "Spaced offset", "text": "{date offset=\"+2y +5M\" format=\"yyyy-MM\"}", "keyword": "!so" },
      { "name": "Code", "text": "fn main() { println!(\"{}\", 1); } \\{clipboard} {{literal}} C:\\path", "keyword": "!code" },
      { "name": "No keyword", "text": "Only searchable" },
      { "name": "Empty", "text": "", "keyword": "!empty" },
      "junk"
    ]"########;
 let plist = r########"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>groupInfo</key>
  <dict>
    <key>groupName</key><string>Work &amp; Co</string>
    <key>expandAfterMode</key><integer>2</integer>
  </dict>
  <key>snippetsTE2</key>
  <array>
    <dict>
      <key>abbreviation</key><string>sig</string>
      <key>abbreviationMode</key><integer>0</integer>
      <key>creationDate</key><date>2020-05-06T07:08:09Z</date>
      <key>modificationDate</key><date>2021-05-06T07:08:09Z</date>
      <key>label</key><string>Signature</string>
      <key>plainText</key><string>Best regards,
%filltext:name=Sender:default=Me%%|</string>
      <key>snippetType</key><integer>0</integer>
      <key>uuidString</key><string>6E4FE1E9-0000-4000-8000-000000000001</string>
    </dict>
    <dict>
      <key>abbreviation</key><string>ddate</string>
      <key>abbreviationMode</key><integer>1</integer>
      <key>plainText</key><string>%A, %B %e, %Y</string>
      <key>snippetType</key><integer>0</integer>
    </dict>
    <dict>
      <key>abbreviation</key><string>both</string>
      <key>plainText</key><string>%snippet:sig% + %snippet:missing% + %clipboard</string>
    </dict>
    <dict>
      <key>abbreviation</key><string>rich</string>
      <key>plainText</key><string>Bold &amp; 50% done</string>
      <key>snippetType</key><integer>1</integer>
    </dict>
    <dict>
      <key>abbreviation</key><string>script</string>
      <key>plainText</key><string>do shell script "ls"</string>
      <key>snippetType</key><integer>2</integer>
    </dict>
    <dict>
      <key>abbreviation</key><string></string>
      <key>label</key><string>Loose</string>
      <key>plainText</key><string>No trigger</string>
    </dict>
    <dict>
      <key>abbreviation</key><string>broken</string>
    </dict>
  </array>
</dict>
</plist>"########;
 let csv = "\u{feff}Abbreviation,Content,Label,Group\r\n;hi,\"Hello,\r\nworld %Y\",Greeting,Chat\r\n;bye,Bye,,Chat\r\n;x,X,,Other\r\n;no,,,\r\n";
 let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/espanso");
 let mut files = serde_json::Map::new();
 fn walk(root:&Path, current:&Path, files:&mut serde_json::Map<String,serde_json::Value>) {
 for entry in std::fs::read_dir(current).unwrap() { let p = entry.unwrap().path(); if p.is_dir() { walk(root,&p,files); } else { files.insert(p.strip_prefix(root).unwrap().to_string_lossy().to_string(),json!(std::fs::read_to_string(p).unwrap())); } }
 }
 walk(&fixture,&fixture,&mut files);
 // The importers stamp `Utc::now()` and draw `Uuid::new_v4()` ids; both are pinned below so the
 // file is byte-identical across runs. Ids already in the inputs (TextExpander `uuidString`) stay.
 let since = chrono::Utc::now() - chrono::Duration::seconds(1);
 let mut imports = json!([
 {"source":"raycast","file":"snippets.json","text":raycast,"expected":raycast::import_json(raycast,"snippets.json").unwrap()},
 {"source":"textexpander","file":"Work.textexpander","text":plist,"expected":textexpander::import_data(plist.as_bytes(),"Work.textexpander").unwrap()},
 {"source":"textexpander","file":"export.csv","text":csv,"expected":textexpander::import_data(csv.as_bytes(),"export.csv").unwrap()},
 {"source":"espanso","files":files.clone(),"expected":espanso::import_dir(&fixture).unwrap()}
 ]);
 let inputs_text = format!("{raycast}{plist}{csv}{}", serde_json::Value::Object(files));
 super::stable::Ids::new(0x1a7e_0000_0000_4000_8000_000000000000,&inputs_text).apply(&mut imports);
 super::stable::clock(&mut imports,since);
 super::write(dir,"imports.json",&imports);

 let yamls = [
r########"matches:
  - trigger: ':x'
    replace: >
      a
      b

      c
"########,
r########"matches:
  - trigger: ':x'
    replace: |+
      a

"########,
r########"matches:
  - trigger: ':x'
    replace: a
      b

      c
"########,
r########"matches:
  - trigger: ':x'
    replace: "a
      b

      c"
"########,
r########"matches:
  - trigger: ':x'
    replace: 'a
      b

      c'
"########,
r########"matches:
  - trigger: ':x'
    replace: "a\
      b"
"########,
r########"matches: [
  {trigger: ':x',
   replace: 'abc'}
]
"########];
 let inputs = dir.join("yaml-inputs"); std::fs::create_dir_all(&inputs).unwrap();
 let mut cases = json!(yamls.iter().map(|text| { let path = inputs.join("case.yml"); std::fs::write(&path,text).unwrap(); json!({"text":text,"expected":espanso::import_files(&[path]).unwrap()}) }).collect::<Vec<_>>());
 super::stable::Ids::new(0x7a31_0000_0000_4000_8000_000000000000,"").apply(&mut cases);
 super::stable::clock(&mut cases,since);
 super::write(dir,"yaml.json",&cases); std::fs::remove_dir_all(inputs).unwrap();
 }
}

/// T2: pins what the Rust importers and `History` generate per run (`Uuid::new_v4` ids and the
/// `Utc::now()` import clock), so two exporter runs write byte-identical files. The Swift replay
/// tests map ids by position and treat import-time stamps as "now", so the pinned values only
/// need to be stable, not meaningful.
mod stable {
    use chrono::{DateTime, Utc};
    use serde_json::Value;
    use uuid::Uuid;

    /// The fixed instant that replaces every import-time stamp.
    pub const CLOCK: &str = "2026-11-01T00:00:00Z";

    /// Renames UUIDs (lowercase ids and the uppercase `{{snippet id="…"}}` form alike) to
    /// `Uuid::from_u128(base | n)` in order of first appearance. Keys are visited in serde_json's
    /// sorted order, so the numbering itself is deterministic. UUIDs found in `keep` are left alone.
    pub struct Ids {
        seen: Vec<String>,
        keep: String,
        base: u128,
    }

    impl Ids {
        pub fn new(base: u128, keep: &str) -> Self {
            Ids { seen: Vec::new(), keep: keep.to_lowercase(), base }
        }

        pub fn apply(&mut self, value: &mut Value) {
            match value {
                Value::String(s) => *s = self.rewrite(s),
                Value::Array(items) => items.iter_mut().for_each(|v| self.apply(v)),
                Value::Object(map) => map.values_mut().for_each(|v| self.apply(v)),
                _ => {}
            }
        }

        fn rewrite(&mut self, s: &str) -> String {
            let bytes = s.as_bytes();
            let mut out = String::with_capacity(s.len());
            let mut i = 0;
            while i < bytes.len() {
                if let Some(candidate) = bytes.get(i..i + 36).filter(|w| is_uuid(w)) {
                    let text = std::str::from_utf8(candidate).expect("ascii");
                    let lower = text.to_lowercase();
                    if self.keep.contains(&lower) {
                        out.push_str(text);
                    } else {
                        let n = match self.seen.iter().position(|x| *x == lower) {
                            Some(n) => n,
                            None => {
                                self.seen.push(lower);
                                self.seen.len() - 1
                            }
                        };
                        let fixed = Uuid::from_u128(self.base | (n as u128 + 1)).hyphenated().to_string();
                        let upper = text.bytes().any(|b| b.is_ascii_uppercase());
                        out.push_str(&if upper { fixed.to_uppercase() } else { fixed });
                    }
                    i += 36;
                } else {
                    let ch = s[i..].chars().next().expect("char boundary");
                    out.push(ch);
                    i += ch.len_utf8();
                }
            }
            out
        }
    }

    fn is_uuid(w: &[u8]) -> bool {
        w.len() == 36
            && w.iter().enumerate().all(|(i, b)| match i {
                8 | 13 | 18 | 23 => *b == b'-',
                _ => b.is_ascii_hexdigit(),
            })
    }

    /// Replaces every RFC 3339 string at or after `since` (the importers' `Utc::now()`) with
    /// [`CLOCK`]. Dates the inputs define (TextExpander `creationDate`) are older and stay.
    pub fn clock(value: &mut Value, since: DateTime<Utc>) {
        match value {
            Value::String(s) => {
                if DateTime::parse_from_rfc3339(s).is_ok_and(|t| t >= since) {
                    *s = CLOCK.to_string();
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|v| clock(v, since)),
            Value::Object(map) => map.values_mut().for_each(|v| clock(v, since)),
            _ => {}
        }
    }
}
