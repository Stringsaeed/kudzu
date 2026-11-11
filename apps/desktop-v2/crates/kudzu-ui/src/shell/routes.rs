//! Where `Shell::open_main` sends the main window. Typed, never a string (`kudzu://navigate`).
use uuid::Uuid;

/// A Settings pane, in sidebar order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsPane {
    #[default]
    General,
    Expansion,
    Apps,
    Clipboard,
    Agents,
    Data,
    Activity,
    About,
}

/// What a new snippet starts with. `content` is user text (a captured selection): it has no
/// `Debug` output and is never logged.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct NewSnippet {
    pub content: String,
}

impl std::fmt::Debug for NewSnippet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewSnippet")
            .field("content_len", &self.content.len())
            .finish()
    }
}

/// A screen of the main window. S41 maps each to its screen module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Route {
    #[default]
    Library,
    Snippet(Uuid),
    NewSnippet(NewSnippet),
    Settings(SettingsPane),
    Clipboard,
    Insights,
    Onboarding,
}

impl Route {
    /// The placeholder's label, and the main window's title until S41.
    pub fn label(&self) -> &'static str {
        match self {
            Route::Library | Route::Snippet(_) => "Library",
            Route::NewSnippet(_) => "New Snippet",
            Route::Settings(_) => "Settings",
            Route::Clipboard => "Clipboard",
            Route::Insights => "Insights",
            Route::Onboarding => "Welcome",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NewSnippet, Route};

    #[test]
    fn a_new_snippet_route_never_prints_its_content() {
        let route = Route::NewSnippet(NewSnippet {
            content: "secret words".into(),
        });
        let printed = format!("{route:?}");
        assert!(!printed.contains("secret"));
        assert!(printed.contains("content_len: 12"));
        assert_eq!(route.label(), "New Snippet");
    }
}
