//! Headless scenes for `--window form`. The demo matches `/form?demo`.
use std::collections::HashMap;
use std::sync::Arc;

use kudzu_app::forms::FormRequest;
use kudzu_app::ports::FormToken;
use kudzu_core::system::AppInfo;
use kudzu_core::template::{FormFieldKind, FormFieldSpec};
use uuid::Uuid;

use crate::shell::windows::FormState;

/// Meeting follow-up in the S02 mock library.
pub const DEMO_SNIPPET: &str = "20493e8e-c034-4ff0-b926-e5ae3e119630";

/// Body used when the fixture library has not installed that snippet (getting-started seed).
/// Nested `;sig` is inlined so the preview does not diagnose a missing id.
pub const DEMO_BODY: &str = "Hi {{field name=\"Name\"}},\n\nThanks for the time today. As discussed, {{cursor}}\n\nBest regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}";

type Scene = (&'static str, fn(&mut FormState));

pub const SCENES: &[Scene] = &[("demo", demo)];

/// `/form?demo`: Meeting follow-up, Name, into Mail.
pub fn demo(state: &mut FormState) {
    state.play_scene(demo_request());
}

pub fn demo_request() -> FormRequest {
    FormRequest {
        token: FormToken::default(),
        snippet_id: Uuid::parse_str(DEMO_SNIPPET).expect("demo snippet id"),
        snippet_name: "Meeting follow-up".into(),
        fields: vec![FormFieldSpec {
            name: "Name".into(),
            kind: FormFieldKind::Text,
            options: vec![],
            default_value: None,
        }],
        initial_values: HashMap::new(),
        target: Some(AppInfo {
            bundle_id: "com.apple.mail".into(),
            name: "Mail".into(),
            pid: None,
            path: None,
        }),
        caret: None,
        selection: None,
    }
}

impl FormState {
    /// Keep this request on screen when `pending()` is empty (headless `--scene demo`).
    pub fn play_scene(&mut self, request: FormRequest) {
        let _ = self
            .svc
            .settings
            .update(|settings| settings.theme = kudzu_app::settings::Theme::System);
        self.model.theme.sync(&self.svc.settings.watch());
        let request = Arc::new(request);
        self.model.scene = Some(request.clone());
        self.load(request);
        self.warm_preview();
    }
}
