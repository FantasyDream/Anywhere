# 001 — Workspace root and macOS build host

The existing `Context-Aware-Launcher` directory is the workspace root; an extra
`context-launcher/` wrapper adds no boundary. Crates and app paths follow the plan.

This host initially has only Xcode Command Line Tools, no full Xcode, Flutter or
Rust. SDKs are installed under ignored `.tools/`, without changing global defaults.
Phase 1's GUI gate was verified with a real Flutter window, accessibility tree and screenshot.

The CLT build script now assembles FlutterMacOS.framework, Dart AOT App.framework,
the Rust dylib and a small Swift window host. This affects build infrastructure
only, not protocol, Core, or the presentation-only rule. No second GUI
implementation is introduced. The framework requires macOS 12, so the host target
and Info.plist use the same minimum. Release build and local signature checks pass.
The hidden engine is started explicitly, and the window frame is restored after
attaching FlutterViewController because its initial view size is zero. Both startup
issues were reproduced and the real window was verified after the fixes.
