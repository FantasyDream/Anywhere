//! Package approval and catalog publication use case; disk mechanics stay in the repository.
use crate::{
    application::web_plugins::{WebPackageApproval, WebPluginCatalog},
    persistence::web_packages::{Receipt, WebPackageRepository, WebResourceFile},
    Application,
};
use anyhow::{ensure, Context, Result};
use anywhere_plugin::web::VerifiedWebPackage;
use anywhere_protocol::web_plugin::{WebCommand, WebPermission, WebPlatform};
use std::{
    collections::{BTreeMap, BTreeSet},
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
    pub available: bool,
    pub permissions: Vec<WebPermission>,
    pub rollback: bool,
    pub builtin: bool,
    pub open_action_id: Option<String>,
}
/// Approval screen for the exact package bytes captured during inspection.
#[derive(Debug)]
pub struct WebPackageReview {
    pub token: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub permissions: Vec<WebPermission>,
    pub updating: bool,
    pub commands: Vec<WebCommand>,
    pub command_count: usize,
    pub resolver_count: usize,
    pub background_action_count: usize,
    pub permissions_added: Vec<WebPermission>,
    pub permissions_removed: Vec<WebPermission>,
    pub permission_scopes: Vec<(WebPermission, WebPermission)>,
    pub same_version: Option<SameWebPackageVersion>,
}

/// Enabled state of an installed package with the same content digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameWebPackageVersion {
    Enabled,
    Disabled,
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
        Self::open_with_builtins(root, platform, &[], app)
    }

    pub fn open_with_builtins(
        root: PathBuf,
        platform: WebPlatform,
        builtins: &[&[u8]],
        app: &Application,
    ) -> Result<Self> {
        let mut packages = Vec::new();
        let mut builtin_ids = BTreeSet::new();
        for archive in builtins {
            let package = VerifiedWebPackage::read(std::io::Cursor::new(*archive))?;
            if package.manifest().platforms.contains(&platform) {
                builtin_ids.insert(package.manifest().id.clone());
                packages.push(package);
            }
        }
        let (repository, mut receipts) = WebPackageRepository::open(root, builtin_ids)?;
        let mut changed = false;
        for package in packages {
            let id = package.manifest().id.clone();
            let digest = package.digest().to_owned();
            repository.stage(&package)?;
            if let Some(existing) = receipts.get_mut(&id) {
                // Keep user's enabled toggle and storage namespace intact when upgrading or migrating.
                if existing.digest != digest || existing.previous.is_some() {
                    existing.digest = digest;
                    existing.previous = None;
                    changed = true;
                }
            } else {
                receipts.insert(
                    id.clone(),
                    Receipt {
                        id: id.clone(),
                        digest,
                        source: format!("builtin:{id}"),
                        enabled: true,
                        previous: None,
                    },
                );
                changed = true;
            }
        }
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
        if changed {
            store.repository.save(&store.receipts)?;
            store.repository.collect_unused(&store.receipts);
        }
        Ok(store)
    }
    pub fn statuses(&self) -> Vec<WebPackageStatus> {
        self.statuses.clone()
    }
    pub fn inspect(&mut self, path: &Path) -> Result<WebPackageReview> {
        ensure!(
            self.pending.is_none(),
            "finish or cancel the pending Web package review"
        );
        // Capture before presenting review. Replacing/deleting the original is harmless.
        ensure!(path.is_file(), "choose a local Web package file");
        let source = std::fs::File::open(path)?;
        ensure!(
            source.metadata()?.is_file(),
            "Web package source is not a regular file"
        );
        let package = VerifiedWebPackage::read(source)?;
        ensure!(
            package.manifest().platforms.contains(&self.platform),
            "unsupported Web platform"
        );
        ensure!(
            !self.repository.is_builtin(&package.manifest().id),
            "built-in Web package cannot be overwritten"
        );
        ensure!(
            self.repository
                .can_insert(&self.receipts, &package.manifest().id),
            "too many Web packages"
        );
        let token = super::token()?;
        let m = package.manifest();
        let installed = self.receipts.get(&m.id);
        let previous = self
            .statuses
            .iter()
            .find(|status| status.id == m.id && status.available)
            .map_or(&[][..], |status| status.permissions.as_slice());
        let mut permissions_added = Vec::new();
        let mut permission_scopes = Vec::new();
        for new in &m.permissions {
            match previous.iter().find(|old| old.name() == new.name()) {
                None => permissions_added.push(new.clone()),
                Some(old) if !same_permission(old, new) => {
                    permission_scopes.push((old.clone(), new.clone()))
                }
                _ => {}
            }
        }
        let permissions_removed = previous
            .iter()
            .filter(|old| m.permissions.iter().all(|new| new.name() != old.name()))
            .cloned()
            .collect();
        let review = WebPackageReview {
            token: token.clone(),
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            digest: package.digest().into(),
            permissions: m.permissions.clone(),
            commands: m.commands.iter().take(8).cloned().collect(),
            command_count: m.commands.len(),
            resolver_count: m
                .background
                .as_ref()
                .map_or(0, |background| background.resolvers.len()),
            background_action_count: m
                .commands
                .iter()
                .filter(|command| !command.runtime.is_webview())
                .count(),
            permissions_added,
            permissions_removed,
            permission_scopes,
            updating: installed.is_some_and(|receipt| receipt.digest != package.digest()),
            same_version: installed
                .filter(|receipt| receipt.digest == package.digest())
                .map(|receipt| {
                    if receipt.enabled {
                        SameWebPackageVersion::Enabled
                    } else {
                        SameWebPackageVersion::Disabled
                    }
                }),
        };
        self.pending = Some(Pending { token, package });
        Ok(review)
    }
    pub fn cancel_review(&mut self, token: &str) {
        // A late page cleanup must not retire another import's review.
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            self.pending = None;
        }
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
        let old = self.receipts.get(&id);
        if let Some(current) = old.filter(|receipt| receipt.digest == digest) {
            self.repository.validate_staged(&package)
                .context("cached Web package is unavailable; roll back or uninstall the package before importing again")?;
            return if current.enabled {
                Ok(())
            } else {
                self.set_enabled(app, &id, true)
            };
        }
        self.repository.stage(&package)?;
        // Explicit same-ID local update approval is required even without new permissions.
        // Identity continuity is local only; it is not a publisher signature.
        let receipt = Receipt {
            id: id.clone(),
            digest: digest.clone(),
            source: match old {
                Some(receipt) => receipt.source.clone(),
                None => super::token()?,
            },
            enabled: true,
            previous: old.map(|receipt| receipt.digest.clone()),
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
        ensure!(
            !self.repository.is_builtin(id),
            "built-in Web package cannot be uninstalled"
        );
        let mut next = self.receipts.clone();
        let removed = next.remove(id).context("Web package not installed")?;
        let namespace = super::storage_namespace(&removed.id, &removed.source);
        self.change(app, next)?;
        app.clear_web_package_diagnostics(id);
        app.clear_web_storage_namespace(&namespace)?;
        self.repository.collect_unused(&self.receipts);
        Ok(())
    }
    pub fn rollback(&mut self, app: &Application, id: &str) -> Result<()> {
        ensure!(
            !self.repository.is_builtin(id),
            "built-in Web package cannot be rolled back"
        );
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
        for status in &prepared.statuses {
            if status.enabled
                && !status.available
                && self
                    .receipts
                    .get(&status.id)
                    .is_none_or(|old| !old.enabled || old.digest != status.digest)
            {
                anyhow::bail!("Web package is unavailable; restore or uninstall it");
            }
        }
        self.repository.save(&next)?;
        if let Err(error) = self.publish(app, prepared) {
            self.repository
                .save(&self.receipts)
                .context("restoring Web receipts")?;
            return Err(error);
        }
        let invalidate_pending = self.pending.as_ref().is_some_and(|pending| {
            let id = &pending.package.manifest().id;
            self.receipts.get(id).map(|r| (&r.digest, r.enabled))
                != next.get(id).map(|r| (&r.digest, r.enabled))
        });
        self.receipts = next;
        if invalidate_pending {
            self.pending = None;
        }
        self.repository.collect_unused(&self.receipts);
        Ok(())
    }
    fn prepare(&self, receipts: &BTreeMap<String, Receipt>) -> Result<PreparedCatalog> {
        let mut catalog = WebPluginCatalog::default();
        let mut statuses = vec![];
        let mut assets = BTreeMap::new();
        for receipt in receipts.values() {
            let builtin = self.repository.is_builtin(&receipt.id);
            let package = self
                .repository
                .package(&receipt.digest)
                .and_then(|package| {
                    ensure!(
                        package.digest() == receipt.digest && package.manifest().id == receipt.id,
                        "installed Web package changed"
                    );
                    Ok(package)
                });
            let package = match package {
                Ok(package) => package,
                Err(_) => {
                    // Receipts remain manageable even when the optional code is unreadable.
                    statuses.push(WebPackageStatus {
                        id: receipt.id.clone(),
                        name: receipt.id.clone(),
                        version: String::new(),
                        entry: String::new(),
                        digest: receipt.digest.clone(),
                        enabled: receipt.enabled,
                        available: false,
                        permissions: vec![],
                        rollback: !builtin && receipt.previous.is_some(),
                        builtin,
                        open_action_id: None,
                    });
                    continue;
                }
            };
            let m = package.manifest();
            statuses.push(WebPackageStatus {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                entry: m.entry.clone(),
                digest: receipt.digest.clone(),
                enabled: receipt.enabled,
                available: true,
                permissions: m.permissions.clone(),
                rollback: !builtin && receipt.previous.is_some(),
                builtin,
                open_action_id: receipt
                    .enabled
                    .then(|| {
                        m.commands
                            .iter()
                            .find(|command| {
                                command.runtime.is_webview() && !command.needs_context()
                            })
                            .map(|command| format!("web:{}:{}", m.id, command.id))
                    })
                    .flatten(),
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

fn same_permission(old: &WebPermission, new: &WebPermission) -> bool {
    match (old, new) {
        (WebPermission::ContextRead { types: old }, WebPermission::ContextRead { types: new })
        | (WebPermission::UrlOpen { origins: old }, WebPermission::UrlOpen { origins: new }) => {
            old.len() == new.len() && old.iter().all(|item| new.contains(item))
        }
        _ => old == new,
    }
}
