//! What `SystemSettings` promises without opening a pane or prompting: reading the grants
//! changes nothing and is repeatable.
use crate::ports::SystemSettings;

pub fn reads(settings: &dyn SystemSettings) {
    let first = settings.permissions();
    assert_eq!(settings.permissions(), first);
}
