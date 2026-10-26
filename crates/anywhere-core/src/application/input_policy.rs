//! Product activation policy. Hosts apply the suggestion only before user editing.
//! Recognition may touch the filesystem; call this on a background worker.
use anywhere_protocol::{ContextCapturePresentation, ContextCaptureStatus, ContextSource};
#[derive(Debug, PartialEq)]
pub struct ActivationInput {
    pub source: ContextSource,
    pub text: String,
    pub valid_until_ms: Option<i64>,
}
pub fn choose_activation_input(
    captures: &[ContextCapturePresentation],
    now_ms: i64,
    selected_enabled: bool,
    clipboard_enabled: bool,
    allow_pending_selection: bool,
    recognizes: impl Fn(&str) -> bool,
) -> Option<ActivationInput> {
    let selected = captures
        .iter()
        .find(|c| c.source == ContextSource::SelectedText);
    if selected_enabled {
        if selected.is_none() && !allow_pending_selection {
            return None;
        }
        if let Some(c) = selected.filter(|c| c.status == ContextCaptureStatus::Captured) {
            if let Some(text) = c.text.as_ref().filter(|t| {
                t.len() <= super::context_capture::MAX_CONTEXT_TEXT_BYTES && recognizes(t)
            }) {
                return Some(ActivationInput {
                    source: c.source,
                    text: text.clone(),
                    valid_until_ms: None,
                });
            }
        }
    }
    if !clipboard_enabled {
        return None;
    }
    let c = captures.iter().find(|c| {
        c.source == ContextSource::Clipboard && c.status == ContextCaptureStatus::Captured
    })?;
    let age = i64::try_from(c.clipboard_age_ms?)
        .ok()?
        .checked_add(now_ms.checked_sub(c.captured_at_ms)?)?;
    if !(0..=anywhere_protocol::policy::CLIPBOARD_PREFILL_LIFETIME_MS as i64).contains(&age) {
        return None;
    }
    let text = c
        .text
        .as_ref()
        .filter(|t| t.len() <= super::context_capture::MAX_CONTEXT_TEXT_BYTES && recognizes(t))?;
    Some(ActivationInput {
        source: c.source,
        text: text.clone(),
        valid_until_ms: Some(
            c.captured_at_ms
                .checked_sub(i64::try_from(c.clipboard_age_ms?).ok()?)?
                .checked_add(anywhere_protocol::policy::CLIPBOARD_PREFILL_LIFETIME_MS as i64)?,
        ),
    })
}
impl super::Application {
    /// Called on the bridge worker. Capture active recognizers once, release publication
    /// locks before recognition / filesystem work, and reject any intervening policy change.
    pub fn choose_activation_input(
        &self,
        captures: &[ContextCapturePresentation],
        now_ms: i64,
        selected_enabled: bool,
        clipboard_enabled: bool,
        allow_pending_selection: bool,
    ) -> Option<ActivationInput> {
        let (rules, actions, policy, version) = {
            let _session = self.session().ok()?;
            let providers = self.resolver.providers.lock().ok()?;
            (
                providers.input_recognizers(),
                self.resolver.base_registry(),
                self.resolver.feature_policy.snapshot(),
                self.resolver
                    .action_catalog_version
                    .load(std::sync::atomic::Ordering::Acquire),
            )
        };
        let current = || {
            self.resolver.feature_policy.is_current(&policy)
                && self
                    .resolver
                    .action_catalog_version
                    .load(std::sync::atomic::Ordering::Acquire)
                    == version
        };
        let started = std::time::Instant::now();
        let result = choose_activation_input(
            captures,
            now_ms,
            selected_enabled,
            clipboard_enabled,
            allow_pending_selection,
            |text| {
                if current() && actions.recognizes_web_input(text) {
                    return true;
                }
                let recognized = rules.iter().any(|rule| {
                    current()
                        && started.elapsed() < std::time::Duration::from_millis(150)
                        && (rule.recognize)(text)
                            .is_some_and(|context| context.type_id != anywhere_protocol::TEXT)
                });
                recognized
            },
        );
        current().then_some(result).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn choose_activation_input(
        captures: &[ContextCapturePresentation],
        now_ms: i64,
        selected: bool,
        clipboard: bool,
        pending: bool,
    ) -> Option<ActivationInput> {
        let rules = crate::application::composition::builtin_providers().input_recognizers();
        super::choose_activation_input(captures, now_ms, selected, clipboard, pending, |text| {
            rules.iter().any(|r| (r.recognize)(text).is_some())
        })
    }
    fn clip(age: Option<u64>) -> ContextCapturePresentation {
        ContextCapturePresentation {
            source: ContextSource::Clipboard,
            status: ContextCaptureStatus::Captured,
            captured_at_ms: 1000,
            clipboard_age_ms: age,
            text: Some("https://example.org/copied".into()),
        }
    }
    #[test]
    fn unrecognized_capture_does_not_offer_content_and_valid_clipboard_can_fall_back() {
        let now = 1000;
        let mut selected = clip(None);
        selected.source = ContextSource::SelectedText;
        let mut copied = clip(Some(0));
        for text in [
            "ordinary selected text",
            "https://",
            "{broken json",
            "{\"a\":1}",
            "true",
            "123",
            "[]",
            "/no-such-activation-path-472951",
        ] {
            selected.text = Some(text.into());
            copied.text = Some(text.into());
            assert!(choose_activation_input(
                &[selected.clone(), copied.clone()],
                now,
                true,
                true,
                false
            )
            .is_none());
            let offer =
                choose_activation_input(&[selected.clone(), clip(Some(0))], now, true, true, false)
                    .unwrap();
            assert_eq!(offer.source, ContextSource::Clipboard);
        }
        for text in ["https://example.org", "https://example.org/selected"] {
            selected.text = Some(text.into());
            assert_eq!(
                choose_activation_input(&[selected.clone(), clip(Some(0))], now, true, true, false)
                    .unwrap()
                    .source,
                ContextSource::SelectedText
            );
        }
    }

    #[test]
    fn file_capture_uses_real_recognition_including_home_aliases() {
        let home = anywhere_platform::paths::home_directory().unwrap();
        let fixture = tempfile::tempdir_in(&home).unwrap();
        let file = fixture.path().join("selected file.txt");
        std::fs::write(&file, "test").unwrap();
        let alias = format!("~/{}", file.strip_prefix(&home).unwrap().display());
        let mut copied = clip(Some(0));
        for text in [
            file.to_str().unwrap(),
            fixture.path().to_str().unwrap(),
            &alias,
        ] {
            copied.text = Some(text.into());
            assert!(choose_activation_input(
                std::slice::from_ref(&copied),
                1000,
                false,
                true,
                false
            )
            .is_some());
        }
        std::fs::remove_file(file).unwrap();
        copied.text = Some(alias);
        assert!(choose_activation_input(&[copied], 1000, false, true, false).is_none());
    }

    #[test]
    fn selection_priority_age_boundary_unknown_age_and_waiting() {
        let empty = ContextCapturePresentation {
            source: ContextSource::SelectedText,
            status: ContextCaptureStatus::Empty,
            captured_at_ms: 1000,
            clipboard_age_ms: None,
            text: None,
        };
        let selected = ContextCapturePresentation {
            status: ContextCaptureStatus::Captured,
            text: Some("https://example.org/selected".into()),
            ..empty.clone()
        };
        let choose = |captures: &[ContextCapturePresentation], now| {
            choose_activation_input(captures, now, true, true, false)
        };
        assert_eq!(
            choose(&[clip(Some(0)), selected], 1000).unwrap().text,
            "https://example.org/selected"
        );
        for age in [14999, 15000] {
            assert!(choose(&[empty.clone(), clip(Some(age))], 1000).is_some());
        }
        for age in [Some(15001), None] {
            assert!(choose(&[empty.clone(), clip(age)], 1000).is_none());
        }
        assert!(choose(&[empty, clip(Some(14900))], 1101).is_none());
        assert!(choose(&[clip(Some(0))], 1000).is_none());
        assert!(choose_activation_input(&[clip(Some(0))], 1000, true, true, true).is_some());
        assert!(
            choose_activation_input(&[clip(Some(u64::MAX))], 1000, false, true, true).is_none()
        );
    }
    fn app() -> crate::Application {
        crate::Application::new(
            std::sync::Arc::new(crate::application::source_catalog_tests::QuietPlatform),
            crate::application::builtin_registry(),
            Box::<crate::core::ranking::usage::MemoryUsage>::default(),
            vec![],
        )
    }
    #[test]
    fn active_catalog_controls_prefill_selection_fallback_and_foundational_paths() {
        use crate::application::settings_service::{SettingsChange, SettingsService};
        let tmp = tempfile::tempdir().unwrap();
        let app = app();
        let mut settings =
            SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
        let mut selected = clip(None);
        selected.source = ContextSource::SelectedText;
        selected.text = Some("https://example.org/selected".into());
        let captures = [selected, clip(Some(0))];
        assert_eq!(
            app.choose_activation_input(&captures, 1000, true, true, false)
                .unwrap()
                .source,
            ContextSource::SelectedText
        );
        settings
            .change(
                &app,
                SettingsChange::Enable {
                    key: "first-party:url".into(),
                    enabled: false,
                },
            )
            .unwrap();
        assert!(app
            .choose_activation_input(&captures, 1000, true, true, false)
            .is_none());
        let mut path = clip(Some(0));
        path.text = Some(tmp.path().to_str().unwrap().into());
        assert!(app
            .choose_activation_input(&[path], 1000, false, true, false)
            .is_some());
    }
    #[test]
    fn input_limits_prevent_recognition_and_new_registered_rules_need_no_dispatch_branch() {
        use crate::core::context::{
            input_recognizer::InputRecognizer, ContextValue, ResolvedContext,
        };
        fn custom(text: &str) -> Option<ResolvedContext> {
            (text == "custom-object").then(|| {
                ResolvedContext::new(
                    "custom.object",
                    ContextValue::Extension(text.into()),
                    ContextSource::UserInput,
                    "custom.rule",
                )
            })
        }
        let app = app();
        app.resolver
            .providers
            .lock()
            .unwrap()
            .register_input_recognizer(
                InputRecognizer {
                    id: "custom.rule".into(),
                    priority: 0,
                    recognize: std::sync::Arc::new(custom),
                },
                None,
            )
            .unwrap();
        let mut captured = clip(Some(0));
        captured.text = Some("custom-object".into());
        assert!(app
            .choose_activation_input(&[captured], 1000, false, true, false)
            .is_some());
        let mut large = clip(Some(0));
        large.text = Some("x".repeat(super::super::context_capture::MAX_CONTEXT_TEXT_BYTES + 1));
        assert!(
            super::choose_activation_input(&[large], 1000, false, true, false, |_| panic!(
                "oversized capture must not reach a recognizer"
            ))
            .is_none()
        );
    }
    #[test]
    fn policy_change_during_blocking_recognition_rejects_the_offer() {
        use crate::core::context::{
            input_recognizer::InputRecognizer, ContextValue, ResolvedContext,
        };
        use std::sync::{Arc, Barrier, Mutex, OnceLock};
        #[derive(Clone)]
        struct Gates {
            entered: Arc<Barrier>,
            release: Arc<Barrier>,
        }
        static GATES: OnceLock<Mutex<Option<Gates>>> = OnceLock::new();
        fn slow(_: &str) -> Option<ResolvedContext> {
            let gates = GATES
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .clone();
            gates.entered.wait();
            gates.release.wait();
            Some(ResolvedContext::new(
                "custom.object",
                ContextValue::Extension("blocked".into()),
                ContextSource::UserInput,
                "custom.rule",
            ))
        }
        let app = app();
        let tmp = tempfile::tempdir().unwrap();
        let mut settings = crate::application::settings_service::SettingsService::open(
            tmp.path().join("extensions.json"),
            vec![],
            &app,
        )
        .unwrap();
        app.resolver
            .providers
            .lock()
            .unwrap()
            .register_input_recognizer(
                InputRecognizer {
                    id: "custom.rule".into(),
                    priority: -1,
                    recognize: Arc::new(slow),
                },
                Some("url"),
            )
            .unwrap();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        *GATES.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(Gates {
            entered: entered.clone(),
            release: release.clone(),
        });
        std::thread::scope(|scope| {
            let work = scope.spawn(|| {
                let mut selected = clip(None);
                selected.source = ContextSource::SelectedText;
                selected.text = Some("blocked object".into());
                app.choose_activation_input(&[selected], 1000, true, false, false)
            });
            entered.wait();
            settings
                .change(
                    &app,
                    crate::application::settings_service::SettingsChange::Enable {
                        key: "first-party:url".into(),
                        enabled: false,
                    },
                )
                .unwrap();
            release.wait();
            assert!(work.join().unwrap().is_none());
        });
        *GATES.get().unwrap().lock().unwrap() = None;
    }
}
