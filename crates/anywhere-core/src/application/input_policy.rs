//! Product activation policy. Hosts apply the suggestion only before user editing.
use anywhere_protocol::{ContextCapturePresentation, ContextCaptureStatus, ContextSource};
#[derive(Debug, PartialEq)]
pub struct ActivationInput {
    pub source: ContextSource,
    pub text: String,
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
            if let Some(text) = c.text.as_ref().filter(|t| !t.trim().is_empty()) {
                return Some(ActivationInput {
                    source: c.source,
                    text: text.clone(),
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
    let text = c.text.as_ref().filter(|t| !t.trim().is_empty())?;
    Some(ActivationInput {
        source: c.source,
        text: text.clone(),
    })
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
            text: Some("copied".into()),
        }
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
            text: Some("selected".into()),
            ..empty.clone()
        };
        let choose = |captures: &[ContextCapturePresentation], now| {
            choose_activation_input(captures, now, true, true, false)
        };
        assert_eq!(
            choose(&[clip(Some(0)), selected], 1000).unwrap().text,
            "selected"
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
