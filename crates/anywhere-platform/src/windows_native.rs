//! Declarations for the Windows native shim in `native/windows.cpp`.
//!
//! Kept apart from `windows.rs` so the whole unsafe boundary is one small file. Every
//! function takes caller-owned buffers with an explicit capacity and retains no pointers.
#![allow(non_snake_case)]

extern "C" {
    pub fn launcher_windows_foreground(
        executable: *mut u16,
        capacity: u32,
        window: *mut isize,
    ) -> i32;
    pub fn launcher_windows_directory(window: isize, output: *mut u16, capacity: u32) -> i32;
    pub fn launcher_windows_open(value: *const u16, kind: i32) -> i32;
    pub fn launcher_windows_copy(value: *const u16, length: u32) -> i32;
    /// Read one plain-text clipboard snapshot into `output`.
    ///
    /// Returns an HRESULT: success with `*length` set, `S_FALSE` when the clipboard holds
    /// no text of a supported kind.
    pub fn launcher_windows_clipboard_text(
        output: *mut u16,
        capacity: u32,
        length: *mut u32,
    ) -> i32;
    /// Resolve one Start Menu shortcut.
    ///
    /// Writes the shortcut's display name, its recorded launch target, and its free-text
    /// description (empty when the shortcut has none).
    pub fn launcher_windows_resolve_shortcut(
        shortcut: *const u16,
        name: *mut u16,
        name_capacity: u32,
        target: *mut u16,
        target_capacity: u32,
        description: *mut u16,
        description_capacity: u32,
    ) -> i32;
}
