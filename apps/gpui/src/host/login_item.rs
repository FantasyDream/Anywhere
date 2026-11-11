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
    #[cfg(target_os = "macos")]
    {
        macos::apply(desired)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = desired;
        anyhow::bail!("Open at login is available on macOS only")
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::LoginItemState;
    use anyhow::{anyhow, bail, Result};
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    pub(super) fn apply(desired: bool) -> Result<LoginItemState> {
        // SAFETY: SMAppService is an application-service wrapper. Registration
        // and status reads are documented thread-safe and do not present UI.
        let service = unsafe { SMAppService::mainAppService() };
        let current = status(&service);

        if desired {
            if current == LoginItemState::Enabled {
                return Ok(current);
            }
            if current == LoginItemState::NotFound {
                bail!("Open at login is unavailable because the app service was not found");
            }
            // SAFETY: The service was obtained from the main application and
            // the call is scoped to this process.
            unsafe {
                service.registerAndReturnError().map_err(|error| {
                    anyhow!("Open at login failed: {}", error.localizedDescription())
                })?;
            }
        } else if matches!(
            current,
            LoginItemState::Enabled | LoginItemState::RequiresApproval
        ) {
            // A registration that requires approval is still registered. Remove
            // it when the user switches the preference off.
            // SAFETY: The service was obtained from the main application.
            unsafe {
                service.unregisterAndReturnError().map_err(|error| {
                    anyhow!(
                        "Open at login could not be disabled: {}",
                        error.localizedDescription()
                    )
                })?;
            }
        }

        let result = status(&service);
        if desired && result != LoginItemState::Enabled {
            if result == LoginItemState::RequiresApproval {
                // SAFETY: This only opens the system Login Items settings pane.
                unsafe { SMAppService::openSystemSettingsLoginItems() };
                bail!("Open at login requires approval in System Settings > General > Login Items");
            }
            bail!("Open at login was not confirmed");
        }
        if !desired
            && !matches!(
                result,
                LoginItemState::NotRegistered | LoginItemState::NotFound
            )
        {
            bail!("Open at login could not be disabled");
        }
        Ok(result)
    }

    fn status(service: &SMAppService) -> LoginItemState {
        // SAFETY: `status` is a synchronous read-only property.
        let status = unsafe { service.status() };
        if status == SMAppServiceStatus::Enabled {
            LoginItemState::Enabled
        } else if status == SMAppServiceStatus::RequiresApproval {
            LoginItemState::RequiresApproval
        } else if status == SMAppServiceStatus::NotRegistered {
            LoginItemState::NotRegistered
        } else {
            LoginItemState::NotFound
        }
    }
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
