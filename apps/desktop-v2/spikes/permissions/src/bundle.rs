//! The running `.app`, from `NSBundle.mainBundle.bundleURL`. A loose executable has none.

use std::path::PathBuf;

use objc2_foundation::{NSString, NSURL};

pub fn running_app() -> Option<PathBuf> {
    let url = main_bundle_url()?;
    let ext = url
        .pathExtension()
        .map(|s| s.to_string().to_lowercase())
        .unwrap_or_default();
    if ext != "app" {
        return None;
    }
    url.path().map(|p| PathBuf::from(p.to_string()))
}

fn main_bundle_url() -> Option<objc2::rc::Retained<NSURL>> {
    use objc2::{class, msg_send, runtime::AnyObject};
    // SAFETY: NSBundle is process-global and the URL is copied out.
    unsafe {
        let bundle: *mut AnyObject = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return None;
        }
        let url: *mut AnyObject = msg_send![bundle, bundleURL];
        objc2::rc::Retained::retain(url.cast())
    }
}

pub fn bundle_id() -> String {
    use objc2::{class, msg_send, runtime::AnyObject};
    unsafe {
        let bundle: *mut AnyObject = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return String::new();
        }
        let id: *mut AnyObject = msg_send![bundle, bundleIdentifier];
        if id.is_null() {
            return String::new();
        }
        objc2::rc::Retained::<NSString>::retain(id.cast())
            .map(|s| s.to_string())
            .unwrap_or_default()
    }
}
