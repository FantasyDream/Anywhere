//! Clipboard snapshot, fact and capture publication use the same source policy.
use super::{failure_status, now_ms, text_context, ContextCaptureService};
use crate::{
    application::session_model::{ActivationCapture, CaptureStatus},
    core::{
        context::CaptureMetadata,
        session::facts::{FactValue, CLIPBOARD},
    },
};
use anywhere_protocol::{ContextSource, SessionId};
use std::future::Future;

/// Policy is frozen before the supplemental task is queued, not when it starts.
pub(in crate::application) struct ClipboardCapture {
    service: ContextCaptureService,
    session_id: SessionId,
    token: Option<u64>,
}

impl ContextCaptureService {
    pub fn prepare_clipboard(&self, session_id: SessionId) -> ClipboardCapture {
        ClipboardCapture {
            service: self.clone(),
            session_id,
            token: self.token(ContextSource::Clipboard),
        }
    }
}

impl ClipboardCapture {
    /// Start the read now; the activation group awaits publication alongside Git and usage.
    pub fn start(self) -> impl Future<Output = ()> + Send + 'static {
        let Self {
            service,
            session_id,
            token,
        } = self;
        let read = token
            .filter(|token| {
                service
                    .policy
                    .lock()
                    .is_ok_and(|policy| policy.allows(ContextSource::Clipboard, *token))
            })
            .map(|_| {
                let platform = service.clipboard.clone();
                tokio::task::spawn_blocking(move || platform.clipboard_snapshot())
            });
        async move {
            let Some(token) = token else { return };
            let Some(read) = read else { return };
            let (mut status, text, age_ms) = match read.await {
                Ok(Ok(snapshot)) => (
                    if snapshot.text.is_some() {
                        CaptureStatus::Captured
                    } else {
                        CaptureStatus::Empty
                    },
                    snapshot.text,
                    snapshot.age_ms,
                ),
                Ok(Err(error)) => (failure_status(&error), None, None),
                Err(_) => (CaptureStatus::Failed, None, None),
            };
            let captured_at_ms = now_ms();
            let mut contexts = Vec::new();
            if let Some(text) = &text {
                match text_context(text.clone(), ContextSource::Clipboard) {
                    Ok(Some(context)) => contexts.push(context),
                    Ok(None) => status = CaptureStatus::Empty,
                    Err(error) => status = failure_status(&error),
                }
            }
            // An unavailable Context must not remain available through SessionFacts.
            let fact_text = if contexts.is_empty() { None } else { text };
            service.publish_clipboard_fact(session_id, token, status, fact_text, age_ms);
            for context in &mut contexts {
                context.capture = Some(CaptureMetadata {
                    captured_at_ms,
                    owning_app: None,
                });
            }
            service
                .publish_capture(
                    token,
                    session_id,
                    ActivationCapture {
                        source: ContextSource::Clipboard,
                        captured_at_ms,
                        status,
                        contexts,
                    },
                )
                .await;
        }
    }
}

impl ContextCaptureService {
    fn publish_clipboard_fact(
        &self,
        session_id: SessionId,
        token: u64,
        status: CaptureStatus,
        text: Option<String>,
        age_ms: Option<u64>,
    ) {
        let changed = {
            let Ok(policy) = self.policy.lock() else {
                return;
            };
            if !policy.allows(ContextSource::Clipboard, token) {
                return;
            }
            let Ok(mut session) = self.resolver.session.lock() else {
                return;
            };
            if session.state.session_id != session_id
                || !self.resolver.is_session_active(session_id)
            {
                return;
            }
            let mut diagnostics = if status == CaptureStatus::Failed {
                vec!["clipboard: read failed".into()]
            } else {
                Vec::new()
            };
            session.identity.clipboard_age_ms = age_ms;
            let changed = text.is_some_and(|text| {
                if let Err(error) = session.record_fact(CLIPBOARD, FactValue::Text(text)) {
                    diagnostics.push(format!("{CLIPBOARD}: {error:#}"));
                    false
                } else {
                    true
                }
            });
            let has_diagnostics = !diagnostics.is_empty();
            session.state.diagnostics.extend(diagnostics);
            changed || has_diagnostics
        };
        if changed {
            let _ = self.resolver.recompute_current(session_id);
        }
    }
}
