use crate::platform::preview::PreviewPoll;
use anyhow::{bail, Context, Result};
use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

pub struct PreviewHost {
    shared: Rc<SharedPreviewPanel>,
    active: Option<Rc<ActivePreview>>,
}

struct SharedPreviewPanel {
    panel: quicklook::QuickLookPanel,
    ownership: PreviewOwnership,
}

thread_local! {
    static SHARED_PREVIEW: OnceCell<Rc<SharedPreviewPanel>> = const { OnceCell::new() };
}

#[derive(Default)]
struct PreviewOwnership(RefCell<Option<Rc<ActivePreview>>>);

impl PreviewOwnership {
    fn replace(&self, active: Rc<ActivePreview>) {
        self.0.replace(Some(active));
    }

    fn is_current(&self, active: &Rc<ActivePreview>) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, active))
    }

    fn clear(&self, active: &Rc<ActivePreview>) -> bool {
        if !self.is_current(active) {
            return false;
        }
        self.0.take();
        true
    }

    fn is_idle(&self) -> bool {
        self.0.borrow().is_none()
    }

    fn belongs_to(&self, owner_title: &str) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|active| active.owner_title == owner_title)
    }
}

struct ActivePreview {
    resource_id: String,
    owner_title: String,
}

impl PreviewHost {
    pub fn new() -> Result<Self> {
        let shared = SHARED_PREVIEW.with(|shared| {
            if let Some(shared) = shared.get() {
                return Ok::<_, anyhow::Error>(shared.clone());
            }
            let panel = quicklook::QuickLookPanel::shared().context("Quick Look is unavailable")?;
            Ok(shared
                .get_or_init(|| {
                    Rc::new(SharedPreviewPanel {
                        panel,
                        ownership: PreviewOwnership::default(),
                    })
                })
                .clone())
        })?;
        Ok(Self {
            shared,
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
            .find(|window| crate::platform::window_matches_title(window, owner_title))
            .context("launcher window is unavailable")?;
        if !owner.isVisible() {
            bail!("launcher window is not visible");
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
        let active = Rc::new(ActivePreview {
            resource_id,
            owner_title: owner_title.to_owned(),
        });
        let previous = self
            .active
            .replace(active.clone())
            .map(|active| active.resource_id.clone());
        self.shared.ownership.replace(active);
        let panel = &self.shared.panel;
        panel.set_items(items);
        panel.reload_if_dirty();
        panel.set_current_preview_item_index(0);
        panel.show();
        Ok(previous)
    }

    pub fn poll(&mut self) -> PreviewPoll {
        let Some(active) = self.active.as_ref() else {
            return PreviewPoll::Open;
        };
        let owns_panel = self.shared.ownership.is_current(active);
        let application_active = owns_panel
            && objc2::MainThreadMarker::new().is_some_and(|marker| {
                objc2_app_kit::NSApplication::sharedApplication(marker).isActive()
            });
        let closed = !application_active || !self.shared.panel.is_visible();
        if !closed {
            return PreviewPoll::Open;
        }
        let resource_id = active.resource_id.clone();
        PreviewPoll::Closed {
            resource_id,
            application_active,
        }
    }

    pub fn owns_panel(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| self.shared.ownership.is_current(active))
    }

    pub fn close(&mut self) -> Option<String> {
        let active = self.active.take()?;
        if !self.shared.ownership.clear(&active) {
            return Some(active.resource_id.clone());
        }
        self.shared.panel.set_items(Vec::new());
        self.shared.panel.hide();
        if let Some(marker) = objc2::MainThreadMarker::new() {
            let application = objc2_app_kit::NSApplication::sharedApplication(marker);
            if application.isActive() && self.shared.ownership.is_idle() {
                if let Some(owner) = application.windows().iter().find(|window| {
                    crate::platform::window_matches_title(window, &active.owner_title)
                }) {
                    if owner.isVisible() {
                        owner.makeKeyAndOrderFront(None);
                    }
                }
            }
        }
        tracing::debug!(resource_id = %active.resource_id, "quick_look_closed");
        Some(active.resource_id.clone())
    }
}

pub(crate) fn dismiss_quick_look(owner_title: &str) {
    SHARED_PREVIEW.with(|shared| {
        if let Some(shared) = shared.get() {
            if shared.ownership.belongs_to(owner_title) {
                shared.panel.hide();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_launcher_preview_cannot_clear_the_shelf_preview() {
        let ownership = PreviewOwnership::default();
        let launcher = Rc::new(ActivePreview {
            resource_id: "launcher-resource".into(),
            owner_title: crate::PRODUCT_NAME.into(),
        });
        let shelf = Rc::new(ActivePreview {
            resource_id: "shelf-resource".into(),
            owner_title: crate::CLIPBOARD_SHELF_TITLE.into(),
        });
        ownership.replace(launcher.clone());
        assert!(ownership.is_current(&launcher));
        ownership.replace(shelf.clone());
        assert!(!ownership.belongs_to(crate::PRODUCT_NAME));
        assert!(ownership.belongs_to(crate::CLIPBOARD_SHELF_TITLE));
        assert!(!ownership.clear(&launcher));
        assert!(ownership.is_current(&shelf));
        assert!(ownership.clear(&shelf));
        assert!(ownership.is_idle());
    }
}
