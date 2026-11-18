use super::{
    plan_window_material, remember_material_plan, WindowAppearance, WindowGlassStyle,
    WindowMaterialEnvironment, WindowMaterialMode, WindowMaterialPlan,
};
use objc2::{
    runtime::{AnyClass, NSObjectProtocol},
    ClassType, MainThreadMarker,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSApplication, NSAutoresizingMaskOptions, NSColor,
    NSGlassEffectView, NSGlassEffectViewStyle, NSUserInterfaceItemIdentification, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWorkspace,
};
use objc2_foundation::{NSArray, NSString};

const MATERIAL_WRAPPER_ID: &str = "dev.anywhere.window-material-wrapper";
const MATERIAL_CONTENT_ID: &str = "dev.anywhere.window-material-content";

fn is_material_wrapper(view: &NSView) -> bool {
    view.identifier()
        .is_some_and(|identifier| identifier.to_string() == MATERIAL_WRAPPER_ID)
}

fn material_content(wrapper: &NSView) -> Option<objc2::rc::Retained<NSView>> {
    if AnyClass::get(c"NSGlassEffectView").is_some() {
        if let Some(glass) = wrapper.downcast_ref::<NSGlassEffectView>() {
            return glass.contentView();
        }
    }
    wrapper.subviews().iter().find(|view| {
        view.identifier()
            .is_some_and(|identifier| identifier.to_string() == MATERIAL_CONTENT_ID)
    })
}

fn restore_content_view(window: &NSWindow, wrapper: &NSView) -> bool {
    let Some(content) = material_content(wrapper) else {
        return false;
    };
    if AnyClass::get(c"NSGlassEffectView").is_some() {
        if let Some(glass) = wrapper.downcast_ref::<NSGlassEffectView>() {
            glass.setContentView(None);
            glass.setTintColor(None);
        }
    }
    content.setFrame(wrapper.bounds());
    content.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    window.setContentView(Some(&content));
    true
}

fn configure_material_wrapper(wrapper: &NSView, plan: WindowMaterialPlan) {
    wrapper.setIdentifier(Some(&NSString::from_str(MATERIAL_WRAPPER_ID)));
    wrapper.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    // AppKit owns the glass shape and its specular rim. Do not compensate
    // with an additional border or tint on the native or GPUI surface.
    wrapper.setAppearance(None);
    wrapper.setWantsLayer(true);
    if let Some(layer) = wrapper.layer() {
        layer.setCornerRadius(plan.corner_radius);
        layer.setMasksToBounds(plan.corner_radius > 0.0);
    }
}

fn configure_glass(
    glass: &NSGlassEffectView,
    plan: WindowMaterialPlan,
    glass_style: WindowGlassStyle,
) {
    // Use one unmodified system material on every product window. AppKit owns
    // its adaptive density, refraction and rim; GPUI adds no compensating veil.
    glass.setStyle(match glass_style {
        WindowGlassStyle::Regular => NSGlassEffectViewStyle::Regular,
    });
    glass.setCornerRadius(plan.corner_radius);
    // Keep the system material unmodified, including its accessibility and
    // Liquid Glass appearance preferences. Nil also clears any previous tint.
    glass.setTintColor(None);
}

fn install_transparent_window_background(window: &NSWindow) {
    window.setOpaque(false);
    // GPUI needs a non-nil near-clear background so AppKit retains the floating
    // window shadow while all visible fill comes from the material adapter.
    let transparent = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.0001);
    window.setBackgroundColor(Some(&transparent));
}

fn appearance_is_dark(appearance: &NSAppearance) -> bool {
    let names = NSArray::from_slice(&[unsafe { objc2_app_kit::NSAppearanceNameAqua }, unsafe {
        objc2_app_kit::NSAppearanceNameDarkAqua
    }]);
    appearance
        .bestMatchFromAppearancesWithNames(&names)
        .is_some_and(|best| &*best == unsafe { objc2_app_kit::NSAppearanceNameDarkAqua })
}

fn read_environment(
    appearance: &NSAppearance,
    liquid_glass_available: bool,
) -> WindowMaterialEnvironment {
    let workspace = NSWorkspace::sharedWorkspace();
    WindowMaterialEnvironment {
        liquid_glass_available,
        appearance: if appearance_is_dark(appearance) {
            WindowAppearance::Dark
        } else {
            WindowAppearance::Light
        },
        reduce_transparency: workspace.accessibilityDisplayShouldReduceTransparency(),
        increase_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
    }
}

pub(super) fn prepare_window_material() {
    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let application = NSApplication::sharedApplication(marker);
    let environment = read_environment(
        &application.effectiveAppearance(),
        AnyClass::get(c"NSGlassEffectView").is_some(),
    );
    let plan = plan_window_material(environment, f64::from(crate::ui::LAUNCHER_CORNER_RADIUS));
    remember_material_plan(plan);
    tracing::debug!(
        mode = ?plan.mode,
        appearance = ?plan.appearance,
        reduce_transparency = environment.reduce_transparency,
        increase_contrast = environment.increase_contrast,
        reduce_motion = environment.reduce_motion,
        "native_window_material_mode_prepared"
    );
}

pub(super) fn install_window_material(
    window: &NSWindow,
    corner_radius: f64,
    glass_style: WindowGlassStyle,
) -> bool {
    let liquid_glass_available = AnyClass::get(c"NSGlassEffectView").is_some();
    let environment = read_environment(&window.effectiveAppearance(), liquid_glass_available);
    let plan = plan_window_material(environment, corner_radius);

    let Some(mut content) = window.contentView() else {
        return false;
    };
    if plan.mode == WindowMaterialMode::Opaque {
        if is_material_wrapper(&content) && !restore_content_view(window, &content) {
            return false;
        }
        window.setOpaque(true);
        window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
        return true;
    }

    install_transparent_window_background(window);

    if is_material_wrapper(&content) {
        let matches = match plan.mode {
            WindowMaterialMode::LiquidGlass => content.isKindOfClass(NSGlassEffectView::class()),
            WindowMaterialMode::Blurred => content.isKindOfClass(NSVisualEffectView::class()),
            WindowMaterialMode::Opaque => false,
        };
        if matches {
            if liquid_glass_available {
                if let Some(glass) = content.downcast_ref::<NSGlassEffectView>() {
                    configure_glass(glass, plan, glass_style);
                }
            }
            configure_material_wrapper(&content, plan);
            return true;
        }
        if !restore_content_view(window, &content) {
            return false;
        }
        let Some(restored) = window.contentView() else {
            return false;
        };
        content = restored;
    }

    let bounds = content.bounds();
    content.setIdentifier(Some(&NSString::from_str(MATERIAL_CONTENT_ID)));
    let wrapper: objc2::rc::Retained<NSView> = match plan.mode {
        WindowMaterialMode::LiquidGlass => {
            let marker =
                MainThreadMarker::new().expect("window material requires AppKit main thread");
            let glass = NSGlassEffectView::new(marker);
            glass.setFrame(bounds);
            configure_glass(&glass, plan, glass_style);
            glass.setContentView(Some(&content));
            glass.into_super()
        }
        WindowMaterialMode::Blurred => {
            let marker =
                MainThreadMarker::new().expect("window material requires AppKit main thread");
            let hud = NSVisualEffectView::new(marker);
            hud.setFrame(bounds);
            hud.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            hud.setMaterial(NSVisualEffectMaterial::HUDWindow);
            hud.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            hud.setState(NSVisualEffectState::Active);
            hud.addSubview(&content);
            hud.into_super()
        }
        WindowMaterialMode::Opaque => unreachable!(),
    };
    configure_material_wrapper(&wrapper, plan);
    content.setFrame(bounds);
    content.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    window.setContentView(Some(&wrapper));
    tracing::debug!(
        mode = ?plan.mode,
        glass_style = ?glass_style,
        appearance = ?plan.appearance,
        reduce_transparency = environment.reduce_transparency,
        increase_contrast = environment.increase_contrast,
        reduce_motion = environment.reduce_motion,
        corner_radius = plan.corner_radius,
        wrapper_class = %wrapper.class().name().to_string_lossy(),
        content_class = %content.class().name().to_string_lossy(),
        content_layer_opaque = content.layer().is_some_and(|layer| layer.isOpaque()),
        wrapper_width = wrapper.bounds().size.width,
        wrapper_height = wrapper.bounds().size.height,
        wrapper_masks_to_bounds = wrapper.layer().is_some_and(|layer| layer.masksToBounds()),
        wrapper_corner_radius = wrapper.layer().map_or(0.0, |layer| layer.cornerRadius()),
        window_opaque = window.isOpaque(),
        custom_glass_tint = liquid_glass_available
            && wrapper
                .downcast_ref::<NSGlassEffectView>()
                .is_some_and(|glass| glass.tintColor().is_some()),
        "native_window_material_installed"
    );
    true
}
