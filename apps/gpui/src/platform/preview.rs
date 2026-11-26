#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewPoll {
    Open,
    Closed {
        resource_id: String,
        application_active: bool,
    },
}

#[cfg(target_os = "macos")]
pub use super::macos::preview::PreviewHost;

#[cfg(target_os = "macos")]
pub(super) use super::macos::preview::dismiss_quick_look;

#[cfg(windows)]
pub use super::windows::preview::PreviewHost;

#[cfg(not(any(target_os = "macos", windows)))]
pub struct PreviewHost;

#[cfg(not(any(target_os = "macos", windows)))]
impl PreviewHost {
    pub fn new() -> anyhow::Result<Self> {
        anyhow::bail!("system previews are unavailable on this platform")
    }

    pub fn show(
        &mut self,
        _resource_id: String,
        _paths: &[String],
        _owner_title: &str,
    ) -> anyhow::Result<Option<String>> {
        anyhow::bail!("system previews are unavailable on this platform")
    }

    pub fn poll(&mut self) -> PreviewPoll {
        PreviewPoll::Open
    }

    pub fn owns_panel(&self) -> bool {
        false
    }

    pub fn close(&mut self) -> Option<String> {
        None
    }
}
