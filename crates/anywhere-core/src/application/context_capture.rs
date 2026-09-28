//! One-shot context reads made for a frozen activation identity.
//!
//! Platform reads run independently of the first snapshot. A completed read is stored
//! in the session, then its seed is resolved through the ordinary provider graph for
//! whichever query revision is current. A late result never starts a new OS read.
use super::{
    launcher_service::Application,
    session_model::{ActivationCapture, CaptureStatus},
    session_runtime::Resolver,
};
use crate::core::context::{CaptureMetadata, ContextValue, ResolvedContext};
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, SelectedTextError};
use anywhere_protocol::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) const MAX_CONTEXT_TEXT_BYTES: usize = 16 * 1024;
const MAX_FINDER_EXPANSION: usize = 8;
const CAPTURE_WAIT: Duration = Duration::from_secs(15);

const FINDER: u8 = 1;
const SELECTION: u8 = 2;
const BROWSER: u8 = 4;
const CLIPBOARD: u8 = 8;

/// A setting change advances the generation, so an in-flight OS read from the
/// previous setting cannot publish even if its source is turned back on quickly.
pub(super) struct CapturePolicy {
    enabled: u8,
    generation: u64,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            enabled: FINDER | SELECTION | BROWSER | CLIPBOARD,
            generation: 0,
        }
    }
}

impl CapturePolicy {
    fn bit(source: ContextSource) -> u8 {
        match source {
            ContextSource::FinderSelection => FINDER,
            ContextSource::SelectedText => SELECTION,
            ContextSource::BrowserCurrentPage => BROWSER,
            ContextSource::Clipboard => CLIPBOARD,
            _ => 0,
        }
    }

    pub(super) fn token(&self, source: ContextSource) -> Option<u64> {
        (self.enabled & Self::bit(source) != 0).then_some(self.generation)
    }

    pub(super) fn allows(&self, source: ContextSource, token: u64) -> bool {
        self.generation == token && self.token(source).is_some()
    }

    pub(super) fn set(&mut self, finder: bool, selected: bool, browser: bool, clipboard: bool) {
        self.enabled = 0;
        for (enabled, bit) in [
            (finder, FINDER),
            (selected, SELECTION),
            (browser, BROWSER),
            (clipboard, CLIPBOARD),
        ] {
            if enabled {
                self.enabled |= bit;
            }
        }
        self.generation = self.generation.wrapping_add(1);
    }
}

impl Application {
    pub fn set_capture_sources(
        &self,
        finder: bool,
        selected_text: bool,
        browser_page: bool,
        clipboard: bool,
    ) {
        if let Ok(mut policy) = self.capture_policy.lock() {
            policy.set(finder, selected_text, browser_page, clipboard);
        }
    }

    pub(super) fn capture_token(&self, source: ContextSource) -> Option<u64> {
        self.capture_policy.lock().ok()?.token(source)
    }

    pub(super) fn spawn_context_captures(
        &self,
        session_id: SessionId,
        frontmost: Option<ApplicationInfo>,
    ) {
        let Some(application) = frontmost else { return };
        let owner = Some(application.bundle_id.clone());

        if application.bundle_id == "com.apple.finder" {
            if let Some(token) = self.capture_token(ContextSource::FinderSelection) {
                let platform = self.platform.clone();
                let target = application.clone();
                spawn_capture(
                    self.resolver.clone(),
                    self.capture_policy.clone(),
                    token,
                    session_id,
                    ContextSource::FinderSelection,
                    owner.clone(),
                    move || finder_contexts(platform.finder_selected_paths(&target)?),
                );
            }
        }

        if self.platform.browser_current_url_supported(&application) {
            if let Some(token) = self.capture_token(ContextSource::BrowserCurrentPage) {
                let platform = self.platform.clone();
                let target = application.clone();
                spawn_capture(
                    self.resolver.clone(),
                    self.capture_policy.clone(),
                    token,
                    session_id,
                    ContextSource::BrowserCurrentPage,
                    owner.clone(),
                    move || {
                        Ok(platform
                            .browser_current_url(&target)?
                            .and_then(browser_context)
                            .into_iter()
                            .collect())
                    },
                );
            }
        }

        if let Some(token) = self.capture_token(ContextSource::SelectedText) {
            let platform = self.platform.clone();
            spawn_capture(
                self.resolver.clone(),
                self.capture_policy.clone(),
                token,
                session_id,
                ContextSource::SelectedText,
                owner,
                move || {
                    Ok(platform
                        .selected_text(&application)?
                        .and_then(|text| text_context(text, ContextSource::SelectedText))
                        .into_iter()
                        .collect())
                },
            );
        }
    }
}

fn spawn_capture(
    resolver: Resolver,
    policy: Arc<Mutex<CapturePolicy>>,
    token: u64,
    session_id: SessionId,
    source: ContextSource,
    owner: Option<String>,
    read: impl FnOnce() -> Result<Vec<ResolvedContext>> + Send + 'static,
) {
    tokio::spawn(async move {
        if !policy
            .lock()
            .is_ok_and(|policy| policy.allows(source, token))
        {
            return;
        }
        let result = tokio::task::spawn_blocking(read).await;
        let captured_at_ms = now_ms();
        let (status, mut contexts) = match result {
            Ok(Ok(contexts)) if contexts.is_empty() => (CaptureStatus::Empty, contexts),
            Ok(Ok(contexts)) => (CaptureStatus::Captured, contexts),
            Ok(Err(error)) => (failure_status(&error), Vec::new()),
            Err(_) => (CaptureStatus::Failed, Vec::new()),
        };
        for context in &mut contexts {
            context.capture = Some(CaptureMetadata {
                captured_at_ms,
                owning_app: owner.clone(),
            });
        }
        publish_capture(
            resolver,
            policy,
            token,
            session_id,
            ActivationCapture {
                source,
                captured_at_ms,
                status,
                contexts,
            },
        )
        .await;
    });
}

pub(super) fn text_context(text: String, source: ContextSource) -> Option<ResolvedContext> {
    if text.trim().is_empty() || text.len() > MAX_CONTEXT_TEXT_BYTES {
        return None;
    }
    Some(ResolvedContext::new(
        TEXT,
        ContextValue::Text(text.into()),
        source,
        "platform.activation",
    ))
}

fn browser_context(raw: String) -> Option<ResolvedContext> {
    if raw.len() > 8 * 1024 {
        return None;
    }
    let url = url::Url::parse(&raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    Some(ResolvedContext::new(
        URL,
        ContextValue::Url(url),
        ContextSource::BrowserCurrentPage,
        "platform.browser_current_page",
    ))
}

fn finder_contexts(paths: Vec<PathBuf>) -> Result<Vec<ResolvedContext>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut contexts = Vec::new();
    if paths.len() > 1 {
        contexts.push(ResolvedContext::new(
            FILE_SELECTION,
            ContextValue::FileSelection(paths.clone().into()),
            ContextSource::FinderSelection,
            "platform.finder_selection",
        ));
    }
    for path in paths.into_iter().take(MAX_FINDER_EXPANSION) {
        let value = if path.is_dir() {
            Some((DIRECTORY, ContextValue::Directory(path)))
        } else if path.is_file() {
            Some((FILE, ContextValue::File(path)))
        } else {
            None
        };
        if let Some((type_id, value)) = value {
            contexts.push(ResolvedContext::new(
                type_id,
                value,
                ContextSource::FinderSelection,
                "platform.finder_selection",
            ));
        }
    }
    Ok(contexts)
}

fn failure_status(error: &anyhow::Error) -> CaptureStatus {
    if let Some(status) = error.downcast_ref::<SelectedTextError>() {
        return match status {
            SelectedTextError::PermissionDenied => CaptureStatus::PermissionDenied,
            SelectedTextError::Unsupported
            | SelectedTextError::ApplicationChanged
            | SelectedTextError::Unavailable => CaptureStatus::Unavailable,
            SelectedTextError::TimedOut => CaptureStatus::Failed,
        };
    }
    // The Automation adapters replace osascript stderr with fixed categories, so
    // matching these messages cannot accidentally retain selected paths or URLs.
    let message = error.to_string();
    if message.contains("permission") || message.contains("not authorized") {
        CaptureStatus::PermissionDenied
    } else {
        CaptureStatus::Failed
    }
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

/// Store the capture first, then enrich the latest committed revision. The watch
/// receiver is subscribed before inspecting state so the first commit cannot be missed.
pub(super) async fn publish_capture(
    resolver: Resolver,
    policy: Arc<Mutex<CapturePolicy>>,
    token: u64,
    session_id: SessionId,
    capture: ActivationCapture,
) {
    let contexts = capture.contexts.clone();
    {
        let Ok(policy) = policy.lock() else { return };
        if !policy.allows(capture.source, token) {
            return;
        }
        let Ok(mut session) = resolver.session.lock() else {
            return;
        };
        if session.state.session_id != session_id || !resolver.is_session_active(session_id) {
            return;
        }
        session.record_capture(capture);
    }
    if contexts.is_empty() {
        let _ = resolver.recompute_current(session_id);
        return;
    }
    let mut updates = resolver.latest_tx.subscribe();
    loop {
        enum Next {
            Wait,
            Resolved,
            Start(Revision),
        }
        let next = {
            let Ok(session) = resolver.session.lock() else {
                return;
            };
            if session.state.session_id != session_id || !resolver.is_session_active(session_id) {
                return;
            }
            if contexts.iter().all(|seed| {
                session.resolution.committed_contexts.iter().any(|held| {
                    held.type_id == seed.type_id
                        && held.value.shared_text() == seed.value.shared_text()
                        && held.source == seed.source
                })
            }) {
                Next::Resolved
            } else if let Some(revision) = session.resolution.committed_revision {
                Next::Start(revision)
            } else {
                Next::Wait
            }
        };
        let revision = match next {
            Next::Resolved => return,
            Next::Wait => {
                if tokio::time::timeout(CAPTURE_WAIT, updates.changed())
                    .await
                    .is_err()
                {
                    return;
                }
                continue;
            }
            Next::Start(revision) => revision,
        };
        let started = Instant::now();
        let execution = resolver.execution(session_id, revision);
        let run = resolver.build_run(&execution, contexts.clone());
        let Some(resolution) = resolver.resolve_until(session_id, run, started).await else {
            if tokio::time::timeout(CAPTURE_WAIT, updates.changed())
                .await
                .is_err()
            {
                return;
            }
            continue;
        };
        match resolver.commit(
            session_id,
            revision,
            resolution.contexts,
            resolution.diagnostics,
            true,
            started,
        ) {
            Ok(Some(_)) => return,
            Ok(None) => {
                if tokio::time::timeout(CAPTURE_WAIT, updates.changed())
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_or_blank_capture_is_not_a_text_context() {
        assert!(text_context(" ".into(), ContextSource::Clipboard).is_none());
        assert!(text_context(
            "x".repeat(MAX_CONTEXT_TEXT_BYTES + 1),
            ContextSource::Clipboard
        )
        .is_none());
    }

    #[test]
    fn browser_seed_requires_a_web_url() {
        assert!(browser_context("https://example.org/a".into()).is_some());
        assert!(browser_context("file:///tmp/a".into()).is_none());
        assert!(browser_context("about:blank".into()).is_none());
    }

    #[test]
    fn source_setting_invalidates_an_in_flight_capture() {
        let mut policy = CapturePolicy::default();
        let token = policy.token(ContextSource::Clipboard).unwrap();
        policy.set(true, true, true, false);
        assert!(!policy.allows(ContextSource::Clipboard, token));
        policy.set(true, true, true, true);
        assert!(!policy.allows(ContextSource::Clipboard, token));
        assert!(policy.token(ContextSource::Clipboard).is_some());
    }
}
