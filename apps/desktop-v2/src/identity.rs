use core_foundation::{
    base::TCFType, bundle::CFBundle, bundle::CFBundleGetIdentifier, string::CFString,
};

const PROD_ID: &str = "sh.saeed.kudzu";
const DEV_ID: &str = "sh.saeed.kudzu.next";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub bundle_id: &'static str,
    pub name: &'static str,
    pub mcp_name: &'static str,
    pub paused: bool,
    pub search_shortcut: &'static str,
    pub clipboard_shortcut: &'static str,
    pub cloud_and_updates: bool,
}

impl Identity {
    pub fn from_bundle_id(id: Option<&str>) -> Self {
        if id == Some(PROD_ID) {
            Self {
                bundle_id: PROD_ID,
                name: "Kudzu",
                mcp_name: "kudzu",
                paused: false,
                search_shortcut: "Control+Alt+Space",
                clipboard_shortcut: "Control+Alt+V",
                cloud_and_updates: true,
            }
        } else {
            Self {
                bundle_id: DEV_ID,
                name: "Kudzu Next",
                mcp_name: "kudzu-next",
                paused: true,
                search_shortcut: "Control+Alt+Shift+Space",
                clipboard_shortcut: "Control+Alt+Shift+V",
                cloud_and_updates: false,
            }
        }
    }

    pub fn resolve(force_dev: bool) -> Self {
        if force_dev {
            return Self::from_bundle_id(None);
        }
        let bundle = CFBundle::main_bundle();
        // CFBundle returns a borrowed identifier which remains valid while the bundle is alive.
        let id = unsafe { CFBundleGetIdentifier(bundle.as_concrete_TypeRef()) };
        let id = if id.is_null() {
            None
        } else {
            Some(unsafe { CFString::wrap_under_get_rule(id) }.to_string())
        };
        Self::from_bundle_id(id.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_table_and_dev_defaults() {
        let prod = Identity::from_bundle_id(Some(PROD_ID));
        assert_eq!(
            (
                prod.bundle_id,
                prod.name,
                prod.mcp_name,
                prod.paused,
                prod.cloud_and_updates
            ),
            (PROD_ID, "Kudzu", "kudzu", false, true)
        );
        assert_eq!(
            (prod.search_shortcut, prod.clipboard_shortcut),
            ("Control+Alt+Space", "Control+Alt+V")
        );
        for id in [None, Some("org.example.other"), Some(DEV_ID)] {
            let dev = Identity::from_bundle_id(id);
            assert_eq!(
                (
                    dev.bundle_id,
                    dev.name,
                    dev.mcp_name,
                    dev.paused,
                    dev.cloud_and_updates
                ),
                (DEV_ID, "Kudzu Next", "kudzu-next", true, false)
            );
            assert_eq!(
                (dev.search_shortcut, dev.clipboard_shortcut),
                ("Control+Alt+Shift+Space", "Control+Alt+Shift+V")
            );
        }
    }
}
