//! Embedded first-party built-in Web packages staged automatically at startup.

pub const JSON_WORKSPACE_PACKAGE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/builtin-web-plugins/dev.anywhere.json-workspace.awplugin"
));

pub const BUILTIN_WEB_PACKAGES: &[&[u8]] = &[JSON_WORKSPACE_PACKAGE];
