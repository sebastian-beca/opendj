//! The app's menus ([`crate::app_menu::MENUS`]) in the macOS menu bar:
//! Settings… (⌘,) in the app menu, then File, Edit… after it.
//! Slint builds the app menu itself (About, Services, Hide, Quit) with no
//! way to add to it, so the items are added to the native menu once it
//! exists.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSEventType, NSMenu, NSMenuItem};
use objc2_foundation::{NSObject, NSString};

use crate::app_menu::{shown, state, Action, Entry, MENUS};

thread_local! {
    /// What a picked item does, and the object the items call.
    static RUN: RefCell<Option<Box<dyn Fn(Action)>>> = RefCell::new(None);
    static TARGET: RefCell<Option<Retained<MenuTarget>>> = const { RefCell::new(None) };
    /// Every item, by what it does, to rename or grey out.
    static ITEMS: RefCell<Vec<(Action, Retained<NSMenuItem>)>> = const { RefCell::new(Vec::new()) };
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this class
    // doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OpenDJMenuTarget"]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(runMenuAction:))]
        fn run_menu_action(&self, sender: Option<&AnyObject>) {
            // The shortcut also reaches the window, which does the work
            // there (or lets a text field take ⌘Z for its typing), so only
            // a click on the item counts.
            if let Some(mtm) = MainThreadMarker::new() {
                let key = NSApplication::sharedApplication(mtm)
                    .currentEvent()
                    .is_some_and(|e| e.r#type() == NSEventType::KeyDown);
                if key {
                    return;
                }
            }
            let Some(item) = sender.and_then(|s| s.downcast_ref::<NSMenuItem>()) else {
                return;
            };
            let Some(action) = Action::from_id(item.tag() as i32) else {
                return;
            };
            RUN.with(|r| {
                if let Some(run) = r.borrow().as_ref() {
                    run(action);
                }
            });
        }
    }
);

fn new_item(
    mtm: MainThreadMarker,
    target: &MenuTarget,
    action: Action,
    title: &str,
    key: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: the selector is one MenuTarget implements, and the target is
    // kept alive in TARGET for as long as the menu exists.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(sel!(runMenuAction:)),
            &NSString::from_str(key),
        )
    };
    unsafe { item.setTarget(Some(target)) };
    item.setTag(action.id() as isize);
    ITEMS.with(|i| i.borrow_mut().push((action, item.clone())));
    item
}

fn fill(mtm: MainThreadMarker, target: &MenuTarget, menu: &NSMenu, entries: &'static [Entry]) {
    for e in shown(entries) {
        match e {
            Entry::Item { action, key, .. } => {
                // As things stand: Undo named for what it takes back, the
                // exports greyed out without a list to work on.
                let (title, enabled) = state(*action);
                let item = new_item(mtm, target, *action, &title, key.unwrap_or(""));
                item.setEnabled(enabled);
                menu.addItem(&item);
            }
            Entry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            Entry::Submenu { title, entries } => {
                let sub = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
                sub.setAutoenablesItems(false);
                fill(mtm, target, &sub, entries);
                let holder = NSMenuItem::new(mtm);
                holder.setTitle(&NSString::from_str(title));
                holder.setSubmenu(Some(&sub));
                menu.addItem(&holder);
            }
        }
    }
}

/// Adds Settings… under About in the app menu and the File, Edit… menus
/// after it, each item calling `run`. Returns false if the menu bar isn't
/// there yet, to try again later.
pub fn install(run: impl Fn(Action) + 'static) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(main) = app.mainMenu() else {
        return false;
    };
    let Some(app_menu) = main.itemAtIndex(0).and_then(|i| i.submenu()) else {
        return false;
    };
    RUN.with(|r| *r.borrow_mut() = Some(Box::new(run)));
    let target: Retained<MenuTarget> = {
        let this = MenuTarget::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    };
    // About, then a line, then Settings… and a line, as in other Mac apps.
    let settings = new_item(mtm, &target, Action::Settings, "Settings…", ",");
    let at = 2.min(app_menu.numberOfItems());
    app_menu.insertItem_atIndex(&settings, at);
    app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(mtm), at + 1);
    // File, Edit… after the app menu, in order.
    let mut at = 1.min(main.numberOfItems());
    for m in MENUS {
        if shown(m.entries).is_empty() {
            continue;
        }
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(m.title));
        menu.setAutoenablesItems(false);
        fill(mtm, &target, &menu, m.entries);
        let holder = NSMenuItem::new(mtm);
        holder.setSubmenu(Some(&menu));
        main.insertItem_atIndex(&holder, at);
        at += 1;
    }
    TARGET.with(|t| *t.borrow_mut() = Some(target));
    true
}

/// Renames an item and greys it out or back in.
pub fn set_item(action: Action, title: &str, enabled: bool) {
    ITEMS.with(|items| {
        for (a, item) in items.borrow().iter() {
            if *a == action {
                item.setTitle(&NSString::from_str(title));
                item.setEnabled(enabled);
            }
        }
    });
}
