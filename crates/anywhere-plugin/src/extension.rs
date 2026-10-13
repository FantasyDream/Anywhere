//! Manifest negotiation and metadata validation; never grants requested access.
use anyhow::{bail, ensure, Context, Result};
use anywhere_protocol::{
    extension::{ExtensionCapability as Capability, FirstPartyManifest},
    DeclarativeAction, PluginCommand, PluginManifest,
};
use std::collections::BTreeSet;
pub fn required_capability(action: &DeclarativeAction) -> Capability {
    match action {
        DeclarativeAction::Copy { .. } => Capability::ClipboardWrite,
        DeclarativeAction::OpenUrl { .. } => Capability::OpenUrl,
        DeclarativeAction::OpenFile { .. } => Capability::OpenFile,
        DeclarativeAction::ExecuteBuiltin { .. } => Capability::InvokeBuiltin,
    }
}
pub fn required_capabilities(commands: &[PluginCommand]) -> Vec<Capability> {
    commands
        .iter()
        .map(|c| required_capability(&c.action))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && version.len() <= 32
        && parts.iter().all(|p| {
            !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().is_ok()
        })
}
fn metadata(
    id: &str,
    name: &str,
    package_version: &str,
    capabilities: &[Capability],
) -> Result<()> {
    ensure!(
        super::identifier(id) && id != "core" && !id.starts_with("core."),
        "invalid or reserved extension id"
    );
    ensure!(
        !name.trim().is_empty() && name.len() <= 200,
        "extension requires a name of 1..200 bytes"
    );
    ensure!(
        version(package_version),
        "extension version must be numeric major.minor.patch"
    );
    ensure!(
        capabilities.len() <= 8 && !capabilities.contains(&Capability::InvokeBuiltin),
        "v2 builtin delegation is not supported"
    );
    ensure!(
        capabilities.iter().copied().collect::<BTreeSet<_>>().len() == capabilities.len(),
        "duplicate extension capability"
    );
    Ok(())
}
pub fn validate_plugin_metadata(manifest: &PluginManifest) -> Result<()> {
    match manifest.protocol_version {
        1 => {
            ensure!(
                manifest.version.is_none(),
                "v1 package version is not supported"
            );
            ensure!(
                manifest.capabilities.is_empty()
                    || manifest.capabilities == required_capabilities(&manifest.commands),
                "invalid legacy capability migration"
            );
        }
        2 => {
            ensure!(
                manifest.capabilities.len() <= 3
                    && manifest.capabilities.iter().all(|c| matches!(
                        c,
                        Capability::ClipboardWrite | Capability::OpenUrl | Capability::OpenFile
                    )),
                "capability is not available to declarative v2 plugins"
            );
            metadata(
                &manifest.id,
                &manifest.name,
                manifest
                    .version
                    .as_deref()
                    .context("v2 requires a package version")?,
                &manifest.capabilities,
            )?;
            for required in required_capabilities(&manifest.commands) {
                ensure!(
                    manifest.capabilities.contains(&required),
                    "undeclared capability: {}",
                    required.as_str()
                );
            }
        }
        other => bail!("unsupported protocol_version {other}; expected 1 or 2"),
    }
    Ok(())
}
pub fn parse_first_party_manifest(raw: &str) -> Result<FirstPartyManifest> {
    ensure!(
        raw.len() <= 16 * 1024,
        "first-party metadata exceeds 16 KiB"
    );
    let m: FirstPartyManifest = toml::from_str(raw).context("invalid first-party manifest")?;
    ensure!(
        m.protocol_version == 2,
        "first-party manifests require protocol_version 2"
    );
    metadata(&m.id, &m.name, &m.version, &m.capabilities)?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v2() -> String {
        super::super::EXAMPLE_PLUGIN.replace(
            "protocol_version = 1",
            "protocol_version = 2\nversion = \"0.8.0\"\ncapabilities = [\"url.open\"]",
        )
    }
    #[test]
    fn legacy_migration_and_v2_negotiation_are_explicit() {
        let legacy = super::super::parse_manifest(super::super::EXAMPLE_PLUGIN).unwrap();
        assert_eq!(
            required_capabilities(&legacy.commands),
            [Capability::OpenUrl]
        );
        assert!(legacy.capabilities.is_empty());
        let serialized = toml::to_string(&legacy).unwrap();
        assert!(super::super::parse_manifest(&serialized).is_ok());
        let modern = super::super::parse_manifest(&v2()).unwrap();
        assert_eq!(legacy.commands[0].id, modern.commands[0].id);
        assert_eq!(legacy.commands[0].accepts, modern.commands[0].accepts);
        for raw in [
            v2().replace("protocol_version = 2", "protocol_version = 99"),
            v2().replace("version = \"0.8.0\"", "version = \"latest\""),
            v2().replace("capabilities = [\"url.open\"]", "capabilities = []"),
            v2().replace("[\"url.open\"]", "[\"url.open\", \"url.open\"]"),
            v2().replace("url.open", "process.spawn"),
            v2().replace("[\"url.open\"]", "[\"url.open\", \"process.start\"]"),
            v2().replace("[\"url.open\"]", "[\"url.open\", \"git.read\"]"),
            v2().replace("[\"url.open\"]", "[\"url.open\", \"storage\"]"),
        ] {
            assert!(super::super::parse_manifest(&raw).is_err(), "{raw}");
        }
    }
    #[test]
    fn v2_cannot_escalate_by_delegating_to_a_native_builtin() {
        let raw = r#"protocol_version = 2
version = "0.8.0"
capabilities = ["builtin.invoke", "clipboard.write"]
id = "example"
name = "Delegation"
[[commands]]
id = "copy"
title = "Copy"
accepts = ["core.text"]
action = {type = "execute_builtin", command_id = "core.json.copy"}
"#;
        assert!(super::super::parse_manifest(raw).is_err());
    }
    #[test]
    fn first_party_metadata_uses_the_same_version_and_capability_contract() {
        let json = include_str!("../../../extensions/json/manifest.toml");
        assert_eq!(
            parse_first_party_manifest(json).unwrap().capabilities,
            [Capability::ClipboardWrite]
        );
        assert!(parse_first_party_manifest(
            &json.replace("protocol_version = 2", "protocol_version = 1")
        )
        .is_err());
        assert!(
            parse_first_party_manifest(&json.replace("clipboard.write", "unknown.operation"))
                .is_err()
        );
        assert!(parse_first_party_manifest(&(json.to_owned() + "runtime = \"shell\"\n")).is_err());
    }
}
