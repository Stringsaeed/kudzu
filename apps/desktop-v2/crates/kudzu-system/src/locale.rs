//! The system locale for date tags. The engine never reads it: S40 passes it as
//! `RenderEnvironment.locale`, and the engine adapter substitutes it for an empty
//! `EngineLibrary.locale`.
use objc2::rc::autoreleasepool;
use objc2_foundation::NSLocale;

/// The current locale identifier without keyword suffixes, e.g. `en_US`; `en_US` when the
/// system reports none. Cheap; safe on any thread.
pub fn current() -> String {
    autoreleasepool(|_| base_identifier(&NSLocale::currentLocale().localeIdentifier().to_string()))
}

/// Drops keyword suffixes such as `@rg=gbzzzz`.
fn base_identifier(identifier: &str) -> String {
    let base = identifier.split('@').next().unwrap_or_default().trim();
    if base.is_empty() {
        "en_US".to_string()
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_suffixes_are_dropped_and_an_empty_identifier_is_en_us() {
        assert_eq!(base_identifier("en_GB@rg=gbzzzz"), "en_GB");
        assert_eq!(base_identifier(" de_DE "), "de_DE");
        assert_eq!(base_identifier(""), "en_US");
        assert_eq!(base_identifier("@calendar=japanese"), "en_US");
        assert!(!current().is_empty() && !current().contains('@'));
    }
}
