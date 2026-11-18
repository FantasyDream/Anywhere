use gpui_kit::{AnyWindowHandle, App, Context, WeakEntity, Window};
use std::{cell::Cell, rc::Rc, time::Duration};

/// AppKit can synchronously deliver resize callbacks when a material is mounted.
/// A queued main-thread callback alone is not a borrow fence: nested event loops
/// can run it before the enclosing GPUI update returns. Probe through AsyncApp's
/// fallible window update, release that borrow, then perform the native operation.
pub(crate) fn defer_native_window_update<T: 'static>(
    owner: WeakEntity<T>,
    window: AnyWindowHandle,
    cx: &App,
    native: impl FnOnce() -> bool + 'static,
    complete: impl FnOnce(&mut T, &mut Window, &mut Context<T>, bool) + 'static,
) {
    let closed = Rc::new(Cell::new(false));
    let window_closed = closed.clone();
    let subscription = cx.on_window_closed(move |_, id| {
        if id == window.window_id() {
            window_closed.set(true);
        }
    });
    cx.spawn(async move |cx| {
        let _subscription = subscription;
        // Do not perform native work inline, even if the executor polls eagerly.
        cx.background_executor().timer(Duration::ZERO).await;
        while !closed.get() && owner.upgrade().is_some() {
            if window.update(cx, |_, _, _| ()).is_ok() {
                // No await or GPUI borrow between the successful probe and AppKit.
                let result = native();
                let _ = window.update(cx, |_, window, cx| {
                    let _ = owner.update(cx, |owner, cx| complete(owner, window, cx, result));
                });
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AppContext as _, Entity, Render, TestAppContext, WindowOptions};
    use std::{cell::Cell, rc::Rc};

    struct TestView(bool);

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
            gpui_kit::div()
        }
    }

    fn open_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<TestView>) {
        cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| TestView(false)),
            )
        })
        .unwrap()
    }

    #[gpui_kit::test]
    fn native_resize_callback_can_reenter_gpui(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let mut async_cx = cx.to_async();
        cx.update(|cx| {
            defer_native_window_update(
                view.downgrade(),
                window,
                cx,
                move || {
                    // Model the synchronous AppKit resize callback made by
                    // setContentView/setStyleMask during material installation.
                    window.update(&mut async_cx, |_, _, _| ()).unwrap();
                    true
                },
                |view, _, _, configured| view.0 = configured,
            );
            assert!(!view.read(cx).0);
        });
        cx.run_until_parked();
        cx.update(|cx| assert!(view.read(cx).0));
    }

    #[gpui_kit::test]
    fn nested_event_loop_retries_after_enclosing_app_borrow(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let calls = Rc::new(Cell::new(0));
        let native_calls = calls.clone();
        let executor = cx.executor();
        cx.update(|cx| {
            defer_native_window_update(
                view.downgrade(),
                window,
                cx,
                move || {
                    native_calls.set(native_calls.get() + 1);
                    true
                },
                |view, _, _, configured| view.0 = configured,
            );
            executor.run_until_parked();
            assert_eq!(calls.get(), 0);
        });
        executor.advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
        assert_eq!(calls.get(), 1);
        cx.update(|cx| assert!(view.read(cx).0));
    }

    #[gpui_kit::test]
    fn a_released_owner_cancels_native_work(cx: &mut TestAppContext) {
        let (window, _view) = open_view(cx);
        let owner = cx.update(|cx| cx.new(|_| TestView(false)));
        let calls = Rc::new(Cell::new(0));
        let native_calls = calls.clone();
        cx.update(|cx| {
            defer_native_window_update(
                owner.downgrade(),
                window,
                cx,
                move || {
                    native_calls.set(native_calls.get() + 1);
                    true
                },
                |_, _, _, _| panic!("released owner was updated"),
            )
        });
        drop(owner);
        cx.run_until_parked();
        assert_eq!(calls.get(), 0);
    }

    #[gpui_kit::test]
    fn a_closed_window_cancels_work_even_if_its_owner_is_retained(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let calls = Rc::new(Cell::new(0));
        let native_calls = calls.clone();
        cx.update(|cx| {
            defer_native_window_update(
                view.downgrade(),
                window,
                cx,
                move || {
                    native_calls.set(native_calls.get() + 1);
                    true
                },
                |_, _, _, _| panic!("closed window was updated"),
            )
        });
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(calls.get(), 0);
        assert_eq!(
            Rc::strong_count(&calls),
            1,
            "cancelled native task retained its callback"
        );
    }
}
