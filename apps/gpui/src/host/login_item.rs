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
    #[cfg(windows)]
    {
        windows::apply(desired)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = desired;
        anyhow::bail!("Open at login is unavailable on this platform")
    }
}

#[cfg(windows)]
mod windows {
    use super::LoginItemState;
    use anyhow::{ensure, Context, Result};
    use windows_registry::CURRENT_USER;

    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Anywhere";

    pub(super) fn apply(desired: bool) -> Result<LoginItemState> {
        if !desired {
            match CURRENT_USER
                .options()
                .write()
                .open(KEY)
                .and_then(|key| key.remove_value(NAME))
            {
                Ok(()) => {}
                Err(error) if error.code().0 == 0x8007_0002_u32 as i32 => {}
                Err(error) => return Err(error.into()),
            }
            return Ok(LoginItemState::NotRegistered);
        }
        let executable = std::env::current_exe()?;
        let executable = executable
            .to_str()
            .context("executable path is not Unicode")?;
        let command = format!("\"{executable}\" --background");
        ensure!(
            command.encode_utf16().count() <= 260,
            "Windows startup command is too long"
        );
        CURRENT_USER.create(KEY)?.set_string(NAME, command)?;
        Ok(LoginItemState::Enabled)
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
            if current == LoginItemState::NotRegistered {
                // SAFETY: The service was obtained from the main application and
                // the call is scoped to this process.
                unsafe {
                    service.registerAndReturnError().map_err(|error| {
                        anyhow!("Open at login failed: {}", error.localizedDescription())
                    })?;
                }
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
                if objc2::MainThreadMarker::new().is_some() {
                    // SAFETY: This only opens the system Login Items settings pane on the main thread.
                    unsafe { SMAppService::openSystemSettingsLoginItems() };
                } else {
                    dispatch2::DispatchQueue::main().exec_async(|| {
                        // SAFETY: Dispatched to the AppKit main queue.
                        unsafe { SMAppService::openSystemSettingsLoginItems() };
                    });
                }
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
