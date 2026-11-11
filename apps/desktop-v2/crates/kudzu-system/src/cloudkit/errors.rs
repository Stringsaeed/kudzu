//! Error classification deliberately accepts no error descriptions or userInfo text.
use kudzu_app::{
    error::PortError,
    ports::{CloudSend, CloudSendFailure},
};
use objc2_cloud_kit::{CKErrorCode, CKErrorDomain};
use objc2_foundation::NSError;

pub(super) fn code(error: &NSError) -> CKErrorCode {
    // Codes from another error domain must not accidentally look like a CK conflict.
    if &*error.domain() == unsafe { CKErrorDomain } {
        CKErrorCode(error.code())
    } else {
        CKErrorCode(-1)
    }
}

pub(super) fn transient(code: CKErrorCode) -> bool {
    matches!(
        code,
        CKErrorCode::NetworkUnavailable
            | CKErrorCode::NetworkFailure
            | CKErrorCode::ServiceUnavailable
            | CKErrorCode::RequestRateLimited
            | CKErrorCode::ZoneBusy
    )
}

/// Returns whether the failed intent must leave the adapter's pending set.
pub(super) fn send_failure(report: &mut CloudSend, code: CKErrorCode) -> bool {
    if transient(code) {
        return false;
    }
    if code == CKErrorCode::QuotaExceeded {
        report.failure = Some(CloudSendFailure::QuotaExceeded);
    } else if report.failure.is_none() {
        report.failure = Some(CloudSendFailure::Permanent);
    }
    true
}

pub(super) fn port(code: CKErrorCode) -> PortError {
    match code {
        CKErrorCode::MissingEntitlement => PortError::Unavailable,
        CKErrorCode::PermissionFailure | CKErrorCode::NotAuthenticated => {
            PortError::PermissionDenied
        }
        CKErrorCode::OperationCancelled => PortError::Cancelled,
        _ => PortError::InvalidData,
    }
}
