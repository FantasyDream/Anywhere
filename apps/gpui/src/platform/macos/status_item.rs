use super::*;

pub(in crate::platform) struct StatusItemTargetIvars {
    actions: tokio::sync::mpsc::Sender<StatusItemAction>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = StatusItemTargetIvars]
    pub(in crate::platform) struct StatusItemTarget;

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

pub(super) fn menu_item(
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

pub(in crate::platform) fn install_status_item(
    actions: tokio::sync::mpsc::Sender<StatusItemAction>,
    labels: &StatusMenuLabels,
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
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(crate::PRODUCT_NAME));
    menu.setAutoenablesItems(false);
    populate_menu(mtm, &menu, &target, labels);
    item.setMenu(Some(&menu));
    Some(StatusItemHost {
        bar,
        item,
        _menu: menu,
        _target: target,
    })
}

pub(super) fn populate_menu(
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

pub(in crate::platform) fn update_status_menu(
    menu: &NSMenu,
    target: &StatusItemTarget,
    labels: &StatusMenuLabels,
) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    populate_menu(mtm, menu, target, labels);
}
