# Local gpui-fast patches

This directory vendors `gpui-fast` 0.1.14 from crates.io.

The tracked change is the re-entrant App borrow guard introduced upstream in
0.1.19. Windows WebView2 creation pumps the thread's message queue while GPUI
still holds its App borrow. A queued GPUI task can therefore call
`AsyncApp::update` recursively and abort on `RefCell::already borrowed`.

The patch:

- exposes the `AppCell`'s inner `RefCell` to the crate's guard module;
- records the current application cell when the platform loop starts;
- exposes `app_is_borrowed()` so the Windows platform can defer re-entrant
  application callbacks.

This is a narrow backport needed by `gpui-kit` 0.9.1, which pins
`gpui-pre-platform` 0.3.12 and cannot be resolved directly against
`gpui-fast` 0.1.19. Remove the patch when the GPUI stack can be upgraded
together.

Two documentation comments in `src/_accessibility.rs` also drop trailing
whitespace so the vendored source passes `git diff --check`. This is a
non-behavioral cleanup.
