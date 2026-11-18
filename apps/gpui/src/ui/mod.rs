pub mod design_tokens;
pub mod styles;
pub mod theme;

use design_tokens::metrics;

pub const LAUNCHER_WIDTH: f32 = 648.0;
pub const SETTINGS_HEIGHT: f32 = 508.0;
pub const LAUNCHER_ROW_HEIGHT: f32 = 56.0;
pub const LAUNCHER_CORNER_RADIUS: f32 = metrics::RADIUS;
pub const TASKS_WINDOW_WIDTH: f32 = 920.0;
pub const TASKS_WINDOW_HEIGHT: f32 = 600.0;

/// Convert Flutter's ARGB token format to GPUI's packed RGBA format.
pub fn argb(value: u32) -> u32 {
    ((value & 0x00ff_ffff) << 8) | (value >> 24)
}
