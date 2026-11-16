//! Isolated material diagnostic; does not open the product runtime or user data.
#[cfg(target_os = "macos")]
mod macos {
    use gpui_kit::{
        div, prelude::*, px, AppContext, Context, Render, TitlebarOptions, Window,
        WindowBackgroundAppearance, WindowOptions,
    };
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication, NSColor,
        NSGlassEffectView, NSGlassEffectViewStyle, NSView,
    };
    use objc2_foundation::NSRect;

    struct Probe;
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .bg(gpui_kit::rgba(0x00000000))
                .p(px(30.))
                .text_color(gpui_kit::rgb(0xffffff))
                .text_size(px(22.))
                .child("GPUI transparent canvas inside native glass")
        }
    }

    fn install() {
        if objc2::runtime::AnyClass::get(c"NSGlassEffectView").is_none() {
            eprintln!("This diagnostic requires macOS 26 or later.");
            return;
        }
        let mtm = MainThreadMarker::new().unwrap();
        let app = NSApplication::sharedApplication(mtm);
        let window = app
            .windows()
            .iter()
            .find(|w| w.title().to_string() == "GPUI Glass Probe")
            .unwrap();
        let content = window.contentView().unwrap();
        let bounds = content.bounds();
        let base = NSView::new(mtm);
        base.setFrame(bounds);
        let colors = [
            NSColor::systemRedColor(),
            NSColor::systemOrangeColor(),
            NSColor::systemGreenColor(),
            NSColor::systemBlueColor(),
            NSColor::systemPurpleColor(),
        ];
        for i in 0..20 {
            let stripe = NSView::new(mtm);
            stripe.setFrame(NSRect::new(
                objc2_foundation::NSPoint::new(i as f64 * 40., 0.),
                objc2_foundation::NSSize::new(40., bounds.size.height),
            ));
            stripe.setWantsLayer(true);
            stripe
                .layer()
                .unwrap()
                .setBackgroundColor(Some(&colors[i % 5].CGColor()));
            base.addSubview(&stripe);
        }
        let glass = NSGlassEffectView::new(mtm);
        glass.setFrame(bounds);
        glass.setStyle(NSGlassEffectViewStyle::Clear);
        glass.setCornerRadius(18.);
        // SAFETY: AppKit's immutable appearance constant is accessed on the main thread.
        let dark_aqua = unsafe { NSAppearanceNameDarkAqua };
        glass.setAppearance(NSAppearance::appearanceNamed(dark_aqua).as_deref());
        glass.setContentView(Some(&content));
        glass.setWantsLayer(true);
        glass.layer().unwrap().setCornerRadius(18.);
        glass.layer().unwrap().setMasksToBounds(true);
        window.setContentView(Some(&base));
        base.addSubview(&glass);
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
    }

    pub fn run() {
        gpui_kit::application().run(|cx| {
            gpui_kit::init(cx);
            if std::env::var_os("GLASS_PROBE_CLEAR_ROOT").is_some() {
                gpui_kit::component::Theme::update(cx, |theme| {
                    let clear: gpui_kit::Hsla = gpui_kit::rgba(0x00000000).into();
                    theme.tokens.background = clear.into();
                });
            }
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds::new(
                        gpui_kit::point(px(700.), px(300.)),
                        gpui_kit::size(px(800.), px(300.)),
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("GPUI Glass Probe".into()),
                        ..Default::default()
                    }),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| Probe),
            )
            .unwrap();
            dispatch2::DispatchQueue::main().exec_async(install);
        });
    }
}
fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
}
