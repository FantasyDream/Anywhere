pub(super) mod login_item;
pub(super) mod preview;

use super::{PlatformEvent, StatusItemAction, StatusItemHost, StatusMenuLabels};
use block2::RcBlock;
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy,
    NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification, NSEvent,
    NSEventMask, NSEventModifierFlags, NSFloatingWindowLevel, NSImage, NSMenu, NSMenuItem,
    NSNormalWindowLevel, NSPanel, NSRunningApplication, NSScreen, NSStatusBar,
    NSVariableStatusItemLength, NSView, NSWindow, NSWindowButton, NSWindowCollectionBehavior,
    NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification, NSWindowStyleMask,
    NSWindowTitleVisibility, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceOpenConfiguration,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSRect, NSSize, NSString,
    NSURL,
};
use objc2_quartz_core::CATransaction;
use std::{
    cell::{Cell, RefCell},
    ptr::{self, NonNull},
    sync::atomic::{AtomicU64, Ordering},
};

static WINDOW_ACTIVATION_GENERATION: AtomicU64 = AtomicU64::new(0);

type NotificationObserver = Retained<objc2::runtime::ProtocolObject<dyn NSObjectProtocol>>;

pub(super) fn current_key_event_is_repeat() -> bool {
    KEY_EVENT_IS_REPEAT.with(Cell::get)
}

fn record_key_event_repeat(repeated: bool) {
    KEY_EVENT_IS_REPEAT.with(|value| value.set(repeated));
}

#[cfg(test)]
pub(super) fn set_key_event_is_repeat_for_test(repeated: bool) {
    record_key_event_repeat(repeated);
}

thread_local! {
    static PLATFORM_OBSERVERS: RefCell<Vec<NotificationObserver>> = const {
        RefCell::new(Vec::new())
    };
    static PLATFORM_EVENT_MONITORS: RefCell<Vec<Retained<AnyObject>>> = const {
        RefCell::new(Vec::new())
    };
    static KEY_EVENT_IS_REPEAT: Cell<bool> = const { Cell::new(false) };
    static LAUNCHER_PLACEMENT: RefCell<Option<LauncherPlacement>> = const {
        RefCell::new(None)
    };
    static SHELF_POSITION: Cell<Option<objc2_foundation::NSPoint>> = const { Cell::new(None) };
    static SHELF_RETURN_DESTINATION: RefCell<Option<Retained<NSRunningApplication>>> = const {
        RefCell::new(None)
    };
}

#[derive(Clone, Copy)]
struct LauncherPlacement {
    screen_x: f64,
    screen_y: f64,
    screen_width: f64,
    screen_height: f64,
    anchor_top: f64,
}

impl LauncherPlacement {
    pub(super) fn matches(self, visible: objc2_foundation::NSRect) -> bool {
        (self.screen_x - visible.origin.x).abs() < 0.5
            && (self.screen_y - visible.origin.y).abs() < 0.5
            && (self.screen_width - visible.size.width).abs() < 0.5
            && (self.screen_height - visible.size.height).abs() < 0.5
    }
}

fn retain_observer(observer: NotificationObserver) {
    PLATFORM_OBSERVERS.with(|observers| observers.borrow_mut().push(observer));
}

fn remember_shelf_destination(application: Retained<NSRunningApplication>) {
    if application.isTerminated()
        || application.processIdentifier() == std::process::id() as i32
        || application.activationPolicy() != NSApplicationActivationPolicy::Regular
    {
        return;
    }
    SHELF_RETURN_DESTINATION.with(|destination| {
        *destination.borrow_mut() = Some(application);
    });
}

pub(super) fn remember_clipboard_shelf_destination() -> bool {
    let Some(application) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return false;
    };
    remember_shelf_destination(application);
    SHELF_RETURN_DESTINATION.with(|destination| destination.borrow().is_some())
}

pub(super) fn return_to_previous_application() -> super::ApplicationReturnResult {
    let destination = SHELF_RETURN_DESTINATION.with(|destination| destination.borrow().clone());
    let Some(application) = destination else {
        tracing::warn!("clipboard_shelf_return_destination_unavailable");
        return super::ApplicationReturnResult::Unavailable;
    };
    if application.isTerminated() {
        tracing::warn!("clipboard_shelf_return_destination_terminated");
        return super::ApplicationReturnResult::Unavailable;
    }
    let activated =
        application.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    tracing::debug!(
        activated,
        pid = application.processIdentifier(),
        "clipboard_shelf_return_destination_activated"
    );
    if activated {
        super::ApplicationReturnResult::Returned
    } else {
        super::ApplicationReturnResult::Failed
    }
}

mod observers;
pub(super) use observers::*;
mod status_item;
pub(super) use status_item::*;
mod window;
pub(super) use window::*;
mod activation;
pub(super) use activation::*;
pub(super) fn default_product_paths(
    executable: &std::path::Path,
) -> anyhow::Result<(std::path::PathBuf, std::path::PathBuf)> {
    use anyhow::Context as _;
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .context("HOME is unavailable")?;
    let data_directory = home
        .join("Library")
        .join("Application Support")
        .join("Anywhere");
    let bundled_plugin_directory = executable
        .parent()
        .and_then(|contents| contents.parent())
        .map(|bundle| bundle.join("Resources").join("plugins"))
        .unwrap_or_else(|| std::path::PathBuf::from("plugins"));
    Ok((data_directory, bundled_plugin_directory))
}
