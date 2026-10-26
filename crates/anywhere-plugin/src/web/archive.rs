use super::{
    icon, manifest, path, preflight, MAX_ARCHIVE_BYTES, MAX_ASSET_BYTES, MAX_COMPRESSION_RATIO,
    MAX_EXPANDED_BYTES, MAX_MANIFEST_BYTES,
};
use anyhow::{ensure, Context, Result};
use anywhere_protocol::web_plugin::WebPluginManifest;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};
use zip::{
    read::{ArchiveOffset, Config},
    CompressionMethod, ZipArchive,
};

/// Validated resource metadata. Constructed only after every member passes CRC and hash checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAsset {
    path: String,
    size: u64,
    sha256: [u8; 32],
    index: usize,
}

impl VerifiedAsset {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn sha256(&self) -> String {
        hex(&self.sha256)
    }
}

/// Installation-time immutable compressed snapshot, not a permanent runtime cache.
/// No filesystem writes, grants, registry updates or code execution occur here.
pub struct VerifiedWebPackage {
    manifest: WebPluginManifest,
    assets: BTreeMap<String, VerifiedAsset>,
    digest: String,
    archive: Vec<u8>,
}

impl VerifiedWebPackage {
    pub fn read(mut source: impl Read) -> Result<Self> {
        let archive = capture(&mut source, MAX_ARCHIVE_BYTES)?;
        let expected_count = preflight::check(&archive)?;
        let mut zip = open(&archive)?;
        ensure!(zip.len() == expected_count, "duplicate Web ZIP entry");
        zip.validate_header_consistency()
            .context("inconsistent Web ZIP headers")?;
        let mut assets = BTreeMap::new();
        let mut nodes = BTreeMap::new();
        let mut file_keys = BTreeSet::new();
        let mut expanded = 0u64;
        for index in 0..zip.len() {
            let file = zip.by_index_data(index)?;
            ensure!(!file.encrypted(), "encrypted Web assets are unsupported");
            ensure!(
                matches!(
                    file.compression(),
                    CompressionMethod::Stored | CompressionMethod::Deflated
                ),
                "unsupported Web asset compression"
            );
            let raw =
                std::str::from_utf8(file.name_raw()).context("Web asset name must be UTF-8")?;
            ensure!(file.name()?.as_ref() == raw, "ambiguous Web asset encoding");
            let directory = file.is_dir();
            let name = if directory {
                raw.strip_suffix('/').unwrap_or(raw)
            } else {
                raw
            };
            path::validate(name)?;
            ensure!(!file.is_symlink(), "Web package links are unsupported");
            if let Some(mode) = file.unix_mode() {
                let kind = mode & 0o170000;
                let expected = if directory { 0o040000 } else { 0o100000 };
                ensure!(
                    (kind == 0 || kind == expected) && mode & 0o7000 == 0,
                    "unsupported Web asset file mode"
                );
            }
            register_path(&mut nodes, &mut file_keys, name, directory)?;
            if directory {
                ensure!(file.size() == 0, "Web directory contains data");
                continue;
            }
            ensure!(path::allowed_asset(name), "unsupported Web asset type");
            let limit = if name == "manifest.json" {
                MAX_MANIFEST_BYTES as u64
            } else {
                MAX_ASSET_BYTES
            };
            ensure!(file.size() <= limit, "Web asset exceeds size limit");
            ensure!(
                file.size()
                    <= file
                        .compressed_size()
                        .max(1)
                        .saturating_mul(MAX_COMPRESSION_RATIO),
                "Web asset compression ratio exceeds limit"
            );
            expanded = expanded
                .checked_add(file.size())
                .context("Web package size overflow")?;
            ensure!(
                expanded <= MAX_EXPANDED_BYTES,
                "Web package exceeds expanded size limit"
            );
            assets.insert(
                name.to_owned(),
                VerifiedAsset {
                    path: name.to_owned(),
                    size: file.size(),
                    sha256: [0; 32],
                    index,
                },
            );
        }
        let manifest_info = assets
            .get("manifest.json")
            .context("missing root manifest.json")?;
        let manifest_bytes = read_bytes(&mut zip, manifest_info, MAX_MANIFEST_BYTES as u64)?;
        let manifest = manifest::parse_manifest(&manifest_bytes)?;
        ensure!(
            manifest.entry.is_empty() || assets.contains_key(&manifest.entry),
            "missing Web entry resource"
        );
        if let Some(background) = &manifest.background {
            let asset = assets
                .get(&background.entry)
                .context("missing background entry resource")?;
            ensure!(
                asset.size <= 256 * 1024,
                "background bundle exceeds 256 KiB"
            );
        }
        if let Some(icon) = &manifest.icon {
            let info = assets.get(icon).context("missing Web icon resource")?;
            ensure!(info.size <= icon::MAX_BYTES, "Web icon exceeds 256 KiB");
            icon::validate(&read_bytes(&mut zip, info, icon::MAX_BYTES)?)?;
        }
        for info in assets.values_mut() {
            info.sha256 = if info.path == "manifest.json" {
                Sha256::digest(&manifest_bytes).into()
            } else {
                hash_asset(&mut zip, info)?
            };
        }
        let digest = content_digest(&assets);
        Ok(Self {
            manifest,
            assets,
            digest,
            archive,
        })
    }

    /// Persist the exact inspected bytes; approval never reopens the import path.
    pub fn write_archive(&self, mut target: impl std::io::Write) -> Result<()> {
        target.write_all(&self.archive)?;
        Ok(())
    }

    pub fn manifest(&self) -> &WebPluginManifest {
        &self.manifest
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn assets(&self) -> impl Iterator<Item = &VerifiedAsset> {
        self.assets.values()
    }

    /// Stable logical identity, deliberately independent of package version/hash.
    pub fn action_id(&self, command: &str) -> Option<String> {
        self.manifest
            .commands
            .iter()
            .any(|item| item.id == command)
            .then(|| format!("web:{}:{command}", self.manifest.id))
    }

    /// Exact catalog lookup; no URL decoding, path repair or case-insensitive fallback.
    /// The host chooses the byte budget, not a page-supplied request field.
    pub fn read_asset(&self, name: &str, max_bytes: u64) -> Result<Vec<u8>> {
        let info = self.assets.get(name).context("unknown Web resource")?;
        let mut zip = open(&self.archive)?;
        let value = read_bytes(&mut zip, info, max_bytes.min(MAX_ASSET_BYTES))?;
        ensure!(
            <[u8; 32]>::from(Sha256::digest(&value)) == info.sha256,
            "Web resource digest mismatch"
        );
        Ok(value)
    }
}

fn capture(reader: &mut impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "Web archive exceeds compressed size limit"
    );
    Ok(bytes)
}

fn open(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>> {
    Ok(ZipArchive::with_config(
        Config {
            archive_offset: ArchiveOffset::Known(0),
        },
        Cursor::new(bytes),
    )?)
}

fn register_path(
    nodes: &mut BTreeMap<String, String>,
    files: &mut BTreeSet<String>,
    name: &str,
    directory: bool,
) -> Result<()> {
    let mut prefix = String::new();
    for part in name.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        let key = path::key(&prefix);
        if let Some(existing) = nodes.get(&key) {
            ensure!(existing == &prefix, "Web asset Unicode/case collision");
        }
        ensure!(!files.contains(&key), "Web asset file/directory collision");
        nodes.insert(key, prefix.clone());
    }
    if !directory {
        let key = path::key(name);
        let prefix = format!("{key}/");
        ensure!(
            !nodes
                .range(prefix.clone()..)
                .next()
                .is_some_and(|(node, _)| node.starts_with(&prefix)),
            "Web asset file/directory collision"
        );
        files.insert(key);
    }
    Ok(())
}

fn read_bytes(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    info: &VerifiedAsset,
    limit: u64,
) -> Result<Vec<u8>> {
    ensure!(
        info.size <= limit,
        "Web resource exceeds caller byte budget"
    );
    let mut value = Vec::new();
    zip.by_index(info.index)?
        .take(limit + 1)
        .read_to_end(&mut value)?;
    ensure!(
        value.len() as u64 == info.size && value.len() as u64 <= limit,
        "Web resource expanded size mismatch"
    );
    Ok(value)
}

fn hash_asset(zip: &mut ZipArchive<Cursor<&[u8]>>, info: &VerifiedAsset) -> Result<[u8; 32]> {
    let mut file = zip.by_index(info.index)?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let remaining = (info.size - size + 1).min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..remaining])?;
        if read == 0 {
            break;
        }
        size += read as u64;
        ensure!(size <= info.size, "Web resource expanded size mismatch");
        hash.update(&buffer[..read]);
    }
    ensure!(size == info.size, "Web resource expanded size mismatch");
    Ok(hash.finalize().into())
}

fn content_digest(assets: &BTreeMap<String, VerifiedAsset>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"anywhere-web-package-v1\0");
    for info in assets.values() {
        hash.update((info.path.len() as u64).to_le_bytes());
        hash.update(info.path.as_bytes());
        hash.update(info.size.to_le_bytes());
        hash.update(info.sha256);
    }
    hex(&hash.finalize().into())
}

fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String");
    }
    value
}
