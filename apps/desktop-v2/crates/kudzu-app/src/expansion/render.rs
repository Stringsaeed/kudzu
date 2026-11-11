//! Content-bearing rendering stays on workers and out of Activity/errors.
use super::{ExpansionError, ExpansionService};
use crate::{
    ports::{Clock, Pasteboard},
    tasks::{Task, TaskError},
    watch::Watch,
};
use chrono::{DateTime, FixedOffset};
use kudzu_core::{
    formatting::{self, Formatting},
    library::model::KudzuLibrary,
    template::{self, RenderContext, RenderResult},
};
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use uuid::Uuid;

/// S40 supplies clipboard.history_text here. Never route it through Activity or agents.
pub type HistoryReader = dyn Fn(usize, Option<&str>) -> Option<String> + Send + Sync;
pub struct RenderEnvironment {
    pub clock: Arc<dyn Clock>,
    pub pasteboard: Arc<dyn Pasteboard>,
    pub locale: String,
    /// Production converts each instant through chrono::Local, preserving DST changes.
    pub local_time: fn(SystemTime) -> DateTime<FixedOffset>,
    pub history: Arc<HistoryReader>,
}
#[derive(Clone, Default)]
pub struct PreviewContext {
    pub snippet_id: Option<Uuid>,
    pub formatting: Option<Formatting>,
    pub selection: Option<String>,
    pub form_values: HashMap<String, String>,
}
pub(super) struct Renderer {
    pub(super) library: Watch<KudzuLibrary>,
    pub(super) environment: RenderEnvironment,
}
impl Renderer {
    pub(super) fn render(
        &self,
        source: &str,
        context: PreviewContext,
    ) -> Result<RenderResult, ExpansionError> {
        let library = self.library.get();
        let resolve = |id| {
            library
                .snippets
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.content.clone())
        };
        let resolve_formatting = |id| {
            library
                .snippets
                .iter()
                .find(|s| s.id == id)
                .and_then(|s| s.formatting.clone())
        };
        let clipboard = self
            .environment
            .pasteboard
            .read()?
            .items
            .iter()
            .flat_map(|item| item.iter())
            .find(|(kind, _)| kind == "public.utf8-plain-text")
            .and_then(|(_, bytes)| String::from_utf8(bytes.clone()).ok());
        let history = |index| (self.environment.history)(index, clipboard.as_deref());
        let render_context = RenderContext {
            now: (self.environment.local_time)(self.environment.clock.now()),
            locale: self.environment.locale.clone(),
            clipboard: clipboard.clone(),
            selection: context.selection,
            form_values: context.form_values,
            root_snippet_id: context.snippet_id,
            resolve_snippet: &resolve,
            resolve_formatting: Some(&resolve_formatting),
            clipboard_history: Some(&history),
            new_uuid: Some(&template::random_uuid),
            pick: Some(&template::random_index),
        };
        Ok(template::render_formatted(
            source,
            context.formatting.as_ref(),
            &render_context,
        ))
    }
}
impl ExpansionService {
    pub fn preview(
        &self,
        source: String,
        context: PreviewContext,
    ) -> Result<Task<Result<RenderResult, ExpansionError>>, TaskError> {
        let renderer = self.renderer.clone();
        Task::spawn("kudzu-preview", move || renderer.render(&source, context))
    }
    pub fn preview_snippet(
        &self,
        id: Uuid,
        selection: Option<String>,
        form_values: HashMap<String, String>,
    ) -> Result<Task<Result<RenderResult, ExpansionError>>, TaskError> {
        let renderer = self.renderer.clone();
        Task::spawn("kudzu-snippet-preview", move || {
            let library = renderer.library.get();
            let snippet = library
                .snippets
                .iter()
                .find(|s| s.id == id)
                .ok_or(ExpansionError::MissingSnippet)?;
            renderer.render(
                &snippet.content,
                PreviewContext {
                    snippet_id: Some(id),
                    formatting: snippet.formatting.clone(),
                    selection,
                    form_values,
                },
            )
        })
    }
    /// Copies only text and optional HTML. Cursor/key actions remain preview metadata.
    pub fn copy_rendered(
        &self,
        result: RenderResult,
    ) -> Result<Task<Result<(), ExpansionError>>, TaskError> {
        let pasteboard = self.renderer.environment.pasteboard.clone();
        Task::spawn("kudzu-copy", move || {
            let mut item = vec![(
                "public.utf8-plain-text".into(),
                result.text.as_bytes().to_vec(),
            )];
            if let Some(spans) = result.formatting {
                item.push((
                    "public.html".into(),
                    formatting::html(&result.text, &spans).into_bytes(),
                ));
            }
            pasteboard.write(&[item])?;
            Ok(())
        })
    }
}
