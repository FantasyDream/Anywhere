use crate::host::{ActionShortcutBinding, HostPreferences};
use anyhow::{Context, Result};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HotKeyAction {
    CurrentApp,
    Universal,
    ExecuteAction(ActionShortcutBinding),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegisteredActionShortcut {
    hotkey: HotKey,
    binding: ActionShortcutBinding,
}

trait HotKeyRegistry: Send {
    fn register(&self, hotkey: HotKey) -> Result<()>;
    fn unregister(&self, hotkey: HotKey) -> Result<()>;
    fn unregister_all(&self, hotkeys: &[HotKey]) -> Result<()>;
}

impl HotKeyRegistry for GlobalHotKeyManager {
    fn register(&self, hotkey: HotKey) -> Result<()> {
        GlobalHotKeyManager::register(self, hotkey).map_err(Into::into)
    }

    fn unregister(&self, hotkey: HotKey) -> Result<()> {
        GlobalHotKeyManager::unregister(self, hotkey).map_err(Into::into)
    }

    fn unregister_all(&self, hotkeys: &[HotKey]) -> Result<()> {
        GlobalHotKeyManager::unregister_all(self, hotkeys).map_err(Into::into)
    }
}

/// Owns registered macOS global hotkeys for the process lifetime.
pub struct HotKeyController {
    manager: Box<dyn HotKeyRegistry>,
    registration_error: Option<String>,
    current: Option<HotKey>,
    universal: Option<HotKey>,
    actions: Vec<RegisteredActionShortcut>,
}

impl HotKeyController {
    pub fn new(preferences: &HostPreferences) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("create global hotkey manager")?;
        Ok(Self::with_registry(Box::new(manager), preferences))
    }

    fn with_registry(manager: Box<dyn HotKeyRegistry>, preferences: &HostPreferences) -> Self {
        let mut controller = Self {
            manager,
            registration_error: None,
            current: None,
            universal: None,
            actions: Vec::new(),
        };
        if let Err(error) = controller.register(preferences) {
            tracing::warn!(error = ?error, "global_hotkeys_unavailable");
        }
        controller
    }

    pub fn registration_error(&self) -> Option<&str> {
        self.registration_error.as_deref()
    }

    pub fn register(&mut self, preferences: &HostPreferences) -> Result<()> {
        let result = self.register_inner(preferences);
        self.registration_error = result.as_ref().err().map(|error| format!("{error:#}"));
        result
    }

    fn register_inner(&mut self, preferences: &HostPreferences) -> Result<()> {
        preferences.validate()?;
        let current = preferences
            .current_app_shortcut
            .parse::<HotKey>()
            .context("parse Current App shortcut")?;
        let universal = preferences
            .universal_shortcut
            .parse::<HotKey>()
            .context("parse Universal Search shortcut")?;
        let actions = preferences
            .action_shortcuts
            .iter()
            .filter(|binding| binding.global)
            .map(|binding| {
                binding
                    .shortcut
                    .parse::<HotKey>()
                    .map(|hotkey| RegisteredActionShortcut {
                        hotkey,
                        binding: binding.clone(),
                    })
                    .with_context(|| format!("parse action shortcut {}", binding.action_id))
            })
            .collect::<Result<Vec<_>>>()?;
        if self.current == Some(current)
            && self.universal == Some(universal)
            && self.actions == actions
        {
            return Ok(());
        }
        let previous_current = self.current;
        let previous_universal = self.universal;
        let previous_actions = self.actions.clone();
        let previous_hotkeys = previous_current
            .iter()
            .chain(previous_universal.iter())
            .chain(previous_actions.iter().map(|action| &action.hotkey))
            .copied()
            .collect::<Vec<_>>();
        if let Err(error) = self.manager.unregister_all(&previous_hotkeys) {
            let rollback = restore_registrations(
                self.manager.as_ref(),
                previous_current,
                previous_universal,
                &previous_actions,
            );
            apply_registration_state(
                self,
                previous_current,
                previous_universal,
                previous_actions,
                rollback,
            );
            return Err(error).context(format!(
                "unregister previous shortcuts; {}",
                rollback.summary()
            ));
        }
        if let Err(error) = self.manager.register(current) {
            let rollback = restore_registrations(
                self.manager.as_ref(),
                previous_current,
                previous_universal,
                &previous_actions,
            );
            apply_registration_state(
                self,
                previous_current,
                previous_universal,
                previous_actions,
                rollback,
            );
            return Err(error).context(format!(
                "register Current App shortcut; {}",
                rollback.summary()
            ));
        }
        if let Err(error) = self.manager.register(universal) {
            let _ = self.manager.unregister(current);
            let rollback = restore_registrations(
                self.manager.as_ref(),
                previous_current,
                previous_universal,
                &previous_actions,
            );
            apply_registration_state(
                self,
                previous_current,
                previous_universal,
                previous_actions,
                rollback,
            );
            return Err(error).context(format!(
                "register Universal Search shortcut; {}",
                rollback.summary()
            ));
        }
        let mut registered_actions: Vec<RegisteredActionShortcut> = Vec::new();
        for action in actions {
            if let Err(error) = self.manager.register(action.hotkey) {
                let _ = self.manager.unregister(current);
                let _ = self.manager.unregister(universal);
                for registered in &registered_actions {
                    let _ = self.manager.unregister(registered.hotkey);
                }
                let rollback = restore_registrations(
                    self.manager.as_ref(),
                    previous_current,
                    previous_universal,
                    &previous_actions,
                );
                apply_registration_state(
                    self,
                    previous_current,
                    previous_universal,
                    previous_actions,
                    rollback,
                );
                return Err(error).context(format!(
                    "register action shortcut {}; {}",
                    action.binding.action_id,
                    rollback.summary()
                ));
            }
            registered_actions.push(action);
        }
        self.current = Some(current);
        self.universal = Some(universal);
        self.actions = registered_actions;
        tracing::debug!(
            current_shortcut = %preferences.current_app_shortcut,
            universal_shortcut = %preferences.universal_shortcut,
            action_shortcut_count = self.actions.len(),
            "global_hotkeys_registered"
        );
        Ok(())
    }

    pub fn action_for_event(&self, event: GlobalHotKeyEvent) -> Option<HotKeyAction> {
        if event.state() != HotKeyState::Pressed {
            return None;
        }
        if self.current.as_ref().map(HotKey::id) == Some(event.id()) {
            Some(HotKeyAction::CurrentApp)
        } else if self.universal.as_ref().map(HotKey::id) == Some(event.id()) {
            Some(HotKeyAction::Universal)
        } else {
            self.actions
                .iter()
                .find(|action| action.hotkey.id() == event.id())
                .map(|action| HotKeyAction::ExecuteAction(action.binding.clone()))
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RegistrationRollback {
    current_restored: bool,
    universal_restored: bool,
    actions_restored: usize,
    actions_expected: usize,
    errors: u8,
}

impl RegistrationRollback {
    fn summary(self) -> String {
        match self.errors {
            0 => "previous bindings restored".to_owned(),
            count => format!("{count} previous binding operation(s) failed during rollback"),
        }
    }
}

fn restore_registrations(
    manager: &dyn HotKeyRegistry,
    current: Option<HotKey>,
    universal: Option<HotKey>,
    actions: &[RegisteredActionShortcut],
) -> RegistrationRollback {
    let current_restored = current.is_none_or(|current| manager.register(current).is_ok());
    let universal_restored = universal.is_none_or(|universal| manager.register(universal).is_ok());
    let actions_restored = actions
        .iter()
        .filter(|action| manager.register(action.hotkey).is_ok())
        .count();
    RegistrationRollback {
        current_restored,
        universal_restored,
        actions_restored,
        actions_expected: actions.len(),
        errors: u8::from(!current_restored)
            + u8::from(!universal_restored)
            + u8::try_from(actions.len().saturating_sub(actions_restored)).unwrap_or(u8::MAX),
    }
}

fn apply_registration_state(
    controller: &mut HotKeyController,
    previous_current: Option<HotKey>,
    previous_universal: Option<HotKey>,
    previous_actions: Vec<RegisteredActionShortcut>,
    rollback: RegistrationRollback,
) {
    controller.current = previous_current.filter(|_| rollback.current_restored);
    controller.universal = previous_universal.filter(|_| rollback.universal_restored);
    controller.actions = if rollback.actions_restored == rollback.actions_expected {
        previous_actions
    } else {
        Vec::new()
    };
}

fn effective_shortcuts(controller: &HotKeyController) -> (Option<String>, Option<String>) {
    (
        controller.current.as_ref().map(format_hotkey),
        controller.universal.as_ref().map(format_hotkey),
    )
}

/// Status hints reflect registrations owned by this process, never desired keys.
pub fn registered_preferences(
    preferences: &HostPreferences,
    controller: Option<&HotKeyController>,
) -> HostPreferences {
    let mut effective = preferences.clone();
    let (current, universal) = controller.map(effective_shortcuts).unwrap_or_default();
    effective.current_app_shortcut = current.unwrap_or_default();
    effective.universal_shortcut = universal.unwrap_or_default();
    effective
}

fn format_hotkey(hotkey: &HotKey) -> String {
    let mut labels = Vec::new();
    if hotkey
        .mods
        .contains(global_hotkey::hotkey::Modifiers::CONTROL)
    {
        labels.push("control");
    }
    if hotkey.mods.contains(global_hotkey::hotkey::Modifiers::ALT) {
        labels.push("alt");
    }
    if hotkey
        .mods
        .contains(global_hotkey::hotkey::Modifiers::SHIFT)
    {
        labels.push("shift");
    }
    if hotkey.mods.contains(global_hotkey::hotkey::Modifiers::META) {
        labels.push("meta");
    }
    let mut value = labels.join("+");
    if !value.is_empty() {
        value.push('+');
    }
    value.push_str(&format!("{:?}", hotkey.key));
    value
}

#[cfg(test)]
fn hotkey_action(
    event: GlobalHotKeyEvent,
    current: Option<u32>,
    universal: Option<u32>,
) -> Option<HotKeyAction> {
    if event.state() != HotKeyState::Pressed {
        return None;
    }
    if current == Some(event.id()) {
        Some(HotKeyAction::CurrentApp)
    } else if universal == Some(event.id()) {
        Some(HotKeyAction::Universal)
    } else {
        None
    }
}

impl Drop for HotKeyController {
    fn drop(&mut self) {
        let _ = self.manager.unregister_all(
            &self
                .current
                .iter()
                .chain(self.universal.iter())
                .chain(self.actions.iter().map(|action| &action.hotkey))
                .copied()
                .collect::<Vec<_>>(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::hotkey::{Code, Modifiers};

    #[test]
    fn pressed_registered_ids_map_to_product_modes() {
        let current = HotKey::new(Some(Modifiers::ALT | Modifiers::META), Code::Space);
        let universal = HotKey::new(Some(Modifiers::META), Code::Space);
        assert_eq!(
            hotkey_action(
                GlobalHotKeyEvent {
                    id: current.id(),
                    state: HotKeyState::Pressed,
                },
                Some(current.id()),
                Some(universal.id())
            ),
            Some(HotKeyAction::CurrentApp)
        );
        assert_eq!(
            hotkey_action(
                GlobalHotKeyEvent {
                    id: universal.id(),
                    state: HotKeyState::Pressed,
                },
                Some(current.id()),
                Some(universal.id())
            ),
            Some(HotKeyAction::Universal)
        );
        assert_eq!(
            hotkey_action(
                GlobalHotKeyEvent {
                    id: current.id(),
                    state: HotKeyState::Released,
                },
                Some(current.id()),
                Some(universal.id())
            ),
            None
        );
    }

    #[test]
    fn effective_hotkeys_round_trip_through_parser_labels() {
        let hotkey = "control+alt+shift+KeyK".parse::<HotKey>().unwrap();
        assert_eq!(format_hotkey(&hotkey), "control+alt+shift+KeyK");
    }
    #[derive(Clone, Default)]
    struct MockRegistry(std::sync::Arc<std::sync::Mutex<RegistryState>>);

    #[derive(Default)]
    struct RegistryState {
        blocked: std::collections::HashSet<u32>,
        owned: std::collections::HashSet<u32>,
    }

    impl HotKeyRegistry for MockRegistry {
        fn register(&self, key: HotKey) -> Result<()> {
            let mut state = self.0.lock().unwrap();
            anyhow::ensure!(!state.blocked.contains(&key.id()), "shortcut is occupied");
            anyhow::ensure!(state.owned.insert(key.id()), "shortcut already owned");
            Ok(())
        }
        fn unregister(&self, key: HotKey) -> Result<()> {
            self.0.lock().unwrap().owned.remove(&key.id());
            Ok(())
        }
        fn unregister_all(&self, keys: &[HotKey]) -> Result<()> {
            for key in keys {
                self.unregister(*key)?;
            }
            Ok(())
        }
    }

    fn pressed(key: &str) -> GlobalHotKeyEvent {
        GlobalHotKeyEvent {
            id: key.parse::<HotKey>().unwrap().id(),
            state: HotKeyState::Pressed,
        }
    }

    #[test]
    fn startup_conflict_is_reported_and_retry_recovers_the_controller() {
        let preferences = HostPreferences::default();
        let registry = MockRegistry::default();
        registry
            .0
            .lock()
            .unwrap()
            .blocked
            .insert(pressed(&preferences.current_app_shortcut).id());
        let mut controller =
            HotKeyController::with_registry(Box::new(registry.clone()), &preferences);
        assert!(controller
            .registration_error()
            .unwrap()
            .contains("register Current App shortcut"));
        let effective = registered_preferences(&preferences, Some(&controller));
        assert!(effective.current_app_shortcut.is_empty());
        assert!(effective.universal_shortcut.is_empty());
        assert_eq!(
            controller.action_for_event(pressed(&preferences.current_app_shortcut)),
            None
        );
        registry.0.lock().unwrap().blocked.clear();
        controller.register(&preferences).unwrap();
        assert!(controller.registration_error().is_none());
        assert_eq!(
            controller.action_for_event(pressed(&preferences.current_app_shortcut)),
            Some(HotKeyAction::CurrentApp)
        );
        assert!(!registered_preferences(&preferences, Some(&controller))
            .universal_shortcut
            .is_empty());
    }

    #[test]
    fn universal_conflict_cleans_up_the_new_current_app_registration() {
        let preferences = HostPreferences::default();
        let registry = MockRegistry::default();
        registry
            .0
            .lock()
            .unwrap()
            .blocked
            .insert(pressed(&preferences.universal_shortcut).id());
        let controller = HotKeyController::with_registry(Box::new(registry.clone()), &preferences);
        assert!(controller
            .registration_error()
            .unwrap()
            .contains("register Universal Search shortcut"));
        assert!(registry.0.lock().unwrap().owned.is_empty());
        assert_eq!(effective_shortcuts(&controller), (None, None));
    }

    #[test]
    fn a_failed_change_preserves_actual_status_hints_until_retry_succeeds() {
        let previous = HostPreferences::default();
        let registry = MockRegistry::default();
        let mut controller = HotKeyController::with_registry(Box::new(registry.clone()), &previous);
        let desired = HostPreferences {
            current_app_shortcut: "control+alt+shift+KeyO".into(),
            universal_shortcut: "control+alt+shift+KeyP".into(),
            ..previous.clone()
        };
        registry
            .0
            .lock()
            .unwrap()
            .blocked
            .insert(pressed(&desired.universal_shortcut).id());
        assert!(controller.register(&desired).is_err());
        assert_eq!(
            controller.action_for_event(pressed(&previous.current_app_shortcut)),
            Some(HotKeyAction::CurrentApp)
        );
        assert_eq!(
            controller.action_for_event(pressed(&desired.current_app_shortcut)),
            None
        );
        let effective = registered_preferences(&desired, Some(&controller));
        assert_eq!(
            pressed(&effective.current_app_shortcut).id(),
            pressed(&previous.current_app_shortcut).id()
        );
        assert_eq!(
            pressed(&effective.universal_shortcut).id(),
            pressed(&previous.universal_shortcut).id()
        );
        registry.0.lock().unwrap().blocked.clear();
        controller.register(&desired).unwrap();
        assert!(controller.registration_error().is_none());
        assert_eq!(
            controller.action_for_event(pressed(&desired.universal_shortcut)),
            Some(HotKeyAction::Universal)
        );
    }

    #[test]
    fn an_unavailable_manager_never_advertises_desired_shortcuts() {
        let effective = registered_preferences(&HostPreferences::default(), None);
        assert!(effective.current_app_shortcut.is_empty());
        assert!(effective.universal_shortcut.is_empty());
        for locale in [crate::l10n::Locale::En, crate::l10n::Locale::Zh] {
            let labels = crate::platform::StatusMenuLabels::new(locale, &effective);
            assert!(!format!("{labels:?}").contains("⌘"));
        }
    }
}
