//! Settings… (⌘,) in the macOS app menu, a File menu with Export ›
//! Mixcloud…, and an Edit menu with Undo (⌘Z) and Find (⌘F).
//! Slint builds the app menu itself (About, Services, Hide, Quit) with no
//! way to add to it, so the items are added to the native menu once it
//! exists.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSEventType, NSMenu, NSMenuItem};
use objc2_foundation::{NSObject, NSString};

thread_local! {
    /// What Settings… does, and the object the menu item calls.
    static OPEN: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
    static TARGET: RefCell<Option<Retained<SettingsTarget>>> = const { RefCell::new(None) };
    /// What Undo does, and its menu item, renamed for what it takes back.
    static UNDO: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
    static UNDO_ITEM: RefCell<Option<Retained<NSMenuItem>>> = const { RefCell::new(None) };
    /// What Find does: into the library search.
    static FIND: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
    /// What File › Export › Mixcloud… does.
    static EXPORT_MIXCLOUD: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this class
    // doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OpenDJSettingsTarget"]
    struct SettingsTarget;

    impl SettingsTarget {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            OPEN.with(|o| {
                if let Some(open) = o.borrow().as_ref() {
                    open();
                }
            });
        }

        #[unsafe(method(undoLastChange:))]
        fn undo_last_change(&self, _sender: Option<&AnyObject>) {
            // ⌘Z also reaches the window, which undoes there (or lets a text
            // field undo its typing), so only a click on the item counts.
            if let Some(mtm) = MainThreadMarker::new() {
                let key = NSApplication::sharedApplication(mtm)
                    .currentEvent()
                    .is_some_and(|e| e.r#type() == NSEventType::KeyDown);
                if key {
                    return;
                }
            }
            UNDO.with(|u| {
                if let Some(undo) = u.borrow().as_ref() {
                    undo();
                }
            });
        }

        #[unsafe(method(exportMixcloud:))]
        fn export_mixcloud(&self, _sender: Option<&AnyObject>) {
            EXPORT_MIXCLOUD.with(|f| {
                if let Some(export) = f.borrow().as_ref() {
                    export();
                }
            });
        }

        #[unsafe(method(findInLibrary:))]
        fn find_in_library(&self, _sender: Option<&AnyObject>) {
            // Going to the search twice (the item and the window both see
            // ⌘F) does no harm.
            FIND.with(|f| {
                if let Some(find) = f.borrow().as_ref() {
                    find();
                }
            });
        }
    }
);

/// Adds Settings… under About in the app menu, calling `open` when picked.
/// Returns false if the menu isn't there yet, to try again later.
pub fn add_settings_item(open: impl Fn() + 'static) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(app_menu) = app
        .mainMenu()
        .and_then(|m| m.itemAtIndex(0))
        .and_then(|i| i.submenu())
    else {
        return false;
    };
    OPEN.with(|o| *o.borrow_mut() = Some(Box::new(open)));
    let target: Retained<SettingsTarget> = {
        let this = SettingsTarget::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    };
    // SAFETY: the selector is one SettingsTarget implements, and the
    // target is kept alive in TARGET for as long as the menu exists.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Settings…"),
            Some(sel!(openSettings:)),
            &NSString::from_str(","),
        )
    };
    unsafe { item.setTarget(Some(&target)) };
    // About, then a line, then Settings… and a line, as in other Mac apps.
    let at = 2.min(app_menu.numberOfItems());
    app_menu.insertItem_atIndex(&item, at);
    app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(mtm), at + 1);
    // An Edit menu after the app menu, with Undo.
    let edit = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Edit"));
    edit.setAutoenablesItems(false);
    // SAFETY: as for Settings… above.
    let undo = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Undo"),
            Some(sel!(undoLastChange:)),
            &NSString::from_str("z"),
        )
    };
    unsafe { undo.setTarget(Some(&target)) };
    undo.setEnabled(false);
    edit.addItem(&undo);
    // SAFETY: as for Settings… above.
    let find = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Find"),
            Some(sel!(findInLibrary:)),
            &NSString::from_str("f"),
        )
    };
    unsafe { find.setTarget(Some(&target)) };
    edit.addItem(&NSMenuItem::separatorItem(mtm));
    edit.addItem(&find);
    let holder = NSMenuItem::new(mtm);
    holder.setSubmenu(Some(&edit));
    // A File menu before Edit: Export › Mixcloud… for the mixed set picked.
    let file = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("File"));
    let export = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Export"));
    // SAFETY: as for Settings… above.
    let mixcloud = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Mixcloud…"),
            Some(sel!(exportMixcloud:)),
            &NSString::from_str(""),
        )
    };
    unsafe { mixcloud.setTarget(Some(&target)) };
    export.addItem(&mixcloud);
    let export_holder = NSMenuItem::new(mtm);
    export_holder.setTitle(&NSString::from_str("Export"));
    export_holder.setSubmenu(Some(&export));
    file.addItem(&export_holder);
    let file_holder = NSMenuItem::new(mtm);
    file_holder.setSubmenu(Some(&file));
    if let Some(main) = app.mainMenu() {
        main.insertItem_atIndex(&holder, 1.min(main.numberOfItems()));
        main.insertItem_atIndex(&file_holder, 1.min(main.numberOfItems()));
    }
    UNDO_ITEM.with(|u| *u.borrow_mut() = Some(undo));
    TARGET.with(|t| *t.borrow_mut() = Some(target));
    true
}

/// What Edit › Undo does when picked.
pub fn on_undo(undo: impl Fn() + 'static) {
    UNDO.with(|u| *u.borrow_mut() = Some(Box::new(undo)));
}

/// What File › Export › Mixcloud… does when picked.
pub fn on_export_mixcloud(export: impl Fn() + 'static) {
    EXPORT_MIXCLOUD.with(|f| *f.borrow_mut() = Some(Box::new(export)));
}

/// What Edit › Find does when picked.
pub fn on_find(find: impl Fn() + 'static) {
    FIND.with(|f| *f.borrow_mut() = Some(Box::new(find)));
}

/// Names Edit › Undo for what it would take back ("Undo Send to In Prep
/// (3 tracks)"), or greys it out when `text` is empty.
pub fn set_undo_text(text: &str) {
    UNDO_ITEM.with(|u| {
        if let Some(item) = u.borrow().as_ref() {
            let title = if text.is_empty() { "Undo" } else { text };
            item.setTitle(&NSString::from_str(title));
            item.setEnabled(!text.is_empty());
        }
    });
}
