use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginItemState {
    Enabled,
    RequiresApproval,
    NotRegistered,
    NotFound,
}

impl LoginItemState {
    pub fn is_enabled(self) -> bool {
        self == Self::Enabled
    }
}

/// Reconciles the process-wide main-app login item with the persisted setting.
///
/// This is intentionally a small system adapter: settings persistence and UI
/// state stay in GPUI/Core. Registration can require user approval in System
/// Settings, so a desired state of `true` is not reported as enabled until the
/// system status confirms it.
pub fn apply(desired: bool) -> Result<LoginItemState> {
    crate::platform::apply_login_item(desired)
}

#[cfg(test)]
mod tests {
    use super::LoginItemState;

    #[test]
    fn only_enabled_counts_as_enabled() {
        assert!(LoginItemState::Enabled.is_enabled());
        assert!(!LoginItemState::RequiresApproval.is_enabled());
        assert!(!LoginItemState::NotRegistered.is_enabled());
        assert!(!LoginItemState::NotFound.is_enabled());
    }
}
