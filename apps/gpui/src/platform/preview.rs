use anyhow::{bail, Context, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewPoll {
    Open,
    Closed {
        resource_id: String,
        application_active: bool,
    },
}

#[cfg(target_os = "macos")]
pub struct PreviewHost {
    panel: quicklook::QuickLookPanel,
    active: Option<ActivePreview>,
}

#[cfg(target_os = "macos")]
struct ActivePreview {
    resource_id: String,
    owner_title: String,
}

#[cfg(target_os = "macos")]
impl PreviewHost {
    pub fn new() -> Result<Self> {
        let panel = quicklook::QuickLookPanel::shared().context("Quick Look is unavailable")?;
        Ok(Self {
            panel,
            active: None,
        })
    }

    pub fn show(
        &mut self,
        resource_id: String,
        paths: &[String],
        owner_title: &str,
    ) -> Result<Option<String>> {
        let marker =
            objc2::MainThreadMarker::new().context("Quick Look requires the main thread")?;
        let application = objc2_app_kit::NSApplication::sharedApplication(marker);
        if !application.isActive() {
            bail!("Quick Look requires the application to be active");
        }
        let owner = application
            .windows()
            .iter()
            .find(|window| window.title().to_string() == owner_title)
            .context("launcher window is unavailable")?;
        if !owner.isVisible() {
            bail!("launcher window is not visible");
        }
        let previous = self
            .active
            .as_ref()
            .map(|active| active.resource_id.clone());
        if previous.is_some() {
            self.close();
        }
        let items = paths
            .iter()
            .map(|path| {
                quicklook::PreviewItem::from_file_url(path, None)
                    .context("preview path is not valid Unicode")
            })
            .collect::<Result<Vec<_>>>()?;
        if items.is_empty() || items.len() > 256 {
            bail!("invalid Quick Look selection");
        }
        self.panel.set_items(items);
        self.panel.reload_if_dirty();
        self.panel.set_current_preview_item_index(0);
        self.panel.show();
        self.active = Some(ActivePreview {
            resource_id,
            owner_title: owner_title.to_owned(),
        });
        Ok(previous)
    }

    pub fn poll(&mut self) -> PreviewPoll {
        let Some(active) = self.active.as_ref() else {
            return PreviewPoll::Open;
        };
        let application_active = objc2::MainThreadMarker::new().is_some_and(|marker| {
            objc2_app_kit::NSApplication::sharedApplication(marker).isActive()
        });
        let closed = !self.panel.is_visible() || !application_active;
        if !closed {
            return PreviewPoll::Open;
        }
        let resource_id = active.resource_id.clone();
        PreviewPoll::Closed {
            resource_id,
            application_active,
        }
    }

    pub fn close(&mut self) -> Option<String> {
        let active = self.active.take()?;
        self.panel.hide();
        self.panel.set_items(Vec::new());
        if let Some(marker) = objc2::MainThreadMarker::new() {
            let application = objc2_app_kit::NSApplication::sharedApplication(marker);
            if application.isActive() {
                if let Some(owner) = application
                    .windows()
                    .iter()
                    .find(|window| window.title().to_string() == active.owner_title)
                {
                    if owner.isVisible() {
                        owner.makeKeyAndOrderFront(None);
                    }
                }
            }
        }
        tracing::debug!(resource_id = %active.resource_id, "quick_look_closed");
        Some(active.resource_id)
    }
}

#[cfg(target_os = "macos")]
pub(super) fn dismiss_quick_look() {
    let Some(marker) = objc2::MainThreadMarker::new() else {
        return;
    };
    // SAFETY: This queries the process-wide shared panel on the main thread.
    if !unsafe { objc2_quick_look_ui::QLPreviewPanel::sharedPreviewPanelExists(marker) } {
        return;
    }
    if let Some(panel) = unsafe { objc2_quick_look_ui::QLPreviewPanel::sharedPreviewPanel(marker) }
    {
        panel.orderOut(None);
    }
}

#[cfg(not(target_os = "macos"))]
pub struct PreviewHost;

#[cfg(not(target_os = "macos"))]
impl PreviewHost {
    pub fn new() -> Result<Self> {
        bail!("Quick Look is available on macOS only")
    }

    pub fn show(
        &mut self,
        _resource_id: String,
        _paths: &[String],
        _owner_title: &str,
    ) -> Result<Option<String>> {
        bail!("Quick Look is available on macOS only")
    }

    pub fn poll(&mut self) -> PreviewPoll {
        PreviewPoll::Open
    }

    pub fn close(&mut self) -> Option<String> {
        None
    }
}
