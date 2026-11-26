use gpui_kit::{rgba, Hsla};

use super::{
    argb,
    design_tokens::{
        colors::{self, FixedColors},
        Palette,
    },
};
use crate::platform::{current_window_appearance, uses_increase_contrast, WindowAppearance};

fn token(value: u32) -> Hsla {
    rgba(argb(value)).into()
}

pub fn surface_is_light() -> bool {
    current_window_appearance() == WindowAppearance::Light
}

pub fn palette_for(
    appearance: WindowAppearance,
    opaque_fallback: bool,
    increase_contrast: bool,
) -> &'static Palette {
    match (appearance, opaque_fallback, increase_contrast) {
        (WindowAppearance::Light, _, false) => &colors::GLASS_LIGHT,
        (WindowAppearance::Light, _, true) => &colors::GLASS_LIGHT_CONTRAST,
        (WindowAppearance::Dark, true, false) => &colors::OPAQUE_DARK,
        (WindowAppearance::Dark, true, true) => &colors::OPAQUE_DARK_CONTRAST,
        (WindowAppearance::Dark, false, false) => &colors::GLASS_DARK,
        (WindowAppearance::Dark, false, true) => &colors::GLASS_DARK_CONTRAST,
    }
}

pub fn palette() -> &'static Palette {
    palette_for(
        current_window_appearance(),
        crate::platform::uses_opaque_fallback(),
        uses_increase_contrast(),
    )
}

pub(crate) fn fixed() -> &'static FixedColors {
    fixed_for(uses_increase_contrast())
}

pub(crate) fn fixed_for(increase_contrast: bool) -> &'static FixedColors {
    if increase_contrast {
        &colors::FIXED_CONTRAST
    } else {
        &colors::FIXED
    }
}

pub fn glass_surface() -> Hsla {
    window()
}

pub fn fallback_surface() -> Hsla {
    token(if surface_is_light() {
        fixed().light_fallback_surface
    } else {
        fixed().fallback_surface
    })
}

pub fn menu_surface() -> Hsla {
    token(fixed().menu_surface)
}

pub fn action_menu_surface() -> Hsla {
    token(palette().action_menu_surface)
}

pub fn action_menu_selection() -> Hsla {
    token(palette().action_menu_selection)
}

pub fn modal_scrim() -> Hsla {
    token(fixed().modal_scrim)
}

pub fn window() -> Hsla {
    token(palette().window)
}

pub fn selected() -> Hsla {
    token(palette().selected_surface)
}

pub fn result_surface(selected: bool) -> Hsla {
    let p = palette();
    token(if selected {
        p.result_surface_selected
    } else {
        p.result_surface
    })
}

pub fn hover() -> Hsla {
    token(palette().hover)
}

pub fn field() -> Hsla {
    token(palette().field)
}

pub fn card() -> Hsla {
    token(palette().card)
}

pub fn control_surface() -> Hsla {
    token(fixed().control_surface)
}

pub fn control_selection() -> Hsla {
    token(fixed().control_selection)
}

pub fn control_border() -> Hsla {
    token(palette().control_border)
}

pub fn on_dark_control_border() -> Hsla {
    token(fixed().on_dark_control_border)
}

pub fn primary_button_surface() -> Hsla {
    token(fixed().primary_button_surface)
}

pub fn button_selection() -> Hsla {
    token(palette().button_selection)
}

pub fn text_selection() -> Hsla {
    token(palette().text_selection)
}

pub fn transparent() -> Hsla {
    token(0x00000000)
}

pub fn border() -> Hsla {
    token(palette().border)
}

pub fn selection_outline() -> Hsla {
    token(palette().selection_outline)
}

pub fn on_dark_selection_outline() -> Hsla {
    token(fixed().on_dark_selection_outline)
}

/// Result rows rely on their fill for selection; the focus-ring outline is
/// kept only when Increase Contrast asks for a stronger boundary.
pub fn result_selection_outline() -> Hsla {
    if uses_increase_contrast() {
        selection_outline()
    } else {
        divider()
    }
}

pub fn on_dark_divider() -> Hsla {
    token(fixed().on_dark_divider)
}

pub fn divider() -> Hsla {
    token(palette().divider)
}

pub fn keycap_surface() -> Hsla {
    token(palette().keycap_surface)
}

pub fn keycap_border() -> Hsla {
    token(palette().keycap_border)
}

pub fn primary() -> Hsla {
    token(palette().primary)
}

pub fn secondary() -> Hsla {
    token(palette().secondary)
}

pub fn muted() -> Hsla {
    token(palette().muted)
}

pub fn accent() -> Hsla {
    token(palette().accent)
}

pub fn accent_fill() -> Hsla {
    token(fixed().accent_fill)
}

pub fn warning() -> Hsla {
    token(palette().warning)
}

pub fn danger() -> Hsla {
    token(palette().danger)
}

pub fn success() -> Hsla {
    token(palette().success)
}

pub fn on_dark_primary() -> Hsla {
    token(fixed().on_dark_primary)
}

pub fn on_dark_secondary() -> Hsla {
    token(fixed().on_dark_secondary)
}

pub fn on_dark_muted() -> Hsla {
    token(fixed().on_dark_muted)
}

pub fn on_dark_accent() -> Hsla {
    token(fixed().on_dark_accent)
}

pub fn on_dark_warning() -> Hsla {
    token(fixed().on_dark_warning)
}

pub fn on_dark_danger() -> Hsla {
    token(fixed().on_dark_danger)
}

pub fn on_dark_success() -> Hsla {
    token(fixed().on_dark_success)
}

pub fn on_dark_hover() -> Hsla {
    token(fixed().on_dark_hover)
}

pub fn on_dark_selected() -> Hsla {
    token(fixed().on_dark_selected_surface)
}

pub fn icon_blue() -> Hsla {
    token(palette().icon_blue)
}

pub fn icon_green() -> Hsla {
    token(palette().icon_green)
}

pub fn icon_coral() -> Hsla {
    token(palette().icon_coral)
}

pub fn icon_purple() -> Hsla {
    token(palette().icon_purple)
}

pub fn icon_amber() -> Hsla {
    token(palette().icon_amber)
}

pub fn icon_pink() -> Hsla {
    token(palette().icon_pink)
}

pub fn icon_neutral() -> Hsla {
    token(palette().icon_neutral)
}

pub fn icon_badge_ink() -> Hsla {
    token(fixed().icon_badge_ink)
}

pub fn sidebar() -> Hsla {
    token(palette().sidebar)
}

pub fn sidebar_selection() -> Hsla {
    token(palette().sidebar_selection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::rgb;

    #[test]
    fn design_token_argb_colors_keep_their_alpha() {
        assert_eq!(selected(), rgba(0xffffff22).into());
        assert_eq!(glass_surface(), rgba(0x16191e66).into());
        assert_eq!(field(), rgba(0x20232a80).into());
        assert_eq!(menu_surface(), rgba(0x2b2e35ff).into());
        assert_eq!(token(colors::FIXED.content_scrim), rgba(0x30343800).into());
        assert_eq!(token(colors::FIXED.page_scrim), rgba(0x30343800).into());
        assert_eq!(result_surface(false), rgba(0x282c3300).into());
        assert_eq!(result_surface(true), rgba(0xffffff20).into());
        assert_eq!(modal_scrim(), rgba(0x000000a8).into());
    }

    #[test]
    fn opaque_light_uses_light_ink_with_normal_and_increased_contrast() {
        for contrast in [false, true] {
            assert_eq!(
                palette_for(WindowAppearance::Light, true, contrast).primary,
                palette_for(WindowAppearance::Light, false, contrast).primary,
            );
        }
        assert_eq!(
            token(colors::FIXED.light_fallback_surface),
            rgb(0xf5f6f8).into()
        );
    }

    #[test]
    fn opaque_and_menu_text_keeps_the_dark_surface_ink() {
        assert_eq!(on_dark_primary(), rgb(0xf9fafc).into());
        assert_eq!(on_dark_secondary(), rgb(0xedf0f5).into());
        assert_eq!(on_dark_muted(), rgb(0xdfe4ec).into());
        let opaque = palette_for(WindowAppearance::Dark, true, false);
        assert_eq!(token(opaque.accent), rgb(0xa7caff).into());
        assert_eq!(token(opaque.warning), rgb(0xffc66d).into());
        assert_eq!(token(opaque.danger), rgb(0xff8078).into());
        assert_eq!(token(opaque.success), rgb(0x73cc91).into());
    }

    #[test]
    fn all_six_generated_palettes_preserve_exact_legacy_role_values() {
        let opaque = palette_for(WindowAppearance::Dark, true, false);
        let opaque_contrast = palette_for(WindowAppearance::Dark, true, true);
        let glass_dark = palette_for(WindowAppearance::Dark, false, false);
        let glass_dark_contrast = palette_for(WindowAppearance::Dark, false, true);
        let glass_light = palette_for(WindowAppearance::Light, false, false);
        let glass_light_contrast = palette_for(WindowAppearance::Light, false, true);

        assert_eq!(opaque.accent, 0xffa7_caff);
        assert_eq!(glass_dark.accent, 0xffd4_e6ff);
        assert_eq!(glass_light.accent, 0xff1f_5d99);
        assert_eq!(opaque_contrast.window, 0x8516_191e);
        assert_eq!(glass_dark_contrast.border, 0x66ff_ffff);
        assert_eq!(glass_light_contrast.window, 0xe6f4_f6f9);
        assert_eq!(glass_light_contrast.selection_outline, 0x8a00_0000);
        assert_eq!(glass_light_contrast.muted, glass_light.secondary);
        assert_eq!(opaque_contrast.muted, opaque.secondary);
        assert_eq!(colors::FIXED.on_dark_divider, 0x1fff_ffff);
        assert_eq!(colors::FIXED_CONTRAST.on_dark_divider, 0x42ff_ffff);
    }
}
