pub mod design_tokens;
pub mod theme;

use design_tokens::{colors, metrics};

pub const LAUNCHER_WIDTH: f32 = 648.0;
pub const SETTINGS_HEIGHT: f32 = 508.0;
pub const LAUNCHER_ROW_HEIGHT: f32 = 56.0;
pub const LAUNCHER_CORNER_RADIUS: f32 = 18.0;
pub const TOOL_WINDOW_WIDTH: f32 = 420.0;
pub const TOOL_WINDOW_HEIGHT: f32 = 560.0;
pub const TASKS_WINDOW_WIDTH: f32 = 920.0;
pub const TASKS_WINDOW_HEIGHT: f32 = 600.0;

pub fn argb(value: u32) -> u32 {
    value
}

pub fn default_surface() -> u32 {
    colors::SURFACE
}

pub fn body_size() -> f32 {
    metrics::BODY_SIZE
}
