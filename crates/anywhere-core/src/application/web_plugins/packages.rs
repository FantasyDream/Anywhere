//! Package approval and catalog publication use case; disk mechanics stay in the repository.
use crate::{
    application::web_plugins::{WebPackageApproval, WebPluginCatalog},
    persistence::web_packages::{Receipt, WebPackageRepository, WebResourceFile},
    Application,
};
use anyhow::{ensure, Context, Result};
use anywhere_plugin::web::VerifiedWebPackage;
use anywhere_protocol::web_plugin::{WebPermission, WebPlatform};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct WebPackageStatus {
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub entry: String,
    pub enabled: bool,
    pub permissions: Vec<WebPermission>,
    pub rollback: bool,
}
pub struct WebPackageReview {
    pub token: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub permissions: Vec<WebPermission>,
    pub updating: bool,
}
struct Pending {
    token: String,
    package: VerifiedWebPackage,
}
pub struct WebPackageStore {
    repository: WebPackageRepository,
    platform: WebPlatform,
    receipts: BTreeMap<String, Receipt>,
    pending: Option<Pending>,
    statuses: Vec<WebPackageStatus>,
    assets: BTreeMap<(String, String), WebResourceFile>,
}
impl WebPackageStore {
    pub fn open(root: PathBuf, platform: WebPlatform, app: &Application) -> Result<Self> {
        let (repository, receipts) = WebPackageRepository::open(root)?;
        let mut store = Self {
            repository,
            platform,
            receipts,
            pending: None,
            statuses: vec![],
            assets: BTreeMap::new(),
        };
        let prepared = store.prepare(&store.receipts)?;
        store.publish(app, prepared)?;
        Ok(store)
    }
    pub fn statuses(&self) -> Vec<WebPackageStatus> {
        self.statuses.clone()
    }
    pub fn inspect(&mut self, path: &Path) -> Result<WebPackageReview> {
        // A new inspection supersedes the old review even when validation fails.
        // Release its compressed snapshot before capturing another archive.
        self.pending = None;
        // Capture before presenting review. Replacing/deleting the original is harmless.
        let package = VerifiedWebPackage::read(std::fs::File::open(path)?)?;
        ensure!(
            package.manifest().platforms.contains(&self.platform),
            "unsupported Web platform"
        );
        ensure!(
            self.receipts.len() < 64 || self.receipts.contains_key(&package.manifest().id),
            "too many Web packages"
        );
        let token = super::token()?;
        let m = package.manifest();
        let review = WebPackageReview {
            token: token.clone(),
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            digest: package.digest().into(),
            permissions: m.permissions.clone(),
            updating: self.receipts.contains_key(&m.id),
        };
        self.pending = Some(Pending { token, package });
        Ok(review)
    }
    pub fn cancel_review(&mut self) {
        self.pending = None;
    }
    pub fn approve(&mut self, app: &Application, token: &str) -> Result<()> {
        ensure!(
            self.pending.as_ref().is_some_and(|p| p.token == token),
            "Web package review expired"
        );
        let pending = self.pending.take().context("Web package review expired")?;
        let package = pending.package;
        let id = package.manifest().id.clone();
        let digest = package.digest().to_owned();
        self.repository.stage(&package)?;
        let old = self.receipts.get(&id);
        // Explicit same-ID local update approval is required even without new permissions.
        // Identity continuity is local only; it is not a publisher signature.
        let receipt = Receipt {
            id: id.clone(),
            digest: digest.clone(),
            source: old.map(|r| r.source.clone()).unwrap_or(super::token()?),
            enabled: true,
            previous: old.filter(|r| r.digest != digest).map(|r| r.digest.clone()),
        };
        let mut next = self.receipts.clone();
        next.insert(id, receipt);
        self.change(app, next)
    }
    pub fn set_enabled(&mut self, app: &Application, id: &str, enabled: bool) -> Result<()> {
        let current = self.receipts.get(id).context("Web package not installed")?;
        // A repeated setting is not a catalog mutation: retain document tickets,
        // action generations and the user's draft instead of closing every page.
        if current.enabled == enabled {
            return Ok(());
        }
        let mut next = self.receipts.clone();
        next.get_mut(id).expect("receipt checked above").enabled = enabled;
        self.change(app, next)
    }
    pub fn uninstall(&mut self, app: &Application, id: &str) -> Result<()> {
        let mut next = self.receipts.clone();
        let removed = next.remove(id).context("Web package not installed")?;
        let namespace = super::storage_namespace(&removed.id, &removed.source);
        self.change(app, next)?;
        app.clear_web_storage_namespace(&namespace)?;
        self.repository.collect_unused(&self.receipts);
        Ok(())
    }
    pub fn rollback(&mut self, app: &Application, id: &str) -> Result<()> {
        let mut next = self.receipts.clone();
        let receipt = next.get_mut(id).context("Web package not installed")?;
        let previous = receipt
            .previous
            .take()
            .context("no approved Web version to restore")?;
        receipt.previous = Some(std::mem::replace(&mut receipt.digest, previous));
        self.change(app, next)
    }
    fn change(&mut self, app: &Application, next: BTreeMap<String, Receipt>) -> Result<()> {
        // Validation and persistence failures leave the active catalog and document intact.
        let prepared = self.prepare(&next)?;
        self.repository.save(&next)?;
        if let Err(error) = self.publish(app, prepared) {
            self.repository
                .save(&self.receipts)
                .context("restoring Web receipts")?;
            return Err(error);
        }
        self.receipts = next;
        self.repository.collect_unused(&self.receipts);
        Ok(())
    }
    fn prepare(&self, receipts: &BTreeMap<String, Receipt>) -> Result<PreparedCatalog> {
        let mut catalog = WebPluginCatalog::default();
        let mut statuses = vec![];
        let mut assets = BTreeMap::new();
        for receipt in receipts.values() {
            let package = self.repository.package(&receipt.digest)?;
            ensure!(
                package.digest() == receipt.digest && package.manifest().id == receipt.id,
                "installed Web package changed"
            );
            let m = package.manifest();
            statuses.push(WebPackageStatus {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                digest: receipt.digest.clone(),
                entry: m.entry.clone(),
                enabled: receipt.enabled,
                permissions: m.permissions.clone(),
                rollback: receipt.previous.is_some(),
            });
            if !receipt.enabled {
                continue;
            }
            catalog.merge(WebPluginCatalog::prepare(
                self.platform,
                [(
                    &package,
                    WebPackageApproval {
                        plugin_id: receipt.id.clone(),
                        content_digest: receipt.digest.clone(),
                        permissions: m.permissions.clone(),
                        source_id: receipt.source.clone(),
                    },
                )],
            )?)?;
            if let Some(background) = &m.background {
                let asset = package
                    .assets()
                    .find(|a| a.path() == background.entry)
                    .context("background resource missing")?;
                catalog
                    .entries
                    .get_mut(&m.id)
                    .expect("prepared package")
                    .background_file = Some(self.repository.resource_file(&receipt.digest, asset));
            }
            for a in package.assets().filter(|a| a.path() != "manifest.json") {
                assets.insert(
                    (receipt.digest.clone(), a.path().into()),
                    self.repository.resource_file(&receipt.digest, a),
                );
            }
        }
        Ok(PreparedCatalog {
            catalog,
            statuses,
            assets,
        })
    }
    fn publish(&mut self, app: &Application, prepared: PreparedCatalog) -> Result<()> {
        app.publish_web_catalog(prepared.catalog)?;
        self.assets = prepared.assets;
        self.statuses = prepared.statuses;
        Ok(())
    }
    /// Only metadata from the verified catalog is addressable. The returned descriptor
    /// owns no archive bytes and can be read after releasing the management lock.
    pub fn resource_file(&self, digest: &str, path: &str) -> Result<WebResourceFile> {
        self.assets
            .get(&(digest.into(), path.into()))
            .cloned()
            .context("Web resource is not approved")
    }
    pub fn resource(&self, digest: &str, path: &str) -> Result<Vec<u8>> {
        self.resource_file(digest, path)?.read()
    }
}
struct PreparedCatalog {
    catalog: WebPluginCatalog,
    statuses: Vec<WebPackageStatus>,
    assets: BTreeMap<(String, String), WebResourceFile>,
}
