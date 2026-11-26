use super::*;

pub(in crate::platform) fn open_privacy_settings(
    pane: crate::platform::PrivacySettingsPane,
) -> bool {
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

pub(in crate::platform) fn find_window(title: &str) -> Option<Retained<NSWindow>> {
    let marker = MainThreadMarker::new()?;
    let application = NSApplication::sharedApplication(marker);
    application.windows().iter().find(|window| {
        use objc2_app_kit::NSUserInterfaceItemIdentification as _;
        crate::platform::window_identity(title).is_some_and(|identity| {
            window
                .identifier()
                .is_some_and(|current| current.to_string() == identity)
        })
    })
}

pub(in crate::platform) fn focus_gpui_view(
    window: &NSWindow,
    view: &NSView,
    class: &AnyClass,
) -> bool {
    if view.isKindOfClass(class) {
        return window.makeFirstResponder(Some(view));
    }
    view.subviews()
        .iter()
        .any(|subview| focus_gpui_view(window, &subview, class))
}

pub(in crate::platform) fn restore_gpui_first_responder(window: &NSWindow) -> bool {
    let Some(class) = AnyClass::get(c"GPUIView") else {
        return false;
    };
    let Some(content) = window.contentView() else {
        return false;
    };
    focus_gpui_view(window, &content, class)
}

pub(in crate::platform) fn launcher_window_style_mask() -> NSWindowStyleMask {
    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel
}

pub(in crate::platform) fn clipboard_shelf_window_style_mask() -> NSWindowStyleMask {
    NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::FullSizeContentView
}

pub(in crate::platform) fn settings_window_style_mask() -> NSWindowStyleMask {
    NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::FullSizeContentView
}

pub(in crate::platform) fn tasks_window_style_mask() -> NSWindowStyleMask {
    NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::FullSizeContentView
}

pub(in crate::platform) fn tool_window_style_mask() -> NSWindowStyleMask {
    tasks_window_style_mask()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform) enum NativeSurface {
    Launcher,
    ClipboardShelf,
    Settings,
    Tasks,
    ToolWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform) enum WindowLayerPolicy {
    Spotlight,
    FloatingTool,
    Normal,
}

impl WindowLayerPolicy {
    pub(in crate::platform) const fn uses_floating_level(self) -> bool {
        !matches!(self, Self::Normal)
    }

    pub(in crate::platform) const fn joins_all_spaces(self) -> bool {
        !matches!(self, Self::Normal)
    }

    pub(in crate::platform) const fn uses_panel_semantics(self) -> bool {
        !matches!(self, Self::Normal)
    }
}

impl NativeSurface {
    pub(in crate::platform) const fn glass_style(
        self,
    ) -> crate::platform::material::WindowGlassStyle {
        match self {
            Self::Launcher
            | Self::ClipboardShelf
            | Self::Settings
            | Self::Tasks
            | Self::ToolWindow => crate::platform::material::WindowGlassStyle::Regular,
        }
    }
}

pub(in crate::platform) const fn window_layer_policy(surface: NativeSurface) -> WindowLayerPolicy {
    match surface {
        NativeSurface::Launcher => WindowLayerPolicy::Spotlight,
        NativeSurface::ClipboardShelf => WindowLayerPolicy::FloatingTool,
        NativeSurface::Settings | NativeSurface::Tasks | NativeSurface::ToolWindow => {
            WindowLayerPolicy::Normal
        }
    }
}

pub(in crate::platform) fn apply_window_layer(window: &NSWindow, policy: WindowLayerPolicy) {
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

pub(in crate::platform) fn show_standard_button(
    window: &NSWindow,
    button: NSWindowButton,
    enabled: bool,
) {
    if let Some(button) = window.standardWindowButton(button) {
        button.setHidden(false);
        button.setEnabled(enabled);
    }
}

fn hide_standard_buttons(window: &NSWindow) {
    for button in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(button) {
            button.setHidden(true);
        }
    }
}

pub(in crate::platform) fn configure_window(window: &NSWindow) {
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
    hide_standard_buttons(window);
    apply_window_layer(window, window_layer_policy(NativeSurface::Launcher));
    window.setAnimationBehavior(objc2_app_kit::NSWindowAnimationBehavior::None);
    window.setMovableByWindowBackground(false);
    window.setHasShadow(true);
    // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    window.setAcceptsMouseMovedEvents(true);
    if !crate::platform::material::install_window_material(
        window,
        f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        NativeSurface::Launcher.glass_style(),
    ) {
        tracing::warn!("native_window_material_failed");
    }
    if !restore_gpui_first_responder(window) {
        tracing::warn!("native_launcher_first_responder_missing");
    }
}

pub(in crate::platform) fn configure_settings_window_style(window: &NSWindow) {
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
    window.setAnimationBehavior(objc2_app_kit::NSWindowAnimationBehavior::Default);
    window.setMovableByWindowBackground(false);
    window.setHasShadow(true);
    // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    window.setAcceptsMouseMovedEvents(true);
    if !crate::platform::material::install_window_material(
        window,
        f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        NativeSurface::Settings.glass_style(),
    ) {
        tracing::warn!("native_settings_material_failed");
    }
    if !restore_gpui_first_responder(window) {
        tracing::warn!("native_settings_first_responder_missing");
    }
}

pub(in crate::platform) fn configure_tasks_window_style(window: &NSWindow) {
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
    if !crate::platform::material::install_window_material(
        window,
        f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        NativeSurface::Tasks.glass_style(),
    ) {
        tracing::warn!("native_tasks_material_failed");
    }
    if !restore_gpui_first_responder(window) {
        tracing::warn!("native_tasks_first_responder_missing");
    }
}

pub(in crate::platform) fn configure_clipboard_shelf_window_style(window: &NSWindow) {
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
    window.setMovableByWindowBackground(true);
    window.setHasShadow(true);
    // Keep explicit close as the lifecycle owner: losing focus or being
    // covered by another application must not hide the Shelf.
    window.setHidesOnDeactivate(false);
    // SAFETY: this runs on the AppKit main thread and the window lives for the app lifetime.
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    window.setAcceptsMouseMovedEvents(true);
    if !crate::platform::material::install_window_material(
        window,
        f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        NativeSurface::ClipboardShelf.glass_style(),
    ) {
        tracing::warn!("native_clipboard_shelf_material_failed");
    }
    if !restore_gpui_first_responder(window) {
        tracing::warn!("native_clipboard_shelf_first_responder_missing");
    }
}

pub(in crate::platform) fn launcher_frame(
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
        crate::platform::launcher_anchor_top(visible.origin.y, visible.size.height)
    } else {
        LAUNCHER_PLACEMENT
            .with(|placement| {
                placement
                    .borrow()
                    .filter(|placement| placement.matches(visible))
                    .map(|placement| placement.anchor_top)
            })
            .unwrap_or_else(|| {
                crate::platform::launcher_anchor_top(visible.origin.y, visible.size.height)
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
        crate::platform::launcher_origin_y(visible.origin.y, size.height, anchor_top),
    );
    Some(NSRect::new(origin, size))
}

pub(in crate::platform) fn constrained_window_size(visible: NSRect, requested: NSSize) -> NSSize {
    let (width, height) = crate::platform::constrained_window_extent(
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

pub(in crate::platform) fn position_launcher_window(
    window: &NSWindow,
    marker: MainThreadMarker,
    use_pointer: bool,
) {
    let Some(frame) = launcher_frame(window, marker, use_pointer, window.frame().size) else {
        return;
    };
    window.setFrameOrigin(frame.origin);
}

pub(in crate::platform) fn resize_launcher_window_preserving_top(width: f32, height: f32) -> bool {
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
    let Some(frame) = launcher_frame(&window, marker, false, size) else {
        return false;
    };
    if (current.origin.x - frame.origin.x).abs() <= 0.5
        && (current.origin.y - frame.origin.y).abs() <= 0.5
        && (current.size.width - frame.size.width).abs() <= 0.5
        && (current.size.height - frame.size.height).abs() <= 0.5
    {
        return true;
    }
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
        width = frame.size.width,
        height = frame.size.height,
        x = frame.origin.x,
        y = frame.origin.y,
        "launcher_window_resized_atomically"
    );
    true
}

pub(in crate::platform) fn position_clipboard_shelf_window_on_screen(
    window: &NSWindow,
    marker: MainThreadMarker,
) {
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
    let (x, y) = crate::platform::bottom_trailing_origin(
        visible.origin.x,
        visible.origin.y,
        visible.size.width,
        visible.size.height,
        window.frame().size.width,
        window.frame().size.height,
    );
    let position = SHELF_POSITION
        .with(Cell::get)
        .unwrap_or(objc2_foundation::NSPoint::new(x, y));
    let size = window.frame().size;
    window.setFrameOrigin(objc2_foundation::NSPoint::new(
        position.x.clamp(
            visible.origin.x,
            (visible.origin.x + visible.size.width - size.width).max(visible.origin.x),
        ),
        position.y.clamp(
            visible.origin.y,
            (visible.origin.y + visible.size.height - size.height).max(visible.origin.y),
        ),
    ));
}

pub(in crate::platform) fn configure_main_window(title: &str) -> bool {
    let Some(window) = find_window(title) else {
        return false;
    };
    configure_window(&window);
    true
}

pub(in crate::platform) fn configure_settings_window(title: &str) -> bool {
    let Some(window) = find_window(title) else {
        return false;
    };
    configure_settings_window_style(&window);
    window.center();
    true
}

pub(in crate::platform) fn configure_tasks_window(title: &str) -> bool {
    let Some(window) = find_window(title) else {
        return false;
    };
    configure_tasks_window_style(&window);
    window.center();
    true
}

pub(in crate::platform) fn configure_tool_window_style(window: &NSWindow) {
    let style_mask = tool_window_style_mask();
    if window.styleMask() != style_mask {
        window.setStyleMask(style_mask);
    }
    window.setTitlebarAppearsTransparent(true);
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    show_standard_button(window, NSWindowButton::CloseButton, true);
    show_standard_button(window, NSWindowButton::MiniaturizeButton, true);
    show_standard_button(window, NSWindowButton::ZoomButton, true);
    apply_window_layer(window, window_layer_policy(NativeSurface::ToolWindow));
    window.setMovableByWindowBackground(false);
    window.setHasShadow(true);
    window.setAcceptsMouseMovedEvents(true);
    window.setMinSize(objc2_foundation::NSSize::new(
        f64::from(crate::ui::LAUNCHER_WIDTH),
        f64::from(crate::ui::TOOL_WINDOW_MIN_HEIGHT),
    ));
    if !crate::platform::material::install_window_material(
        window,
        f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        NativeSurface::ToolWindow.glass_style(),
    ) {
        tracing::warn!("native_tool_window_material_failed");
    }
    if !restore_gpui_first_responder(window) {
        tracing::warn!("native_tool_window_first_responder_missing");
    }
}

pub(in crate::platform) fn configure_tool_window_number(window_number: isize) {
    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let application = NSApplication::sharedApplication(marker);
    if let Some(window) = application
        .windows()
        .iter()
        .find(|window| window.windowNumber() == window_number)
    {
        configure_tool_window_style(&window);
    }
}

pub(in crate::platform) fn refresh_tool_window_materials() -> bool {
    let Some(marker) = MainThreadMarker::new() else {
        return false;
    };
    crate::platform::material::prepare_window_material();
    let application = NSApplication::sharedApplication(marker);
    let mut refreshed = false;
    for window in application.windows() {
        use objc2_app_kit::NSUserInterfaceItemIdentification as _;
        if window
            .identifier()
            .is_some_and(|current| current.to_string() == crate::platform::TOOL_WINDOW_IDENTITY)
        {
            refreshed |= crate::platform::material::install_window_material(
                &window,
                f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
                NativeSurface::ToolWindow.glass_style(),
            );
        }
    }
    refreshed
}

pub(in crate::platform) fn refresh_window_material(title: &str) -> bool {
    crate::platform::material::prepare_window_material();
    let Some(window) = find_window(title) else {
        return false;
    };
    crate::platform::material::install_window_material(
        &window,
        f64::from(crate::ui::LAUNCHER_CORNER_RADIUS),
        if title == crate::PRODUCT_NAME {
            NativeSurface::Launcher.glass_style()
        } else {
            crate::platform::material::WindowGlassStyle::Regular
        },
    )
}

pub(in crate::platform) fn position_main_window() -> bool {
    let Some(window) = find_window(crate::PRODUCT_NAME) else {
        return false;
    };
    let Some(marker) = MainThreadMarker::new() else {
        return false;
    };
    position_launcher_window(&window, marker, false);
    true
}

pub(in crate::platform) fn show_main_window() {
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
    if let Ok(delay) = dispatch2::DispatchTime::try_from(std::time::Duration::from_millis(280)) {
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

pub(in crate::platform) fn hide_main_window() {
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
        .any(|window| window.isVisible() && crate::platform::is_auxiliary_product_window(&window));
    if application.isActive() && !auxiliary_visible {
        application.hide(None);
    }
}

pub(in crate::platform) fn launcher_should_dismiss_for_focus_loss() -> bool {
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

pub(in crate::platform) fn configure_clipboard_shelf_window(title: &str) -> bool {
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

pub(in crate::platform) fn position_clipboard_shelf_window() -> bool {
    let Some(marker) = MainThreadMarker::new() else {
        return false;
    };
    let Some(window) = find_window(crate::CLIPBOARD_SHELF_TITLE) else {
        return false;
    };
    position_clipboard_shelf_window_on_screen(&window, marker);
    true
}

pub(in crate::platform) fn remember_clipboard_shelf_position() {
    if let Some(window) = find_window(crate::CLIPBOARD_SHELF_TITLE) {
        SHELF_POSITION.with(|position| position.set(Some(window.frame().origin)));
    }
}

pub(in crate::platform) fn clipboard_shelf_is_key_window() -> bool {
    find_window(crate::CLIPBOARD_SHELF_TITLE).is_some_and(|window| window.isKeyWindow())
}
