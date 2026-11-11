//! The read contract runs on a private, uniquely named NSPasteboard, so a real pasteboard
//! (with real promised data) is exercised without touching the user's clipboard. The full
//! contract on the general pasteboard writes the clipboard: it is `#[ignore]`d, for S71 to run
//! by hand with `cargo test -p kudzu-system -- --ignored general_pasteboard`.
use super::*;
use kudzu_app::fixtures::contract::pasteboard::{self as contract, Offered, PasteboardHarness};
use objc2::{define_class, msg_send, rc::Retained, runtime::ProtocolObject, AnyThread};
use objc2_app_kit::{
    NSPasteboardItem, NSPasteboardItemDataProvider, NSPasteboardType, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSObject, NSObjectProtocol};
use std::cell::RefCell;

define_class!(
    /// Promises data and never provides it, as a source app that quit or failed would.
    #[unsafe(super(NSObject))]
    #[name = "KudzuS35SilentProvider"]
    struct SilentProvider;

    unsafe impl NSObjectProtocol for SilentProvider {}

    unsafe impl NSPasteboardItemDataProvider for SilentProvider {
        #[unsafe(method(pasteboard:item:provideDataForType:))]
        fn provide(
            &self,
            _pasteboard: Option<&NSPasteboard>,
            _item: &NSPasteboardItem,
            _kind: &NSPasteboardType,
        ) {
        }
    }
);

impl SilentProvider {
    fn new() -> Retained<Self> {
        // SAFETY: NSObject's designated initialiser; the class adds no ivars.
        unsafe { msg_send![Self::alloc(), init] }
    }
}

/// Writes items as another app would, onto `board`.
struct Raw {
    board: Retained<NSPasteboard>,
    /// Kept alive while the pasteboard may ask them for data.
    providers: RefCell<Vec<Retained<SilentProvider>>>,
}

impl Raw {
    fn copy(&self, items: &[Vec<Offered<'_>>]) {
        autoreleasepool(|_| {
            let objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = items
                .iter()
                .map(|entries| {
                    let item = NSPasteboardItem::new();
                    for (kind, data) in entries {
                        let kind = NSString::from_str(kind);
                        match data {
                            Some(bytes) => {
                                assert!(item.setData_forType(&NSData::with_bytes(bytes), &kind));
                            }
                            None => {
                                let provider = SilentProvider::new();
                                let types = NSArray::from_retained_slice(&[kind]);
                                assert!(item.setDataProvider_forTypes(
                                    ProtocolObject::from_ref(&*provider),
                                    &types
                                ));
                                self.providers.borrow_mut().push(provider);
                            }
                        }
                    }
                    ProtocolObject::from_retained(item)
                })
                .collect();
            self.board.clearContents();
            assert!(self
                .board
                .writeObjects(&NSArray::from_retained_slice(&objects)));
        });
    }
}

struct Named {
    adapter: SystemPasteboard,
    raw: Raw,
}

impl Named {
    fn new() -> Self {
        let board = NSPasteboard::pasteboardWithUniqueName();
        let name = board.name().to_string();
        Self {
            adapter: SystemPasteboard::named(name),
            raw: Raw {
                board,
                providers: RefCell::default(),
            },
        }
    }
}

impl Drop for Named {
    fn drop(&mut self) {
        // SAFETY: a void NSPasteboard method (not in the bindings) on a board only we use.
        let () = unsafe { msg_send![&*self.raw.board, releaseGlobally] };
    }
}

impl PasteboardHarness for Named {
    fn board(&self) -> &dyn Pasteboard {
        &self.adapter
    }
    fn external_copy(&self, items: &[Vec<Offered<'_>>]) {
        self.raw.copy(items);
    }
}

#[test]
fn a_named_pasteboard_passes_the_read_contract() {
    contract::reads(&Named::new());
}

#[test]
fn named_pasteboards_are_read_only_through_the_adapter() {
    let named = Named::new();
    named.external_copy(&[vec![(kudzu_core::clipboard::uti::PLAIN_TEXT, Some(b"x"))]]);
    let before = named.adapter.change_count();
    assert_eq!(named.adapter.write(&[]), Err(PortError::Unavailable));
    assert_eq!(named.adapter.change_count(), before);
    assert!(!named.adapter.is_own(before));
}

/// Reads the user's clipboard state only through counts and type names; writes nothing.
#[test]
fn the_general_pasteboard_refuses_a_count_that_is_not_current() {
    let board = SystemPasteboard::general();
    let count = board.change_count();
    assert!(board.has_private_marker(count + 1).is_err());
    assert!(!board.is_own(count + 1));
}

struct General(SystemPasteboard, Raw);
impl PasteboardHarness for General {
    fn board(&self) -> &dyn Pasteboard {
        &self.0
    }
    fn external_copy(&self, items: &[Vec<Offered<'_>>]) {
        self.1.copy(items);
    }
}

/// S71: replaces the user's clipboard. Run by hand on a test Mac, then copy something again.
#[test]
#[ignore = "writes the general pasteboard; S71 runs it by hand"]
fn general_pasteboard_passes_the_full_contract() {
    let general = General(
        SystemPasteboard::general(),
        Raw {
            board: NSPasteboard::generalPasteboard(),
            providers: RefCell::default(),
        },
    );
    contract::reads(&general);
    contract::writes(&general);
}
