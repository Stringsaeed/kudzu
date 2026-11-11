//! RUNE-GAP(#141, #561): delete the closure target when Rune owns native menu callbacks.
use objc2::{define_class, msg_send, rc::Retained, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::{MainThreadMarker, NSObject, NSString};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub(crate) type Callback = Rc<dyn Fn()>;

fn dispatch(actions: &RefCell<BTreeMap<isize, Callback>>, tag: isize, enabled: bool) {
    let callback = actions.borrow().get(&tag).cloned();
    if enabled {
        if let Some(callback) = callback {
            callback();
        }
    }
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "KudzuGapMenuTarget"]
    #[ivars = RefCell<BTreeMap<isize, Callback>>]
    pub(crate) struct Target;
    impl Target {
        #[unsafe(method(kudzuGapAction:))]
        fn action(&self, sender: &NSMenuItem) {
            // A callback may update or destroy its owner. Release the borrow first and
            // keep the closure alive independently of the target and menu.
            dispatch(self.ivars(), sender.tag(), sender.isEnabled());
        }
    }
);

impl Target {
    pub(crate) fn new(mtm: MainThreadMarker, actions: BTreeMap<isize, Callback>) -> Retained<Self> {
        // SAFETY: NSObject initialization of our main-thread class with initialized ivars.
        unsafe {
            msg_send![
                super(Self::alloc(mtm).set_ivars(RefCell::new(actions))),
                init
            ]
        }
    }

    pub(crate) fn item(
        &self,
        mtm: MainThreadMarker,
        title: &str,
        tag: isize,
    ) -> Retained<NSMenuItem> {
        // SAFETY: selector is implemented above; owner retains the target and clears it on drop.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str(title),
                Some(sel!(kudzuGapAction:)),
                &NSString::new(),
            )
        };
        item.setTag(tag);
        unsafe {
            item.setTarget(Some(self));
        }
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn callback_can_remove_itself_and_disabled_items_do_not_call_it() {
        let actions = Rc::new(RefCell::new(BTreeMap::<isize, Callback>::new()));
        let called = Rc::new(Cell::new(0));
        let weak = Rc::downgrade(&actions);
        let count = called.clone();
        actions.borrow_mut().insert(
            7,
            Rc::new(move || {
                weak.upgrade().unwrap().borrow_mut().remove(&7);
                count.set(count.get() + 1);
            }),
        );
        dispatch(&actions, 7, false);
        assert_eq!(called.get(), 0);
        dispatch(&actions, 7, true);
        assert_eq!(called.get(), 1);
        assert!(actions.borrow().is_empty());
    }
}
