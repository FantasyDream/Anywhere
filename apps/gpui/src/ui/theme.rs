use gpui_kit::{rgb, rgba, Hsla};

use super::{argb, design_tokens::colors};

fn token(value: u32) -> Hsla {
    rgba(argb(value)).into()
}

fn with_contrast(value: u32, alpha: u8) -> Hsla {
    if crate::platform::uses_increase_contrast() {
        token((u32::from(alpha) << 24) | (value & 0x00ff_ffff))
    } else {
        token(value)
    }
}

pub fn glass_surface() -> Hsla {
    token(colors::WINDOW)
}

pub fn fallback_surface() -> Hsla {
    token(colors::FALLBACK_SURFACE)
}

pub fn menu_surface() -> Hsla {
    token(colors::MENU_SURFACE)
}

pub fn content_scrim() -> Hsla {
    with_contrast(colors::CONTENT_SCRIM, 0xb3)
}

pub fn modal_scrim() -> Hsla {
    token(colors::MODAL_SCRIM)
}

pub fn window() -> Hsla {
    token(colors::WINDOW)
}

pub fn selected() -> Hsla {
    token(colors::SELECTED_SURFACE)
}

pub fn hover() -> Hsla {
    token(colors::HOVER)
}

pub fn field() -> Hsla {
    token(colors::FIELD)
}

pub fn card() -> Hsla {
    token(colors::CARD)
}

pub fn control_surface() -> Hsla {
    token(colors::CONTROL_SURFACE)
}

pub fn control_selection() -> Hsla {
    token(colors::CONTROL_SELECTION)
}

pub fn control_border() -> Hsla {
    token(colors::CONTROL_BORDER)
}

pub fn primary_button_surface() -> Hsla {
    token(colors::PRIMARY_BUTTON_SURFACE)
}

pub fn button_selection() -> Hsla {
    token(colors::BUTTON_SELECTION)
}

pub fn text_selection() -> Hsla {
    token(colors::TEXT_SELECTION)
}

pub fn transparent() -> Hsla {
    token(0x00000000)
}

pub fn border() -> Hsla {
    with_contrast(colors::BORDER, 0x66)
}

pub fn selection_outline() -> Hsla {
    with_contrast(colors::SELECTION_OUTLINE, 0x68)
}

pub fn divider() -> Hsla {
    with_contrast(colors::DIVIDER, 0x42)
}

pub fn keycap_surface() -> Hsla {
    token(colors::KEYCAP_SURFACE)
}

pub fn keycap_border() -> Hsla {
    token(colors::KEYCAP_BORDER)
}

pub fn primary() -> Hsla {
    rgb(colors::PRIMARY & 0x00ff_ffff).into()
}

pub fn secondary() -> Hsla {
    rgb(colors::SECONDARY & 0x00ff_ffff).into()
}

pub fn muted() -> Hsla {
    if crate::platform::uses_increase_contrast() {
        secondary()
    } else {
        rgb(colors::MUTED & 0x00ff_ffff).into()
    }
}

pub fn accent() -> Hsla {
    rgb(colors::ACCENT & 0x00ff_ffff).into()
}

pub fn accent_fill() -> Hsla {
    token(colors::ACCENT_FILL)
}

pub fn warning() -> Hsla {
    rgb(colors::WARNING & 0x00ff_ffff).into()
}

pub fn danger() -> Hsla {
    rgb(colors::DANGER & 0x00ff_ffff).into()
}

pub fn success() -> Hsla {
    rgb(colors::SUCCESS & 0x00ff_ffff).into()
}

pub fn icon_blue() -> Hsla {
    rgb(colors::ICON_BLUE & 0x00ff_ffff).into()
}

pub fn icon_green() -> Hsla {
    rgb(colors::ICON_GREEN & 0x00ff_ffff).into()
}

pub fn icon_coral() -> Hsla {
    rgb(colors::ICON_CORAL & 0x00ff_ffff).into()
}

pub fn icon_purple() -> Hsla {
    rgb(colors::ICON_PURPLE & 0x00ff_ffff).into()
}

pub fn icon_amber() -> Hsla {
    rgb(colors::ICON_AMBER & 0x00ff_ffff).into()
}

pub fn icon_pink() -> Hsla {
    rgb(colors::ICON_PINK & 0x00ff_ffff).into()
}

pub fn icon_neutral() -> Hsla {
    rgb(colors::ICON_NEUTRAL & 0x00ff_ffff).into()
}

pub fn icon_badge_ink() -> Hsla {
    rgb(colors::ICON_BADGE_INK & 0x00ff_ffff).into()
}

pub fn sidebar() -> Hsla {
    token(colors::SIDEBAR)
}

pub fn sidebar_selection() -> Hsla {
    token(colors::SIDEBAR_SELECTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_token_argb_colors_keep_their_alpha() {
        assert_eq!(selected(), rgba(0xffffff0c).into());
        assert_eq!(glass_surface(), rgba(0x30343800).into());
        assert_eq!(field(), rgba(0x20232a80).into());
        assert_eq!(menu_surface(), rgba(0x2b2e35ff).into());
        assert_eq!(content_scrim(), rgba(0x303438a0).into());
        assert_eq!(modal_scrim(), rgba(0x000000a8).into());
    }
}
