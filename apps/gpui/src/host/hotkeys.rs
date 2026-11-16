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

/// Owns registered macOS global hotkeys for the process lifetime.
pub struct HotKeyController {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
    universal: Option<HotKey>,
    actions: Vec<RegisteredActionShortcut>,
}

impl HotKeyController {
    pub fn new(preferences: &HostPreferences) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("create global hotkey manager")?;
        let mut controller = Self {
            manager,
            current: None,
            universal: None,
            actions: Vec::new(),
        };
        if let Err(error) = controller.register(preferences) {
            tracing::warn!(error = ?error, "global_hotkeys_unavailable");
        }
        Ok(controller)
    }

    pub fn register(&mut self, preferences: &HostPreferences) -> Result<()> {
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
                &self.manager,
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
                &self.manager,
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
                &self.manager,
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
                    &self.manager,
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
    manager: &GlobalHotKeyManager,
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

pub fn effective_shortcuts(controller: &HotKeyController) -> (Option<String>, Option<String>) {
    (
        controller.current.as_ref().map(format_hotkey),
        controller.universal.as_ref().map(format_hotkey),
    )
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
}
