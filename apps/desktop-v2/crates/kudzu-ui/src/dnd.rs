//! Shared payloads for sidebar and snippet-list drags.

use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragPayload {
    Snippets(Vec<Uuid>),
    Group(Uuid),
}
