mod material;
mod preview;

pub use material::{
    current_appearance as current_window_appearance, current_mode as current_window_material,
    uses_increase_contrast, uses_opaque_fallback, uses_reduce_motion, WindowAppearance,
};
pub use preview::{PreviewHost, PreviewPoll};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformEvent {
    LauncherLostKey,
    LauncherBecameKey,
    ApplicationLostActive,
    ApplicationBecameActive,
    SettingsEscape,
    WebViewEscape,
    TasksKey {
        key: TasksNavigationKey,
        repeated: bool,
    },
    AccessibilityDisplayChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TasksNavigationKey {
    Escape,
    Up,
    Down,
    Enter,
}

fn tasks_navigation_key(key_code: u16, shift: bool) -> Option<TasksNavigationKey> {
    if shift {
        return None;
    }
    match key_code {
        53 => Some(TasksNavigationKey::Escape),
        125 => Some(TasksNavigationKey::Down),
        126 => Some(TasksNavigationKey::Up),
        36 | 76 => Some(TasksNavigationKey::Enter),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn current_key_event_is_repeat() -> bool {
    macos::current_key_event_is_repeat()
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn set_key_event_is_repeat_for_test(repeated: bool) {
    macos::set_key_event_is_repeat_for_test(repeated);
}

#[cfg(all(test, not(target_os = "macos")))]
pub(crate) fn set_key_event_is_repeat_for_test(_repeated: bool) {}

#[cfg(not(target_os = "macos"))]
pub(crate) fn current_key_event_is_repeat() -> bool {
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivacySettingsPane {
    Accessibility,
    Automation,
    Clipboard,
    FullDiskAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationReturnResult {
    Returned,
    Unavailable,
    Failed,
}

impl PrivacySettingsPane {
    fn target(self) -> &'static str {
        match self {
            Self::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Self::Automation => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"
            }
            Self::Clipboard => {
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension"
            }
            Self::FullDiskAccess => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"
            }
        }
    }
}

pub fn open_privacy_settings(pane: PrivacySettingsPane) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::open_privacy_settings(pane)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pane;
        false
    }
}

pub fn install_platform_observers() -> tokio::sync::mpsc::Receiver<PlatformEvent> {
    #[cfg(target_os = "macos")]
    {
        macos::install_platform_observers()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (_sender, receiver) = tokio::sync::mpsc::channel(64);
        receiver
    }
}

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

pub(crate) fn prepare_window_material() {
    material::prepare_window_material();
}

pub fn show_main_window() {
    #[cfg(target_os = "macos")]
    macos::show_main_window();
}

pub fn remember_clipboard_shelf_destination() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::remember_clipboard_shelf_destination()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn return_to_previous_application() -> ApplicationReturnResult {
    #[cfg(target_os = "macos")]
    {
        macos::return_to_previous_application()
    }
    #[cfg(not(target_os = "macos"))]
    {
        ApplicationReturnResult::Unavailable
    }
}

/// Reveals the development preview after the launch callback has returned.
pub fn show_main_window_after_launch() {
    #[cfg(target_os = "macos")]
    {
        if let Ok(delay) = dispatch2::DispatchTime::try_from(std::time::Duration::from_secs(1)) {
            let _ = dispatch2::DispatchQueue::main().after(delay, || {
                tracing::debug!("preview_launcher_show_requested");
                macos::show_main_window();
            });
        } else {
            tracing::error!("preview_launch_delay_unrepresentable");
        }
    }
}

pub fn hide_main_window() {
    #[cfg(target_os = "macos")]
    preview::dismiss_quick_look();
    #[cfg(target_os = "macos")]
    macos::hide_main_window();
}

pub fn configure_main_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::configure_main_window(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

/// Configures the native window after the GPUI launch callback has returned.
///
/// AppKit can synchronously emit move/resize callbacks while changing a window
/// style or position. Running those changes during GPUI's launch callback would
/// re-enter the App while it is mutably borrowed.
pub fn configure_main_window_after_launch(title: &str) {
    #[cfg(target_os = "macos")]
    {
        let title = title.to_owned();
        dispatch2::DispatchQueue::main().exec_async(move || {
            if !configure_main_window(&title) {
                tracing::warn!("launcher_window_configuration_failed");
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
    }
}

/// Applies the Launcher style after the current GPUI update returns.
pub fn configure_main_window_after_update(title: &str) {
    configure_main_window_after_update_then(title, || {});
}

/// Applies the Launcher style after the current GPUI update, then invokes a
/// main-thread completion before GPUI resumes focus work.
pub fn configure_main_window_after_update_then(
    title: &str,
    completion: impl FnOnce() + Send + 'static,
) {
    #[cfg(target_os = "macos")]
    {
        let title = title.to_owned();
        dispatch2::DispatchQueue::main().exec_async(move || {
            if !configure_main_window(&title) {
                tracing::warn!("launcher_window_configuration_failed");
            }
            completion();
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        completion();
    }
}

pub fn configure_settings_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::configure_settings_window(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

/// Applies the Settings style after the current GPUI update returns.
pub fn configure_settings_window_after_update(title: &str) {
    #[cfg(target_os = "macos")]
    {
        let title = title.to_owned();
        dispatch2::DispatchQueue::main().exec_async(move || {
            if !configure_settings_window(&title) {
                tracing::warn!("settings_window_configuration_failed");
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
    }
}

pub fn configure_tasks_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::configure_tasks_window(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

/// Applies the Tasks style after the current GPUI update returns.
pub fn configure_tasks_window_after_update(title: &str) {
    #[cfg(target_os = "macos")]
    {
        let title = title.to_owned();
        dispatch2::DispatchQueue::main().exec_async(move || {
            if !configure_tasks_window(&title) {
                tracing::warn!("tasks_window_configuration_failed");
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
    }
}

pub fn activate_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::activate_window(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

/// Activates an existing auxiliary window after the current GPUI update.
///
/// `deminiaturize` and `orderFront` can synchronously request a new frame.
/// Calling them while GPUI still mutably owns the App would reject that frame
/// with `RefCell already borrowed`.
pub fn activate_window_after_update(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        if !macos::window_exists(title) {
            return false;
        }
        let title = title.to_owned();
        dispatch2::DispatchQueue::main().exec_async(move || {
            if !macos::activate_window(&title) {
                tracing::warn!(%title, "auxiliary_window_activation_failed");
            }
        });
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

pub fn window_exists(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::window_exists(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

pub fn refresh_window_material(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::refresh_window_material(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

pub fn refresh_all_window_materials() -> bool {
    let main = refresh_window_material(crate::PRODUCT_NAME);
    let shelf = refresh_window_material(crate::CLIPBOARD_SHELF_TITLE);
    let tasks = refresh_window_material(crate::TASKS_WINDOW_TITLE);
    main || shelf || tasks
}

pub fn position_main_window() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::position_main_window()
    }
    #[cfg(not(target_os = "macos"))]
    {
        focus_loss_should_dismiss(false, false)
    }
}

/// Re-anchors the launcher after GPUI has finished a resize update.
///
/// AppKit keeps the native frame's bottom edge stable when a window grows.
/// Dispatching the reposition keeps the visible top edge fixed without
/// re-entering GPUI while it still mutably owns the window.
pub fn position_main_window_after_update() {
    #[cfg(target_os = "macos")]
    {
        dispatch2::DispatchQueue::main().exec_async(|| {
            if !macos::position_main_window() {
                tracing::warn!("launcher_window_position_failed");
            }
        });
    }
}

#[cfg(target_os = "macos")]
static LAUNCHER_RESIZE_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Resizes and re-anchors the launcher in one AppKit frame update.
///
/// `NSWindow::setContentSize` keeps the bottom edge fixed. Repositioning in a
/// later main-queue turn exposes an intermediate frame whose content has the
/// new height but the old top anchor. A single `setFrame:display:` avoids that
/// mismatch. The native resize also suppresses implicit layer animations and
/// draws the resized content before presenting the new frame. Rapid candidate
/// updates coalesce to the latest size so stale queued frames cannot win.
pub fn resize_main_window_preserving_top(width: f32, height: f32) -> bool {
    #[cfg(target_os = "macos")]
    {
        if !macos::window_exists(crate::PRODUCT_NAME) {
            return false;
        }
        let generation = LAUNCHER_RESIZE_GENERATION
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .wrapping_add(1);
        dispatch2::DispatchQueue::main().exec_async(move || {
            if LAUNCHER_RESIZE_GENERATION.load(std::sync::atomic::Ordering::Acquire) != generation {
                return;
            }
            if !macos::resize_launcher_window_preserving_top(width, height) {
                tracing::warn!("launcher_window_resize_failed");
            }
        });
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (width, height);
        false
    }
}

pub(crate) fn constrained_window_extent(
    visible_width: f32,
    visible_height: f32,
    requested_width: f32,
    requested_height: f32,
    preferred_inset: f32,
) -> (f32, f32) {
    let inset_x = preferred_inset.min(visible_width.max(0.0) / 4.0);
    let inset_y = preferred_inset.min(visible_height.max(0.0) / 4.0);
    (
        requested_width.min((visible_width - inset_x * 2.0).max(1.0)),
        requested_height.min((visible_height - inset_y * 2.0).max(1.0)),
    )
}

pub fn launcher_should_dismiss_for_focus_loss() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::launcher_should_dismiss_for_focus_loss()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn configure_clipboard_shelf_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::configure_clipboard_shelf_window(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        false
    }
}

pub fn configure_clipboard_shelf_window_after_update(title: &str) {
    #[cfg(target_os = "macos")]
    {
        let title = title.to_owned();
        dispatch2::DispatchQueue::main().exec_async(move || {
            if !configure_clipboard_shelf_window(&title) {
                tracing::warn!("clipboard_shelf_window_configuration_failed");
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusItemAction {
    ShowCurrentApp,
    ShowUniversal,
    OpenTasks,
    OpenSettings,
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusMenuLabels {
    pub current_app: String,
    pub universal: String,
    pub running_tasks: String,
    pub settings: String,
    pub quit: String,
}

impl StatusMenuLabels {
    pub fn new(locale: crate::l10n::Locale, preferences: &crate::host::HostPreferences) -> Self {
        Self {
            current_app: status_action_label(
                crate::l10n::current_app(locale),
                &preferences.current_app_shortcut,
            ),
            universal: status_action_label(
                crate::l10n::universal_search(locale),
                &preferences.universal_shortcut,
            ),
            running_tasks: crate::l10n::running_tasks_menu(locale),
            settings: crate::l10n::settings_menu(locale),
            quit: crate::l10n::quit_anywhere(locale),
        }
    }
}

pub struct StatusItemHost {
    #[cfg(target_os = "macos")]
    bar: objc2::rc::Retained<objc2_app_kit::NSStatusBar>,
    #[cfg(target_os = "macos")]
    item: objc2::rc::Retained<objc2_app_kit::NSStatusItem>,
    #[cfg(target_os = "macos")]
    _menu: objc2::rc::Retained<objc2_app_kit::NSMenu>,
    #[cfg(target_os = "macos")]
    _target: objc2::rc::Retained<macos::StatusItemTarget>,
}

impl StatusItemHost {
    pub fn update_labels(&self, labels: StatusMenuLabels) -> bool {
        #[cfg(target_os = "macos")]
        {
            macos::update_status_menu(&self._menu, &self._target, labels);
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = labels;
            false
        }
    }
}

pub fn install_status_item(
    actions: tokio::sync::mpsc::Sender<StatusItemAction>,
    labels: StatusMenuLabels,
) -> Option<StatusItemHost> {
    #[cfg(target_os = "macos")]
    {
        macos::install_status_item(actions, labels)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = actions;
        let _ = labels;
        None
    }
}

fn status_action_label(title: String, shortcut: &str) -> String {
    match macos_shortcut_label(shortcut) {
        Some(shortcut) => format!("{title}  {shortcut}"),
        None => title,
    }
}

pub(crate) fn macos_shortcut_label(shortcut: &str) -> Option<String> {
    use global_hotkey::hotkey::Modifiers;

    let hotkey = shortcut.parse::<global_hotkey::hotkey::HotKey>().ok()?;
    let mut label = String::new();
    if hotkey.mods.contains(Modifiers::CONTROL) {
        label.push('⌃');
    }
    if hotkey.mods.contains(Modifiers::ALT) {
        label.push('⌥');
    }
    if hotkey.mods.contains(Modifiers::SHIFT) {
        label.push('⇧');
    }
    if hotkey.mods.contains(Modifiers::META) {
        label.push('⌘');
    }
    label.push_str(&status_key_label(hotkey.key));
    Some(label)
}

fn status_key_label(code: global_hotkey::hotkey::Code) -> String {
    use global_hotkey::hotkey::Code;

    let debug = format!("{code:?}");
    if let Some(letter) = debug.strip_prefix("Key") {
        if letter.len() == 1 {
            return letter.to_owned();
        }
    }
    if let Some(digit) = debug.strip_prefix("Digit") {
        if digit.len() == 1 {
            return digit.to_owned();
        }
    }
    match code {
        Code::Space => "Space".to_owned(),
        Code::Enter => "↩".to_owned(),
        Code::Escape => "⎋".to_owned(),
        Code::Tab => "⇥".to_owned(),
        Code::Backspace => "⌫".to_owned(),
        Code::Delete => "⌦".to_owned(),
        Code::ArrowUp => "↑".to_owned(),
        Code::ArrowDown => "↓".to_owned(),
        Code::ArrowLeft => "←".to_owned(),
        Code::ArrowRight => "→".to_owned(),
        _ => debug,
    }
}

fn launcher_anchor_top(visible_bottom: f64, visible_height: f64) -> f64 {
    visible_bottom + visible_height * 0.8
}

fn launcher_origin_y(visible_bottom: f64, window_height: f64, anchor_top: f64) -> f64 {
    (anchor_top - window_height).max(visible_bottom)
}

fn bottom_trailing_origin(
    visible_x: f64,
    visible_y: f64,
    visible_width: f64,
    visible_height: f64,
    window_width: f64,
) -> (f64, f64) {
    let inset = 16.0_f64.min((visible_width.min(visible_height) - 1.0).max(0.0) * 0.5);
    (
        visible_x + visible_width - inset - window_width,
        visible_y + inset,
    )
}

pub fn position_clipboard_shelf_window() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::position_clipboard_shelf_window()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn clipboard_shelf_is_key_window() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::clipboard_shelf_is_key_window()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
mod macos {
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
        NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification,
        NSEvent, NSEventMask, NSEventModifierFlags, NSFloatingWindowLevel, NSImage, NSMenu,
        NSMenuItem, NSNormalWindowLevel, NSPanel, NSRunningApplication, NSScreen, NSStatusBar,
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
        fn matches(self, visible: objc2_foundation::NSRect) -> bool {
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

    pub(super) fn install_platform_observers() -> tokio::sync::mpsc::Receiver<PlatformEvent> {
        let (sender, receiver) = tokio::sync::mpsc::channel(64);
        let Some(marker) = MainThreadMarker::new() else {
            return receiver;
        };
        let application = NSApplication::sharedApplication(marker);
        let Some(window) = find_window(crate::PRODUCT_NAME) else {
            return receiver;
        };
        let center = NSNotificationCenter::defaultCenter();
        let workspace_center = NSWorkspace::sharedWorkspace().notificationCenter();

        let lost_sender = sender.clone();
        let lost_key = RcBlock::new(move |_: NonNull<NSNotification>| {
            tracing::debug!("launcher_lost_key_notification");
            if lost_sender
                .try_send(PlatformEvent::LauncherLostKey)
                .is_err()
            {
                tracing::warn!("platform_event_queue_full");
            }
        });
        unsafe {
            retain_observer(center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidResignKeyNotification),
                Some(window.as_ref()),
                None,
                &lost_key,
            ));
        }

        let key_sender = sender.clone();
        let became_key = RcBlock::new(move |_: NonNull<NSNotification>| {
            tracing::debug!("launcher_became_key_notification");
            if key_sender
                .try_send(PlatformEvent::LauncherBecameKey)
                .is_err()
            {
                tracing::warn!("platform_event_queue_full");
            }
        });
        unsafe {
            retain_observer(center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidBecomeKeyNotification),
                Some(window.as_ref()),
                None,
                &became_key,
            ));
        }

        let inactive_sender = sender.clone();
        let lost_active = RcBlock::new(move |_: NonNull<NSNotification>| {
            tracing::debug!("launcher_application_lost_active_notification");
            if inactive_sender
                .try_send(PlatformEvent::ApplicationLostActive)
                .is_err()
            {
                tracing::warn!("platform_event_queue_full");
            }
        });
        unsafe {
            retain_observer(center.addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidResignActiveNotification),
                Some(application.as_ref()),
                None,
                &lost_active,
            ));
        }

        let active_sender = sender.clone();
        let became_active = RcBlock::new(move |_: NonNull<NSNotification>| {
            tracing::debug!("launcher_application_became_active_notification");
            if active_sender
                .try_send(PlatformEvent::ApplicationBecameActive)
                .is_err()
            {
                tracing::warn!("platform_event_queue_full");
            }
        });
        unsafe {
            retain_observer(center.addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidBecomeActiveNotification),
                Some(application.as_ref()),
                None,
                &became_active,
            ));
        }

        let accessibility_sender = sender.clone();
        let accessibility_changed = RcBlock::new(move |_: NonNull<NSNotification>| {
            if accessibility_sender
                .try_send(PlatformEvent::AccessibilityDisplayChanged)
                .is_err()
            {
                tracing::warn!("platform_event_queue_full");
            }
        });
        unsafe {
            retain_observer(workspace_center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
                None,
                None,
                &accessibility_changed,
            ));
        }

        let destination_block = RcBlock::new(move |_: NonNull<NSNotification>| {
            if let Some(application) = NSWorkspace::sharedWorkspace().frontmostApplication() {
                remember_shelf_destination(application);
            }
        });
        unsafe {
            retain_observer(workspace_center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidActivateApplicationNotification),
                None,
                None,
                &destination_block,
            ));
        }
        // The app can start hidden while another Regular application is already
        // frontmost, before any activation notification is delivered. Seed the
        // return target here so opening the Launcher does not overwrite it with
        // this accessory process.
        if !remember_clipboard_shelf_destination() {
            tracing::debug!("clipboard_shelf_destination_unavailable_at_startup");
        }

        let key_sender = sender.clone();
        let key_marker = marker;
        let key_monitor = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            let event_ptr = event.as_ptr();
            let event = unsafe { event.as_ref() };
            record_key_event_repeat(event.isARepeat());
            let Some(window) = event
                .window(key_marker)
                .or_else(|| NSApplication::sharedApplication(key_marker).keyWindow())
            else {
                return event_ptr;
            };
            if !window.isKeyWindow() {
                return event_ptr;
            }
            let modifiers = event.modifierFlags();
            if modifiers.intersects(
                NSEventModifierFlags::Command
                    | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Option,
            ) {
                return event_ptr;
            }
            let first_responder = window.firstResponder();
            let composing = first_responder.as_ref().is_some_and(|responder| {
                // GPUIView implements NSTextInputClient. Passing the event through
                // while marked text exists lets the active IME cancel composition.
                let responder: &AnyObject = responder.as_ref();
                let responds: objc2::runtime::Bool =
                    unsafe { msg_send![responder, respondsToSelector: sel!(hasMarkedText)] };
                responds.as_bool() && {
                    let marked: objc2::runtime::Bool =
                        unsafe { msg_send![responder, hasMarkedText] };
                    marked.as_bool()
                }
            });
            if composing {
                return event_ptr;
            }
            let title = window.title().to_string();
            if title == crate::TASKS_WINDOW_TITLE {
                let Some(key) = super::tasks_navigation_key(
                    event.keyCode(),
                    modifiers.contains(NSEventModifierFlags::Shift),
                ) else {
                    return event_ptr;
                };
                tracing::debug!(?key, "tasks_navigation_key_intercepted");
                let repeated = event.isARepeat();
                if key_sender
                    .try_send(PlatformEvent::TasksKey { key, repeated })
                    .is_err()
                {
                    tracing::warn!("platform_event_queue_full");
                }
                return ptr::null_mut();
            }
            if event.keyCode() != 53 || title != crate::PRODUCT_NAME {
                return event_ptr;
            }
            let web_responder = first_responder.as_ref().is_some_and(|responder| {
                responder.class().name().to_string_lossy().starts_with("WK")
            });
            if web_responder {
                tracing::debug!("web_view_escape_intercepted");
                if key_sender.try_send(PlatformEvent::WebViewEscape).is_err() {
                    tracing::warn!("platform_event_queue_full");
                }
                return ptr::null_mut();
            }
            if !window.styleMask().contains(NSWindowStyleMask::Closable) {
                return event_ptr;
            }
            tracing::debug!("settings_escape_intercepted");
            if key_sender.try_send(PlatformEvent::SettingsEscape).is_err() {
                tracing::warn!("platform_event_queue_full");
            }
            ptr::null_mut()
        });
        if let Some(monitor) = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::KeyDown,
                &key_monitor,
            )
        } {
            PLATFORM_EVENT_MONITORS.with(|monitors| monitors.borrow_mut().push(monitor));
        } else {
            tracing::warn!("platform_key_monitor_unavailable");
        }
        receiver
    }

    pub(super) struct StatusItemTargetIvars {
        actions: tokio::sync::mpsc::Sender<StatusItemAction>,
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[ivars = StatusItemTargetIvars]
        pub(super) struct StatusItemTarget;

        unsafe impl NSObjectProtocol for StatusItemTarget {}

        impl StatusItemTarget {
            #[unsafe(method(showCurrentApp:))]
            fn show_current_app(&self, _sender: Option<&AnyObject>) {
                tracing::debug!("status_menu_show_current_app");
                let _ = self
                    .ivars()
                    .actions
                    .try_send(StatusItemAction::ShowCurrentApp);
            }

            #[unsafe(method(showUniversal:))]
            fn show_universal(&self, _sender: Option<&AnyObject>) {
                tracing::debug!("status_menu_show_universal");
                let _ = self
                    .ivars()
                    .actions
                    .try_send(StatusItemAction::ShowUniversal);
            }

            #[unsafe(method(openTasks:))]
            fn open_tasks(&self, _sender: Option<&AnyObject>) {
                tracing::debug!("status_menu_open_tasks");
                let _ = self
                    .ivars()
                    .actions
                    .try_send(StatusItemAction::OpenTasks);
            }

            #[unsafe(method(openSettings:))]
            fn open_settings(&self, _sender: Option<&AnyObject>) {
                tracing::debug!("status_menu_open_settings");
                let _ = self
                    .ivars()
                    .actions
                    .try_send(StatusItemAction::OpenSettings);
            }

            #[unsafe(method(quitAnywhere:))]
            fn quit_anywhere(&self, _sender: Option<&AnyObject>) {
                tracing::debug!("status_menu_quit");
                let _ = self.ivars().actions.try_send(StatusItemAction::Quit);
            }
        }
    );

    impl StatusItemTarget {
        fn new(
            mtm: MainThreadMarker,
            actions: tokio::sync::mpsc::Sender<StatusItemAction>,
        ) -> Retained<Self> {
            let target = Self::alloc(mtm).set_ivars(StatusItemTargetIvars { actions });
            unsafe { msg_send![super(target), init] }
        }
    }

    fn menu_item(
        mtm: MainThreadMarker,
        title: &str,
        action: objc2::runtime::Sel,
        key_equivalent: &str,
        target: &StatusItemTarget,
    ) -> Retained<NSMenuItem> {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key_equivalent),
            )
        };
        // SAFETY: `StatusItemHost` retains the target and menu for the app lifetime.
        unsafe { item.setTarget(Some(target.as_ref())) };
        item.setEnabled(true);
        item
    }

    pub(super) fn install_status_item(
        actions: tokio::sync::mpsc::Sender<StatusItemAction>,
        labels: StatusMenuLabels,
    ) -> Option<StatusItemHost> {
        let mtm = MainThreadMarker::new()?;
        let bar = NSStatusBar::systemStatusBar();
        let item = bar.statusItemWithLength(NSVariableStatusItemLength);
        let button = item.button(mtm)?;
        let target = StatusItemTarget::new(mtm, actions);
        if let Some(image) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str("magnifyingglass"),
            Some(&NSString::from_str(crate::PRODUCT_NAME)),
        ) {
            button.setImage(Some(&image));
        } else {
            button.setTitle(&NSString::from_str("A"));
        }
        button.setToolTip(Some(&NSString::from_str(crate::PRODUCT_NAME)));
        let menu =
            NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(crate::PRODUCT_NAME));
        menu.setAutoenablesItems(false);
        populate_menu(mtm, &menu, &target, &labels);
        item.setMenu(Some(&menu));
        Some(StatusItemHost {
            bar,
            item,
            _menu: menu,
            _target: target,
        })
    }

    fn populate_menu(
        mtm: MainThreadMarker,
        menu: &NSMenu,
        target: &StatusItemTarget,
        labels: &StatusMenuLabels,
    ) {
        tracing::debug!(
            current_app = %labels.current_app,
            universal = %labels.universal,
            running_tasks = %labels.running_tasks,
            settings = %labels.settings,
            quit = %labels.quit,
            "status_menu_populated"
        );
        menu.removeAllItems();
        menu.addItem(&menu_item(
            mtm,
            &labels.current_app,
            sel!(showCurrentApp:),
            "",
            target,
        ));
        menu.addItem(&menu_item(
            mtm,
            &labels.universal,
            sel!(showUniversal:),
            "",
            target,
        ));
        menu.addItem(&menu_item(
            mtm,
            &labels.running_tasks,
            sel!(openTasks:),
            "",
            target,
        ));
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&menu_item(
            mtm,
            &labels.settings,
            sel!(openSettings:),
            ",",
            target,
        ));
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&menu_item(
            mtm,
            &labels.quit,
            sel!(quitAnywhere:),
            "q",
            target,
        ));
    }

    pub(super) fn update_status_menu(
        menu: &NSMenu,
        target: &StatusItemTarget,
        labels: StatusMenuLabels,
    ) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        populate_menu(mtm, menu, target, &labels);
    }

    pub(super) fn open_privacy_settings(pane: super::PrivacySettingsPane) -> bool {
        let Some(url) = NSURL::URLWithString(&NSString::from_str(pane.target())) else {
            tracing::warn!(?pane, "privacy_settings_url_invalid");
            return false;
        };
        let opened = NSWorkspace::sharedWorkspace().openURL(&url);
        if !opened {
            tracing::warn!(?pane, "privacy_settings_open_failed");
        }
        opened
    }

    fn find_window(title: &str) -> Option<Retained<NSWindow>> {
        let marker = MainThreadMarker::new()?;
        let application = NSApplication::sharedApplication(marker);
        application
            .windows()
            .iter()
            .find(|window| window.title().to_string() == title)
    }

    fn focus_gpui_view(window: &NSWindow, view: &NSView, class: &AnyClass) -> bool {
        if view.isKindOfClass(class) {
            return window.makeFirstResponder(Some(view));
        }
        view.subviews()
            .iter()
            .any(|subview| focus_gpui_view(window, &subview, class))
    }

    fn restore_gpui_first_responder(window: &NSWindow) -> bool {
        let Some(class) = AnyClass::get(c"GPUIView") else {
            return false;
        };
        let Some(content) = window.contentView() else {
            return false;
        };
        focus_gpui_view(window, &content, class)
    }

    pub(super) fn launcher_window_style_mask() -> NSWindowStyleMask {
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel
    }

    pub(super) fn clipboard_shelf_window_style_mask() -> NSWindowStyleMask {
        NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView
    }

    pub(super) fn settings_window_style_mask() -> NSWindowStyleMask {
        NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::FullSizeContentView
    }

    pub(super) fn tasks_window_style_mask() -> NSWindowStyleMask {
        NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum NativeSurface {
        Launcher,
        ClipboardShelf,
        Settings,
        Tasks,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum WindowLayerPolicy {
        Spotlight,
        FloatingTool,
        Normal,
    }

    impl WindowLayerPolicy {
        pub(super) const fn uses_floating_level(self) -> bool {
            !matches!(self, Self::Normal)
        }

        pub(super) const fn joins_all_spaces(self) -> bool {
            !matches!(self, Self::Normal)
        }

        pub(super) const fn uses_panel_semantics(self) -> bool {
            !matches!(self, Self::Normal)
        }
    }

    pub(super) const fn window_layer_policy(surface: NativeSurface) -> WindowLayerPolicy {
        match surface {
            NativeSurface::Launcher => WindowLayerPolicy::Spotlight,
            NativeSurface::ClipboardShelf => WindowLayerPolicy::FloatingTool,
            NativeSurface::Settings | NativeSurface::Tasks => WindowLayerPolicy::Normal,
        }
    }

    fn apply_window_layer(window: &NSWindow, policy: WindowLayerPolicy) {
        if policy.uses_floating_level() {
            window.setLevel(NSFloatingWindowLevel);
        } else {
            window.setLevel(NSNormalWindowLevel);
        }
        if policy.joins_all_spaces() {
            window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary,
            );
        } else {
            window.setCollectionBehavior(NSWindowCollectionBehavior::Default);
        }
        if let Some(panel) = window.downcast_ref::<NSPanel>() {
            panel.setFloatingPanel(policy.uses_panel_semantics());
            panel.setBecomesKeyOnlyIfNeeded(false);
        }
        window.setHidesOnDeactivate(false);
    }

    fn show_standard_button(window: &NSWindow, button: NSWindowButton, enabled: bool) {
        if let Some(button) = window.standardWindowButton(button) {
            button.setHidden(false);
            button.setEnabled(enabled);
        }
    }

    fn configure_window(window: &NSWindow) {
        let style_mask = launcher_window_style_mask();
        if window.styleMask() != style_mask {
            window.setStyleMask(style_mask);
            if !restore_gpui_first_responder(window) {
                tracing::warn!("native_launcher_first_responder_missing_after_style");
            }
        }
        if let Some(panel) = window.downcast_ref::<NSPanel>() {
            panel.setFloatingPanel(true);
            panel.setBecomesKeyOnlyIfNeeded(false);
        }
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        for button in [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ] {
            if let Some(button) = window.standardWindowButton(button) {
                button.setHidden(true);
            }
        }
        apply_window_layer(window, window_layer_policy(NativeSurface::Launcher));
        window.setMovableByWindowBackground(false);
        window.setHasShadow(true);
        // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setAcceptsMouseMovedEvents(true);
        if !super::material::install_window_material(
            window,
            f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        ) {
            tracing::warn!("native_window_material_failed");
        }
        if !restore_gpui_first_responder(window) {
            tracing::warn!("native_launcher_first_responder_missing");
        }
    }

    fn configure_settings_window_style(window: &NSWindow) {
        let style_mask = settings_window_style_mask();
        if window.styleMask() != style_mask {
            window.setStyleMask(style_mask);
        }
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        show_standard_button(window, NSWindowButton::CloseButton, true);
        show_standard_button(window, NSWindowButton::MiniaturizeButton, true);
        show_standard_button(window, NSWindowButton::ZoomButton, false);
        apply_window_layer(window, window_layer_policy(NativeSurface::Settings));
        window.setMovableByWindowBackground(false);
        window.setHasShadow(true);
        // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setAcceptsMouseMovedEvents(true);
        if !super::material::install_window_material(
            window,
            f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        ) {
            tracing::warn!("native_settings_material_failed");
        }
        if !restore_gpui_first_responder(window) {
            tracing::warn!("native_settings_first_responder_missing");
        }
    }

    fn configure_tasks_window_style(window: &NSWindow) {
        let style_mask = tasks_window_style_mask();
        if window.styleMask() != style_mask {
            window.setStyleMask(style_mask);
        }
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        show_standard_button(window, NSWindowButton::CloseButton, true);
        show_standard_button(window, NSWindowButton::MiniaturizeButton, true);
        show_standard_button(window, NSWindowButton::ZoomButton, true);
        apply_window_layer(window, window_layer_policy(NativeSurface::Tasks));
        window.setMovableByWindowBackground(false);
        window.setHasShadow(true);
        // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setAcceptsMouseMovedEvents(true);
        window.setMinSize(objc2_foundation::NSSize::new(760.0, 420.0));
        if !super::material::install_window_material(
            window,
            f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        ) {
            tracing::warn!("native_tasks_material_failed");
        }
    }

    fn configure_clipboard_shelf_window_style(window: &NSWindow) {
        let style_mask = clipboard_shelf_window_style_mask();
        if window.styleMask() != style_mask {
            tracing::warn!(
                expected = ?style_mask,
                actual = ?window.styleMask(),
                "clipboard_shelf_native_style_mismatch"
            );
        }
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        show_standard_button(window, NSWindowButton::CloseButton, true);
        show_standard_button(window, NSWindowButton::MiniaturizeButton, true);
        show_standard_button(window, NSWindowButton::ZoomButton, true);
        apply_window_layer(window, window_layer_policy(NativeSurface::ClipboardShelf));
        window.setMovableByWindowBackground(false);
        window.setHasShadow(true);
        // Keep explicit close as the lifecycle owner: losing focus or being
        // covered by another application must not hide the Shelf.
        window.setHidesOnDeactivate(false);
        // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setAcceptsMouseMovedEvents(true);
        if !super::material::install_window_material(
            window,
            f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        ) {
            tracing::warn!("native_clipboard_shelf_material_failed");
        }
        if !restore_gpui_first_responder(window) {
            tracing::warn!("native_clipboard_shelf_first_responder_missing");
        }
    }

    fn launcher_frame(
        window: &NSWindow,
        marker: MainThreadMarker,
        use_pointer: bool,
        size: NSSize,
    ) -> Option<NSRect> {
        let pointer = NSEvent::mouseLocation();
        let pointer_frame = use_pointer.then(|| {
            NSScreen::screens(marker)
                .iter()
                .find(|screen| {
                    let frame = screen.frame();
                    pointer.x >= frame.origin.x
                        && pointer.x <= frame.origin.x + frame.size.width
                        && pointer.y >= frame.origin.y
                        && pointer.y <= frame.origin.y + frame.size.height
                })
                .map(|screen| screen.visibleFrame())
        });
        let visible = pointer_frame
            .flatten()
            .or_else(|| window.screen().map(|screen| screen.visibleFrame()))
            .or_else(|| NSScreen::mainScreen(marker).map(|screen| screen.visibleFrame()));
        let visible = visible?;
        let size = constrained_window_size(visible, size);
        let centered_x = visible.origin.x + (visible.size.width - size.width) * 0.5;
        let max_x = visible.origin.x + (visible.size.width - size.width).max(0.0);
        let anchor_top = if use_pointer {
            super::launcher_anchor_top(visible.origin.y, visible.size.height)
        } else {
            LAUNCHER_PLACEMENT
                .with(|placement| {
                    placement
                        .borrow()
                        .filter(|placement| placement.matches(visible))
                        .map(|placement| placement.anchor_top)
                })
                .unwrap_or_else(|| {
                    super::launcher_anchor_top(visible.origin.y, visible.size.height)
                })
        };
        LAUNCHER_PLACEMENT.with(|placement| {
            *placement.borrow_mut() = Some(LauncherPlacement {
                screen_x: visible.origin.x,
                screen_y: visible.origin.y,
                screen_width: visible.size.width,
                screen_height: visible.size.height,
                anchor_top,
            });
        });
        let origin = objc2_foundation::NSPoint::new(
            centered_x.clamp(visible.origin.x, max_x),
            super::launcher_origin_y(visible.origin.y, size.height, anchor_top),
        );
        Some(NSRect::new(origin, size))
    }

    pub(super) fn constrained_window_size(visible: NSRect, requested: NSSize) -> NSSize {
        let (width, height) = super::constrained_window_extent(
            visible.size.width as f32,
            visible.size.height as f32,
            requested.width as f32,
            requested.height as f32,
            crate::ui::design_tokens::metrics::SPACE_LG,
        );
        NSSize {
            width: f64::from(width),
            height: f64::from(height),
        }
    }

    fn position_launcher_window(window: &NSWindow, marker: MainThreadMarker, use_pointer: bool) {
        let Some(frame) = launcher_frame(window, marker, use_pointer, window.frame().size) else {
            return;
        };
        window.setFrameOrigin(frame.origin);
    }

    pub(super) fn resize_launcher_window_preserving_top(width: f32, height: f32) -> bool {
        let Some(window) = find_window(crate::PRODUCT_NAME) else {
            return false;
        };
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        let current = window.frame();
        let size = NSSize {
            width: f64::from(width),
            height: f64::from(height),
        };
        if (current.size.width - size.width).abs() <= 0.5
            && (current.size.height - size.height).abs() <= 0.5
        {
            return true;
        }
        let Some(frame) = launcher_frame(&window, marker, false, size) else {
            return false;
        };
        // Input and result updates replace the GPUI scene immediately. Letting
        // AppKit animate the glass/backing layers would stretch the old Metal
        // drawable while that scene is being resized. Present geometry and the
        // freshly drawn content together, without a native resize transition.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        CATransaction::setAnimationDuration(0.0);
        let visible = window.isVisible();
        window.setFrame_display_animate(frame, false, false);
        if let Some(content) = window.contentView() {
            content.layoutSubtreeIfNeeded();
            if visible {
                content.displayIfNeeded();
            }
        }
        if visible {
            window.displayIfNeeded();
        }
        CATransaction::commit();
        tracing::debug!(
            width = size.width,
            height = size.height,
            x = frame.origin.x,
            y = frame.origin.y,
            "launcher_window_resized_atomically"
        );
        true
    }

    fn position_clipboard_shelf_window_on_screen(window: &NSWindow, marker: MainThreadMarker) {
        let pointer = NSEvent::mouseLocation();
        let visible = NSScreen::screens(marker)
            .iter()
            .find(|screen| {
                let frame = screen.frame();
                pointer.x >= frame.origin.x
                    && pointer.x <= frame.origin.x + frame.size.width
                    && pointer.y >= frame.origin.y
                    && pointer.y <= frame.origin.y + frame.size.height
            })
            .map(|screen| screen.visibleFrame())
            .or_else(|| window.screen().map(|screen| screen.visibleFrame()))
            .or_else(|| NSScreen::mainScreen(marker).map(|screen| screen.visibleFrame()));
        let Some(visible) = visible else {
            return;
        };
        let (x, y) = super::bottom_trailing_origin(
            visible.origin.x,
            visible.origin.y,
            visible.size.width,
            visible.size.height,
            window.frame().size.width,
        );
        window.setFrameOrigin(objc2_foundation::NSPoint::new(x, y));
    }

    fn activate_launcher(application: &NSApplication, window: &NSWindow) -> bool {
        if window.isMiniaturized() {
            window.deminiaturize(None);
            tracing::debug!(title = %window.title(), "window_deminiaturized_for_activation");
        }
        // Activation brings the app's current key window forward. A minimized
        // Shelf or Tasks window can still own that identity while the Launcher
        // is hidden, so make the Launcher key before activating the app.
        window.orderFrontRegardless();
        window.makeKeyAndOrderFront(None);
        window.makeKeyWindow();
        let running_application = NSRunningApplication::currentApplication();
        let mut requested = running_application.activateWithOptions(launcher_activation_options());
        if should_activate_with_workspace(application.isActive()) {
            // A status-item action is owned by the system menu process. Direct
            // self-activation is ignored in that event context, so route the
            // reuse request through LaunchServices instead of briefly changing
            // the Accessory activation policy and leaking a Dock icon.
            if queue_workspace_activation(&running_application) {
                schedule_activation_key_recovery(&window.title().to_string());
                requested = true;
            }
        }
        window.orderFrontRegardless();
        window.makeKeyAndOrderFront(None);
        window.makeKeyWindow();
        // AppKit can leave the panel itself as first responder after ordering
        // or reactivation even when the style mask has not changed.
        if !restore_gpui_first_responder(window) {
            tracing::warn!("native_launcher_first_responder_missing_after_activation");
        }
        requested
    }

    fn queue_workspace_activation(running_application: &NSRunningApplication) -> bool {
        let Some(bundle_url) = running_application.bundleURL() else {
            return false;
        };
        let configuration = NSWorkspaceOpenConfiguration::configuration();
        configuration.setActivates(true);
        configuration.setCreatesNewApplicationInstance(false);
        configuration.setAddsToRecentItems(false);
        NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
            &bundle_url,
            &configuration,
            None,
        );
        true
    }

    fn schedule_activation_key_recovery(title: &str) {
        let generation = WINDOW_ACTIVATION_GENERATION.load(Ordering::Acquire);
        for delay_ms in [120, 320] {
            let title = title.to_owned();
            let Ok(delay) =
                dispatch2::DispatchTime::try_from(std::time::Duration::from_millis(delay_ms))
            else {
                continue;
            };
            let _ = dispatch2::DispatchQueue::main().after(delay, move || {
                if WINDOW_ACTIVATION_GENERATION.load(Ordering::Acquire) != generation {
                    return;
                }
                let Some(marker) = MainThreadMarker::new() else {
                    return;
                };
                let application = NSApplication::sharedApplication(marker);
                if !application.isActive() {
                    tracing::debug!(%title, delay_ms, "activation_key_recovery_not_active");
                    return;
                }
                let Some(window) = find_window(&title) else {
                    return;
                };
                if window.isMiniaturized() {
                    window.deminiaturize(None);
                }
                window.orderFrontRegardless();
                window.makeKeyAndOrderFront(None);
                window.makeKeyWindow();
                if !restore_gpui_first_responder(&window) {
                    tracing::warn!(
                        %title,
                        "activation_key_recovery_first_responder_missing"
                    );
                }
                tracing::debug!(
                    %title,
                    delay_ms,
                    visible = window.isVisible(),
                    key = window.isKeyWindow(),
                    app_active = application.isActive(),
                    first_responder = ?window.firstResponder(),
                    "activation_key_recovery_finished"
                );
            });
        }
    }

    pub(super) fn should_activate_with_workspace(application_is_active: bool) -> bool {
        !application_is_active
    }

    pub(super) fn launcher_activation_options() -> NSApplicationActivationOptions {
        NSApplicationActivationOptions::empty()
    }

    pub fn activate_window(title: &str) -> bool {
        let Some(window) = find_window(title) else {
            return false;
        };
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        let application = NSApplication::sharedApplication(marker);
        let activated = activate_launcher(&application, &window);
        tracing::debug!(
            %title,
            activated,
            visible = window.isVisible(),
            key = window.isKeyWindow(),
            app_active = application.isActive(),
            "auxiliary_window_activation_finished"
        );
        activated
    }

    pub fn window_exists(title: &str) -> bool {
        find_window(title).is_some()
    }

    pub fn configure_main_window(title: &str) -> bool {
        let Some(window) = find_window(title) else {
            return false;
        };
        configure_window(&window);
        true
    }

    pub fn configure_settings_window(title: &str) -> bool {
        let Some(window) = find_window(title) else {
            return false;
        };
        configure_settings_window_style(&window);
        window.center();
        true
    }

    pub fn configure_tasks_window(title: &str) -> bool {
        let Some(window) = find_window(title) else {
            return false;
        };
        configure_tasks_window_style(&window);
        window.center();
        true
    }

    pub fn refresh_window_material(title: &str) -> bool {
        let Some(window) = find_window(title) else {
            return false;
        };
        super::material::install_window_material(
            &window,
            f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        )
    }

    pub fn position_main_window() -> bool {
        let Some(window) = find_window(crate::PRODUCT_NAME) else {
            return false;
        };
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        position_launcher_window(&window, marker, false);
        true
    }

    pub fn show_main_window() {
        let Some(marker) = MainThreadMarker::new() else {
            return;
        };
        let application = NSApplication::sharedApplication(marker);
        application.unhide(None);
        let Some(window) = find_window(crate::PRODUCT_NAME) else {
            tracing::warn!("launcher_window_missing_for_show");
            return;
        };
        let activation_generation = WINDOW_ACTIVATION_GENERATION
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        // Accessory applications can be activated programmatically while
        // remaining absent from the Dock. Keep that policy throughout the
        // invocation and only activate the panel that owns keyboard focus.
        position_launcher_window(&window, marker, true);
        let activation_requested = activate_launcher(&application, &window);
        tracing::debug!(
            visible = window.isVisible(),
            activation_policy = ?application.activationPolicy(),
            activation_requested,
            app_active = application.isActive(),
            "launcher_window_show_started"
        );
        tracing::debug!(
            visible = window.isVisible(),
            key = window.isKeyWindow(),
            can_become_key = window.canBecomeKeyWindow(),
            "launcher_window_show_finished"
        );
        if let Ok(delay) = dispatch2::DispatchTime::try_from(std::time::Duration::from_millis(60)) {
            let _ = dispatch2::DispatchQueue::main().after(delay, move || {
                if WINDOW_ACTIVATION_GENERATION.load(Ordering::Acquire) != activation_generation {
                    return;
                }
                let Some(marker) = MainThreadMarker::new() else {
                    return;
                };
                let Some(window) = find_window(crate::PRODUCT_NAME) else {
                    return;
                };
                let application = NSApplication::sharedApplication(marker);
                // A delayed AppKit activation may replace the responder again.
                // Repair only a window responder, preserving WebKit and IME owners.
                if window.isVisible() && window.isKeyWindow() {
                    let window_class = AnyClass::get(c"NSWindow");
                    let missing_input = window.firstResponder().is_none_or(|responder| {
                        window_class.is_some_and(|class| responder.isKindOfClass(class))
                    });
                    if missing_input {
                        restore_gpui_first_responder(&window);
                    }
                }
                tracing::debug!(
                    visible = window.isVisible(),
                    key = window.isKeyWindow(),
                    app_active = application.isActive(),
                    first_responder = ?window.firstResponder(),
                    "launcher_window_key_probe_60ms"
                );
                if window.isVisible() && !window.isKeyWindow() {
                    let activation_requested = activate_launcher(&application, &window);
                    tracing::debug!(
                        activation_requested,
                        app_active = application.isActive(),
                        key = window.isKeyWindow(),
                        can_become_key = window.canBecomeKeyWindow(),
                        "launcher_window_key_retry_finished"
                    );
                }
            });
        }
        if let Ok(delay) = dispatch2::DispatchTime::try_from(std::time::Duration::from_millis(280))
        {
            let _ = dispatch2::DispatchQueue::main().after(delay, move || {
                if WINDOW_ACTIVATION_GENERATION.load(Ordering::Acquire) != activation_generation {
                    return;
                }
                let Some(marker) = MainThreadMarker::new() else {
                    return;
                };
                let Some(window) = find_window(crate::PRODUCT_NAME) else {
                    return;
                };
                let application = NSApplication::sharedApplication(marker);
                // A delayed AppKit activation may replace the responder again.
                // Repair only a window responder, preserving WebKit and IME owners.
                if window.isVisible() && window.isKeyWindow() {
                    let window_class = AnyClass::get(c"NSWindow");
                    let missing_input = window.firstResponder().is_none_or(|responder| {
                        window_class.is_some_and(|class| responder.isKindOfClass(class))
                    });
                    if missing_input {
                        restore_gpui_first_responder(&window);
                    }
                }
                tracing::debug!(
                    visible = window.isVisible(),
                    key = window.isKeyWindow(),
                    app_active = application.isActive(),
                    first_responder = ?window.firstResponder(),
                    "launcher_window_key_probe_280ms"
                );
                if window.isVisible() && !window.isKeyWindow() {
                    let activation_requested = activate_launcher(&application, &window);
                    if window.isKeyWindow() {
                        tracing::debug!(
                            activation_requested,
                            app_active = application.isActive(),
                            key = true,
                            can_become_key = window.canBecomeKeyWindow(),
                            "launcher_window_key_activation_recovered"
                        );
                    } else {
                        tracing::warn!(
                            activation_requested,
                            app_active = application.isActive(),
                            key = false,
                            can_become_key = window.canBecomeKeyWindow(),
                            "launcher_window_key_activation_failed"
                        );
                    }
                }
            });
        }
    }

    pub fn hide_main_window() {
        WINDOW_ACTIVATION_GENERATION.fetch_add(1, Ordering::AcqRel);
        LAUNCHER_PLACEMENT.with(|placement| {
            *placement.borrow_mut() = None;
        });
        let Some(marker) = MainThreadMarker::new() else {
            return;
        };
        let application = NSApplication::sharedApplication(marker);
        if let Some(window) = find_window(crate::PRODUCT_NAME) {
            tracing::debug!(
                key = window.isKeyWindow(),
                app_active = application.isActive(),
                "launcher_window_hiding"
            );
            window.orderOut(None);
        }
        if application.activationPolicy() != NSApplicationActivationPolicy::Accessory {
            application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        }
        let auxiliary_visible = application
            .windows()
            .iter()
            .any(|window| window.isVisible() && window.title().to_string() != crate::PRODUCT_NAME);
        if application.isActive() && !auxiliary_visible {
            application.hide(None);
        }
    }

    pub fn launcher_should_dismiss_for_focus_loss() -> bool {
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        let _ = marker;
        let Some(window) = find_window(crate::PRODUCT_NAME) else {
            return false;
        };
        if !window.isVisible() {
            return false;
        }
        if window.attachedSheet().is_some()
            || NSApplication::sharedApplication(marker)
                .modalWindow()
                .is_some()
        {
            return false;
        }
        !window.isKeyWindow()
    }

    pub fn configure_clipboard_shelf_window(title: &str) -> bool {
        let Some(window) = find_window(title) else {
            return false;
        };
        configure_clipboard_shelf_window_style(&window);
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        position_clipboard_shelf_window_on_screen(&window, marker);
        true
    }

    pub fn position_clipboard_shelf_window() -> bool {
        let Some(marker) = MainThreadMarker::new() else {
            return false;
        };
        let Some(window) = find_window(crate::CLIPBOARD_SHELF_TITLE) else {
            return false;
        };
        position_clipboard_shelf_window_on_screen(&window, marker);
        true
    }

    pub fn clipboard_shelf_is_key_window() -> bool {
        find_window(crate::CLIPBOARD_SHELF_TITLE).is_some_and(|window| window.isKeyWindow())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_menu_shortcuts_use_macos_labels() {
        assert_eq!(
            macos_shortcut_label("meta+Space").as_deref(),
            Some("⌘Space")
        );
        assert_eq!(
            macos_shortcut_label("meta+alt+Space").as_deref(),
            Some("⌥⌘Space")
        );
        assert_eq!(
            macos_shortcut_label("shift+control+KeyK").as_deref(),
            Some("⌃⇧K")
        );
    }

    #[test]
    fn tasks_window_navigation_uses_native_key_codes() {
        assert_eq!(
            tasks_navigation_key(53, false),
            Some(TasksNavigationKey::Escape)
        );
        assert_eq!(
            tasks_navigation_key(126, false),
            Some(TasksNavigationKey::Up)
        );
        assert_eq!(
            tasks_navigation_key(125, false),
            Some(TasksNavigationKey::Down)
        );
        assert_eq!(
            tasks_navigation_key(36, false),
            Some(TasksNavigationKey::Enter)
        );
        assert_eq!(
            tasks_navigation_key(76, false),
            Some(TasksNavigationKey::Enter)
        );
        assert_eq!(tasks_navigation_key(0, false), None);
    }

    #[test]
    fn tasks_window_navigation_passes_through_shift_modified_keys() {
        for key_code in [53, 125, 126, 36, 76] {
            assert_eq!(tasks_navigation_key(key_code, true), None);
        }
    }

    #[test]
    fn status_menu_uses_the_configured_locale_and_shortcuts() {
        let preferences = crate::host::HostPreferences::default();
        let english = StatusMenuLabels::new(crate::l10n::Locale::En, &preferences);
        let chinese = StatusMenuLabels::new(crate::l10n::Locale::Zh, &preferences);

        assert_eq!(english.current_app, "Current App  ⌥⌘Space");
        assert_eq!(english.universal, "Universal Search  ⌘Space");
        assert_eq!(english.running_tasks, "Running Tasks…");
        assert_eq!(chinese.current_app, "当前应用  ⌥⌘Space");
        assert_eq!(chinese.running_tasks, "运行中的任务…");
        assert_eq!(chinese.settings, "设置…");
    }

    #[test]
    fn privacy_settings_use_stable_system_anchors() {
        assert!(PrivacySettingsPane::Accessibility
            .target()
            .ends_with("Privacy_Accessibility"));
        assert!(PrivacySettingsPane::Automation
            .target()
            .ends_with("Privacy_Automation"));
        assert!(PrivacySettingsPane::Clipboard
            .target()
            .ends_with("PrivacySecurity.extension"));
        assert!(PrivacySettingsPane::FullDiskAccess
            .target()
            .ends_with("Privacy_AllFiles"));
    }

    #[test]
    fn launcher_resizes_keep_the_initial_top_anchor() {
        let visible_bottom = 25.0;
        let anchor_top = launcher_anchor_top(visible_bottom, 1_000.0);
        assert_eq!(anchor_top, 825.0);

        for height in [72.0, 215.0, 402.5, 514.5, 520.0] {
            let origin_y = launcher_origin_y(visible_bottom, height, anchor_top);
            assert_eq!(origin_y + height, anchor_top);
        }
    }

    #[test]
    fn constrained_window_extent_keeps_tool_pages_inside_the_usable_screen() {
        let inset = crate::ui::design_tokens::metrics::SPACE_LG;
        assert_eq!(
            constrained_window_extent(800.0, 600.0, 920.0, 600.0, inset),
            (800.0 - inset * 2.0, 600.0 - inset * 2.0)
        );
        assert_eq!(
            constrained_window_extent(1_440.0, 875.0, 920.0, 600.0, inset),
            (920.0, 600.0)
        );
        let (width, height) = constrained_window_extent(4.0, 4.0, 920.0, 600.0, inset);
        assert!(width > 0.0 && width <= 4.0);
        assert!(height > 0.0 && height <= 4.0);
    }

    #[test]
    fn clipboard_shelf_uses_the_flutter_bottom_trailing_origin() {
        assert_eq!(
            bottom_trailing_origin(0.0, 25.0, 1_440.0, 875.0, 480.0),
            (944.0, 41.0)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launcher_uses_a_borderless_panel_without_a_system_title_frame() {
        use objc2_app_kit::NSWindowStyleMask;

        let style = macos::launcher_window_style_mask();
        assert!(style.contains(NSWindowStyleMask::Borderless));
        assert!(style.contains(NSWindowStyleMask::NonactivatingPanel));
        assert!(!style.contains(NSWindowStyleMask::Titled));
        assert!(!style.contains(NSWindowStyleMask::FullSizeContentView));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_launcher_and_clipboard_shelf_use_the_floating_window_layer() {
        use macos::{window_layer_policy, NativeSurface, WindowLayerPolicy};

        assert_eq!(
            window_layer_policy(NativeSurface::Launcher),
            WindowLayerPolicy::Spotlight
        );
        assert_eq!(
            window_layer_policy(NativeSurface::ClipboardShelf),
            WindowLayerPolicy::FloatingTool
        );
        assert_eq!(
            window_layer_policy(NativeSurface::Settings),
            WindowLayerPolicy::Normal
        );
        assert_eq!(
            window_layer_policy(NativeSurface::Tasks),
            WindowLayerPolicy::Normal
        );
        assert!(WindowLayerPolicy::Spotlight.uses_floating_level());
        assert!(WindowLayerPolicy::Spotlight.joins_all_spaces());
        assert!(WindowLayerPolicy::Spotlight.uses_panel_semantics());
        assert!(WindowLayerPolicy::FloatingTool.uses_floating_level());
        assert!(WindowLayerPolicy::FloatingTool.joins_all_spaces());
        assert!(WindowLayerPolicy::FloatingTool.uses_panel_semantics());
        assert!(!WindowLayerPolicy::Normal.uses_floating_level());
        assert!(!WindowLayerPolicy::Normal.joins_all_spaces());
        assert!(!WindowLayerPolicy::Normal.uses_panel_semantics());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn auxiliary_windows_keep_their_native_traffic_lights() {
        use objc2_app_kit::NSWindowStyleMask;

        for style in [
            macos::clipboard_shelf_window_style_mask(),
            macos::settings_window_style_mask(),
            macos::tasks_window_style_mask(),
        ] {
            assert!(style.contains(NSWindowStyleMask::Titled));
            assert!(style.contains(NSWindowStyleMask::Closable));
            assert!(style.contains(NSWindowStyleMask::Miniaturizable));
            assert!(style.contains(NSWindowStyleMask::FullSizeContentView));
            assert!(!style.contains(NSWindowStyleMask::NonactivatingPanel));
        }

        assert!(macos::clipboard_shelf_window_style_mask().contains(NSWindowStyleMask::Resizable));
        assert!(macos::tasks_window_style_mask().contains(NSWindowStyleMask::Resizable));
        assert!(!macos::settings_window_style_mask().contains(NSWindowStyleMask::Resizable));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launcher_activation_does_not_restore_unrelated_minimized_windows() {
        use objc2_app_kit::NSApplicationActivationOptions;

        let options = macos::launcher_activation_options();
        assert!(!options.contains(NSApplicationActivationOptions::ActivateAllWindows));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn inactive_application_uses_the_workspace_activation_fallback() {
        assert!(macos::should_activate_with_workspace(false));
        assert!(!macos::should_activate_with_workspace(true));
    }
}
