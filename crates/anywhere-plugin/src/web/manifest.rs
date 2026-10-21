use super::{path, MAX_MANIFEST_BYTES};
use anyhow::{ensure, Context, Result};
use anywhere_protocol::web_plugin::{
    WebPermission, WebPluginManifest, API_VERSION, SCHEMA_VERSION,
};
use std::collections::BTreeSet;

pub fn parse_manifest(bytes: &[u8]) -> Result<WebPluginManifest> {
    ensure!(
        bytes.len() <= MAX_MANIFEST_BYTES,
        "Web manifest exceeds 128 KiB"
    );
    // Derived structs reject duplicate/unknown fields; serde_json bounds recursion.
    let manifest = serde_json::from_slice(bytes).context("invalid Web manifest JSON")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Repeat at registration: protocol DTOs can also be constructed without JSON.
pub fn validate_manifest(manifest: &WebPluginManifest) -> Result<()> {
    ensure!(
        manifest.schema_version == SCHEMA_VERSION,
        "unsupported Web schema"
    );
    ensure!(
        manifest.api_version == API_VERSION,
        "unsupported Web API version"
    );
    ensure!(
        namespace(&manifest.id) && manifest.id.len() <= 160,
        "invalid Web plugin id"
    );
    ensure!(
        !["core", "anywhere", "first-party", "web", "plugin"]
            .contains(&manifest.id.split('.').next().unwrap_or_default()),
        "reserved Web plugin id"
    );
    ensure!(label(&manifest.name, 200), "invalid Web plugin name");
    ensure!(manifest.version.len() <= 128, "invalid package version");
    semver::Version::parse(&manifest.version).context("invalid package version")?;
    ensure!(
        !manifest.platforms.is_empty() && manifest.platforms.len() <= 2,
        "invalid Web platforms"
    );
    let platforms: BTreeSet<_> = manifest.platforms.iter().collect();
    ensure!(
        platforms.len() == manifest.platforms.len(),
        "duplicate Web platform"
    );
    path::validate(&manifest.entry)?;
    ensure!(
        manifest.entry.ends_with(".html"),
        "Web entry must be local HTML"
    );
    if let Some(icon) = &manifest.icon {
        path::validate(icon)?;
        ensure!(icon.ends_with(".png"), "Web icon must be PNG");
    }
    ensure!(manifest.permissions.len() <= 5, "too many Web permissions");
    let mut permissions = BTreeSet::new();
    for permission in &manifest.permissions {
        ensure!(
            permissions.insert(permission.name()),
            "duplicate Web permission"
        );
        if let WebPermission::ContextRead { types } = permission {
            ensure!(
                !types.is_empty() && types.len() <= 16,
                "invalid Context projection types"
            );
            unique_types(types)?;
            ensure!(
                types.iter().all(|kind| matches!(
                    kind.as_str(),
                    anywhere_protocol::TEXT
                        | anywhere_protocol::URL
                        | anywhere_protocol::FILE
                        | anywhere_protocol::DIRECTORY
                        | anywhere_protocol::FILE_SELECTION
                )),
                "unsupported Context projection type"
            );
        }
    }
    ensure!(
        !manifest.commands.is_empty() && manifest.commands.len() <= 32,
        "expected 1..32 Web commands"
    );
    let mut commands = BTreeSet::new();
    for command in &manifest.commands {
        ensure!(
            segment(&command.id) && commands.insert(&command.id),
            "invalid or duplicate Web command id"
        );
        ensure!(segment(&command.entrypoint), "invalid Web entrypoint");
        ensure!(label(&command.title, 200), "invalid Web command title");
        ensure!(command.keywords.len() <= 16, "too many Web keywords");
        let mut keywords = BTreeSet::new();
        for keyword in &command.keywords {
            ensure!(
                label(keyword, 100) && keywords.insert(keyword),
                "invalid or duplicate Web keyword"
            );
        }
        ensure!(command.accepts.len() <= 16, "too many Web context matchers");
        unique_types(&command.accepts)?;
        let matchers: BTreeSet<_> = command.input_matchers.iter().collect();
        ensure!(
            matchers.len() <= 8 && matchers.len() == command.input_matchers.len(),
            "invalid or duplicate Web input matcher"
        );
        if !matchers.is_empty() {
            ensure!(
                command
                    .accepts
                    .iter()
                    .any(|kind| kind == anywhere_protocol::TEXT),
                "Web input matchers require accepting core.text"
            );
            ensure!(manifest.permissions.iter().any(|permission| matches!(permission,
                WebPermission::ContextRead { types } if types.iter().any(|kind| kind == anywhere_protocol::TEXT))),
                "Web input matchers require context.read for core.text");
        }
    }
    Ok(())
}

fn unique_types(types: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for kind in types {
        ensure!(
            namespace(kind) && kind.len() <= 160 && seen.insert(kind),
            "invalid or duplicate Web context type"
        );
    }
    Ok(())
}

fn label(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn namespace(value: &str) -> bool {
    value.contains('.') && value.split('.').all(segment)
}

fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
}
