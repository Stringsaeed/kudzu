//! Injected UUID generation; fixtures never use process randomness.
use uuid::Uuid;
pub trait IdSource: Send + Sync {
    fn next_id(&self) -> Uuid;
}
pub struct RandomIds;
impl IdSource for RandomIds {
    fn next_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
