//! dnd: in-app drag and drop (S42 owns this module). S50 added only the payload its rows carry;
//! S42's version wins when the two merge.

/// What an in-app drag carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragPayload {
    Snippets(Vec<uuid::Uuid>),
    Group(uuid::Uuid),
}
