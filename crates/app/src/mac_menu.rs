//! Settings… (⌘,) in the macOS app menu. Slint builds the app
//! menu itself (About, Services, Hide, Quit) with no way to add to it, so
//! the item is added to the native menu once it exists.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSMenuItem};
use objc2_foundation::{NSObject, NSString};

thread_local! {
    /// What Settings… does, and the object the menu item calls.
    static OPEN: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
    static TARGET: RefCell<Option<Retained<SettingsTarget>>> = const { RefCell::new(None) };
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
    TARGET.with(|t| *t.borrow_mut() = Some(target));
    true
}
