//! RUNE-GAP(#141): replace the status item when Rune supports macOS tray items.
//! RUNE-GAP(#561): replace the app-menu patch when Rune supports its Quit item.
use crate::spike::{Panel, State};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem};
use objc2_foundation::{MainThreadMarker, NSData, NSObject, NSString};
use rune::AppHandle;
use std::cell::RefCell;

struct Actions {
    main: AppHandle<State>,
    panel: AppHandle<Panel>,
}
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "KudzuS10MenuTarget"]
    #[ivars = Actions]
    struct Target;
    impl Target {
        #[unsafe(method(s10Action:))]
        fn action(&self, sender: &NSMenuItem) {
            let actions = self.ivars();
            match sender.tag() {
                1 => { actions.main.set_dock_icon(true); actions.main.open(); schedule_patch(actions.main.clone()); }
                2 => { actions.main.hide(); actions.panel.hide(); actions.main.set_dock_icon(false); eprintln!("S10 close windows"); }
                3 => actions.panel.open(),
                4 => swap_image(),
                5 => { actions.main.set_dock_icon(true); schedule_patch(actions.main.clone()); }
                6 => actions.main.set_dock_icon(false),
                7 => actions.main.quit(),
                _ => {}
            }
        }
    }
);

struct Owned {
    status: Retained<NSStatusItem>,
    target: Retained<Target>,
    alternate: bool,
}
thread_local! { static OWNED: RefCell<Option<Owned>> = const { RefCell::new(None) }; }

fn menu_item(
    mtm: MainThreadMarker,
    title: &str,
    key: &str,
    tag: isize,
    target: &Target,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(sel!(s10Action:)),
            &NSString::from_str(key),
        )
    };
    item.setTag(tag);
    unsafe {
        item.setTarget(Some(target));
    }
    item
}

fn image(mtm: MainThreadMarker, alternate: bool) -> Retained<NSImage> {
    let bytes: &[u8] = if alternate {
        include_bytes!("../../../assets/tray/paused.png")
    } else {
        include_bytes!("../../../assets/tray/normal.png")
    };
    let data = NSData::with_bytes(bytes);
    let image = NSImage::initWithData(mtm.alloc(), &data).expect("tray image");
    image.setTemplate(true);
    image
}

pub fn install(main: AppHandle<State>, panel: AppHandle<Panel>) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let target: Retained<Target> = unsafe {
        msg_send![
            super(Target::alloc(mtm).set_ivars(Actions { main, panel })),
            init
        ]
    };
    let status = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
    let button = status
        .button(MainThreadMarker::new().unwrap())
        .expect("status button");
    button.setImage(Some(&image(mtm, false)));
    button.setTitle(&NSString::from_str("Dev"));
    button.setToolTip(Some(&NSString::from_str("Kudzu S10 spike")));
    let menu = NSMenu::new(mtm);
    for (title, tag) in [
        ("Open Kudzu…", 1),
        ("Close Windows", 2),
        ("Show launcher", 3),
        ("Swap glyph", 4),
        ("Dock on", 5),
        ("Dock off", 6),
        ("Quit Kudzu", 7),
    ] {
        menu.addItem(&menu_item(mtm, title, "", tag, &target));
    }
    status.setMenu(Some(&menu));
    OWNED.with(|slot| {
        *slot.borrow_mut() = Some(Owned {
            status,
            target,
            alternate: false,
        })
    });
    eprintln!("S10 status item installed windowless, title=Dev template=true");
}

fn swap_image() {
    OWNED.with(|slot| {
        if let Some(owned) = slot.borrow_mut().as_mut() {
            owned.alternate = !owned.alternate;
            if let Some(button) = owned.status.button(MainThreadMarker::new().unwrap()) {
                button.setImage(Some(&image(
                    MainThreadMarker::new().unwrap(),
                    owned.alternate,
                )));
            }
            eprintln!("S10 swapped template image alternate={}", owned.alternate);
        }
    });
}

fn schedule_patch(main: AppHandle<State>) {
    // Dock promotion is queued; a later update sees the resulting menu.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        main.update(|_, _| patch_app_menu());
    });
}

fn patch_app_menu() {
    let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
    let Some(root) = app.mainMenu() else {
        eprintln!("S10 patch failed: no root menu");
        return;
    };
    let Some(menu) = root.itemAtIndex(0).and_then(|item| item.submenu()) else {
        eprintln!("S10 patch failed: no app menu");
        return;
    };
    let Some(index) = (0..menu.numberOfItems())
        .find(|&i| menu.itemAtIndex(i).and_then(|item| item.action()) == Some(sel!(terminate:)))
    else {
        eprintln!("S10 patch failed: no terminate item");
        return;
    };
    let quit = menu.itemAtIndex(index).unwrap();
    if let Some(show_all) = (0..menu.numberOfItems()).find_map(|i| {
        menu.itemAtIndex(i)
            .filter(|item| item.action() == Some(sel!(unhideAllApplications:)))
    }) {
        menu.removeItem(&show_all);
    }
    if let Some(services) = (0..menu.numberOfItems()).find(|&i| {
        menu.itemAtIndex(i)
            .is_some_and(|item| item.submenu().is_some())
    }) {
        if !menu
            .itemAtIndex(services + 1)
            .is_some_and(|item| item.isSeparatorItem())
        {
            menu.insertItem_atIndex(
                &NSMenuItem::separatorItem(MainThreadMarker::new().unwrap()),
                services + 1,
            );
        }
    }
    quit.setKeyEquivalent(&NSString::from_str(""));
    quit.setTitle(&NSString::from_str("Quit Kudzu"));
    OWNED.with(|slot| {
        let slot = slot.borrow();
        let Some(owned) = slot.as_ref() else { return };
        if !(0..menu.numberOfItems()).any(|i| {
            menu.itemAtIndex(i)
                .is_some_and(|item| item.action() == Some(sel!(s10Action:)) && item.tag() == 2)
        }) {
            menu.insertItem_atIndex(
                &menu_item(
                    MainThreadMarker::new().unwrap(),
                    "Close Windows",
                    "q",
                    2,
                    &owned.target,
                ),
                menu.indexOfItem(&quit),
            );
        }
    });
    let titles: Vec<_> = (0..menu.numberOfItems())
        .filter_map(|i| menu.itemAtIndex(i))
        .map(|i| format!("{} [{}]", i.title(), i.keyEquivalent()))
        .collect();
    eprintln!("S10 app menu: {}", titles.join(" | "));
}

pub fn auto_inspect() {
    swap_image();
    OWNED.with(|slot| {
        let slot = slot.borrow();
        let owned = slot.as_ref().expect("installed");
        let button = owned
            .status
            .button(MainThreadMarker::new().unwrap())
            .unwrap();
        let image = button.image().unwrap();
        eprintln!(
            "S10 observed title={} template={} menu_items={}",
            button.title(),
            image.isTemplate(),
            owned
                .status
                .menu(MainThreadMarker::new().unwrap())
                .unwrap()
                .numberOfItems()
        );
    });
}

pub fn auto_patch() {
    patch_app_menu();
    patch_app_menu();
}

pub fn auto_close() {
    OWNED.with(|slot| {
        let slot = slot.borrow();
        let owned = slot.as_ref().unwrap();
        let item = owned
            .status
            .menu(MainThreadMarker::new().unwrap())
            .unwrap()
            .itemAtIndex(1)
            .unwrap();
        let _: () = unsafe { msg_send![&*owned.target, s10Action: &*item] };
    });
}
