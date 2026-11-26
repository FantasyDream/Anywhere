use gpui_kit::{Window, WindowBackgroundAppearance};
use std::{
    cell::RefCell,
    ffi::c_void,
    ptr::{null, null_mut},
};
use windows::{
    UI::Color,
    core::{implement, Interface, Result as WinResult, GUID, HSTRING, PCWSTR},
    Foundation::{IPropertyValue, PropertyValue},
    Graphics::Effects::{
        IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectSource_Impl, IGraphicsEffect_Impl,
    },
    System::DispatcherQueueController,
    Win32::{
        Foundation::{E_INVALIDARG, HWND as WinHwnd},
        Graphics::Direct2D::CLSID_D2D1GaussianBlur,
        System::WinRT::{
            Composition::ICompositorDesktopInterop,
            CreateDispatcherQueueController, DispatcherQueueOptions,
            Graphics::Direct2D::{
                IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl,
                GRAPHICS_EFFECT_PROPERTY_MAPPING,
            },
            DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT,
        },
    },
    UI::Composition::{
        CompositionColorBrush, CompositionEffectSourceParameter,
        CompositionRoundedRectangleGeometry, CompositionSpriteShape, CompositionVisualSurface,
        Compositor, ContainerVisual, Desktop::DesktopWindowTarget, DropShadow, ShapeVisual,
        SpriteVisual,
    },
};
use windows_numerics::{Vector2, Vector3};
use windows_sys::{
    w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Dwm::{
            DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_NONE, DWMWA_BORDER_COLOR,
            DWMWA_COLOR_NONE, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_HOSTBACKDROPBRUSH,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        },
        Graphics::Gdi::{
            ClientToScreen, GetMonitorInfoW, MonitorFromWindow, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        },
        System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
        UI::{
            Controls::MARGINS,
            HiDpi::GetDpiForWindow,
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
            Shell::{DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos,
                GetWindowLongW, GetWindowRect, IsIconic, IsWindow, IsWindowVisible, IsZoomed,
                RegisterClassW, SendMessageW, SetWindowLongW, SetWindowPos, ShowWindow,
                GWL_EXSTYLE, GWL_STYLE, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT,
                HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND_NOTOPMOST, HWND_TOPMOST, MINMAXINFO,
                SPI_SETNONCLIENTMETRICS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOSENDCHANGING, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE,
                USER_DEFAULT_SCREEN_DPI, WM_ACTIVATE, WM_CAPTURECHANGED, WM_DPICHANGED,
                WM_GETMINMAXINFO, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOVE, WM_NCDESTROY, WM_NCHITTEST,
                WM_NCLBUTTONDOWN, WM_SETTINGCHANGE, WM_SHOWWINDOW, WM_SIZE, WM_THEMECHANGED,
                WM_WINDOWPOSCHANGED,
                WNDCLASSW, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
                WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
                WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
            },
        },
    },
};

const BACKDROP_CLASS: windows_sys::core::PCWSTR = w!("Anywhere.CompositionBackdrop");
const BACKDROP_SUBCLASS_ID: usize = 1;
const BLUR_STANDARD_DEVIATION: f32 = 24.0;
const D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED: u32 = 1;
const D2D1_BORDER_MODE_HARD: u32 = 1;
const SHADOW_BLUR_RADIUS_DIP: f32 = 28.0;
const SHADOW_OFFSET_Y_DIP: f32 = 8.0;
const SHADOW_PAD_DIP: f32 = 36.0;
const RESIZE_EDGE_DIP: f32 = 6.0;
const WCA_ACCENT_POLICY: u32 = 19;

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttribData {
    attrib: u32,
    pv_data: *mut c_void,
    cb_data: usize,
}

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct GaussianBlurEffect {
    source: IGraphicsEffectSource,
}

impl IGraphicsEffectSource_Impl for GaussianBlurEffect_Impl {}

impl IGraphicsEffect_Impl for GaussianBlurEffect_Impl {
    fn Name(&self) -> WinResult<HSTRING> {
        Ok(HSTRING::from("AnywhereBackdropBlur"))
    }

    fn SetName(&self, _name: &HSTRING) -> WinResult<()> {
        Ok(())
    }
}

impl IGraphicsEffectD2D1Interop_Impl for GaussianBlurEffect_Impl {
    fn GetEffectId(&self) -> WinResult<GUID> {
        Ok(CLSID_D2D1GaussianBlur)
    }

    fn GetNamedPropertyMapping(
        &self,
        _name: &PCWSTR,
        _index: *mut u32,
        _mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> WinResult<()> {
        Err(E_INVALIDARG.into())
    }

    fn GetPropertyCount(&self) -> WinResult<u32> {
        Ok(3)
    }

    fn GetProperty(&self, index: u32) -> WinResult<IPropertyValue> {
        let inspectable = match index {
            0 => PropertyValue::CreateSingle(BLUR_STANDARD_DEVIATION)?,
            1 => PropertyValue::CreateUInt32(D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED)?,
            2 => PropertyValue::CreateUInt32(D2D1_BORDER_MODE_HARD)?,
            _ => return Err(E_INVALIDARG.into()),
        };
        inspectable.cast()
    }

    fn GetSource(&self, index: u32) -> WinResult<IGraphicsEffectSource> {
        if index == 0 {
            Ok(self.source.clone())
        } else {
            Err(E_INVALIDARG.into())
        }
    }

    fn GetSourceCount(&self) -> WinResult<u32> {
        Ok(1)
    }
}

struct CompositionRuntime {
    _dispatcher: DispatcherQueueController,
    compositor: Compositor,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct BackdropLayout {
    width: i32,
    height: i32,
    dpi: u32,
    zoomed: bool,
    transparent: bool,
    dark: bool,
}

#[derive(Clone, Copy)]
struct ResizeDrag {
    edge: u32,
    start_cursor: POINT,
    start_rect: RECT,
    min_width: i32,
    min_height: i32,
}

struct BackdropHost {
    backdrop_hwnd: HWND,
    resizable: bool,
    _target: DesktopWindowTarget,
    _root: ContainerVisual,
    shadow_visual: SpriteVisual,
    shadow: DropShadow,
    mask_visual: ShapeVisual,
    mask_geometry: CompositionRoundedRectangleGeometry,
    mask_surface: CompositionVisualSurface,
    blur_visual: SpriteVisual,
    blur_geometry: CompositionRoundedRectangleGeometry,
    border_visual: ShapeVisual,
    border_geometry: CompositionRoundedRectangleGeometry,
    border_shape: CompositionSpriteShape,
    border_brush: CompositionColorBrush,
    outer_border_geometry: CompositionRoundedRectangleGeometry,
    outer_border_shape: CompositionSpriteShape,
    outer_border_brush: CompositionColorBrush,
    layout: RefCell<Option<BackdropLayout>>,
    resize_drag: RefCell<Option<ResizeDrag>>,
}

thread_local! {
    static RUNTIME: RefCell<Option<Result<CompositionRuntime, ()>>> = const { RefCell::new(None) };
}

impl BackdropHost {
    fn new(owner_hwnd: HWND, resizable: bool) -> Option<Self> {
        let compositor = compositor()?;
        let backdrop_hwnd = create_backdrop_window()?;
        let setup = (|| -> WinResult<Self> {
            let root = compositor.CreateContainerVisual()?;

            // Rounded mask surface for hardware-accelerated DropShadow.
            let mask_geometry = compositor.CreateRoundedRectangleGeometry()?;
            let mask_shape = compositor.CreateSpriteShapeWithGeometry(&mask_geometry)?;
            let white_brush = compositor.CreateColorBrushWithColor(Color {
                A: 255,
                R: 255,
                G: 255,
                B: 255,
            })?;
            mask_shape.SetFillBrush(&white_brush)?;
            let mask_visual = compositor.CreateShapeVisual()?;
            mask_visual.Shapes()?.Append(&mask_shape)?;
            let mask_surface = compositor.CreateVisualSurface()?;
            mask_surface.SetSourceVisual(&mask_visual)?;
            let mask_brush = compositor.CreateSurfaceBrushWithSurface(&mask_surface)?;

            let shadow = compositor.CreateDropShadow()?;
            shadow.SetMask(&mask_brush)?;
            let shadow_visual = compositor.CreateSpriteVisual()?;
            shadow_visual.SetShadow(&shadow)?;

            // Real-time frosted glass blur sampling behind the window.
            let source: IGraphicsEffectSource =
                CompositionEffectSourceParameter::Create(&HSTRING::from("Source"))?.cast()?;
            let effect: IGraphicsEffect = GaussianBlurEffect { source }.into();
            let factory = compositor.CreateEffectFactory(&effect)?;
            let brush = factory.CreateBrush()?;
            let backdrop_brush = compositor.CreateHostBackdropBrush()?;
            brush.SetSourceParameter(&HSTRING::from("Source"), &backdrop_brush)?;

            let blur_visual = compositor.CreateSpriteVisual()?;
            blur_visual.SetBrush(&brush)?;
            let blur_geometry = compositor.CreateRoundedRectangleGeometry()?;
            let clip = compositor.CreateGeometricClipWithGeometry(&blur_geometry)?;
            blur_visual.SetClip(&clip)?;

            // Crisp 1px outer rim matching macOS native glass window border.
            let border_geometry = compositor.CreateRoundedRectangleGeometry()?;
            let border_shape = compositor.CreateSpriteShapeWithGeometry(&border_geometry)?;
            let border_brush = compositor.CreateColorBrushWithColor(border_color(true))?;
            border_shape.SetStrokeBrush(&border_brush)?;
            let outer_border_geometry = compositor.CreateRoundedRectangleGeometry()?;
            let outer_border_shape =
                compositor.CreateSpriteShapeWithGeometry(&outer_border_geometry)?;
            let outer_border_brush =
                compositor.CreateColorBrushWithColor(outer_border_color(true))?;
            outer_border_shape.SetStrokeBrush(&outer_border_brush)?;
            let border_visual = compositor.CreateShapeVisual()?;
            let border_shapes = border_visual.Shapes()?;
            border_shapes.Append(&outer_border_shape)?;
            border_shapes.Append(&border_shape)?;

            let children = root.Children()?;
            children.InsertAtTop(&shadow_visual)?;
            children.InsertAtTop(&blur_visual)?;
            children.InsertAtTop(&border_visual)?;

            let interop: ICompositorDesktopInterop = compositor.cast()?;
            let target =
                unsafe { interop.CreateDesktopWindowTarget(WinHwnd(backdrop_hwnd), false)? };
            target.SetRoot(&root)?;

            Ok(Self {
                backdrop_hwnd,
                resizable,
                _target: target,
                _root: root,
                shadow_visual,
                shadow,
                mask_visual,
                mask_geometry,
                mask_surface,
                blur_visual,
                blur_geometry,
                border_visual,
                border_geometry,
                border_shape,
                border_brush,
                outer_border_geometry,
                outer_border_shape,
                outer_border_brush,
                layout: RefCell::new(None),
                resize_drag: RefCell::new(None),
            })
        })();
        match setup {
            Ok(host) => {
                host.sync(owner_hwnd);
                Some(host)
            }
            Err(error) => {
                tracing::warn!(%error, "windows_composition_backdrop_setup_failed");
                unsafe { DestroyWindow(backdrop_hwnd) };
                None
            }
        }
    }

    fn sync(&self, owner_hwnd: HWND) {
        if unsafe { IsWindow(self.backdrop_hwnd) } == 0 {
            return;
        }
        let visible = unsafe { IsWindowVisible(owner_hwnd) != 0 && IsIconic(owner_hwnd) == 0 };
        if !visible {
            unsafe { ShowWindow(self.backdrop_hwnd, SW_HIDE) };
            return;
        }
        let mut client = RECT::default();
        if unsafe { GetClientRect(owner_hwnd, &mut client) } == 0 {
            return;
        }
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        if width <= 0 || height <= 0 {
            unsafe { ShowWindow(self.backdrop_hwnd, SW_HIDE) };
            return;
        }
        let mut origin = POINT {
            x: client.left,
            y: client.top,
        };
        if unsafe { ClientToScreen(owner_hwnd, &mut origin) } == 0 {
            return;
        }
        let dpi = unsafe { GetDpiForWindow(owner_hwnd) };
        let zoomed = unsafe { IsZoomed(owner_hwnd) != 0 };
        let transparent = transparency_enabled();
        let dark = crate::platform::current_window_appearance()
            == crate::platform::WindowAppearance::Dark;
        let scale = if dpi > 0 {
            dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
        } else {
            1.0
        };
        let pad = if zoomed {
            0
        } else {
            (SHADOW_PAD_DIP * scale).ceil() as i32
        };

        let desired = BackdropLayout {
            width,
            height,
            dpi,
            zoomed,
            transparent,
            dark,
        };
        let mut cached = self.layout.borrow_mut();
        if *cached != Some(desired) {
            let size = Vector2 {
                X: width as f32,
                Y: height as f32,
            };
            let radius = if zoomed {
                0.0
            } else {
                (crate::ui::LAUNCHER_CORNER_RADIUS * scale)
                    .min(size.X / 2.0)
                    .min(size.Y / 2.0)
            };
            let corner = Vector2 {
                X: radius,
                Y: radius,
            };
            let pad_f = pad as f32;
            let content_offset = Vector3 {
                X: pad_f,
                Y: pad_f,
                Z: 0.0,
            };

            let _ = self.mask_visual.SetSize(size);
            let _ = self.mask_geometry.SetSize(size);
            let _ = self.mask_geometry.SetCornerRadius(corner);
            let _ = self.mask_surface.SetSourceSize(size);
            let _ = self.shadow.SetBlurRadius(SHADOW_BLUR_RADIUS_DIP * scale);
            let _ = self.shadow.SetOffset(Vector3 {
                X: 0.0,
                Y: SHADOW_OFFSET_Y_DIP * scale,
                Z: 0.0,
            });
            let _ = self.shadow.SetColor(shadow_color(dark));
            let _ = self.shadow_visual.SetOffset(content_offset);
            let _ = self.shadow_visual.SetSize(size);
            let _ = self.shadow_visual.SetIsVisible(!zoomed);

            let _ = self.blur_visual.SetOffset(content_offset);
            let _ = self.blur_visual.SetSize(size);
            let _ = self.blur_geometry.SetSize(size);
            let _ = self.blur_geometry.SetCornerRadius(corner);
            let _ = self.blur_visual.SetIsVisible(transparent);

            let stroke = scale.round().max(1.0);
            let outer_stroke = 1.0_f32;
            let border_size = Vector2 {
                X: size.X + stroke,
                Y: size.Y + stroke,
            };
            let border_radius = radius + stroke * 0.5;
            let outer_size = Vector2 {
                X: size.X + stroke * 2.0 + outer_stroke,
                Y: size.Y + stroke * 2.0 + outer_stroke,
            };
            let outer_radius = radius + stroke + outer_stroke * 0.5;
            let _ = self.border_brush.SetColor(border_color(dark));
            let _ = self.border_shape.SetStrokeThickness(stroke);
            let _ = self.border_shape.SetOffset(Vector2 {
                X: outer_stroke + stroke * 0.5,
                Y: outer_stroke + stroke * 0.5,
            });
            let _ = self.border_geometry.SetSize(border_size);
            let _ = self.border_geometry.SetCornerRadius(Vector2 {
                X: border_radius,
                Y: border_radius,
            });
            let _ = self.outer_border_brush.SetColor(outer_border_color(dark));
            let _ = self.outer_border_shape.SetStrokeThickness(outer_stroke);
            let _ = self.outer_border_shape.SetOffset(Vector2 {
                X: outer_stroke * 0.5,
                Y: outer_stroke * 0.5,
            });
            let _ = self.outer_border_geometry.SetSize(outer_size);
            let _ = self.outer_border_geometry.SetCornerRadius(Vector2 {
                X: outer_radius,
                Y: outer_radius,
            });
            let border_inset = stroke + outer_stroke;
            let _ = self.border_visual.SetOffset(Vector3 {
                X: pad_f - border_inset,
                Y: pad_f - border_inset,
                Z: 0.0,
            });
            let _ = self.border_visual.SetSize(Vector2 {
                X: size.X + border_inset * 2.0,
                Y: size.Y + border_inset * 2.0,
            });
            let _ = self.border_visual.SetIsVisible(!zoomed);

            *cached = Some(desired);
        }
        drop(cached);

        sync_topmost_band(owner_hwnd, self.backdrop_hwnd);
        unsafe {
            SetWindowPos(
                self.backdrop_hwnd,
                owner_hwnd,
                origin.x - pad,
                origin.y - pad,
                width + pad * 2,
                height + pad * 2,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOSENDCHANGING,
            );
        }
    }
}

fn shadow_color(dark: bool) -> Color {
    if dark {
        Color {
            A: 128,
            R: 0,
            G: 0,
            B: 0,
        }
    } else {
        Color {
            A: 72,
            R: 0,
            G: 0,
            B: 0,
        }
    }
}

fn border_color(dark: bool) -> Color {
    if dark {
        Color {
            A: 56,
            R: 255,
            G: 255,
            B: 255,
        }
    } else {
        Color {
            A: 120,
            R: 255,
            G: 255,
            B: 255,
        }
    }
}

fn outer_border_color(dark: bool) -> Color {
    if dark {
        Color {
            A: 72,
            R: 0,
            G: 0,
            B: 0,
        }
    } else {
        Color {
            A: 54,
            R: 0,
            G: 0,
            B: 0,
        }
    }
}

fn sync_topmost_band(owner_hwnd: HWND, backdrop_hwnd: HWND) {
    let owner_topmost =
        (unsafe { GetWindowLongW(owner_hwnd, GWL_EXSTYLE) } & WS_EX_TOPMOST as i32) != 0;
    let backdrop_topmost =
        (unsafe { GetWindowLongW(backdrop_hwnd, GWL_EXSTYLE) } & WS_EX_TOPMOST as i32) != 0;
    if owner_topmost != backdrop_topmost {
        let insert_after = if owner_topmost {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        unsafe {
            SetWindowPos(
                backdrop_hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOSENDCHANGING,
            );
        }
    }
}

impl Drop for BackdropHost {
    fn drop(&mut self) {
        if unsafe { IsWindow(self.backdrop_hwnd) } != 0 {
            unsafe { DestroyWindow(self.backdrop_hwnd) };
        }
    }
}

fn compositor() -> Option<Compositor> {
    RUNTIME.with_borrow_mut(|slot| {
        let runtime = slot.get_or_insert_with(|| {
            let options = DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            };
            let dispatcher =
                unsafe { CreateDispatcherQueueController(options) }.map_err(|error| {
                    tracing::warn!(%error, "windows_dispatcher_queue_init_failed");
                })?;
            let compositor = Compositor::new().map_err(|error| {
                tracing::warn!(%error, "windows_compositor_init_failed");
            })?;
            Ok(CompositionRuntime {
                _dispatcher: dispatcher,
                compositor,
            })
        });
        runtime
            .as_ref()
            .ok()
            .map(|runtime| runtime.compositor.clone())
    })
}

fn create_backdrop_window() -> Option<HWND> {
    let instance = unsafe { GetModuleHandleW(null()) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(DefWindowProcW),
        hInstance: instance,
        lpszClassName: BACKDROP_CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassW(&class) };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP,
            BACKDROP_CLASS,
            null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        )
    };
    if hwnd.is_null() {
        None
    } else {
        // Non-UWP Win32 windows must opt into HostBackdropBrush sampling via DWM.
        set_attribute(hwnd, DWMWA_USE_HOSTBACKDROPBRUSH, 1);
        set_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND);
        set_attribute(hwnd, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE as i32);
        Some(hwnd)
    }
}

pub(super) fn transparency_enabled() -> bool {
    !matches!(
        windows_registry::CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|key| key.get_u32("EnableTransparency")),
        Ok(0)
    )
}

pub(super) struct InstalledBackdrop {
    pub(super) client_width: i32,
    pub(super) client_height: i32,
}

pub(super) fn install(hwnd: HWND, window: &Window) -> Option<InstalledBackdrop> {
    let mut client = RECT::default();
    let has_client = unsafe { GetClientRect(hwnd, &mut client) } != 0;
    let client_width = if has_client {
        client.right - client.left
    } else {
        0
    };
    let client_height = if has_client {
        client.bottom - client.top
    } else {
        0
    };

    let resizable = strip_window_frame(hwnd);
    update_appearance(hwnd);
    window.set_background_appearance(WindowBackgroundAppearance::Transparent);
    // gpui-fast-windows applies ACCENT_ENABLE_TRANSPARENTGRADIENT (state=2) for Transparent,
    // which composites opaque dark pixels on Windows 11. Reset to ACCENT_DISABLED and extend
    // the DWM frame so DirectComposition alpha blends cleanly over the composition backdrop.
    disable_window_composition_accent(hwnd);
    set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_NONE);
    set_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND);
    set_attribute(hwnd, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE as i32);
    let full_sheet = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &full_sheet);
    }

    if let Some(host) = BackdropHost::new(hwnd, resizable) {
        let raw = Box::into_raw(Box::new(host));
        if unsafe {
            SetWindowSubclass(
                hwnd,
                Some(backdrop_subclass_proc),
                BACKDROP_SUBCLASS_ID,
                raw as usize,
            )
        } != 0
        {
            return Some(InstalledBackdrop {
                client_width,
                client_height,
            });
        }
        unsafe {
            drop(Box::from_raw(raw));
        }
    }

    None
}

pub(super) fn finalize_install(hwnd: HWND, installed: InstalledBackdrop) {
    if unsafe { IsWindow(hwnd) } == 0 {
        return;
    }
    // Re-apply ACCENT_DISABLED in case GPUI applied window_background after the builder callback.
    disable_window_composition_accent(hwnd);
    if installed.client_width > 0 && installed.client_height > 0 {
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                installed.client_width,
                installed.client_height,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER,
            );
        }
    } else {
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
    // Synchronously refresh gpui-fast-windows's cached WindowBorderOffset to (0, 0)
    // before any subsequent window.resize() runs.
    unsafe {
        let _ = SendMessageW(hwnd, WM_SETTINGCHANGE, SPI_SETNONCLIENTMETRICS as usize, 0);
    }
    sync_for_window(hwnd);
}

unsafe extern "system" fn backdrop_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass_id: usize,
    ref_data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        unsafe { RemoveWindowSubclass(hwnd, Some(backdrop_subclass_proc), subclass_id) };
        if ref_data != 0 {
            unsafe {
                drop(Box::from_raw(ref_data as *mut BackdropHost));
            }
        }
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    if ref_data != 0 {
        let host = unsafe { &*(ref_data as *const BackdropHost) };
        match message {
            WM_GETMINMAXINFO => {
                let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
                clamp_maximize_to_work_area(hwnd, lparam);
                return result;
            }
            WM_NCHITTEST if host.resizable && unsafe { IsZoomed(hwnd) } == 0 => {
                if let Some(edge) = hit_test_resize_edge(hwnd, lparam) {
                    return edge as LRESULT;
                }
            }
            WM_NCLBUTTONDOWN if host.resizable && is_resize_edge(wparam as u32) => {
                begin_resize_drag(hwnd, host, wparam as u32);
                return 0;
            }
            WM_MOUSEMOVE if host.resize_drag.borrow().is_some() => {
                update_resize_drag(hwnd, host);
                return 0;
            }
            WM_LBUTTONUP | WM_CAPTURECHANGED if host.resize_drag.borrow().is_some() => {
                host.resize_drag.borrow_mut().take();
                if message == WM_LBUTTONUP {
                    unsafe { ReleaseCapture() };
                }
                return 0;
            }
            _ => {}
        }
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if ref_data != 0
        && matches!(
            message,
            WM_WINDOWPOSCHANGED
                | WM_SIZE
                | WM_MOVE
                | WM_SHOWWINDOW
                | WM_DPICHANGED
                | WM_ACTIVATE
                | WM_SETTINGCHANGE
                | WM_THEMECHANGED
        )
    {
        if message == WM_THEMECHANGED
            || (message == WM_SETTINGCHANGE && wparam != SPI_SETNONCLIENTMETRICS as usize)
        {
            super::notify_accessibility_display_changed();
        }
        let host = unsafe { &*(ref_data as *const BackdropHost) };
        host.sync(hwnd);
    }
    result
}

fn strip_window_frame(hwnd: HWND) -> bool {
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) };
    let resizable = (style & WS_THICKFRAME as i32) != 0;
    let borderless = style
        & !((WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) as i32);
    if style != borderless {
        unsafe {
            // Apply style bits now; finalize_install issues SWP_FRAMECHANGED on the next tick
            // so WM_MOVE/WM_SIZE never re-enter GPUI while open_window holds an App borrow.
            SetWindowLongW(hwnd, GWL_STYLE, borderless);
        }
    }
    resizable
}

fn clamp_maximize_to_work_area(hwnd: HWND, lparam: LPARAM) {
    if lparam == 0 {
        return;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return;
    }
    let minmax = unsafe { &mut *(lparam as *mut MINMAXINFO) };
    minmax.ptMaxPosition.x = info.rcWork.left - info.rcMonitor.left;
    minmax.ptMaxPosition.y = info.rcWork.top - info.rcMonitor.top;
    minmax.ptMaxSize.x = info.rcWork.right - info.rcWork.left;
    minmax.ptMaxSize.y = info.rcWork.bottom - info.rcWork.top;
}

fn is_resize_edge(hit: u32) -> bool {
    matches!(
        hit,
        HTLEFT
            | HTRIGHT
            | HTTOP
            | HTTOPLEFT
            | HTTOPRIGHT
            | HTBOTTOM
            | HTBOTTOMLEFT
            | HTBOTTOMRIGHT
    )
}

fn hit_test_resize_edge(hwnd: HWND, lparam: LPARAM) -> Option<u32> {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return None;
    }
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = if dpi > 0 {
        dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
    } else {
        1.0
    };
    let margin = (RESIZE_EDGE_DIP * scale).round().max(4.0) as i32;
    let left = x >= rect.left && x < rect.left + margin;
    let right = x <= rect.right && x > rect.right - margin;
    let top = y >= rect.top && y < rect.top + margin;
    let bottom = y <= rect.bottom && y > rect.bottom - margin;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(HTTOPLEFT),
        (_, true, true, _) => Some(HTTOPRIGHT),
        (true, _, _, true) => Some(HTBOTTOMLEFT),
        (_, true, _, true) => Some(HTBOTTOMRIGHT),
        (true, _, _, _) => Some(HTLEFT),
        (_, true, _, _) => Some(HTRIGHT),
        (_, _, true, _) => Some(HTTOP),
        (_, _, _, true) => Some(HTBOTTOM),
        _ => None,
    }
}

fn begin_resize_drag(hwnd: HWND, host: &BackdropHost, edge: u32) {
    let mut start_cursor = POINT::default();
    let mut start_rect = RECT::default();
    if unsafe { GetCursorPos(&mut start_cursor) == 0 || GetWindowRect(hwnd, &mut start_rect) == 0 }
    {
        return;
    }
    let mut minmax = MINMAXINFO::default();
    unsafe {
        DefSubclassProc(
            hwnd,
            WM_GETMINMAXINFO,
            0,
            std::ptr::from_mut(&mut minmax) as LPARAM,
        );
    }
    let min_width = minmax.ptMinTrackSize.x.max(240);
    let min_height = minmax.ptMinTrackSize.y.max(120);
    *host.resize_drag.borrow_mut() = Some(ResizeDrag {
        edge,
        start_cursor,
        start_rect,
        min_width,
        min_height,
    });
    unsafe { SetCapture(hwnd) };
}

fn update_resize_drag(hwnd: HWND, host: &BackdropHost) {
    let Some(drag) = *host.resize_drag.borrow() else {
        return;
    };
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) } == 0 {
        return;
    }
    let dx = cursor.x - drag.start_cursor.x;
    let dy = cursor.y - drag.start_cursor.y;
    let mut rect = drag.start_rect;
    match drag.edge {
        HTLEFT | HTTOPLEFT | HTBOTTOMLEFT => {
            rect.left = (drag.start_rect.left + dx).min(drag.start_rect.right - drag.min_width);
        }
        HTRIGHT | HTTOPRIGHT | HTBOTTOMRIGHT => {
            rect.right = (drag.start_rect.right + dx).max(drag.start_rect.left + drag.min_width);
        }
        _ => {}
    }
    match drag.edge {
        HTTOP | HTTOPLEFT | HTTOPRIGHT => {
            rect.top = (drag.start_rect.top + dy).min(drag.start_rect.bottom - drag.min_height);
        }
        HTBOTTOM | HTBOTTOMLEFT | HTBOTTOMRIGHT => {
            rect.bottom = (drag.start_rect.bottom + dy).max(drag.start_rect.top + drag.min_height);
        }
        _ => {}
    }
    unsafe {
        SetWindowPos(
            hwnd,
            null_mut(),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

fn sync_for_window(hwnd: HWND) {
    let mut ref_data = 0usize;
    if unsafe {
        GetWindowSubclass(
            hwnd,
            Some(backdrop_subclass_proc),
            BACKDROP_SUBCLASS_ID,
            &mut ref_data,
        )
    } != 0
        && ref_data != 0
    {
        let host = unsafe { &*(ref_data as *const BackdropHost) };
        host.sync(hwnd);
    }
}

pub(super) fn update_appearance(hwnd: HWND) {
    let dark = i32::from(
        crate::platform::current_window_appearance() == crate::platform::WindowAppearance::Dark,
    );
    set_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, dark);
    sync_for_window(hwnd);
    unsafe {
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

fn disable_window_composition_accent(hwnd: HWND) {
    type SetWindowCompositionAttributeFn =
        unsafe extern "system" fn(HWND, *mut WindowCompositionAttribData) -> i32;
    let module = unsafe { GetModuleHandleW(w!("user32.dll")) };
    if module.is_null() {
        return;
    }
    let Some(proc) =
        (unsafe { GetProcAddress(module, c"SetWindowCompositionAttribute".as_ptr().cast()) })
    else {
        return;
    };
    let set_wca: SetWindowCompositionAttributeFn = unsafe { std::mem::transmute(proc) };
    let mut policy = AccentPolicy {
        accent_state: 0,
        accent_flags: 0,
        gradient_color: 0,
        animation_id: 0,
    };
    let mut data = WindowCompositionAttribData {
        attrib: WCA_ACCENT_POLICY,
        pv_data: std::ptr::from_mut(&mut policy).cast(),
        cb_data: size_of::<AccentPolicy>(),
    };
    unsafe {
        let _ = set_wca(hwnd, &mut data);
    }
}

fn set_attribute(hwnd: HWND, attribute: i32, value: i32) -> bool {
    (unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute as u32,
            std::ptr::from_ref(&value).cast(),
            size_of::<i32>() as u32,
        )
    }) >= 0
}
