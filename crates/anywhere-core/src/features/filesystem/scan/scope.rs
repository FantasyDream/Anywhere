//! Query-local scope preparation and independent verification of provisional index hits.
use super::{query::insert_top, RankedHit, ScanStore, PAGE_SIZE};
use anyhow::{bail, Context, Result};
use anywhere_platform::{FileSearchHit, FileSearchOptions};
use std::path::{Path, PathBuf};

pub(super) struct ScanScope {
    pub(super) options: FileSearchOptions,
    pub(super) system_exclusions: anywhere_platform::search_exclusions::SearchExclusions,
    pub(super) direct_only: bool,
    pub(super) exact_path: Option<PathBuf>,
}
impl ScanScope {
    pub(super) fn new(
        options: FileSearchOptions,
        direct_only: bool,
        exact_path: Option<PathBuf>,
    ) -> Self {
        Self {
            system_exclusions: anywhere_platform::search_exclusions::SearchExclusions::for_search(
                &options.roots,
            ),
            options,
            direct_only,
            exact_path,
        }
    }
    pub(super) fn authorized_link_target(&self, path: &Path) -> bool {
        !self.options.restrict_to_roots
            || path.canonicalize().is_ok_and(|target| {
                self.options.roots.iter().any(|root| {
                    root.canonicalize()
                        .is_ok_and(|root| target.starts_with(root))
                })
            })
    }
    pub(super) fn configured_roots(&self) -> Result<Vec<PathBuf>> {
        if self.options.roots.is_empty() {
            Ok(vec![
                anywhere_platform::paths::home_directory().context("home directory unavailable")?
            ])
        } else {
            Ok(self.options.roots.clone())
        }
    }
}
impl ScanStore {
    pub fn verify_index_hits(
        &self,
        hits: Vec<FileSearchHit>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Vec<RankedHit> {
        let roots: Vec<_> = self
            .scope
            .configured_roots()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|root| root.canonicalize().ok())
            .collect();
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut seen = std::collections::BTreeSet::new();
        for hit in hits.into_iter().take(PAGE_SIZE) {
            if cancelled() {
                break;
            }
            if self
                .scope
                .system_exclusions
                .excludes(&hit.path, hit.is_directory)
            {
                continue;
            }
            if !seen.insert(hit.path.clone()) {
                continue;
            }
            let Some(ranked) = self.query.rank(&hit.path, hit.is_directory) else {
                continue;
            };
            let Ok(kind) = std::fs::symlink_metadata(&hit.path).map(|m| m.file_type()) else {
                continue;
            };
            if kind.is_symlink() && !self.scope.authorized_link_target(&hit.path) {
                continue;
            }
            // Spotlight can expose metadata from a directory whose contents
            // cannot be read. Such a hit is not an authorized scan result.
            if self.scope.options.restrict_to_roots {
                let directory = if kind.is_dir() {
                    hit.path.as_path()
                } else {
                    let Some(parent) = hit.path.parent() else {
                        continue;
                    };
                    parent
                };
                if std::fs::read_dir(directory).is_err() {
                    continue;
                }
            }
            if kind.is_dir() != hit.is_directory
                || !(kind.is_dir() || kind.is_file() || kind.is_symlink())
            {
                continue;
            }
            let allowed = roots.iter().any(|root| {
                let Ok(relative) = hit.path.strip_prefix(root) else {
                    return false;
                };
                let mut path = root.clone();
                for part in relative.components() {
                    path.push(part.as_os_str());
                    if !self.scope.options.include_hidden
                        && anywhere_platform::search_storage::is_hidden(&path, part.as_os_str())
                            .unwrap_or(true)
                    {
                        return false;
                    }
                    if (path != hit.path || kind.is_dir())
                        && self
                            .scope
                            .options
                            .excluded_directory_names
                            .iter()
                            .any(|name| part.as_os_str() == name.as_str())
                    {
                        return false;
                    }
                    // Descendants through a directory link are outside traversal coverage.
                    if path != hit.path
                        && path
                            .symlink_metadata()
                            .map_or(true, |m| m.file_type().is_symlink())
                    {
                        return false;
                    }
                }
                true
            });
            if allowed {
                insert_top(&mut top, ranked);
            }
        }
        top
    }
}

pub(super) fn prepare_query(
    text: &str,
    mut options: FileSearchOptions,
) -> Result<(String, FileSearchOptions, bool, Option<PathBuf>)> {
    let raw = text.trim();
    let path = anywhere_platform::paths::expand_home(raw).context("home directory unavailable")?;
    if path.components().count() <= 1 && !path.is_absolute() {
        return Ok((raw.into(), options, false, None));
    }
    let path = if path.is_absolute() {
        path
    } else {
        let base = options
            .roots
            .first()
            .cloned()
            .or_else(anywhere_platform::paths::home_directory)
            .context("search directory unavailable")?;
        base.join(path)
    };
    if options.restrict_to_roots {
        let lexical = anywhere_platform::search_storage::lexical_scope_path(&path);
        if !options.roots.iter().any(|root| {
            lexical.starts_with(anywhere_platform::search_storage::lexical_scope_path(root))
        }) {
            bail!("This path is outside the authorized search directories. Update Search settings to authorize it.");
        }
        // A lexical child can be a link into an unapproved directory. Check the
        // resolved target as well, before enumerating it.
        let target = path.canonicalize().or_else(|_| {
            path.parent()
                .context("path has no parent directory")?
                .canonicalize()
                .map_err(anyhow::Error::from)
        })?;
        if !options.roots.iter().any(|root| {
            root.canonicalize()
                .is_ok_and(|root| target.starts_with(root))
        }) {
            bail!("This path is outside the authorized search directories. Update Search settings to authorize it.");
        }
    }
    let (root, name, exact) = if path.is_dir() {
        (path.clone(), String::new(), None)
    } else {
        (
            path.parent()
                .context("path has no parent directory")?
                .to_owned(),
            path.file_name()
                .and_then(|name| name.to_str())
                .context("invalid filename")?
                .to_owned(),
            if path.is_file() {
                Some(path.clone())
            } else {
                None
            },
        )
    };
    let root = root.canonicalize().unwrap_or(root);
    let exact = exact.map(|path| root.join(path.file_name().expect("validated filename")));
    options.roots = vec![root];
    Ok((name, options, true, exact))
}
