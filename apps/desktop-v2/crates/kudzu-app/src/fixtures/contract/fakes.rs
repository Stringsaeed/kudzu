//! Each fake passes the suite its macOS adapter passes.
use super::{
    apps::{AppsHarness, PNG_MAGIC},
    engine::{EngineHarness, NativeLog, NativeSide},
    pasteboard::{Offered, PasteboardHarness},
    thumbnails::ThumbnailHarness,
};
use crate::{
    fixtures::{
        apps::FakeApps, engine::FakeEngine, pasteboard::FakePasteboard,
        settings_panes::FakeSystemSettings, thumbnails::FakeThumbnailer,
    },
    ports::{
        AppCatalog, EngineInstances, Expansion, ExpansionEngine, FileIcons, FormChannel,
        FormRequest, FormToken, KudzuWindows, Pasteboard, TargetApp, Thumbnail, Thumbnailer,
        TriggerLost,
    },
};
use kudzu_core::{
    activity::ActivityEvent,
    system::{AppInfo, EngineStatus},
};
use uuid::Uuid;

struct Engine(FakeEngine);
impl NativeSide for Engine {
    fn request_form(&self, native: u64, snippet: Uuid, target: Option<AppInfo>) {
        self.0.emit_form(FormRequest {
            token: self.0.form_token(native),
            snippet_id: snippet,
            snippet_name: "Form".into(),
            fields: vec![],
            target,
            caret: None,
            selection: None,
        });
    }
    fn report_status(&self, status: EngineStatus) {
        self.0.emit_status(status);
    }
    fn report_expansion(&self, expansion: Expansion) {
        self.0.emit_expansion(expansion);
    }
    fn report_activity(&self, event: ActivityEvent) {
        self.0.emit_activity(event);
    }
    fn report_trigger_lost(&self, lost: TriggerLost) {
        self.0.emit_trigger_lost(lost);
    }
    fn ask_history(&self, index: usize, current: Option<&str>) -> Option<String> {
        self.0.events()?.clipboard_history(index, current)
    }
    fn log(&self) -> NativeLog {
        let state = self.0.state.lock().unwrap();
        let native = |token: FormToken| state.instance.native_token(token).unwrap();
        NativeLog {
            inserts: state
                .insertions
                .iter()
                .map(|r| (r.snippet, r.target.clone(), r.selection.clone()))
                .collect(),
            submissions: state
                .submissions
                .iter()
                .map(|(t, values, s)| (native(*t), values.clone(), *s))
                .collect(),
            cancels: state.cancelled.iter().map(|t| native(*t)).collect(),
            focus: state.focus.iter().map(|(t, f)| (native(*t), *f)).collect(),
            windows: state.windows.clone(),
            reloads: (state.reloads > 0)
                .then(|| (state.library.snippets.len(), state.library.locale.clone()))
                .into_iter()
                .collect(),
        }
    }
}
impl EngineHarness for Engine {
    fn engine(&self) -> &dyn ExpansionEngine {
        &self.0
    }
    fn forms(&self) -> &dyn FormChannel {
        &self.0
    }
    fn windows(&self) -> &dyn KudzuWindows {
        &self.0
    }
    fn native(&self) -> Option<&dyn NativeSide> {
        Some(self)
    }
    fn foreign_token(&self, native: u64) -> FormToken {
        let other = EngineInstances::default();
        other.next_engine();
        other.next_engine().form_token(native)
    }
    fn own_token(&self, native: u64) -> FormToken {
        self.0.form_token(native)
    }
}

#[test]
fn fake_engine_passes_the_engine_contract() {
    super::engine::without_native(&Engine(FakeEngine::default()));
    super::engine::with_native(&Engine(FakeEngine::default()));
}

struct Board(FakePasteboard);
impl PasteboardHarness for Board {
    fn board(&self) -> &dyn Pasteboard {
        &self.0
    }
    fn external_copy(&self, items: &[Vec<Offered<'_>>]) {
        self.0.external_copy(
            items
                .iter()
                .map(|item| {
                    item.iter()
                        .map(|(kind, data)| (kind.to_string(), data.unwrap_or_default().to_vec()))
                        .collect()
                })
                .collect(),
        );
    }
}

#[test]
fn fake_pasteboard_passes_the_pasteboard_contract() {
    super::pasteboard::reads(&Board(FakePasteboard::default()));
    super::pasteboard::writes(&Board(FakePasteboard::default()));
}

/// A PNG signature and IHDR with this size; the fakes never decode.
fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut png = PNG_MAGIC.to_vec();
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&width.to_be_bytes());
    png.extend_from_slice(&height.to_be_bytes());
    png
}

struct Apps(FakeApps);
impl AppsHarness for Apps {
    fn catalog(&self) -> &dyn AppCatalog {
        &self.0
    }
    fn target(&self) -> &dyn TargetApp {
        &self.0
    }
    fn file_icons(&self) -> &dyn FileIcons {
        &self.0
    }
    fn existing_path(&self) -> String {
        "/Applications/Mail.app".into()
    }
}

#[test]
fn fake_apps_pass_the_apps_contract() {
    let fake = FakeApps::default();
    *fake.installed.lock().unwrap() = ["Calendar", "mail", "Notes"]
        .into_iter()
        .map(|name| AppInfo {
            bundle_id: format!("com.apple.{}", name.to_lowercase()),
            name: name.into(),
            pid: None,
            path: None,
        })
        .collect();
    let mut icons = fake.icons.lock().unwrap();
    icons.insert("com.apple.calendar".into(), png_header(32, 32));
    icons.insert("/Applications/Mail.app".into(), png_header(64, 64));
    drop(icons);
    let apps = Apps(fake);
    super::apps::catalog(&apps);
    super::apps::file_icons(&apps);
    super::apps::target_reads(&apps);
}

struct Thumbs(FakeThumbnailer);
impl ThumbnailHarness for Thumbs {
    fn thumbnailer(&self) -> &dyn Thumbnailer {
        &self.0
    }
    fn sample(&self) -> (Vec<u8>, u32, u32) {
        (png_header(256, 128), 256, 128)
    }
}

#[test]
fn fake_thumbnailer_passes_the_thumbnail_contract() {
    let fake = FakeThumbnailer::default();
    *fake.output.lock().unwrap() = Some(Thumbnail {
        png: png_header(64, 32),
        width: 256,
        height: 128,
    });
    super::thumbnails::bounds(&Thumbs(fake));
}

#[test]
fn fake_settings_pass_the_settings_contract() {
    super::settings_panes::reads(&FakeSystemSettings::default());
}
