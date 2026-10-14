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
) -> Option<ActivationInput> {
    let selected = captures
        .iter()
        .find(|c| c.source == ContextSource::SelectedText);
    if selected_enabled {
        if selected.is_none() && !allow_pending_selection {
            return None;
        }
        if let Some(c) = selected.filter(|c| c.status == ContextCaptureStatus::Captured) {
            if let Some(text) = c.text.as_ref().filter(|t| has_object_context(t)) {
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
    let text = c.text.as_ref().filter(|t| has_object_context(t))?;
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
/// Generic text is a seed, not a reason to open the content workspace.
/// Reuse the real recognizers so paths, JSON primitives and URLs keep their
/// normal validity rules; malformed objects never become activation suggestions.
fn has_object_context(text: &str) -> bool {
    crate::features::recognized_contexts(text)
        .iter()
        .any(|context| context.type_id != anywhere_protocol::TEXT)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        for text in ["https://example.org", "{\"a\":1}", "true", "123", "[]"] {
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
}
