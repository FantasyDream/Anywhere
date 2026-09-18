//! Filesystem contexts: a path the user typed, and the directory the ambient application
//! was showing.
//!
//! The path recognizer is a provider like any other, not a separate resolver pipeline: it
//! accepts `core.text` and produces `core.file` or `core.directory`. The directory capture is
//! memoized per session and single-flight, because the platform call behind it is expensive
//! and a second revision must reuse the running one rather than start another.
use crate::core::context::provider::{ContextProvider, ProviderDescriptor};
use crate::core::context::{ContextSource, ContextValue, ExecutionContext, ResolvedContext};
use anyhow::{bail, Context, Result};
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use async_trait::async_trait;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::watch;

/// `core.text` to `core.file` / `core.directory`.
pub struct PathProvider {
    descriptor: ProviderDescriptor,
}
impl Default for PathProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.path".into(),
                accepts: vec![TEXT.into()],
                produces: vec![FILE.into(), DIRECTORY.into()],
                // Between the URL and JSON recognizers, reproducing the documented
                // recognition order exactly: a string that is both a URL and a path stays a
                // URL, and one that is both a JSON document and a path stays a path.
                priority: 5,
            },
        }
    }
}
#[async_trait]
impl ContextProvider for PathProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        _: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let ContextValue::Text(raw) = &input.value else {
            return Ok(vec![]);
        };
        if raw.trim().is_empty() {
            return Ok(vec![]);
        }
        // The check touches the file system, so it belongs on a blocking boundary.
        let raw = raw.clone();
        let resolved = tokio::task::spawn_blocking(move || recognize_path(&raw))
            .await
            .context("path recognition task failed")?;
        Ok(resolved.into_iter().collect())
    }
}

pub fn recognize_path(raw: &str) -> Option<ResolvedContext> {
    let path = PathBuf::from(raw)
        .canonicalize()
        .or_else(|_| PathBuf::from(raw.trim()).canonicalize())
        .ok()?;
    let (type_id, value) = if path.is_dir() {
        (DIRECTORY, ContextValue::Directory(path))
    } else if path.is_file() {
        (FILE, ContextValue::File(path))
    } else {
        return None;
    };
    Some(ResolvedContext::new(
        type_id,
        value,
        ContextSource::UserInput,
        "core.path",
    ))
}

/// Resolves the original application directory at most once per session.
///
/// Capture starts on first use and is memoized in the session, so a later revision
/// reuses the running task instead of starting a second Finder Automation request.
/// The original application identity is frozen before the host takes focus and is
/// never re-read afterwards.
pub struct CurrentDirectoryProvider {
    descriptor: ProviderDescriptor,
    memo: Arc<DirectoryMemo>,
}
impl CurrentDirectoryProvider {
    pub fn new(memo: Arc<DirectoryMemo>) -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "platform.current_directory".into(),
                accepts: vec![APPLICATION.into()],
                produces: vec![DIRECTORY.into()],
                priority: 0,
            },
            memo,
        }
    }
}
#[async_trait]
impl ContextProvider for CurrentDirectoryProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let ContextValue::Application { bundle_id, .. } = &input.value else {
            return Ok(vec![]);
        };
        // The memo is bound to the application that actually owned the directory.
        if bundle_id != self.memo.bundle_id() {
            return Ok(vec![]);
        }
        match self.memo.directory(execution).await {
            Ok(Some(directory)) => Ok(vec![ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(directory),
                ContextSource::ApplicationContext,
                &self.descriptor.id,
            )]),
            Ok(None) => Ok(vec![]),
            // An abandoned capture is not a failure the user should see: the revision that
            // asked for it no longer exists.
            Err(error) if !execution.is_current() => {
                tracing::debug!(error = %error, "directory_capture_abandoned");
                Ok(vec![])
            }
            Err(error) => bail!("{error}"),
        }
    }
}

/// Session-scoped, single-flight capture of one application's current directory.
pub struct DirectoryMemo {
    bundle_id: String,
    rx: watch::Receiver<Option<Result<Option<PathBuf>, String>>>,
    /// Held so the channel stays open for later revisions.
    _tx: watch::Sender<Option<Result<Option<PathBuf>, String>>>,
}

impl DirectoryMemo {
    /// Start the capture immediately. The task is independent of any request, so
    /// early typing cannot lose the directory and no second request can duplicate it.
    pub fn start(platform: Arc<dyn Platform>, frontmost: ApplicationInfo) -> Arc<Self> {
        let (tx, rx) = watch::channel(None);
        let publisher = tx.clone();
        let bundle_id = frontmost.bundle_id.clone();
        let bundle_id_for_log = bundle_id.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            let result =
                tokio::task::spawn_blocking(move || platform.application_directory(&frontmost))
                    .await
                    .unwrap_or_else(|error| Err(anyhow::anyhow!("directory task failed: {error}")))
                    .map_err(|error| format!("{error:#}"));
            tracing::info!(
                bundle_id = bundle_id_for_log,
                duration_us = started.elapsed().as_micros() as u64,
                found = result
                    .as_ref()
                    .map(|value| value.is_some())
                    .unwrap_or(false),
                "directory_capture_finished"
            );
            let _ = publisher.send(Some(result));
        });
        Arc::new(Self {
            bundle_id,
            rx,
            _tx: tx,
        })
    }

    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    /// Wait for the capture. A second caller shares the same result.
    ///
    /// The wait is abandoned as soon as this call's session or revision is superseded.
    /// Letting it run to the 30s timeout would keep a blocking worker parked on a result
    /// no revision can use any more — the caller that is still current has its own wait.
    pub async fn directory(&self, execution: &ExecutionContext) -> Result<Option<PathBuf>, String> {
        let mut rx = self.rx.clone();
        if rx.borrow().is_none() {
            tokio::select! {
                biased;
                changed = rx.changed() => {
                    if changed.is_err() {
                        return Err("original application directory capture ended".into());
                    }
                }
                _ = execution.superseded() => {
                    return Err("directory capture abandoned: revision superseded".into());
                }
                _ = tokio::time::sleep(DIRECTORY_TIMEOUT) => {
                    return Err("original application directory capture timed out".into());
                }
            }
        }
        let settled = rx.borrow().clone();
        match settled {
            Some(result) => result,
            None => Err("original application directory capture timed out".into()),
        }
    }
}

const DIRECTORY_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_protocol::{DIRECTORY, FILE, TEXT};

    fn types(input: &str) -> Vec<String> {
        crate::features::recognized_contexts(input)
            .into_iter()
            .map(|context| context.type_id)
            .collect()
    }

    #[test]
    fn only_existing_regular_files_and_directories_are_paths() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a file.txt");
        std::fs::write(&file, "test").unwrap();
        assert_eq!(types(file.to_str().unwrap()), [TEXT, FILE]);
        let spaced_file = directory.path().join("trailing space ");
        std::fs::write(&spaced_file, "test").unwrap();
        let resolved = recognize_path(spaced_file.to_str().unwrap()).unwrap();
        assert_eq!(
            resolved.value.text(),
            spaced_file.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(types(directory.path().to_str().unwrap()), [TEXT, DIRECTORY]);
        assert_eq!(
            types(directory.path().join("missing").to_str().unwrap()),
            [TEXT]
        );
    }
}
