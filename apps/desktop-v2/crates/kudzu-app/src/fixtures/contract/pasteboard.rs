//! What every `Pasteboard` promises the clipboard service. [`reads`] needs only another app's
//! copies; [`writes`] has the port itself write. The macOS adapter runs [`reads`] on a private
//! named pasteboard, and both suites on the general pasteboard only when asked (`#[ignore]`):
//! tests never write the user's clipboard.
use crate::ports::Pasteboard;
use kudzu_core::clipboard::{uti, PasteItem};

/// One type another app puts on the pasteboard. `None` declares the type without ever
/// providing its data (promised data that cannot be read).
pub type Offered<'a> = (&'a str, Option<&'a [u8]>);

pub trait PasteboardHarness {
    fn board(&self) -> &dyn Pasteboard;
    /// Another app replaces the pasteboard with these items.
    fn external_copy(&self, items: &[Vec<Offered<'_>>]);
}

const CONCEALED: &str = "org.nspasteboard.ConcealedType";

fn has(item: &PasteItem, kind: &str, data: &[u8]) -> bool {
    item.iter().any(|(t, d)| t == kind && d == data)
}

pub fn reads(h: &dyn PasteboardHarness) {
    let board = h.board();
    // Markers survive with empty and with promised-but-unreadable data, item by item.
    h.external_copy(&[
        vec![(uti::PLAIN_TEXT, Some(b"secret")), (CONCEALED, Some(b""))],
        vec![(uti::PNG, None)],
    ]);
    let count = board.change_count();
    assert!(!board.is_own(count), "another app's copy is not Kudzu's");
    assert_eq!(board.has_private_marker(count), Ok(true));
    let snapshot = board.read().unwrap();
    assert_eq!(snapshot.change_count, count);
    assert!(!snapshot.own);
    assert_eq!(snapshot.items.len(), 2, "every item is kept");
    assert!(has(&snapshot.items[0], uti::PLAIN_TEXT, b"secret"));
    assert!(has(&snapshot.items[0], CONCEALED, b""));
    assert!(has(&snapshot.items[1], uti::PNG, b""));

    // A count that has moved on is an error, never an answer about the newer contents.
    h.external_copy(&[vec![(uti::PLAIN_TEXT, Some(b"hello"))]]);
    let newer = board.change_count();
    assert_ne!(newer, count);
    assert!(board.has_private_marker(count).is_err());
    assert!(!board.is_own(count));
    assert_eq!(board.has_private_marker(newer), Ok(false));
    assert_eq!(board.read_text().unwrap().as_deref(), Some("hello"));

    // Text-only reads see no text in an image copy.
    h.external_copy(&[vec![(uti::PNG, Some(b"\x89PNG"))]]);
    assert_eq!(board.read_text().unwrap(), None);
    assert!(has(&board.read().unwrap().items[0], uti::PNG, b"\x89PNG"));
}

pub fn writes(h: &dyn PasteboardHarness) {
    let board = h.board();
    let items: Vec<PasteItem> = vec![
        vec![
            (uti::PLAIN_TEXT.into(), b"kudzu".to_vec()),
            (uti::HTML.into(), b"<b>kudzu</b>".to_vec()),
        ],
        vec![(uti::PLAIN_TEXT.into(), b"second".to_vec())],
    ];
    let count = board.write(&items).unwrap();
    assert_eq!(board.change_count(), count);
    assert!(board.is_own(count), "Kudzu's own write is recognised");
    let snapshot = board.read().unwrap();
    assert!(snapshot.own);
    assert_eq!(snapshot.change_count, count);
    for (written, read) in items.iter().zip(&snapshot.items) {
        for (kind, data) in written {
            assert!(has(read, kind, data));
        }
    }
    assert_eq!(board.read_text().unwrap().as_deref(), Some("kudzu"));
    h.external_copy(&[vec![(uti::PLAIN_TEXT, Some(b"theirs"))]]);
    let theirs = board.change_count();
    assert!(!board.is_own(theirs));
    assert!(
        !board.is_own(count),
        "a stale own count is not the current contents"
    );
}
