//! Native Windows material environment, appearance, and accessibility readers.
use super::{
    plan_window_material, remember_material_plan, WindowAppearance, WindowMaterialEnvironment,
};
use std::cell::Cell;
use windows_sys::Win32::UI::{
    Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
    WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST},
};

thread_local! {
    static APPEARANCE_OVERRIDE: Cell<Option<WindowAppearance>> = const { Cell::new(None) };
}

fn transparency_enabled() -> bool {
    !matches!(
        windows_registry::CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|key| key.get_u32("EnableTransparency")),
        Ok(0)
    )
}

fn system_appearance() -> WindowAppearance {
    if let Some(appearance) = APPEARANCE_OVERRIDE.get() {
        return appearance;
    }
    if matches!(
        windows_registry::CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|key| key.get_u32("AppsUseLightTheme")),
        Ok(0)
    ) {
        return WindowAppearance::Dark;
    }
    WindowAppearance::Light
}

fn high_contrast_enabled() -> bool {
    let mut info = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        dwFlags: 0,
        lpszDefaultScheme: std::ptr::null_mut(),
    };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            info.cbSize,
            std::ptr::from_mut(&mut info).cast(),
            0,
        )
    };
    ok != 0 && (info.dwFlags & HCF_HIGHCONTRASTON) != 0
}

fn reduce_motion_enabled() -> bool {
    let mut animations_enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            std::ptr::from_mut(&mut animations_enabled).cast(),
            0,
        )
    };
    ok != 0 && animations_enabled == 0
}

fn read_environment() -> WindowMaterialEnvironment {
    let backdrop_available = crate::platform::windows::launcher_has_backdrop().unwrap_or(true);
    let reduce_transparency = !backdrop_available || !transparency_enabled();
    WindowMaterialEnvironment {
        liquid_glass_available: false,
        appearance: system_appearance(),
        reduce_transparency,
        increase_contrast: high_contrast_enabled(),
        reduce_motion: reduce_motion_enabled(),
    }
}

pub(super) fn prepare_window_material() {
    let environment = read_environment();
    let plan = plan_window_material(environment, f64::from(crate::ui::LAUNCHER_CORNER_RADIUS));
    remember_material_plan(plan);
    tracing::debug!(
        mode = ?plan.mode,
        appearance = ?plan.appearance,
        reduce_transparency = environment.reduce_transparency,
        increase_contrast = environment.increase_contrast,
        reduce_motion = environment.reduce_motion,
        "windows_native_window_material_mode_prepared"
    );
}

pub(super) fn set_application_appearance(appearance: Option<WindowAppearance>) -> bool {
    APPEARANCE_OVERRIDE.set(appearance);
    let previous = super::current_appearance();
    prepare_window_material();
    previous != super::current_appearance()
}
