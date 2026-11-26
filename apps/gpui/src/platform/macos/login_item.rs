use crate::host::LoginItemState;
use anyhow::{anyhow, bail, Result};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

pub(crate) fn apply(desired: bool) -> Result<LoginItemState> {
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
