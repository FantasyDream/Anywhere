mod material;
mod preview;
#[cfg(windows)]
#[doc(hidden)]
pub mod windows;

pub(crate) use material::set_application_appearance;
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

#[cfg(any(target_os = "macos", test))]
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

#[cfg(any(target_os = "macos", test))]
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
    #[cfg(windows)]
    {
        windows::install_observers()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let (_sender, receiver) = tokio::sync::mpsc::channel(64);
        receiver
    }
}

/// Check minimum requirements for desktop product hosts.
pub fn require_supported_host() {
    #[cfg(target_os = "macos")]
    {
        let major = objc2_foundation::NSProcessInfo::processInfo()
            .operatingSystemVersion()
            .majorVersion as u32;
        assert!(
            major >= crate::MINIMUM_MACOS_MAJOR,
            "{} requires macOS {} or later",
            crate::PRODUCT_NAME,
            crate::MINIMUM_MACOS_MAJOR
        );
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        panic!("{} requires macOS or Windows", crate::PRODUCT_NAME);
    }
}

pub(crate) fn prepare_window_material() {
    material::prepare_window_material();
}

pub fn show_main_window() {
    #[cfg(target_os = "macos")]
    macos::show_main_window();
    #[cfg(windows)]
    windows::activate(crate::PRODUCT_NAME);
}

pub fn remember_clipboard_shelf_destination() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::remember_clipboard_shelf_destination()
    }
    #[cfg(windows)]
    {
        windows::remember_destination()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        false
    }
}

pub fn return_to_previous_application() -> ApplicationReturnResult {
    #[cfg(target_os = "macos")]
    {
        macos::return_to_previous_application()
    }
    #[cfg(windows)]
    {
        windows::return_to_previous_application()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
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
    preview::dismiss_quick_look(crate::PRODUCT_NAME);
    #[cfg(target_os = "macos")]
    macos::hide_main_window();
    #[cfg(windows)]
    windows::hide_main_window();
}

pub fn configure_main_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::configure_main_window(title)
    }
    #[cfg(windows)]
    {
        windows::window_exists(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = title;
        false
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
    #[cfg(windows)]
    {
        windows::window_exists(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
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
    #[cfg(windows)]
    {
        windows::activate(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
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
    #[cfg(windows)]
    {
        windows::activate(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = title;
        false
    }
}

/// Assign identity from the actual native view once, before deferred configuration.
pub fn bind_window_identity(window: &gpui_kit::Window, title: &str) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSUserInterfaceItemIdentification as _;
        use raw_window_handle::RawWindowHandle;
        let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // SAFETY: GPUI owns this live AppKit view for the borrowed window.
        let view = unsafe { &*handle.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
        if let (Some(native), Some(identity)) = (view.window(), window_identity(title)) {
            native.setIdentifier(Some(&objc2_foundation::NSString::from_str(identity)));
        }
    }
    #[cfg(windows)]
    windows::bind_window(window, title);
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = (window, title);
}

#[cfg(target_os = "macos")]
pub(in crate::platform) const TOOL_WINDOW_IDENTITY: &str = "dev.anywhere.window.tool";

#[cfg(target_os = "macos")]
fn window_identity(title: &str) -> Option<&'static str> {
    match title {
        crate::PRODUCT_NAME => Some("dev.anywhere.window.main"),
        crate::CLIPBOARD_SHELF_TITLE => Some("dev.anywhere.window.clipboard-shelf"),
        crate::TASKS_WINDOW_TITLE => Some("dev.anywhere.window.tasks"),
        _ => None,
    }
}

pub fn configure_tool_window(window: &gpui_kit::Window) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSUserInterfaceItemIdentification as _;
        use raw_window_handle::RawWindowHandle;
        let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // SAFETY: GPUI owns this live AppKit view for the borrowed window.
        let view = unsafe { &*handle.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
        if let Some(native) = view.window() {
            native.setIdentifier(Some(&objc2_foundation::NSString::from_str(
                TOOL_WINDOW_IDENTITY,
            )));
            let number = native.windowNumber();
            dispatch2::DispatchQueue::main().exec_async(move || {
                macos::configure_tool_window_number(number);
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

#[cfg(target_os = "macos")]
pub(in crate::platform) fn window_matches_title(
    window: &objc2_app_kit::NSWindow,
    title: &str,
) -> bool {
    use objc2_app_kit::NSUserInterfaceItemIdentification as _;
    if let Some(identity) = window_identity(title) {
        if let Some(current) = window.identifier() {
            return current.to_string() == identity;
        }
    }
    window.title().to_string() == title
}

#[cfg(target_os = "macos")]
pub(in crate::platform) fn is_auxiliary_product_window(window: &objc2_app_kit::NSWindow) -> bool {
    use objc2_app_kit::NSUserInterfaceItemIdentification as _;
    window.identifier().is_some_and(|current| {
        matches!(
            current.to_string().as_str(),
            "dev.anywhere.window.clipboard-shelf"
                | "dev.anywhere.window.tasks"
                | TOOL_WINDOW_IDENTITY
        )
    })
}

pub fn remember_clipboard_shelf_position() {
    #[cfg(target_os = "macos")]
    macos::remember_clipboard_shelf_position();
}

pub fn window_exists(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::window_exists(title)
    }
    #[cfg(windows)]
    {
        windows::window_exists(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
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
    #[cfg(windows)]
    {
        material::prepare_window_material();
        windows::window_exists(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = title;
        false
    }
}

pub fn refresh_tool_window_materials() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::refresh_tool_window_materials()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn refresh_all_window_materials() -> bool {
    material::prepare_window_material();
    let main = refresh_window_material(crate::PRODUCT_NAME);
    let shelf = refresh_window_material(crate::CLIPBOARD_SHELF_TITLE);
    let tasks = refresh_window_material(crate::TASKS_WINDOW_TITLE);
    let tools = refresh_tool_window_materials();
    main || shelf || tasks || tools
}

pub fn position_main_window() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::position_main_window()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
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
    #[cfg(windows)]
    {
        windows::should_dismiss()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        false
    }
}

pub fn configure_clipboard_shelf_window(title: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::configure_clipboard_shelf_window(title)
    }
    #[cfg(windows)]
    {
        windows::configure_shelf(title)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
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
    #[cfg(windows)]
    windows::configure_shelf(title);
    #[cfg(not(any(target_os = "macos", windows)))]
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
    #[cfg(windows)]
    tray: windows::Tray,
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
    pub fn update_labels(&self, labels: &StatusMenuLabels) -> bool {
        #[cfg(target_os = "macos")]
        {
            macos::update_status_menu(&self._menu, &self._target, labels);
            true
        }
        #[cfg(windows)]
        {
            self.tray.update_labels(labels)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = labels;
            false
        }
    }
}

pub fn install_status_item(
    actions: tokio::sync::mpsc::Sender<StatusItemAction>,
    labels: &StatusMenuLabels,
) -> Option<StatusItemHost> {
    #[cfg(target_os = "macos")]
    {
        macos::install_status_item(actions, labels)
    }
    #[cfg(windows)]
    {
        windows::Tray::install(actions, labels).map(|tray| StatusItemHost { tray })
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = actions;
        let _ = labels;
        None
    }
}

fn status_action_label(title: String, shortcut: &str) -> String {
    match shortcut_label(shortcut) {
        Some(shortcut) => format!("{title}  {shortcut}"),
        None => title,
    }
}

pub(crate) fn shortcut_label(shortcut: &str) -> Option<String> {
    #[cfg(windows)]
    {
        windows_shortcut_label(shortcut)
    }
    #[cfg(not(windows))]
    {
        macos_shortcut_label(shortcut)
    }
}

#[cfg(windows)]
fn windows_shortcut_label(shortcut: &str) -> Option<String> {
    use global_hotkey::hotkey::Modifiers;
    let hotkey = shortcut.parse::<global_hotkey::hotkey::HotKey>().ok()?;
    let mut label = String::new();
    for (modifier, name) in [
        (Modifiers::CONTROL, "Ctrl+"),
        (Modifiers::ALT, "Alt+"),
        (Modifiers::SHIFT, "Shift+"),
        (Modifiers::META, "Win+"),
    ] {
        if hotkey.mods.contains(modifier) {
            label.push_str(name);
        }
    }
    label.push_str(&status_key_label(hotkey.key));
    Some(label)
}

#[cfg(any(not(windows), test))]
fn macos_shortcut_label(shortcut: &str) -> Option<String> {
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

#[cfg(any(target_os = "macos", test))]
fn launcher_anchor_top(visible_bottom: f64, visible_height: f64) -> f64 {
    visible_bottom + visible_height * 0.8
}

#[cfg(any(target_os = "macos", test))]
fn launcher_origin_y(visible_bottom: f64, window_height: f64, anchor_top: f64) -> f64 {
    (anchor_top - window_height).max(visible_bottom)
}

#[cfg(any(target_os = "macos", test))]
fn bottom_trailing_origin(
    visible_x: f64,
    visible_y: f64,
    visible_width: f64,
    visible_height: f64,
    window_width: f64,
    window_height: f64,
) -> (f64, f64) {
    let inset = 16.0_f64.min((visible_width.min(visible_height) - 1.0).max(0.0) * 0.5);
    (
        (visible_x + visible_width - inset - window_width).max(visible_x),
        (visible_y + inset).min((visible_y + visible_height - window_height).max(visible_y)),
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
    #[cfg(windows)]
    {
        windows::shelf_is_active()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
impl Drop for StatusItemHost {
    fn drop(&mut self) {
        self.bar.removeStatusItem(&self.item);
    }
}

/// Keeps the exact system WebView dependencies visible to native integration
/// tests even when a test target only exercises the surrounding platform layer.
#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_launcher_labels_keep_the_windows_modifier_distinct() {
        assert_eq!(
            shortcut_label("control+alt+Space").as_deref(),
            Some("Ctrl+Alt+Space")
        );
        assert_eq!(
            shortcut_label("meta+shift+KeyK").as_deref(),
            Some("Shift+Win+K")
        );
    }

    #[test]
    fn status_menu_shortcuts_use_macos_labels() {
        assert_eq!(
            macos_shortcut_label("control+alt+Space").as_deref(),
            Some("⌃⌥Space")
        );
        assert_eq!(macos_shortcut_label("alt+Space").as_deref(), Some("⌥Space"));
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

        assert_eq!(english.current_app, "Current App  ⌃⌥Space");
        assert_eq!(english.universal, "Universal Search  ⌥Space");
        assert_eq!(english.running_tasks, "Running Tasks…");
        assert_eq!(chinese.current_app, "当前应用  ⌃⌥Space");
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
            bottom_trailing_origin(0.0, 25.0, 1_440.0, 875.0, 480.0, 520.),
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
    fn native_surfaces_keep_window_layers_and_use_regular_glass() {
        use macos::{window_layer_policy, NativeSurface, WindowLayerPolicy};
        use material::WindowGlassStyle;

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
        assert_eq!(
            window_layer_policy(NativeSurface::ToolWindow),
            WindowLayerPolicy::Normal
        );
        assert_eq!(
            NativeSurface::Launcher.glass_style(),
            WindowGlassStyle::Regular
        );
        for surface in [
            NativeSurface::ClipboardShelf,
            NativeSurface::Settings,
            NativeSurface::Tasks,
            NativeSurface::ToolWindow,
        ] {
            assert_eq!(surface.glass_style(), WindowGlassStyle::Regular);
        }
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
            macos::tool_window_style_mask(),
        ] {
            assert!(style.contains(NSWindowStyleMask::Titled));
            assert!(style.contains(NSWindowStyleMask::Closable));
            assert!(style.contains(NSWindowStyleMask::Miniaturizable));
            assert!(style.contains(NSWindowStyleMask::FullSizeContentView));
            assert!(!style.contains(NSWindowStyleMask::NonactivatingPanel));
        }

        assert!(macos::clipboard_shelf_window_style_mask().contains(NSWindowStyleMask::Resizable));
        assert!(macos::tasks_window_style_mask().contains(NSWindowStyleMask::Resizable));
        assert!(macos::tool_window_style_mask().contains(NSWindowStyleMask::Resizable));
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
