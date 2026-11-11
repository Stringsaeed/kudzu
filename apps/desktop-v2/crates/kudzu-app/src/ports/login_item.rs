//! Login-item adapter; registration is performed off the UI thread.
use crate::error::PortError;
pub trait LoginItem: Send + Sync {
    fn enabled(&self) -> Result<bool, PortError>;
    fn set_enabled(&self, enabled: bool) -> Result<(), PortError>;
}
