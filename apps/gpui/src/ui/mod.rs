pub mod design_tokens;
pub mod styles;
pub mod theme;

use design_tokens::metrics;
use std::borrow::Cow;

pub const LAUNCHER_WIDTH: f32 = 648.0;
pub const SETTINGS_HEIGHT: f32 = 508.0;
pub const LAUNCHER_ROW_HEIGHT: f32 = metrics::ROW_HEIGHT;
pub const LAUNCHER_CORNER_RADIUS: f32 = metrics::RADIUS;
pub const TASKS_WINDOW_WIDTH: f32 = 920.0;
pub const TASKS_WINDOW_HEIGHT: f32 = 600.0;
pub const TOOL_WINDOW_MIN_HEIGHT: f32 = 360.0;

/// Register the same text faces before GPUI measures any product content.
pub(crate) fn load_fonts(cx: &gpui_kit::App) {
    let fonts: [&'static [u8]; 5] = [
        include_bytes!("../../../../resources/fonts/NotoSansSC-Regular.ttf"),
        include_bytes!("../../../../resources/fonts/NotoSansSC-Medium.ttf"),
        include_bytes!("../../../../resources/fonts/NotoSansSC-SemiBold.ttf"),
        include_bytes!("../../../../resources/fonts/NotoSansSC-Bold.ttf"),
        include_bytes!("../../../../resources/fonts/FiraMono-Regular.ttf"),
    ];
    if let Err(error) = cx
        .text_system()
        .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
    {
        tracing::warn!(%error, "product_fonts_load_failed");
    }
}

/// Convert Flutter's ARGB token format to GPUI's packed RGBA format.
pub fn argb(value: u32) -> u32 {
    ((value & 0x00ff_ffff) << 8) | (value >> 24)
}
