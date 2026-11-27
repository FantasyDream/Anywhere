# Local gpui-fast-windows patches

This directory vendors `gpui-fast-windows` 0.1.14 from crates.io.

The tracked change backports the re-entrant application event queue introduced
upstream in 0.1.19. WebView2 creation pumps the Windows message queue from
inside `Window::draw`. A queued GPUI foreground task handled by that nested
pump tries to borrow the already borrowed App and aborts the process.

The patch queues application-facing `WM_GPUI_*` messages while
`gpui::app_is_borrowed()` is true and drains them from the platform loop or its
retry timer. AccessKit events still raise directly because they only depend on
the window adapter.

This patch depends on the local `gpui-fast` backport in `third_party/gpui-fast`
and should be removed together with it when the surrounding `gpui-kit` stack is
upgraded.
