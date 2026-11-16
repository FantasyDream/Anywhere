use crate::host::{WebDocument, WebPackageReview};
use anywhere_protocol::ui::DetailView;
use gpui_kit::Entity;
use gpui_webview::WebView;
use std::sync::{Arc, Mutex};

pub(super) struct WebIpcRequest {
    pub handle: String,
    pub origin: String,
    pub frame: String,
}

pub(super) struct ActiveWebBinding {
    pub handle: String,
    pub origin: String,
}

#[derive(Default)]
pub(super) struct WebSurfaceState {
    pub review: Option<WebPackageReview>,
    pub invocation_id: Option<String>,
    pub document: Option<WebDocument>,
    pub view: Option<Entity<WebView>>,
    pub ready: bool,
    pub error: Option<String>,
    pub actions_open: bool,
    pub action_selected: usize,
    pub state_handle: Option<String>,
    pub state_version: u64,
    pub binding: Option<Arc<Mutex<ActiveWebBinding>>>,
    pub resume_generation: u64,
    pub resume_busy: bool,
    pub plugin_detail: Option<DetailView>,
    pub unsupported_tool: Option<(String, String)>,
}
