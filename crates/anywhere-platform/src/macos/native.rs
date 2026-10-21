//! The Objective-C shim in `native/macos.m`.
//!
//! One file for the whole FFI boundary: every function hands back owned memory that the
//! matching `launcher_free_*` releases, and no pointer outlives the call that produced it.
#![allow(non_snake_case)]

use std::os::raw::{c_char, c_uint};

extern "C" {
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
    pub fn launcher_preferred_languages() -> *mut c_char;
    pub fn launcher_bundle_localized_names(
        path: *const c_char,
        languages: *const c_char,
    ) -> *mut c_char;
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
    pub fn launcher_clipboard_format_guard(format: *const c_char, capacity: u64) -> i32;
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
