use gpui_kit::{rgb, rgba, Hsla};

use super::{argb, design_tokens::colors};
use crate::platform::{current_window_appearance, uses_increase_contrast, WindowAppearance};

fn token(value: u32) -> Hsla {
    rgba(argb(value)).into()
}

fn rgb_token(value: u32) -> Hsla {
    rgb(value & 0x00ff_ffff).into()
}

fn with_contrast(value: u32, alpha: u8) -> Hsla {
    if uses_increase_contrast() {
        token((u32::from(alpha) << 24) | (value & 0x00ff_ffff))
    } else {
        token(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassTone {
    Light,
    Dark,
}

fn glass_tone_for(appearance: WindowAppearance, opaque_fallback: bool) -> GlassTone {
    if appearance == WindowAppearance::Light && !opaque_fallback {
        GlassTone::Light
    } else {
        GlassTone::Dark
    }
}

fn glass_tone() -> GlassTone {
    glass_tone_for(
        current_window_appearance(),
        crate::platform::uses_opaque_fallback(),
    )
}

pub fn glass_surface_is_light() -> bool {
    glass_tone() == GlassTone::Light
}

fn glass_color(light: u32, dark: u32) -> Hsla {
    rgb_token(match glass_tone() {
        GlassTone::Light => light,
        GlassTone::Dark => dark,
    })
}

fn adaptive_surface(light: u32, dark: u32) -> Hsla {
    token(match glass_tone() {
        GlassTone::Light => light,
        GlassTone::Dark => dark,
    })
}

pub fn glass_surface() -> Hsla {
    window()
}

pub fn fallback_surface() -> Hsla {
    token(colors::FALLBACK_SURFACE)
}

pub fn menu_surface() -> Hsla {
    token(colors::MENU_SURFACE)
}

pub fn modal_scrim() -> Hsla {
    token(colors::MODAL_SCRIM)
}

pub fn window() -> Hsla {
    let tone = glass_tone();
    with_contrast(
        match tone {
            GlassTone::Light => colors::GLASS_LIGHT_WINDOW,
            GlassTone::Dark => colors::WINDOW,
        },
        if tone == GlassTone::Light { 0x4d } else { 0x85 },
    )
}

pub fn selected() -> Hsla {
    adaptive_surface(
        colors::GLASS_LIGHT_SELECTED_SURFACE,
        colors::SELECTED_SURFACE,
    )
}

pub fn result_surface(selected: bool) -> Hsla {
    let value = match (selected, glass_tone()) {
        (true, GlassTone::Light) => colors::GLASS_LIGHT_RESULT_SURFACE_SELECTED,
        (false, GlassTone::Light) => colors::GLASS_LIGHT_RESULT_SURFACE,
        (true, GlassTone::Dark) => colors::RESULT_SURFACE_SELECTED,
        (false, GlassTone::Dark) => colors::RESULT_SURFACE,
    };
    token(value)
}

pub fn hover() -> Hsla {
    adaptive_surface(colors::GLASS_LIGHT_HOVER, colors::HOVER)
}

pub fn field() -> Hsla {
    adaptive_surface(colors::GLASS_LIGHT_FIELD, colors::FIELD)
}

pub fn card() -> Hsla {
    adaptive_surface(colors::GLASS_LIGHT_CARD, colors::CARD)
}

pub fn control_surface() -> Hsla {
    token(colors::CONTROL_SURFACE)
}

pub fn control_selection() -> Hsla {
    token(colors::CONTROL_SELECTION)
}

pub fn control_border() -> Hsla {
    let tone = glass_tone();
    with_contrast(
        match tone {
            GlassTone::Light => colors::GLASS_LIGHT_CONTROL_BORDER,
            GlassTone::Dark => colors::CONTROL_BORDER,
        },
        if tone == GlassTone::Light { 0x5c } else { 0x52 },
    )
}

pub fn on_dark_control_border() -> Hsla {
    with_contrast(colors::ON_DARK_CONTROL_BORDER, 0x52)
}

pub fn primary_button_surface() -> Hsla {
    token(colors::PRIMARY_BUTTON_SURFACE)
}

pub fn button_selection() -> Hsla {
    adaptive_surface(
        colors::GLASS_LIGHT_BUTTON_SELECTION,
        colors::BUTTON_SELECTION,
    )
}

pub fn text_selection() -> Hsla {
    token(match glass_tone() {
        GlassTone::Light => colors::GLASS_LIGHT_TEXT_SELECTION,
        GlassTone::Dark => colors::TEXT_SELECTION,
    })
}

pub fn transparent() -> Hsla {
    token(0x00000000)
}

pub fn border() -> Hsla {
    let tone = glass_tone();
    with_contrast(
        match tone {
            GlassTone::Light => colors::GLASS_LIGHT_BORDER,
            GlassTone::Dark => colors::BORDER,
        },
        0x66,
    )
}

pub fn selection_outline() -> Hsla {
    let tone = glass_tone();
    with_contrast(
        match tone {
            GlassTone::Light => colors::GLASS_LIGHT_SELECTION_OUTLINE,
            GlassTone::Dark => colors::SELECTION_OUTLINE,
        },
        if tone == GlassTone::Light { 0x8a } else { 0x68 },
    )
}

pub fn on_dark_selection_outline() -> Hsla {
    with_contrast(colors::ON_DARK_SELECTION_OUTLINE, 0x68)
}

pub fn on_dark_divider() -> Hsla {
    with_contrast(colors::ON_DARK_DIVIDER, 0x42)
}

pub fn divider() -> Hsla {
    let tone = glass_tone();
    with_contrast(
        match tone {
            GlassTone::Light => colors::GLASS_LIGHT_DIVIDER,
            GlassTone::Dark => colors::DIVIDER,
        },
        0x42,
    )
}

pub fn keycap_surface() -> Hsla {
    adaptive_surface(colors::GLASS_LIGHT_KEYCAP_SURFACE, colors::KEYCAP_SURFACE)
}

pub fn keycap_border() -> Hsla {
    adaptive_surface(colors::GLASS_LIGHT_KEYCAP_BORDER, colors::KEYCAP_BORDER)
}

pub fn primary() -> Hsla {
    glass_color(colors::GLASS_LIGHT_PRIMARY, colors::PRIMARY)
}

pub fn secondary() -> Hsla {
    glass_color(colors::GLASS_LIGHT_SECONDARY, colors::SECONDARY)
}

pub fn muted() -> Hsla {
    if uses_increase_contrast() {
        secondary()
    } else {
        glass_color(colors::GLASS_LIGHT_MUTED, colors::MUTED)
    }
}

pub fn accent() -> Hsla {
    if crate::platform::uses_opaque_fallback() {
        rgb_token(colors::ACCENT)
    } else {
        glass_color(colors::GLASS_LIGHT_ACCENT, colors::GLASS_DARK_ACCENT)
    }
}

pub fn accent_fill() -> Hsla {
    token(colors::ACCENT_FILL)
}

pub fn warning() -> Hsla {
    if crate::platform::uses_opaque_fallback() {
        rgb_token(colors::WARNING)
    } else {
        glass_color(colors::GLASS_LIGHT_WARNING, colors::GLASS_DARK_WARNING)
    }
}

pub fn danger() -> Hsla {
    if crate::platform::uses_opaque_fallback() {
        rgb_token(colors::DANGER)
    } else {
        glass_color(colors::GLASS_LIGHT_DANGER, colors::GLASS_DARK_DANGER)
    }
}

pub fn success() -> Hsla {
    if crate::platform::uses_opaque_fallback() {
        rgb_token(colors::SUCCESS)
    } else {
        glass_color(colors::GLASS_LIGHT_SUCCESS, colors::GLASS_DARK_SUCCESS)
    }
}

pub fn on_dark_primary() -> Hsla {
    rgb_token(colors::ON_DARK_PRIMARY)
}

pub fn on_dark_secondary() -> Hsla {
    rgb_token(colors::ON_DARK_SECONDARY)
}

pub fn on_dark_muted() -> Hsla {
    rgb_token(colors::ON_DARK_MUTED)
}

pub fn on_dark_accent() -> Hsla {
    rgb_token(colors::ON_DARK_ACCENT)
}

pub fn on_dark_warning() -> Hsla {
    rgb_token(colors::ON_DARK_WARNING)
}

pub fn on_dark_danger() -> Hsla {
    rgb_token(colors::ON_DARK_DANGER)
}

pub fn on_dark_success() -> Hsla {
    rgb_token(colors::ON_DARK_SUCCESS)
}

pub fn on_dark_hover() -> Hsla {
    token(colors::ON_DARK_HOVER)
}

pub fn on_dark_selected() -> Hsla {
    token(colors::ON_DARK_SELECTED_SURFACE)
}

pub fn icon_blue() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_BLUE, colors::ICON_BLUE)
}

pub fn icon_green() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_GREEN, colors::ICON_GREEN)
}

pub fn icon_coral() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_CORAL, colors::ICON_CORAL)
}

pub fn icon_purple() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_PURPLE, colors::ICON_PURPLE)
}

pub fn icon_amber() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_AMBER, colors::ICON_AMBER)
}

pub fn icon_pink() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_PINK, colors::ICON_PINK)
}

pub fn icon_neutral() -> Hsla {
    glass_color(colors::GLASS_LIGHT_ICON_NEUTRAL, colors::ICON_NEUTRAL)
}

pub fn icon_badge_ink() -> Hsla {
    rgb_token(colors::ICON_BADGE_INK)
}

pub fn sidebar() -> Hsla {
    adaptive_surface(colors::GLASS_LIGHT_SIDEBAR, colors::SIDEBAR)
}

pub fn sidebar_selection() -> Hsla {
    adaptive_surface(
        colors::GLASS_LIGHT_SIDEBAR_SELECTION,
        colors::SIDEBAR_SELECTION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_token_argb_colors_keep_their_alpha() {
        assert_eq!(selected(), rgba(0x18223028).into());
        assert_eq!(glass_surface(), rgba(0x16191e66).into());
        assert_eq!(field(), rgba(0x20232a80).into());
        assert_eq!(menu_surface(), rgba(0x2b2e35ff).into());
        assert_eq!(token(colors::CONTENT_SCRIM), rgba(0x30343800).into());
        assert_eq!(token(colors::PAGE_SCRIM), rgba(0x30343800).into());
        assert_eq!(result_surface(false), rgba(0x282c3300).into());
        assert_eq!(result_surface(true), rgba(0x18223028).into());
        assert_eq!(modal_scrim(), rgba(0x000000a8).into());
    }

    #[test]
    fn glass_light_palette_is_selected_only_for_light_transparent_material() {
        assert_eq!(
            glass_tone_for(WindowAppearance::Light, false),
            GlassTone::Light
        );
        for (appearance, opaque) in [
            (WindowAppearance::Light, true),
            (WindowAppearance::Dark, false),
            (WindowAppearance::Dark, true),
        ] {
            assert_eq!(glass_tone_for(appearance, opaque), GlassTone::Dark);
        }
    }

    #[test]
    fn opaque_and_menu_text_keeps_the_dark_surface_ink() {
        assert_eq!(on_dark_primary(), rgb(0xf9fafc).into());
        assert_eq!(on_dark_secondary(), rgb(0xedf0f5).into());
        assert_eq!(on_dark_muted(), rgb(0xdfe4ec).into());
        assert_eq!(accent(), rgb(0xa7caff).into());
        assert_eq!(warning(), rgb(0xffc66d).into());
        assert_eq!(danger(), rgb(0xff8078).into());
        assert_eq!(success(), rgb(0x73cc91).into());
    }
}
