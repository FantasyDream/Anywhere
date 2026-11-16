//! Platform-neutral window backdrop policy and platform material adapters.
//!
//! Product pages render on a fully transparent GPUI canvas. The platform
//! adapter owns the backdrop surface, window clipping and accessibility
//! fallback. Windows can later plug a Mica/Acrylic adapter into the same policy
//! without changing Launcher, Settings, Tasks or Web surfaces.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowMaterialMode {
    /// macOS 26+ Liquid Glass.
    LiquidGlass,
    /// System blur on older macOS releases.
    Blurred,
    /// Accessibility or unsupported-host fallback.
    Opaque,
}

impl WindowMaterialMode {
    const fn code(self) -> u8 {
        match self {
            Self::LiquidGlass => 1,
            Self::Blurred => 2,
            Self::Opaque => 3,
        }
    }

    fn from_code(value: u8) -> Self {
        match value {
            1 => Self::LiquidGlass,
            2 => Self::Blurred,
            _ => Self::Opaque,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowAppearance {
    Light,
    Dark,
}

impl WindowAppearance {
    const fn code(self) -> u8 {
        match self {
            Self::Light => 0,
            Self::Dark => 1,
        }
    }

    fn from_code(value: u8) -> Self {
        if value == 1 {
            Self::Dark
        } else {
            Self::Light
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowMaterialEnvironment {
    pub liquid_glass_available: bool,
    pub appearance: WindowAppearance,
    pub reduce_transparency: bool,
    pub increase_contrast: bool,
    pub reduce_motion: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowMaterialPlan {
    pub mode: WindowMaterialMode,
    pub appearance: WindowAppearance,
    pub corner_radius: f64,
    pub increase_contrast: bool,
    pub reduce_motion: bool,
}

static MATERIAL_MODE: AtomicU8 = AtomicU8::new(WindowMaterialMode::Opaque.code());
static APPEARANCE: AtomicU8 = AtomicU8::new(WindowAppearance::Light.code());
static INCREASE_CONTRAST: AtomicBool = AtomicBool::new(false);
static REDUCE_MOTION: AtomicBool = AtomicBool::new(false);

pub fn current_mode() -> WindowMaterialMode {
    WindowMaterialMode::from_code(MATERIAL_MODE.load(Ordering::Acquire))
}

pub fn current_appearance() -> WindowAppearance {
    WindowAppearance::from_code(APPEARANCE.load(Ordering::Acquire))
}

pub fn uses_opaque_fallback() -> bool {
    current_mode() == WindowMaterialMode::Opaque
}

pub fn uses_increase_contrast() -> bool {
    INCREASE_CONTRAST.load(Ordering::Acquire)
}

pub fn uses_reduce_motion() -> bool {
    REDUCE_MOTION.load(Ordering::Acquire)
}

fn material_mode(environment: WindowMaterialEnvironment) -> WindowMaterialMode {
    if environment.reduce_transparency {
        WindowMaterialMode::Opaque
    } else if environment.liquid_glass_available {
        WindowMaterialMode::LiquidGlass
    } else {
        WindowMaterialMode::Blurred
    }
}

pub(crate) fn plan_window_material(
    environment: WindowMaterialEnvironment,
    corner_radius: f64,
) -> WindowMaterialPlan {
    WindowMaterialPlan {
        mode: material_mode(environment),
        appearance: environment.appearance,
        corner_radius,
        increase_contrast: environment.increase_contrast,
        reduce_motion: environment.reduce_motion,
    }
}

pub(super) fn remember_material_plan(plan: WindowMaterialPlan) {
    MATERIAL_MODE.store(plan.mode.code(), Ordering::Release);
    APPEARANCE.store(plan.appearance.code(), Ordering::Release);
    INCREASE_CONTRAST.store(plan.increase_contrast, Ordering::Release);
    REDUCE_MOTION.store(plan.reduce_motion, Ordering::Release);
}

pub(crate) fn prepare_window_material() {
    #[cfg(target_os = "macos")]
    {
        macos::prepare_window_material();
    }
    #[cfg(not(target_os = "macos"))]
    {
        remember_material_plan(WindowMaterialPlan {
            mode: WindowMaterialMode::Opaque,
            appearance: WindowAppearance::Light,
            corner_radius: 0.0,
            increase_contrast: false,
            reduce_motion: false,
        });
    }
}

pub(crate) fn install_window_material(
    #[cfg(target_os = "macos")] window: &objc2_app_kit::NSWindow,
    corner_radius: f64,
) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::install_window_material(window, corner_radius)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // A future Windows adapter maps the same plan to DWM SystemBackdrop
        // (Mica/Acrylic) and its accessibility fallback.
        let _ = corner_radius;
        prepare_window_material();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(
        liquid_glass_available: bool,
        appearance: WindowAppearance,
        reduce_transparency: bool,
        increase_contrast: bool,
    ) -> WindowMaterialEnvironment {
        WindowMaterialEnvironment {
            liquid_glass_available,
            appearance,
            reduce_transparency,
            increase_contrast,
            reduce_motion: false,
        }
    }

    #[test]
    fn liquid_glass_prefers_native_glass_and_honors_reduce_transparency() {
        assert_eq!(
            material_mode(environment(true, WindowAppearance::Dark, false, false)),
            WindowMaterialMode::LiquidGlass
        );
        assert_eq!(
            material_mode(environment(false, WindowAppearance::Dark, false, false)),
            WindowMaterialMode::Blurred
        );
        assert_eq!(
            material_mode(environment(true, WindowAppearance::Dark, true, false)),
            WindowMaterialMode::Opaque
        );
    }

    #[test]
    fn system_appearance_and_accessibility_do_not_replace_native_glass() {
        for appearance in [WindowAppearance::Light, WindowAppearance::Dark] {
            let plan = plan_window_material(environment(true, appearance, false, true), 18.0);
            assert_eq!(plan.mode, WindowMaterialMode::LiquidGlass);
            assert_eq!(plan.appearance, appearance);
            assert!(plan.increase_contrast);
            assert!(!plan.reduce_motion);
            assert_eq!(plan.corner_radius, 18.0);
        }
    }

    #[test]
    fn reduce_motion_is_forwarded_without_changing_the_native_material() {
        let mut environment = environment(true, WindowAppearance::Dark, false, false);
        environment.reduce_motion = true;
        let plan = plan_window_material(environment, 18.0);
        assert_eq!(plan.mode, WindowMaterialMode::LiquidGlass);
        assert!(plan.reduce_motion);
    }
}
