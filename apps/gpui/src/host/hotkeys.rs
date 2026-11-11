use crate::host::HostPreferences;
use anyhow::{Context, Result};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotKeyAction {
    CurrentApp,
    Universal,
}

/// Owns registered macOS global hotkeys for the process lifetime.
pub struct HotKeyController {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
    universal: Option<HotKey>,
}

impl HotKeyController {
    pub fn new(preferences: &HostPreferences) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("create global hotkey manager")?;
        let mut controller = Self {
            manager,
            current: None,
            universal: None,
        };
        if let Err(error) = controller.register(preferences) {
            tracing::warn!(%error, "global_hotkeys_unavailable");
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
        let previous_current = self.current;
        let previous_universal = self.universal;
        if let Some(hotkey) = previous_current {
            self.manager.unregister(hotkey)?;
        }
        if let Some(hotkey) = previous_universal {
            self.manager.unregister(hotkey)?;
        }
        if let Err(error) = self.manager.register(current) {
            restore_registrations(&self.manager, previous_current, previous_universal);
            self.current = previous_current;
            self.universal = previous_universal;
            return Err(error).context("register Current App shortcut");
        }
        if let Err(error) = self.manager.register(universal) {
            let _ = self.manager.unregister(current);
            restore_registrations(&self.manager, previous_current, previous_universal);
            self.current = previous_current;
            self.universal = previous_universal;
            return Err(error).context("register Universal Search shortcut");
        }
        self.current = Some(current);
        self.universal = Some(universal);
        tracing::debug!(
            current_shortcut = %preferences.current_app_shortcut,
            universal_shortcut = %preferences.universal_shortcut,
            "global_hotkeys_registered"
        );
        Ok(())
    }

    pub fn action_for_event(&self, event: GlobalHotKeyEvent) -> Option<HotKeyAction> {
        hotkey_action(
            event,
            self.current.as_ref().map(HotKey::id),
            self.universal.as_ref().map(HotKey::id),
        )
    }
}

fn restore_registrations(
    manager: &GlobalHotKeyManager,
    current: Option<HotKey>,
    universal: Option<HotKey>,
) {
    if let Some(current) = current {
        let _ = manager.register(current);
    }
    if let Some(universal) = universal {
        let _ = manager.register(universal);
    }
}

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
}
