//! Metadata for code compiled into the App, not a versioned plugin protocol.
use anyhow::{ensure, Result};
use anywhere_protocol::extension::ExtensionCapability;
use std::collections::BTreeSet;

/// Built-in features share the App release; they cannot negotiate a package or ABI version.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub capabilities: &'static [ExtensionCapability],
}

impl FeatureDescriptor {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        capabilities: &'static [ExtensionCapability],
    ) -> Self {
        Self {
            id,
            name,
            capabilities,
        }
    }

    /// Metadata validity is separate from Host approval of the requested operations.
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            valid_id(self.id),
            "invalid or reserved builtin feature id: {}",
            self.id
        );
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 200,
            "builtin feature requires a name of 1..200 bytes"
        );
        ensure!(
            self.capabilities.len() <= 8
                && !self
                    .capabilities
                    .contains(&ExtensionCapability::InvokeBuiltin),
            "invalid builtin feature capabilities"
        );
        ensure!(
            self.capabilities
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                == self.capabilities.len(),
            "duplicate builtin feature capability"
        );
        Ok(())
    }
}

pub(crate) const fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || id_is_reserved(bytes) {
        return false;
    }
    let mut index = 0;
    let mut start = true;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'.' {
            if start {
                return false;
            }
            start = true;
        } else {
            let lower = byte >= b'a' && byte <= b'z';
            if start && !lower {
                return false;
            }
            if !lower && !(byte >= b'0' && byte <= b'9') && byte != b'_' && byte != b'-' {
                return false;
            }
            start = false;
        }
        index += 1;
    }
    !start
}

const fn id_is_reserved(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && bytes[0] == b'c'
        && bytes[1] == b'o'
        && bytes[2] == b'r'
        && bytes[3] == b'e'
        && (bytes.len() == 4 || bytes[4] == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExtensionCapability as Capability;

    #[test]
    fn native_metadata_rejects_reserved_ids_empty_names_and_invalid_permissions() {
        for id in ["", "Bad", "bad/path", "bad..path", "core", "core.tool"] {
            assert!(FeatureDescriptor::new(id, "Tool", &[]).validate().is_err());
        }
        assert!(FeatureDescriptor::new("core_tools", "Tool", &[])
            .validate()
            .is_ok());
        for name in ["", " \t\n"] {
            assert!(FeatureDescriptor::new("tool", name, &[])
                .validate()
                .is_err());
        }
        assert!(
            FeatureDescriptor::new("tool", "工具", &[Capability::ClipboardWrite])
                .validate()
                .is_ok()
        );
        assert!(FeatureDescriptor::new(
            "tool",
            "Tool",
            &[Capability::ClipboardWrite, Capability::ClipboardWrite]
        )
        .validate()
        .is_err());
        assert!(
            FeatureDescriptor::new("tool", "Tool", &[Capability::InvokeBuiltin])
                .validate()
                .is_err()
        );
    }
}
