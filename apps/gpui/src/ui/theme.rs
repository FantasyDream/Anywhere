use gpui_kit::{rgb, rgba, Hsla};

use super::design_tokens::colors;

pub fn glass_surface() -> Hsla {
    rgba(colors::SURFACE).into()
}

pub fn window() -> Hsla {
    rgba(colors::WINDOW).into()
}

pub fn selected() -> Hsla {
    rgba(colors::SELECTED_SURFACE).into()
}

pub fn hover() -> Hsla {
    rgba(colors::HOVER).into()
}

pub fn field() -> Hsla {
    rgba(colors::FIELD).into()
}

pub fn border() -> Hsla {
    rgba(colors::BORDER).into()
}

pub fn divider() -> Hsla {
    rgba(colors::DIVIDER).into()
}

pub fn primary() -> Hsla {
    rgb(colors::PRIMARY & 0x00ff_ffff).into()
}

pub fn secondary() -> Hsla {
    rgb(colors::SECONDARY & 0x00ff_ffff).into()
}

pub fn muted() -> Hsla {
    rgb(colors::MUTED & 0x00ff_ffff).into()
}

pub fn accent() -> Hsla {
    rgb(colors::ACCENT & 0x00ff_ffff).into()
}

pub fn warning() -> Hsla {
    rgb(colors::WARNING & 0x00ff_ffff).into()
}

pub fn danger() -> Hsla {
    rgb(colors::DANGER & 0x00ff_ffff).into()
}

pub fn sidebar() -> Hsla {
    rgba(colors::SIDEBAR).into()
}

pub fn sidebar_selection() -> Hsla {
    rgba(colors::SIDEBAR_SELECTION).into()
}
