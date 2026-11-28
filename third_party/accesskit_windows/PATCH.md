# Local AccessKit Windows patch

This directory vendors `accesskit_windows` 0.34.0 from crates.io. The source
came from AccessKit commit `c88605b96d04431f9c3c792464a0f2f253480e94`,
under `platforms/windows`. `LICENSE-MIT` and `LICENSE-APACHE` are copied from
that commit.

The only source changes make UIA calls non-fatal when a client still queries a
provider after its HWND has been destroyed:

- `util::client_top_left` returns the logical origin when `ClientToScreen`
  reports an invalid window.
- `QueuedEvents::raise` ignores errors from the two UIA event delivery calls.

Anywhere reproduces the upstream failure by opening the built-in JSON Workspace
in an independent window through Windows UI Automation, closing that window,
and then observing a panic at `client_top_left` from a non-unwinding
`extern "system"` callback.

Keep this patch limited to teardown safety. Re-check it before changing the
`accesskit_windows` version.
