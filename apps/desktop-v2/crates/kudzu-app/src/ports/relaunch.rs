//! Explicit restart only; services decide when restarting is allowed.
use crate::error::PortError;
pub trait Relauncher: Send + Sync {
    fn relaunch(&self) -> Result<(), PortError>;
}
