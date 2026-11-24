use super::*;

pub(in crate::platform) fn install_platform_observers() -> tokio::sync::mpsc::Receiver<PlatformEvent>
{
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

    let key_sender = sender;
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
                let marked: objc2::runtime::Bool = unsafe { msg_send![responder, hasMarkedText] };
                marked.as_bool()
            }
        });
        if composing {
            return event_ptr;
        }
        if crate::platform::window_matches_title(&window, crate::TASKS_WINDOW_TITLE) {
            let Some(key) = crate::platform::tasks_navigation_key(
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
        if event.keyCode() != 53
            || !crate::platform::window_matches_title(&window, crate::PRODUCT_NAME)
        {
            return event_ptr;
        }
        let web_responder = first_responder
            .as_ref()
            .is_some_and(|responder| responder.class().name().to_string_lossy().starts_with("WK"));
        if web_responder {
            // WebKit must deliver Escape to the focused control first. The
            // bootstrap forwards unhandled keys through document-bound IPC.
            return event_ptr;
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
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &key_monitor)
    } {
        PLATFORM_EVENT_MONITORS.with(|monitors| monitors.borrow_mut().push(monitor));
    } else {
        tracing::warn!("platform_key_monitor_unavailable");
    }
    receiver
}
