mod preview;

pub use preview::{PreviewHost, PreviewPoll};

/// The GPUI product targets macOS 15+.
pub fn require_supported_host() {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/bin/sw_vers")
            .args(["-productVersion"])
            .output()
            .ok();
        let major = output
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|version| version.trim().split('.').next()?.parse::<u32>().ok())
            .unwrap_or(0);
        assert!(
            major >= crate::MINIMUM_MACOS_MAJOR,
            "{} requires macOS {} or later",
            crate::PRODUCT_NAME,
            crate::MINIMUM_MACOS_MAJOR
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        panic!(
            "{} is currently implemented for macOS only",
            crate::PRODUCT_NAME
        );
    }
}

pub fn show_main_window() {
    #[cfg(target_os = "macos")]
    macos::show_main_window();
}

pub fn hide_main_window() {
    #[cfg(target_os = "macos")]
    preview::dismiss_quick_look();
    #[cfg(target_os = "macos")]
    macos::hide_main_window();
}

pub fn set_window_floating(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::set_window_floating(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

pub struct StatusItemHost {
    #[cfg(target_os = "macos")]
    bar: objc2::rc::Retained<objc2_app_kit::NSStatusBar>,
    #[cfg(target_os = "macos")]
    item: objc2::rc::Retained<objc2_app_kit::NSStatusItem>,
    #[cfg(target_os = "macos")]
    _target: objc2::rc::Retained<macos::StatusItemTarget>,
}

pub fn install_status_item() -> Option<StatusItemHost> {
    #[cfg(target_os = "macos")]
    {
        macos::install_status_item()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::StatusItemHost;
    use objc2::{
        define_class, msg_send, rc::Retained, runtime::AnyObject, sel, MainThreadMarker,
        MainThreadOnly,
    };
    use objc2_app_kit::{
        NSApplication, NSFloatingWindowLevel, NSStatusBar, NSVariableStatusItemLength,
    };
    use objc2_foundation::{NSObject, NSObjectProtocol, NSString};

    #[derive(Debug, Default)]
    pub(super) struct StatusItemTargetIvars;

    define_class!(
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[ivars = StatusItemTargetIvars]
        pub(super) struct StatusItemTarget;

        unsafe impl NSObjectProtocol for StatusItemTarget {}

        impl StatusItemTarget {
            #[unsafe(method(showAnywhere:))]
            fn show_anywhere(&self, _sender: Option<&AnyObject>) {
                super::show_main_window();
            }
        }
    );

    impl StatusItemTarget {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let target = Self::alloc(mtm).set_ivars(StatusItemTargetIvars);
            unsafe { msg_send![super(target), init] }
        }
    }

    pub(super) fn install_status_item() -> Option<StatusItemHost> {
        let mtm = MainThreadMarker::new()?;
        let bar = NSStatusBar::systemStatusBar();
        let item = bar.statusItemWithLength(NSVariableStatusItemLength);
        let button = item.button(mtm)?;
        let target = StatusItemTarget::new(mtm);
        button.setTitle(&NSString::from_str("A"));
        button.setToolTip(Some(&NSString::from_str(crate::PRODUCT_NAME)));
        // SAFETY: The target implements showAnywhere: with the NSMenuItem-compatible
        // sender signature, and both objects are retained for the app lifetime.
        unsafe {
            button.setTarget(Some(target.as_ref()));
            button.setAction(Some(sel!(showAnywhere:)));
        }
        Some(StatusItemHost {
            bar,
            item,
            _target: target,
        })
    }

    pub fn show_main_window() {
        let Some(marker) = MainThreadMarker::new() else {
            return;
        };
        let application = NSApplication::sharedApplication(marker);
        application.activate();
        if let Some(window) = application
            .windows()
            .iter()
            .find(|window| window.title().to_string() == crate::PRODUCT_NAME)
        {
            window.makeKeyAndOrderFront(None);
        }
    }

    pub fn hide_main_window() {
        let Some(marker) = MainThreadMarker::new() else {
            return;
        };
        let application = NSApplication::sharedApplication(marker);
        if let Some(window) = application
            .windows()
            .iter()
            .find(|window| window.title().to_string() == crate::PRODUCT_NAME)
        {
            window.orderOut(None);
        }
    }

    pub fn set_window_floating(title: &str) -> bool {
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        let application = NSApplication::sharedApplication(marker);
        let Some(window) = application
            .windows()
            .iter()
            .find(|window| window.title().to_string() == title)
        else {
            return false;
        };
        window.setLevel(NSFloatingWindowLevel);
        true
    }
}

#[cfg(target_os = "macos")]
impl Drop for StatusItemHost {
    fn drop(&mut self) {
        self.bar.removeStatusItem(&self.item);
    }
}

/// Keeps the exact system WebView dependencies visible to native integration
/// tests even when a test target only exercises the surrounding platform layer.
#[allow(dead_code)]
pub fn native_webview_dependency_types() -> [std::any::TypeId; 2] {
    [
        std::any::TypeId::of::<gpui_webview::WebView>(),
        std::any::TypeId::of::<wry::WebView>(),
    ]
}
