/// Product-level tool surfaces remain inside the shared launcher window.
/// Host destinations implemented by the GPUI presentation layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolSurface {
    #[default]
    None,
    FileSearch,
    ClipboardHistory,
    ApplicationHistory,
    WebPlugin,
    RunningTasks,
}

impl ToolSurface {
    pub fn is_embedded(self) -> bool {
        !matches!(self, Self::None | Self::RunningTasks)
    }
}
