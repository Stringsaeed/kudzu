//! Infrastructure errors never retain input, paths, URLs, or third-party error messages.
//! Domain services define their own `thiserror` enums and wrap these safe causes.

/// A safe I/O cause. `io::Error` itself may contain arbitrary user content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Storage operation failed ({kind:?}).")]
pub struct IoFailure {
    pub kind: std::io::ErrorKind,
}
impl From<std::io::Error> for IoFailure {
    fn from(error: std::io::Error) -> Self {
        Self { kind: error.kind() }
    }
}

/// Port adapters classify external errors instead of forwarding their raw descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    #[error("Permission is required.")]
    PermissionDenied,
    #[error("The application is no longer available.")]
    TargetGone,
    #[error("The operation timed out.")]
    TimedOut,
    #[error("The response is too large.")]
    TooLarge,
    #[error("The response could not be read.")]
    InvalidData,
    #[error("This feature is unavailable.")]
    Unavailable,
    #[error("The operation failed.")]
    Io(#[from] IoFailure),
}
