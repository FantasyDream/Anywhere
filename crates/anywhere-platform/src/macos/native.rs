//! FFI declarations and buffer ownership for the macOS Objective-C shims.
#![allow(non_snake_case)]

use std::{
    os::raw::{c_char, c_uint},
    ptr::NonNull,
};

extern "C" {
    pub fn launcher_spotlight_paths(
        name: *const c_char,
        roots_json: *const c_char,
        timeout_ms: c_uint,
        capacity: c_uint,
        cancelled: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
        context: *mut std::ffi::c_void,
        length: *mut c_uint,
        status: *mut i32,
    ) -> *mut u8;
    pub fn launcher_open_resource(
        target: *const c_char,
        is_file: i32,
        application: *const c_char,
        operation: i32,
    ) -> i32;
    pub fn launcher_run_compiled_script(
        source: *const c_char,
        pid: i32,
        bundle: *const c_char,
        instance: *const c_char,
        timeout_ms: c_uint,
        capacity: c_uint,
        length: *mut c_uint,
        status: *mut i32,
    ) -> *mut u8;
    pub fn launcher_frontmost_application() -> *mut c_char;
    pub fn launcher_application_identity_matches(
        pid: i32,
        bundle_id: *const c_char,
        instance_id: *const c_char,
    ) -> i32;
    pub fn launcher_application_identity_status(
        pid: i32,
        bundle_id: *const c_char,
        instance_id: *const c_char,
    ) -> i32;
    pub fn launcher_free_string(value: *mut c_char);
    pub fn launcher_free_bytes(value: *mut u8);
    pub fn launcher_bundle_application(path: *const c_char) -> *mut c_char;
    pub fn launcher_launch_application(path: *const c_char) -> i32;
    pub fn launcher_application_icon(
        path: *const c_char,
        size: c_uint,
        out_length: *mut c_uint,
    ) -> *mut u8;
    /// Metadata only. Returns -1 when Cocoa cannot query the pasteboard.
    pub fn launcher_clipboard_change_count() -> i64;
    pub fn launcher_clipboard_history_access_allowed() -> i32;
    pub fn launcher_clipboard_text_guard(capacity: u64) -> i32;
    pub fn launcher_clipboard_read_format(
        format: *const c_char,
        capacity: u64,
        out_length: *mut c_uint,
        out_status: *mut i32,
    ) -> *mut u8;
    pub fn launcher_clipboard_item_count() -> u64;

    pub fn launcher_accessibility_trusted(prompt: i32) -> i32;
    pub fn launcher_application_version(bundle_id: *const c_char) -> *mut c_char;
    pub fn launcher_discover_menu_actions(application_json: *const c_char) -> *mut c_char;
    /// JSON status and optional bounded text; returned memory uses launcher_free_string.
    pub fn launcher_selected_text(application_json: *const c_char) -> *mut c_char;
    pub fn launcher_editor_project_window(application_json: *const c_char) -> *mut c_char;
    pub fn launcher_perform_menu_action(
        application_json: *const c_char,
        locator_json: *const c_char,
    ) -> i32;
}

/// An owned result allocated by a shim and freed with its matching allocator.
pub(super) struct NativeBytes {
    pointer: NonNull<u8>,
    length: usize,
}

impl NativeBytes {
    /// # Safety
    /// A non-null pointer must be a unique shim allocation with `length`
    /// initialized bytes, releasable with `launcher_free_bytes`.
    pub(super) unsafe fn from_raw(pointer: *mut u8, length: c_uint) -> Option<Self> {
        NonNull::new(pointer).map(|pointer| Self {
            pointer,
            length: length as usize,
        })
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        // SAFETY: the constructor establishes the allocation and its length.
        unsafe { std::slice::from_raw_parts(self.pointer.as_ptr(), self.length) }
    }
}

impl Drop for NativeBytes {
    fn drop(&mut self) {
        // SAFETY: this owner releases its native allocation exactly once.
        unsafe { launcher_free_bytes(self.pointer.as_ptr()) };
    }
}
