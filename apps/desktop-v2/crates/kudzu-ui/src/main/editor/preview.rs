//! The preview (EDT-16, `snippet-editor.tsx:170-196`, `:466-485`): the draft rendered 120 ms
//! after the last edit, on a worker (`rune::Latest`, so a newer request drops an older answer),
//! with the editor's sample selection. Its badges come from the source at once. When the
//! rendered text changes after 600 ms of stillness the body settles in place; while typing it
//! stays still.

use std::collections::HashMap;
use std::sync::Arc;

use kudzu_app::expansion::{ExpansionService, PreviewContext};
use kudzu_core::formatting::Formatting;
use kudzu_core::template::{self, Node, RenderResult, TemplateTag};
use rune::{col, row, text, El, Latest, Sequence, Waker};
use uuid::Uuid;

use super::model::Timed;
use crate::shell::MainState;
use crate::theme::{motion, radius, Theme};
use crate::widgets::formatted_text::formatted_text;

pub const SECTION_KEY: &str = "editor-preview";
pub const BODY_KEY: &str = "editor-preview-body";
/// `useDebounced(draft, 120)`.
pub const DEBOUNCE: f64 = 0.120;
/// A change after this much stillness settles in (`snippet-editor.tsx:181`).
pub const STILLNESS: f64 = 0.600;
/// `animate-settle`.
pub const SETTLE_SECS: f64 = motion::MODERATE as f64;
/// What `{{selection}}` shows in the editor (`lib/backend.ts`).
pub const SAMPLE_SELECTION: &str = "selected text";
pub const EMPTY: &str = "Nothing to show yet.";

/// What the badges say about the draft's source (`analyze` in `lib/template.ts`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub field_names: Vec<String>,
    pub uses_clipboard: bool,
    pub uses_selection: bool,
    pub presses_keys: bool,
    /// A `{{key}}` after `{{cursor}}`: the caret goes only after the last key press.
    pub cursor_cancelled: bool,
}

impl Facts {
    pub fn of(source: &str) -> Facts {
        let parsed = template::parse(source);
        let info = template::analyze(&parsed);
        let mut field_names: Vec<String> = Vec::new();
        for f in &info.form_fields {
            if !field_names.contains(&f.name) {
                field_names.push(f.name.clone());
            }
        }
        let mut cursor = false;
        let mut cursor_cancelled = false;
        for node in &parsed.nodes {
            if let Node::Tag(tag) = node {
                match template::typed(tag) {
                    TemplateTag::Cursor => cursor = true,
                    TemplateTag::Key { .. } if cursor => cursor_cancelled = true,
                    _ => {}
                }
            }
        }
        Facts {
            field_names,
            uses_clipboard: info.uses_clipboard,
            uses_selection: info.uses_selection,
            presses_keys: info.has_key_presses,
            cursor_cancelled,
        }
    }

    /// `(copy, tone, title)` per badge, in Tauri's order.
    pub fn badges(&self) -> Vec<(String, Tone, Option<&'static str>)> {
        let mut out = Vec::new();
        if !self.field_names.is_empty() {
            out.push((
                format!("Asks for {}", self.field_names.join(", ")),
                Tone::Warning,
                None,
            ));
        }
        if self.uses_clipboard {
            out.push(("Uses clipboard".into(), Tone::Success, None));
        }
        if self.uses_selection {
            out.push((
                "Selection: Quick Search only".into(),
                Tone::Success,
                Some(
                    "Typing a trigger replaces the selection, so the selected text is empty there.",
                ),
            ));
        }
        if self.presses_keys {
            out.push((
                "Presses keys".into(),
                Tone::Neutral,
                Some("Key presses are sent in order with the text. This snippet can't be undone with Backspace."),
            ));
        }
        if self.cursor_cancelled {
            out.push((
                "Cursor before a key press is ignored".into(),
                Tone::Warning,
                Some("The caret can only be placed in the text after the last key press."),
            ));
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Warning,
    Success,
    Neutral,
}

/// A rendered answer and what it was for.
#[derive(Clone, Debug)]
struct Answer {
    id: Uuid,
    result: RenderResult,
}

/// The request in flight and the shown result.
pub struct Preview {
    latest: Latest<Option<Answer>>,
    /// When the debounced request is due, on the frame clock.
    due: Option<f64>,
    pub shown: Option<RenderResult>,
    for_id: Option<Uuid>,
    changed_at: Option<f64>,
    /// The settle cue, and how many have played (its element's key).
    pub cue: Timed,
    pub cues: u32,
    /// Headless tooling runs (`--tree`, `--snap`, `--script`) render on the UI thread, so a
    /// snapshot never races the worker (PLAN §1.3). The app and the tests use the worker.
    pub(crate) inline: bool,
}

impl Preview {
    pub fn new(wake: Waker) -> Preview {
        Preview {
            latest: Latest::new(wake),
            due: None,
            shown: None,
            for_id: None,
            changed_at: None,
            cue: Timed::default(),
            cues: 0,
            inline: rune::is_headless(),
        }
    }

    /// A snippet opened: nothing shows until its first render, which goes at once.
    pub fn opened(&mut self, id: Uuid, now: f64) {
        self.latest.cancel();
        self.shown = None;
        self.for_id = Some(id);
        self.changed_at = None;
        self.cue.stop();
        self.due = Some(now);
    }

    pub fn closed(&mut self) {
        self.latest.cancel();
        self.shown = None;
        self.for_id = None;
        self.due = None;
        self.cue.stop();
    }

    /// The content or formatting changed at `now`.
    pub fn edited(&mut self, now: f64) {
        self.due = Some(now + DEBOUNCE);
    }

    pub fn due(&self) -> Option<f64> {
        self.due
    }

    /// Whether a request is waiting out the debounce or running.
    pub fn is_pending(&self) -> bool {
        self.due.is_some() || self.latest.is_pending()
    }

    /// An answer for the open snippet at `now`: a change after stillness cues the settle.
    fn land(&mut self, result: RenderResult, now: f64) -> bool {
        if let Some(prev) = &self.shown {
            let formatting = |r: &RenderResult| r.formatting.as_ref().map(Vec::len);
            if prev.text != result.text || formatting(prev) != formatting(&result) {
                if self.changed_at.is_none_or(|at| now - at > STILLNESS) {
                    self.cue.start(now, SETTLE_SECS);
                    self.cues += 1;
                }
                self.changed_at = Some(now);
            }
        }
        let changed = self.shown.as_ref() != Some(&result);
        self.shown = Some(result);
        changed
    }
}

fn render(
    expansion: &ExpansionService,
    id: Uuid,
    source: String,
    formatting: Option<Formatting>,
) -> Option<Answer> {
    let context = PreviewContext {
        snippet_id: Some(id),
        formatting,
        selection: Some(SAMPLE_SELECTION.into()),
        form_values: HashMap::new(),
    };
    let task = expansion.preview(source, context).ok()?;
    match task.wait() {
        Ok(Ok(result)) => Some(Answer { id, result }),
        _ => None,
    }
}

/// Sends the due request and lands answers for the open snippet. True to rebuild.
pub(crate) fn poll(s: &mut MainState) -> bool {
    let now = s.frame.clock_secs;
    let mut changed = s.editor.preview.cue.expire(now);
    if s.editor.preview.due.is_some_and(|due| now >= due) {
        s.editor.preview.due = None;
        if let Some(d) = s.editor.draft() {
            let (id, source, formatting) = (d.id, d.content.clone(), d.formatting.clone());
            let expansion = Arc::clone(&s.svc.expansion);
            if s.editor.preview.inline {
                if let Some(answer) = render(&expansion, id, source, formatting) {
                    changed |= s.editor.preview.land(answer.result, now);
                }
            } else {
                s.editor
                    .preview
                    .latest
                    .request((id, source, formatting), move |(id, source, formatting)| {
                        render(&expansion, id, source, formatting)
                    });
            }
        }
    }
    if let Some(Some(answer)) = s.editor.preview.latest.take() {
        if s.editor.open_id() == Some(answer.id) {
            changed |= s.editor.preview.land(answer.result, now);
        }
    }
    changed
}

/// Scenes: the first render, waited for here so a snapshot never races the worker.
pub(crate) fn render_now(s: &mut MainState) {
    let Some(d) = s.editor.draft() else {
        return;
    };
    let answer = render(
        &s.svc.expansion,
        d.id,
        d.content.clone(),
        d.formatting.clone(),
    );
    s.editor.preview.latest.cancel();
    s.editor.preview.due = None;
    if let Some(answer) = answer {
        s.editor.preview.shown = Some(answer.result);
    }
}

pub fn view(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let source = s.editor.draft().map(|d| d.content.as_str()).unwrap_or("");
    let mut heading = row().gap(8.0).items_center().wrap().child(
        text("Preview")
            .font(theme.ty.control_xs)
            .color(c.muted_foreground),
    );
    for (copy, tone, title) in Facts::of(source).badges() {
        let (bg, fg) = match tone {
            Tone::Warning => (c.warning.alpha(0.12), c.warning),
            Tone::Success => (c.success.alpha(0.12), c.success),
            Tone::Neutral => (c.foreground.alpha(0.06), c.muted_foreground),
        };
        let mut badge = row()
            .key(format!("editor-badge-{copy}"))
            .px(6.0)
            .py(1.0)
            .radius(4.0)
            .fill(bg)
            .transition(crate::motion::check_in())
            .child(
                text(copy.clone())
                    .font(rune::TextStyle::new(10.5, 500.0).lh(14.0))
                    .color(fg),
            );
        if let Some(title) = title {
            badge = badge.help(title);
        }
        heading = heading.child(badge);
    }

    let preview = &s.editor.preview;
    let body_text = match &preview.shown {
        Some(r) if !r.text.is_empty() || !r.actions.is_empty() => {
            formatted_text(theme, r, theme.ty.preview, c.foreground, None)
        }
        _ => text(EMPTY).font(theme.ty.preview).color(c.faint),
    };
    let mut body = col()
        .key(format!("{BODY_KEY}-{}", preview.cues))
        .w_full()
        .child(body_text);
    if preview.cue.running() {
        body = body.sequence(Sequence::new(&motion::beats::SETTLE));
    }
    let well = col()
        .key("editor-preview-well")
        .w_full()
        .min_h(48.0)
        .px(16.0)
        .py(12.0)
        .radius(radius::XL)
        .fill(c.foreground.alpha(0.025))
        .child(body);
    let well = theme
        .depth
        .well
        .iter()
        .fold(well, |e, shadow| e.box_shadow(*shadow));
    col()
        .key(SECTION_KEY)
        .role("region")
        .label("Preview")
        .w_full()
        .gap(6.0)
        .child(heading)
        .child(well)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(text: &str) -> RenderResult {
        RenderResult {
            text: text.into(),
            formatting: None,
            cursor_offset: None,
            diagnostics: Vec::new(),
            actions: Vec::new(),
        }
    }

    #[test]
    fn badges_follow_the_source_in_tauris_order_and_copy() {
        let f = Facts::of(
            r#"{{field name="Name"}} {{choice name="Plan" options="A|B"}} {{field name="Name"}} {{clipboard}} {{selection}} {{cursor}} {{key name="tab"}}"#,
        );
        let copy: Vec<String> = f.badges().into_iter().map(|b| b.0).collect();
        assert_eq!(
            copy,
            [
                "Asks for Name, Plan",
                "Uses clipboard",
                "Selection: Quick Search only",
                "Presses keys",
                "Cursor before a key press is ignored",
            ]
        );
        assert!(!Facts::of(r#"{{key name="tab"}}{{cursor}}"#).cursor_cancelled);
        assert!(Facts::of("plain").badges().is_empty());
    }

    #[test]
    fn a_change_after_stillness_settles_and_typing_stays_still() {
        let mut p = Preview::new(Arc::new(|| {}));
        p.opened(Uuid::nil(), 0.0);
        assert!(p.land(result("a"), 0.1));
        assert_eq!(p.cues, 0, "the first render is not a change");
        p.land(result("ab"), 0.2);
        assert_eq!(p.cues, 1, "nothing had changed before it");
        p.land(result("abc"), 0.5);
        assert_eq!(p.cues, 1, "within 600 ms of the last change");
        p.land(result("abcd"), 1.2);
        assert_eq!(p.cues, 2);
        assert!(p.cue.running());
        assert!(p.cue.expire(1.2 + SETTLE_SECS), "the cue expires");
        assert!(!p.land(result("abcd"), 3.0), "the same text is no change");
        assert_eq!(p.cues, 2);
    }
}
