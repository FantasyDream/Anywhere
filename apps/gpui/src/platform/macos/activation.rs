use super::*;

pub(super) fn activate_launcher(application: &NSApplication, window: &NSWindow) -> bool {
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

pub(super) fn queue_workspace_activation(running_application: &NSRunningApplication) -> bool {
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

pub(super) fn schedule_activation_key_recovery(title: &str) {
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

pub(in crate::platform) fn should_activate_with_workspace(application_is_active: bool) -> bool {
    !application_is_active
}

pub(in crate::platform) fn launcher_activation_options() -> NSApplicationActivationOptions {
    NSApplicationActivationOptions::empty()
}

pub(in crate::platform) fn activate_window(title: &str) -> bool {
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

pub(in crate::platform) fn window_exists(title: &str) -> bool {
    find_window(title).is_some()
}
