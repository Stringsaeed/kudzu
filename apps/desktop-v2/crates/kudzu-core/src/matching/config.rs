//! Global inputs to trigger matching and the insertion precedence chain.

use std::collections::{HashMap, HashSet};

use crate::library::groups::EffectiveSnippet;
use crate::library::model::InsertionMethod;

/// Global inputs to trigger matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchingConfig {
    pub default_insertion: InsertionMethod,
    /// Apps where nothing ever expands.
    pub denied_bundle_ids: HashSet<String>,
    /// Global per-app insertion overrides.
    pub app_insertions: HashMap<String, InsertionMethod>,
    /// Punctuation that ends a word for whole-word snippets; whitespace always does.
    pub separators: HashSet<char>,
}

impl MatchingConfig {
    /// `. , ! ? ; : ) ] } ' "`, plus the common punctuation of other scripts (Spanish, French,
    /// typographic quotes and dashes, CJK, Arabic, Hebrew, Devanagari). Symbols that join words
    /// (`-`, `_`, `@`, `/`) and the ASCII opening brackets are not separators.
    pub fn default_separators() -> HashSet<char> {
        const ASCII: &str = ".,!?;:)]}'\"";
        const OTHER: &str =
            "¿¡«»‹›“”‘’„‚…–—。、，．！？；：（）［］｛｝「」『』【】〈〉《》،؛؟۔׃־।॥";
        ASCII.chars().chain(OTHER.chars()).collect()
    }

    /// Password managers and the login window.
    pub fn default_denied_bundle_ids() -> HashSet<String> {
        [
            "com.1password.1password",
            "com.agilebits.onepassword7",
            "com.agilebits.onepassword-osx",
            "com.bitwarden.desktop",
            "com.apple.keychainaccess",
            "com.apple.Passwords",
            "com.apple.loginwindow",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    /// Terminals prefer paste.
    pub fn default_app_insertions() -> HashMap<String, InsertionMethod> {
        [
            "com.apple.Terminal",
            "com.googlecode.iterm2",
            "dev.warp.Warp-Stable",
            "com.mitchellh.ghostty",
            "org.alacritty",
            "net.kovidgoyal.kitty",
            "com.github.wez.wezterm",
        ]
        .into_iter()
        .map(|b| (b.to_string(), InsertionMethod::Paste))
        .collect()
    }

    /// Whitespace, newline or a configured separator.
    pub fn is_separator(&self, c: char) -> bool {
        c.is_whitespace() || self.separators.contains(&c)
    }
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            default_insertion: InsertionMethod::Automatic,
            denied_bundle_ids: Self::default_denied_bundle_ids(),
            app_insertions: Self::default_app_insertions(),
            separators: Self::default_separators(),
        }
    }
}
/// The shared tail of the insertion precedence chain.
pub(crate) fn resolve_insertion(
    rule_override: Option<InsertionMethod>,
    own: Option<InsertionMethod>,
    bundle_id: Option<&str>,
    config: &MatchingConfig,
) -> InsertionMethod {
    rule_override
        .or(own)
        .or_else(|| bundle_id.and_then(|id| config.app_insertions.get(id).copied()))
        .unwrap_or(config.default_insertion)
}

impl EffectiveSnippet {
    /// Insertion for `bundle_id`: listed rule override ▸ snippet ▸ group ▸ global per-app ▸ default.
    pub fn insertion_for(
        &self,
        bundle_id: Option<&str>,
        config: &MatchingConfig,
    ) -> InsertionMethod {
        let rule_override = self.rule.as_ref().and_then(|rule| {
            let id = bundle_id?;
            rule.bundle_ids
                .iter()
                .any(|b| b == id)
                .then_some(rule.insertion)
                .flatten()
        });
        resolve_insertion(rule_override, self.insertion, bundle_id, config)
    }
}
