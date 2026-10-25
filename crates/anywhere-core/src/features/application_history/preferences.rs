//! Preference versions preserve explicit custom paths and disabled sources.
use super::model::*;
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Preferences {
    pub schema: u32,
    pub sources: Vec<SourceConfiguration>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPreferences {
    schema: u32,
    sources: Vec<SourcePreference>,
}

pub(super) fn decode(
    bytes: Option<&[u8]>,
    descriptors: &[SourceDescriptor],
) -> Result<Vec<SourceConfiguration>> {
    let Some(bytes) = bytes else {
        return Ok(vec![]);
    };
    #[derive(Deserialize)]
    struct Version {
        schema: u32,
    }
    let version: Version = serde_json::from_slice(bytes)?;
    let sources = match version.schema {
        1 => {
            let p: LegacyPreferences = serde_json::from_slice(bytes)?;
            ensure!(p.schema == 1, "unsupported history preferences");
            let mut migrated: Vec<_> = p
                .sources
                .into_iter()
                .map(|s| SourceConfiguration {
                    id: s.id,
                    mode: SourceMode::Custom,
                    path: Some(s.path),
                    action_enabled: true,
                })
                .collect();
            // Old omissions meant off. Preserve that choice during migration.
            for descriptor in descriptors {
                if descriptor.built_in && !migrated.iter().any(|s| s.id == descriptor.id) {
                    migrated.push(SourceConfiguration {
                        id: descriptor.id.clone(),
                        mode: SourceMode::Disabled,
                        path: None,
                        action_enabled: false,
                    });
                }
            }
            migrated
        }
        2 => serde_json::from_slice::<Preferences>(bytes)?.sources,
        _ => bail!("unsupported history preferences"),
    };
    validate(&sources)?;
    Ok(sources)
}

pub(super) fn validate(sources: &[SourceConfiguration]) -> Result<()> {
    // Removed user adapters keep inactive settings without blocking the catalog.
    ensure!(sources.len() <= 64, "too many history sources");
    let mut ids = std::collections::BTreeSet::new();
    for source in sources {
        ensure!(
            ids.insert(&source.id)
                && crate::features::descriptor::valid_id(&source.id)
                && source.id.len() <= 64,
            "invalid or duplicate source"
        );
        ensure!(
            match (&source.mode, &source.path) {
                (SourceMode::Custom, Some(path)) =>
                    path.is_absolute() && path.as_os_str().len() <= 16 * 1024,
                (SourceMode::Automatic | SourceMode::Disabled, None) => true,
                _ => false,
            },
            "invalid source path"
        );
    }
    Ok(())
}
