//! Declarative manifests; no executable runtime or UI.
use anyhow::{bail, Context, Result};
use launcher_protocol::{DeclarativeAction, PluginManifest};
use std::{collections::HashSet, path::Path};

pub const EXAMPLE_GOOGLE: &str = include_str!("../../../plugins/examples/google.toml");
const VARIABLE: &str = "{{context.value}}";

pub fn parse_manifest(raw: &str) -> Result<PluginManifest> {
    if raw.len() > 256 * 1024 {
        bail!("manifest exceeds 256 KiB");
    }
    let manifest: PluginManifest = toml::from_str(raw).context("invalid plugin TOML")?;
    if manifest.protocol_version != 1 {
        bail!(
            "unsupported protocol_version {}; expected 1",
            manifest.protocol_version
        );
    }
    if !identifier(&manifest.id) || manifest.id == "core" || manifest.id.starts_with("core.") {
        bail!("invalid or reserved plugin id");
    }
    if manifest.name.trim().is_empty()
        || manifest.commands.is_empty()
        || manifest.commands.len() > 100
    {
        bail!("plugin name and 1..100 commands are required");
    }
    let mut ids = HashSet::new();
    for command in &manifest.commands {
        if !identifier(&command.id) || !ids.insert(&command.id) {
            bail!("invalid or duplicate command id: {}", command.id);
        }
        if command.title.trim().is_empty() || command.title.len() > 200 {
            bail!("command {} requires a title of 1..200 bytes", command.id);
        }
        if command.accepts.is_empty() || command.accepts.iter().any(|kind| !type_identifier(kind)) {
            bail!("command {} has invalid context matcher", command.id);
        }
        if !(-50..=50).contains(&command.base_priority) {
            bail!("base_priority must be in -50..50");
        }
        match &command.action {
            DeclarativeAction::OpenUrl { url } => {
                validate_template(url)?;
                let sample = if url == VARIABLE {
                    "https://example.com".into()
                } else {
                    url.replace(VARIABLE, "example")
                };
                validate_url(&sample)?;
            }
            DeclarativeAction::OpenFile { path } => {
                validate_template(path)?;
                if path.is_empty() {
                    bail!("open_file requires path");
                }
            }
            DeclarativeAction::Copy { text } => validate_template(text)?,
            DeclarativeAction::ExecuteBuiltin { command_id } => {
                if !command_id.starts_with("core.") || !identifier(command_id) {
                    bail!("execute_builtin requires a core command id");
                }
            }
        }
    }
    Ok(manifest)
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.split('.').all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
        })
}
fn type_identifier(value: &str) -> bool {
    value.contains('.') && identifier(value)
}

pub fn validate_template(template: &str) -> Result<()> {
    if template.len() > 64 * 1024 {
        bail!("template exceeds 64 KiB");
    }
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        if rest[..start].contains("}}") {
            bail!("unmatched template delimiter");
        }
        let end = rest[start..]
            .find("}}")
            .context("unclosed template variable")?
            + start
            + 2;
        if &rest[start..end] != VARIABLE {
            bail!("unknown template variable: {}", &rest[start..end]);
        }
        rest = &rest[end..];
    }
    if rest.contains("}}") {
        bail!("unmatched template delimiter");
    }
    Ok(())
}

/// URL substitutions are data: percent-encode query/path values, never shell-escape.
pub fn interpolate(template: &str, value: &str, for_url: bool) -> Result<String> {
    validate_template(template)?;
    let replacement = if for_url && template != VARIABLE {
        value
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect::<String>()
    } else {
        value.to_owned()
    };
    let occurrences = template.matches(VARIABLE).count();
    let rendered_size = replacement
        .len()
        .checked_mul(occurrences)
        .and_then(|size| size.checked_add(template.len() - occurrences * VARIABLE.len()));
    if rendered_size.is_none_or(|size| size > 4 * 1024 * 1024) {
        bail!("expanded action exceeds 4 MiB");
    }
    let rendered = template.replace(VARIABLE, &replacement);
    if for_url {
        validate_url(&rendered)?;
    }
    Ok(rendered)
}

fn validate_url(raw: &str) -> Result<()> {
    let url = url::Url::parse(raw).context("invalid open_url URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("open_url only permits absolute http/https URLs");
    }
    Ok(())
}

#[derive(Default)]
pub struct Discovery {
    pub manifests: Vec<PluginManifest>,
    pub diagnostics: Vec<String>,
}

/// Discover flat *.toml manifests and <plugin>/manifest.toml; deterministic path order.
pub fn discover(directory: &Path) -> Discovery {
    let mut result = Discovery::default();
    if !directory.exists() {
        return result;
    }
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            result
                .diagnostics
                .push(format!("{}: {error}", directory.display()));
            return result;
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_dir() {
                    let manifest = path.join("manifest.toml");
                    if manifest.is_file() {
                        paths.push(manifest);
                    }
                } else if path.extension().is_some_and(|ext| ext == "toml") {
                    paths.push(path);
                }
            }
            Err(error) => result
                .diagnostics
                .push(format!("{}: {error}", directory.display())),
        }
    }
    paths.sort();
    for path in paths {
        let load = (|| -> Result<PluginManifest> {
            if std::fs::metadata(&path)?.len() > 256 * 1024 {
                bail!("manifest exceeds 256 KiB");
            }
            parse_manifest(&std::fs::read_to_string(&path)?)
        })();
        match load {
            Ok(manifest) => result.manifests.push(manifest),
            Err(error) => {
                let diagnostic = format!("{}: {error:#}", path.display());
                tracing::error!(path = %path.display(), error = %error, "plugin_load_failure");
                result.diagnostics.push(diagnostic);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_manifest_and_all_validation_failures() {
        assert_eq!(parse_manifest(EXAMPLE_GOOGLE).unwrap().commands.len(), 1);
        for raw in [
            EXAMPLE_GOOGLE.replace("protocol_version = 1", "protocol_version = 2"),
            EXAMPLE_GOOGLE.replace("open_url", "run_shell"),
            EXAMPLE_GOOGLE.replace("context.value", "context.secret"),
            EXAMPLE_GOOGLE.replace("core.text", "bad matcher"),
            EXAMPLE_GOOGLE.replace("https://www.google.com", "javascript:alert(1)"),
            EXAMPLE_GOOGLE.replace("url =", "missing ="),
            format!(
                "{}\n{}",
                EXAMPLE_GOOGLE,
                &EXAMPLE_GOOGLE[EXAMPLE_GOOGLE.find("[[commands]]").unwrap()..]
            ),
        ] {
            assert!(
                parse_manifest(&raw).is_err(),
                "accepted invalid manifest: {raw}"
            );
        }
        for template in [
            "{{context.value",
            "context.value}}",
            "{{{context.value}}}",
            "{{unknown}}",
        ] {
            assert!(validate_template(template).is_err());
        }
        assert!(validate_template(r#"{"data":"{{context.value}}"}"#).is_ok());
    }
    #[test]
    fn interpolation_encodes_url_data_and_preserves_copy_text() {
        let value = "hello 世界 &admin=true#x/%";
        let result = interpolate(
            "https://www.google.com/search?q={{context.value}}",
            value,
            true,
        )
        .unwrap();
        let url = url::Url::parse(&result).unwrap();
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![("q".into(), value.into())]
        );
        assert_eq!(
            interpolate("{{context.value}}", value, false).unwrap(),
            value
        );
        assert!(interpolate(VARIABLE, "javascript:alert(1)", true).is_err());
        assert!(interpolate(&VARIABLE.repeat(8), &"a".repeat(1024 * 1024), false).is_err());
    }
}
