//! In-memory adapters for tests and headless scenes. Construction opens no host resources.
//! `mock-state.json` is copied verbatim from the S02 reference capture.
pub mod apps;
pub mod clock;
pub mod cloudkit;
pub mod engine;
pub mod fetch;
pub mod files;
pub mod hotkey_registrar;
pub mod ids;
pub mod login_item;
pub mod pasteboard;
pub mod relaunch;
pub mod settings_panes;
pub mod thumbnails;
pub mod update_install;

use crate::{error::PortError, ports::Ports};
use kudzu_core::{
    clipboard::History,
    library::model::KudzuLibrary,
    system::{AppInfo, EngineStatus, PermissionStatus, TapMode},
};
use std::sync::Arc;

/// Typed core fixtures plus the complete capture for later service-specific scene builders.
/// `raw` is fixture input only; production views pull service Watches.
pub struct SampleState {
    pub library: KudzuLibrary,
    pub clipboard: History,
    pub apps: Vec<AppInfo>,
    pub status: EngineStatus,
    pub locale: String,
    pub timezone: String,
    pub raw: serde_json::Value,
}
impl SampleState {
    pub fn load() -> Result<Self, PortError> {
        use serde::Deserialize;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct App {
            bundle_id: String,
            name: String,
            path: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Status {
            can_post_events: bool,
            can_expand: bool,
            paused: bool,
            tap: String,
            permissions: Permissions,
            secure_input: bool,
            secure_input_owner: Option<String>,
            summary: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Permissions {
            accessibility: bool,
            input_monitoring: bool,
        }
        #[derive(Deserialize)]
        struct Metadata {
            locale: String,
            timezone: String,
        }
        #[derive(Deserialize)]
        struct Capture {
            library: KudzuLibrary,
            clipboard: History,
            apps: Vec<App>,
            status: Status,
            fixture: Metadata,
        }
        let raw: serde_json::Value = serde_json::from_str(include_str!("mock-state.json"))
            .map_err(|_| PortError::InvalidData)?;
        let data: Capture =
            serde_json::from_value(raw.clone()).map_err(|_| PortError::InvalidData)?;
        let tap = match data.status.tap.as_str() {
            "active" => TapMode::Active,
            "listenOnly" => TapMode::ListenOnly,
            "off" => TapMode::Off,
            _ => return Err(PortError::InvalidData),
        };
        let status = EngineStatus {
            can_post_events: data.status.can_post_events,
            can_expand: data.status.can_expand,
            paused: data.status.paused,
            tap,
            permissions: PermissionStatus {
                accessibility: data.status.permissions.accessibility,
                input_monitoring: data.status.permissions.input_monitoring,
            },
            secure_input: data.status.secure_input,
            secure_input_owner: data.status.secure_input_owner,
            summary: data.status.summary,
        };
        Ok(Self {
            library: data.library,
            clipboard: data.clipboard,
            apps: data
                .apps
                .into_iter()
                .map(|app| AppInfo {
                    bundle_id: app.bundle_id,
                    name: app.name,
                    path: app.path,
                    pid: None,
                })
                .collect(),
            status,
            locale: data.fixture.locale,
            timezone: data.fixture.timezone,
            raw,
        })
    }
}

/// Keep the concrete handles to script failures and inspect effects; pass `ports()` to composition.
pub struct Fixtures {
    pub sample: SampleState,
    pub engine: Arc<engine::FakeEngine>,
    pub pasteboard: Arc<pasteboard::FakePasteboard>,
    pub thumbnails: Arc<thumbnails::FakeThumbnailer>,
    pub apps: Arc<apps::FakeApps>,
    pub settings: Arc<settings_panes::FakeSystemSettings>,
    pub login_item: Arc<login_item::FakeLoginItem>,
    pub files: Arc<files::FakeFiles>,
    pub cloud: Arc<cloudkit::FakeCloud>,
    pub installer: Arc<update_install::FakeInstaller>,
    pub relauncher: Arc<relaunch::FakeRelauncher>,
    pub fetch: Arc<fetch::FakeFetch>,
    pub clock: Arc<clock::FakeClock>,
    pub hotkeys: Arc<hotkey_registrar::FakeHotkeys>,
    pub ids: Arc<ids::FixedIds>,
}
impl Fixtures {
    pub fn new() -> Result<Self, PortError> {
        let sample = SampleState::load()?;
        let engine = Arc::new(engine::FakeEngine::default());
        engine.state.lock().unwrap().status = sample.status.clone();
        let apps = Arc::new(apps::FakeApps::default());
        *apps.installed.lock().unwrap() = sample.apps.clone();
        let settings = Arc::new(settings_panes::FakeSystemSettings::default());
        *settings.permissions.lock().unwrap() = sample.status.permissions;
        let seed = sample.raw["fixture"]["seed"]
            .as_u64()
            .ok_or(PortError::InvalidData)?;
        Ok(Self {
            sample,
            engine,
            apps,
            settings,
            pasteboard: Arc::default(),
            thumbnails: Arc::default(),
            login_item: Arc::default(),
            files: Arc::default(),
            cloud: Arc::default(),
            installer: Arc::default(),
            relauncher: Arc::default(),
            fetch: Arc::default(),
            clock: Arc::default(),
            hotkeys: Arc::default(),
            ids: Arc::new(ids::FixedIds::new(seed.into())),
        })
    }
    pub fn ports(&self) -> Ports {
        Ports {
            engine: self.engine.clone(),
            pasteboard: self.pasteboard.clone(),
            thumbnails: self.thumbnails.clone(),
            apps: self.apps.clone(),
            settings: self.settings.clone(),
            login_item: self.login_item.clone(),
            files: self.files.clone(),
            cloud: self.cloud.clone(),
            installer: self.installer.clone(),
            relauncher: self.relauncher.clone(),
            fetch: self.fetch.clone(),
            clock: self.clock.clone(),
            hotkeys: self.hotkeys.clone(),
            ids: self.ids.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PortError;
    use crate::ports::*;
    use std::{sync::Arc, time::Duration};

    #[test]
    fn fixture_inputs_and_identity_are_repeatable_without_host_state() {
        let a = Fixtures::new().unwrap();
        let b = Fixtures::new().unwrap();
        assert_eq!(a.sample.library.snippets.len(), 18);
        assert_eq!(a.sample.library.groups.len(), 4);
        assert_eq!(a.sample.library.snippets[0].name, "Email signature");
        assert_eq!(
            (a.sample.locale.as_str(), a.sample.timezone.as_str()),
            ("en-US", "UTC")
        );
        assert_eq!(a.ports().clock.now(), b.ports().clock.now());
        let first = a.ports().ids.next_id();
        assert_eq!(first, b.ports().ids.next_id());
        assert_ne!(first, a.ports().ids.next_id());
        assert_eq!(a.sample.raw["fixture"]["clock"], "2026-11-11T12:00:00.000Z");
        assert_eq!(
            a.ports()
                .clock
                .now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            1_794_398_400
        );
    }

    #[test]
    fn pasteboard_owns_its_writes_and_external_copies_are_distinct() {
        let fake = super::pasteboard::FakePasteboard::default();
        let items = vec![vec![("public.utf8-plain-text".into(), b"sample".to_vec())]];
        let count = fake.write(&items).unwrap();
        assert_eq!(fake.change_count(), count);
        assert!(fake.read().unwrap().own);
        fake.external_copy(items.clone());
        assert!(!fake.read().unwrap().own);
        assert_eq!(fake.read().unwrap().items, items);
        assert_eq!(fake.paste(None), Err(PortError::PermissionDenied));
    }

    #[test]
    fn fetch_enforces_caps_and_fake_system_adapters_record_effects() {
        let f = Fixtures::new().unwrap();
        let p = f.ports();
        f.fetch
            .respond("https://example.test/feed", Ok(vec![1, 2, 3]));
        let request = FetchRequest {
            url: "https://example.test/feed".into(),
            timeout: Duration::from_secs(1),
            max_bytes: 2,
        };
        assert_eq!(p.fetch.get(&request), Err(PortError::TooLarge));
        p.login_item.set_enabled(true).unwrap();
        assert!(p.login_item.enabled().unwrap());
        p.files
            .reveal(std::path::Path::new("fixture.json"))
            .unwrap();
        assert_eq!(f.files.revealed.lock().unwrap().len(), 1);
        p.settings.open(SettingsPane::Accessibility).unwrap();
        assert_eq!(
            *f.settings.opened.lock().unwrap(),
            [SettingsPane::Accessibility]
        );
        p.hotkeys
            .register(ShortcutSlot::QuickSearch, "Ctrl+Alt+Space")
            .unwrap();
        assert_eq!(
            f.hotkeys
                .registered
                .lock()
                .unwrap()
                .get(&ShortcutSlot::QuickSearch)
                .unwrap(),
            "Ctrl+Alt+Space"
        );
        p.hotkeys.unregister(ShortcutSlot::QuickSearch).unwrap();
        assert!(f.hotkeys.registered.lock().unwrap().is_empty());
        p.relauncher.relaunch().unwrap();
        assert_eq!(*f.relauncher.calls.lock().unwrap(), 1);
        assert_eq!(p.apps.installed().unwrap().len(), 12);
        assert_eq!(p.apps.icon("missing", 64).unwrap(), None);
        assert!(p.thumbnails.thumbnail(&[], 480).unwrap().is_none());
        assert!(!p.installer.probe().unwrap().needs_move);
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Ports>();
        send_sync::<crate::persist::Writer>();
        let _: Arc<dyn Clock> = p.clock;
    }

    #[test]
    fn engine_callbacks_are_typed_and_suppression_is_independent_of_focus() {
        let engine = super::engine::FakeEngine::default();
        let events = Arc::new(super::engine::FakeEngineEvents::default());
        engine.start(events.clone()).unwrap();
        let expansion = Expansion {
            snippet_id: uuid::Uuid::from_u128(1),
            chars_saved: 12,
        };
        engine.emit_expansion(expansion);
        assert_eq!(*events.expansions.lock().unwrap(), [expansion]);
        engine.set_suppressed(true);
        engine.panel_focused(5, false);
        assert!(engine.state.lock().unwrap().suppressed);
        assert_eq!(engine.state.lock().unwrap().focus, [(5, false)]);
        engine.stop();
        engine.emit_expansion(expansion);
        assert_eq!(events.expansions.lock().unwrap().len(), 1);
    }

    #[test]
    fn cloud_fake_can_deliver_old_generations_for_service_fencing_tests() {
        let cloud = super::cloudkit::FakeCloud::default();
        let events = Arc::new(super::cloudkit::FakeCloudEvents::default());
        cloud.start(1, Some(vec![1, 2]), events.clone()).unwrap();
        assert_eq!(cloud.checkpoint().unwrap(), [1, 2]);
        cloud.stop();
        cloud.start(2, None, events.clone()).unwrap();
        cloud.deliver(1, CloudBatch::default());
        cloud.deliver(
            2,
            CloudBatch {
                full_fetch: true,
                ..Default::default()
            },
        );
        let received = events.fetched.lock().unwrap();
        assert_eq!(
            received
                .iter()
                .map(|(generation, _)| *generation)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(received[1].1.full_fetch);
    }
}
