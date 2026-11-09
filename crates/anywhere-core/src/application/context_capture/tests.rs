use super::*;
use crate::application::Application;

#[test]
fn long_or_blank_capture_is_not_a_text_context() {
    assert!(text_context(" ".into(), ContextSource::Clipboard)
        .unwrap()
        .is_none());
    assert!(text_context(
        "x".repeat(MAX_CONTEXT_TEXT_BYTES + 1),
        ContextSource::Clipboard
    )
    .is_err());
}

#[test]
fn browser_seed_requires_a_web_url() {
    assert!(browser_context("https://example.org/a".into())
        .unwrap()
        .is_some());
    assert!(browser_context("file:///tmp/a".into()).unwrap().is_none());
    assert!(browser_context("about:blank".into()).unwrap().is_none());
    assert_eq!(
        failure_status(&browser_context("x".repeat(8193)).unwrap_err()),
        CaptureStatus::TooLarge
    );
}

#[test]
fn source_errors_are_typed_and_never_inferred_from_error_text() {
    for (error, expected) in [
        (
            ContextReadError::PermissionDenied,
            CaptureStatus::PermissionDenied,
        ),
        (ContextReadError::Unsupported, CaptureStatus::Unsupported),
        (
            ContextReadError::ApplicationChanged,
            CaptureStatus::ApplicationChanged,
        ),
        (ContextReadError::TimedOut, CaptureStatus::TimedOut),
        (ContextReadError::TooLarge, CaptureStatus::TooLarge),
        (ContextReadError::TooManyItems, CaptureStatus::TooManyItems),
        (ContextReadError::Unavailable, CaptureStatus::Unavailable),
    ] {
        assert_eq!(failure_status(&anyhow::Error::new(error)), expected);
    }
    assert_eq!(
        failure_status(&anyhow::anyhow!(
            "permission is a word in unknown diagnostics"
        )),
        CaptureStatus::Failed
    );
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

#[tokio::test]
async fn reenabled_sources_reject_old_capture_publications() {
    for source in [
        ContextSource::FinderSelection,
        ContextSource::SelectedText,
        ContextSource::BrowserCurrentPage,
        ContextSource::Clipboard,
    ] {
        let app = Application::new(
            Arc::new(crate::application::source_catalog_tests::QuietPlatform),
            crate::core::action::descriptor::ActionRegistry::default(),
            Box::<crate::core::ranking::usage::MemoryUsage>::default(),
            vec![],
        );
        let id = app.begin_activation().unwrap();
        let token = app.context_capture.token(source).unwrap();
        app.set_capture_sources(false, false, false, false);
        app.set_capture_sources(true, true, true, true);
        app.context_capture
            .publish_capture(
                token,
                id,
                ActivationCapture {
                    source,
                    captured_at_ms: now_ms(),
                    status: CaptureStatus::PermissionDenied,
                    contexts: vec![],
                },
            )
            .await;
        let session = app.session().unwrap();
        assert!(session.identity.captures.is_empty(), "{source:?}");
        assert_eq!(session.resolution.inputs_epoch, 0, "{source:?}");
    }
}
