//! Disk adapter for bounded Web package receipts and immutable resources. No catalog
//! publication, Application access, permission decisions or UI operations belong here.
use anyhow::{ensure, Context, Result};
use anywhere_plugin::web::{VerifiedAsset, VerifiedWebPackage, MAX_ASSET_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_THIRD_PARTY_RECEIPTS: usize = 64;
const MAX_RECEIPTS_BYTES: usize = 256 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub id: String,
    pub digest: String,
    pub source: String,
    pub enabled: bool,
    pub previous: Option<String>,
}

pub(crate) struct WebPackageRepository {
    root: PathBuf,
    builtin_ids: BTreeSet<String>,
}
#[derive(Clone)]
pub struct WebResourceFile {
    root: PathBuf,
    path: PathBuf,
    size: u64,
    sha256: String,
}
impl WebResourceFile {
    pub fn read(&self) -> Result<Vec<u8>> {
        let bytes = read_regular_within(&self.path, Some(&self.root), self.size)?;
        ensure!(
            bytes.len() as u64 == self.size && hex::encode(Sha256::digest(&bytes)) == self.sha256,
            "Web resource changed"
        );
        Ok(bytes)
    }
}
impl WebPackageRepository {
    pub fn open(
        root: PathBuf,
        builtin_ids: BTreeSet<String>,
    ) -> Result<(Self, BTreeMap<String, Receipt>)> {
        fs::create_dir_all(&root)?;
        let root = fs::canonicalize(root)?;
        let ledger = root.join("receipts.json");
        let repository = Self { root, builtin_ids };
        let receipts = if ledger.exists() {
            let bytes = read_regular(&ledger, MAX_RECEIPTS_BYTES as u64)?;
            let receipts: BTreeMap<String, Receipt> = serde_json::from_slice(&bytes)?;
            repository.validate_count(&receipts)?;
            for (id, item) in &receipts {
                ensure!(
                    id == &item.id
                        && valid_digest(&item.digest)
                        && item.previous.as_ref().is_none_or(|p| valid_digest(p)),
                    "invalid Web receipt"
                );
            }
            receipts
        } else {
            BTreeMap::new()
        };
        Ok((repository, receipts))
    }
    pub fn is_builtin(&self, id: &str) -> bool {
        self.builtin_ids.contains(id)
    }
    pub fn can_insert(&self, receipts: &BTreeMap<String, Receipt>, id: &str) -> bool {
        self.is_builtin(id)
            || receipts.contains_key(id)
            || self.third_party_count(receipts) < MAX_THIRD_PARTY_RECEIPTS
    }
    fn third_party_count(&self, receipts: &BTreeMap<String, Receipt>) -> usize {
        receipts.keys().filter(|id| !self.is_builtin(id)).count()
    }
    fn validate_count(&self, receipts: &BTreeMap<String, Receipt>) -> Result<()> {
        ensure!(
            self.third_party_count(receipts) <= MAX_THIRD_PARTY_RECEIPTS,
            "too many Web receipts"
        );
        Ok(())
    }
    pub fn package(&self, digest: &str) -> Result<VerifiedWebPackage> {
        let path = self.root.join(digest).join("package.awplugin");
        VerifiedWebPackage::read(std::io::Cursor::new(read_regular(
            &path,
            anywhere_plugin::web::MAX_ARCHIVE_BYTES as u64,
        )?))
    }
    pub fn resource_file(&self, digest: &str, asset: &VerifiedAsset) -> WebResourceFile {
        WebResourceFile {
            root: self.root.clone(),
            path: self.root.join(digest).join("assets").join(asset.path()),
            size: asset.size(),
            sha256: asset.sha256(),
        }
    }
    pub fn save(&self, receipts: &BTreeMap<String, Receipt>) -> Result<()> {
        self.validate_count(receipts)?;
        let bytes = serde_json::to_vec(receipts)?;
        ensure!(
            bytes.len() <= MAX_RECEIPTS_BYTES,
            "Web receipts exceed size limit"
        );
        let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(self.root.join("receipts.json"))
            .map_err(|e| e.error)?;
        Ok(())
    }
    pub fn stage(&self, package: &VerifiedWebPackage) -> Result<()> {
        let destination = self.root.join(package.digest());
        if destination.exists() {
            // Same digest is reusable only while both the archive and all extracted
            // assets are intact. Otherwise reapproval would report success but the
            // page would still fail at the resource boundary. Do not overwrite an
            // installed directory in-place; uninstall/reimport is the recovery path.
            let installed = self.package(package.digest())?;
            ensure!(
                installed.digest() == package.digest(),
                "cached Web package changed"
            );
            for asset in package.assets() {
                self.resource_file(package.digest(), asset).read()?;
            }
            return Ok(());
        }
        let stage = tempfile::tempdir_in(&self.root)?;
        let mut archive = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage.path().join("package.awplugin"))?;
        package.write_archive(&mut archive)?;
        archive.sync_all()?;
        for asset in package.assets() {
            let path = stage.path().join("assets").join(asset.path());
            fs::create_dir_all(path.parent().context("invalid Web resource parent")?)?;
            let bytes = package.read_asset(asset.path(), MAX_ASSET_BYTES)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(&bytes)?;
        }
        fs::rename(stage.path(), destination)?;
        Ok(())
    }
    pub fn collect_unused(&self, receipts: &BTreeMap<String, Receipt>) {
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if valid_digest(&name)
                    && !receipts
                        .values()
                        .any(|r| r.digest == name || r.previous.as_ref() == Some(&name))
                {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
    }
}
fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    read_regular_within(path, None, limit)
}

fn read_regular_within(path: &Path, stop_at: Option<&Path>, limit: u64) -> Result<Vec<u8>> {
    // Never follow an installed symlink, including a substituted parent directory.
    for ancestor in path.ancestors() {
        ensure!(
            !fs::symlink_metadata(ancestor)?.file_type().is_symlink(),
            "Web package links are unsupported"
        );
        if stop_at == Some(ancestor) {
            break;
        }
    }
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit,
        "Web resource exceeds limit"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Web resource exceeds limit");
    Ok(bytes)
}
