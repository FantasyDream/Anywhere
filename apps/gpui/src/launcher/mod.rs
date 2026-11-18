mod components;
mod file_icons;
mod icon_cache;
mod model;
mod pages;
mod presentation;
mod snapshot_gate;
mod tool_windows;
pub(crate) use tool_windows::{ClipboardShelfView, TasksWindowView};
mod view;

pub use model::{LauncherApply, LauncherModel, LauncherRow, LauncherScope, LauncherWorkspace};
pub use pages::{
    AppHistoryPageModel, ClipboardPageModel, FileSearchPageModel, TasksPageModel, WebPageModel,
};
pub(crate) use presentation::locale_for_language;
pub use snapshot_gate::SnapshotGate;
pub use view::{launcher_window_size, LauncherEvent, LauncherView};
