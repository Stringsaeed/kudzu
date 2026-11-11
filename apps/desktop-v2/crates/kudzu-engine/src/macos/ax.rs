//! Permissions, Secure Input and Accessibility (caret, selection) queries.

use std::ffi::c_void;

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFTypeRef;

use super::ffi::*;
use crate::{PermissionStatus, Rect};

/// AX calls must never hang the caller on an unresponsive app.
const AX_TIMEOUT_SECONDS: f32 = 0.2;

/// Accessibility and Input Monitoring grants.
pub(crate) fn permissions() -> PermissionStatus {
    unsafe {
        PermissionStatus {
            accessibility: AXIsProcessTrusted() != 0,
            input_monitoring: CGPreflightListenEventAccess(),
        }
    }
}

/// Whether we may post synthetic events.
pub(crate) fn post_event_allowed() -> bool {
    unsafe { CGPreflightPostEventAccess() }
}

/// Shows the system Accessibility prompt (no-op when already trusted).
pub(crate) fn request_accessibility() {
    let options = CFDictionary::from_CFType_pairs(&[(
        CFString::from_static_string("AXTrustedCheckOptionPrompt").as_CFType(),
        CFBoolean::true_value().as_CFType(),
    )]);
    unsafe {
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
    }
}

/// Shows the system Input Monitoring prompt.
pub(crate) fn request_input_monitoring() {
    unsafe {
        CGRequestListenEventAccess();
    }
}

/// Whether Secure Input is on, and the owning process id when the session reports it.
pub(crate) fn secure_input() -> (bool, Option<i32>) {
    if unsafe { IsSecureEventInputEnabled() } == 0 {
        return (false, None);
    }
    // This undocumented session entry is advisory only; never gate detection on it.
    let dict = unsafe { CGSessionCopyCurrentDictionary() };
    if dict.is_null() {
        return (true, None);
    }
    let dict: CFDictionary<CFString, CFType> =
        unsafe { CFDictionary::wrap_under_create_rule(dict) };
    let pid = dict
        .find(CFString::from_static_string("kCGSSessionSecureInputPID"))
        .and_then(|value| value.downcast::<CFNumber>())
        .and_then(|number| number.to_i32())
        .filter(|pid| *pid > 0);
    (true, pid)
}

fn attribute(element: &CFType, name: &'static str) -> Option<CFType> {
    let mut value: CFTypeRef = std::ptr::null();
    let key = CFString::from_static_string(name);
    let error = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef() as AXUIElementRef,
            key.as_concrete_TypeRef(),
            &mut value,
        )
    };
    (error == kAXErrorSuccess && !value.is_null())
        .then(|| unsafe { CFType::wrap_under_create_rule(value) })
}

fn focused_element() -> Option<CFType> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return None;
    }
    let system = unsafe { CFType::wrap_under_create_rule(system as CFTypeRef) };
    unsafe {
        AXUIElementSetMessagingTimeout(system.as_CFTypeRef() as AXUIElementRef, AX_TIMEOUT_SECONDS)
    };
    let focused = attribute(&system, "AXFocusedUIElement")?;
    if focused.type_of() != unsafe { AXUIElementGetTypeID() } {
        return None;
    }
    unsafe {
        AXUIElementSetMessagingTimeout(focused.as_CFTypeRef() as AXUIElementRef, AX_TIMEOUT_SECONDS)
    };
    Some(focused)
}

fn ax_value<T: Default>(value: &CFType, kind: u32) -> Option<T> {
    if value.type_of() != unsafe { AXValueGetTypeID() } {
        return None;
    }
    let mut out = T::default();
    let ok = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef() as AXValueRef,
            kind,
            &mut out as *mut T as *mut c_void,
        )
    };
    (ok != 0).then_some(out)
}

fn valid(rect: &CGRect) -> bool {
    let values = [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ];
    values.iter().all(|v| v.is_finite()) && rect.size.width >= 0.0 && rect.size.height > 0.0
}

fn to_rect(rect: CGRect) -> Rect {
    Rect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    }
}

/// Caret bounds of the focused text element (global, top-left origin), else the element's frame.
pub(crate) fn caret_rect() -> Option<Rect> {
    let element = focused_element()?;
    if let Some(range) = attribute(&element, "AXSelectedTextRange") {
        let mut bounds: CFTypeRef = std::ptr::null();
        let key = CFString::from_static_string("AXBoundsForRange");
        let error = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                element.as_CFTypeRef() as AXUIElementRef,
                key.as_concrete_TypeRef(),
                range.as_CFTypeRef(),
                &mut bounds,
            )
        };
        if error == kAXErrorSuccess && !bounds.is_null() {
            let bounds = unsafe { CFType::wrap_under_create_rule(bounds) };
            if let Some(rect) = ax_value::<CGRect>(&bounds, kAXValueCGRectType).filter(valid) {
                return Some(to_rect(rect));
            }
        }
    }
    let origin = ax_value::<CGPoint>(&attribute(&element, "AXPosition")?, kAXValueCGPointType)?;
    let size = ax_value::<CGSize>(&attribute(&element, "AXSize")?, kAXValueCGSizeType)?;
    Some(CGRect { origin, size }).filter(valid).map(to_rect)
}

/// Selected text of the focused element, when the app exposes it.
pub(crate) fn selected_text() -> Option<String> {
    reported_selection().filter(|s| !s.is_empty())
}

/// What the focused element says is selected: `Some("")` when it reports an empty selection,
/// `None` when it doesn't report one at all (so only a copy can tell).
pub(crate) fn reported_selection() -> Option<String> {
    let element = focused_element()?;
    let value = attribute(&element, "AXSelectedText")?;
    value.downcast::<CFString>().map(|s| s.to_string())
}

pub(crate) struct CaretSnapshot {
    element: CFType,
    location: usize,
}

#[repr(C)]
#[derive(Default)]
struct TextRange {
    location: isize,
    length: isize,
}

fn collapsed_offset(element: &CFType) -> Option<usize> {
    let range = ax_value::<TextRange>(&attribute(element, "AXSelectedTextRange")?, 4)?;
    (range.length == 0)
        .then(|| usize::try_from(range.location).ok())
        .flatten()
}

/// Only offsets are read; the user's surrounding text is never captured.
pub(crate) fn caret_snapshot() -> Option<CaretSnapshot> {
    let element = focused_element()?;
    let location = collapsed_offset(&element)?;
    Some(CaretSnapshot { element, location })
}

impl CaretSnapshot {
    pub(crate) fn inserted_utf16(&self) -> Option<usize> {
        let focused = focused_element()?;
        if focused != self.element {
            return None;
        }
        collapsed_offset(&focused)?.checked_sub(self.location)
    }
}
