//! Read this process's signed entitlement. Never inspect a keychain or provisioning profile.
use kudzu_app::error::PortError;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSArray, NSString};
use std::ffi::c_void;

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecTaskCreateFromSelf(allocator: *const c_void) -> *mut c_void;
    fn SecTaskCopyValueForEntitlement(
        task: *mut c_void,
        entitlement: *const c_void,
        error: *mut *mut c_void,
    ) -> *mut AnyObject;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: *const c_void);
}

pub(super) fn entitled(container: &str) -> bool {
    // SAFETY: Create/Copy follow the CF ownership rules. The returned entitlement is a
    // property-list value; runtime downcasts check its shape before reading it.
    unsafe {
        let task = SecTaskCreateFromSelf(std::ptr::null());
        if task.is_null() {
            return false;
        }
        let key = NSString::from_str("com.apple.developer.icloud-container-identifiers");
        let value = SecTaskCopyValueForEntitlement(
            task,
            Retained::as_ptr(&key).cast(),
            std::ptr::null_mut(),
        );
        CFRelease(task);
        let Some(value) = Retained::from_raw(value) else {
            return false;
        };
        let Ok(values) = value.downcast::<NSArray>() else {
            return false;
        };
        values.iter().any(|value| {
            value
                .downcast_ref::<NSString>()
                .is_some_and(|v| v.to_string() == container)
        })
    }
}

pub(super) fn require<T>(
    entitled: bool,
    create: impl FnOnce() -> Result<T, PortError>,
) -> Result<T, PortError> {
    if !entitled {
        return Err(PortError::Unavailable);
    }
    create()
}
