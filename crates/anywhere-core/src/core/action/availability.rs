//! Whether an applicable action should be shown, and whether it can run.
//!
//! Separate from `accepts`, which answers "does this action apply to this object at all".
//! Visibility is a closed rule over named [`SessionFacts`] values: memory only, no file
//! system, no process, no network. Custom actions declare the same operations against a
//! fact key; they do not add fields or run code.
//!
//! Whether a repository still exists is left to execution, which reports it as a normal
//! failure.
use crate::core::context::{ContextValue, ResolvedContext};
use crate::core::session::facts::{fact_kind, FactKind, SessionFacts, GIT};
use anyhow::{bail, Result};
use anywhere_protocol::Availability;

/// Disabled when Git is known to be unavailable.
pub const GIT_UNAVAILABLE: &str = "Git 不可用";
/// Disabled when the Git fact was not captured.
pub const GIT_UNCONFIRMED: &str = "无法确认 Git 是否可用";

/// What an action inspects to decide visibility. Evaluated by [`evaluate`], never by
/// feature or plugin code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AvailabilityRule {
    #[default]
    Enabled,
    /// A meaningful action excluded from search and scope membership.
    Hidden,
    /// Snapshot of a menu item (or similar) that is visible but not runnable.
    Disabled { reason: String },
    /// Hide when the named text fact represents the same value as this context.
    HideIfFactMatchesContext { fact: String },
    /// Disabled unless the named flag fact is true.
    RequireFlag { fact: String, reason: String },
    /// Project-local capability metadata; evaluation performs no IO.
    RequireContextProperty { key: String },
}

impl AvailabilityRule {
    pub fn hide_if(fact: &str) -> Self {
        Self::HideIfFactMatchesContext { fact: fact.into() }
    }

    pub fn require(fact: &str, reason: &str) -> Self {
        Self::RequireFlag {
            fact: fact.into(),
            reason: reason.into(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Enabled | Self::Hidden => Ok(()),
            Self::RequireContextProperty { key } => {
                if key.trim().is_empty() {
                    bail!("context property key must not be empty");
                }
                Ok(())
            }
            Self::Disabled { .. } => Ok(()),
            Self::HideIfFactMatchesContext { fact } => match fact_kind(fact) {
                Some(FactKind::Text) => Ok(()),
                Some(FactKind::Flag) => bail!("availability fact {fact} must be text"),
                None => bail!("unknown availability fact: {fact}"),
            },
            Self::RequireFlag { fact, .. } => match fact_kind(fact) {
                Some(FactKind::Flag) => Ok(()),
                Some(FactKind::Text) => bail!("availability fact {fact} must be a flag"),
                None => bail!("unknown availability fact: {fact}"),
            },
        }
    }
}

/// Decide what the user sees for one applicable action.
pub fn evaluate(
    rule: &AvailabilityRule,
    context: &ResolvedContext,
    facts: &SessionFacts,
) -> Availability {
    match rule {
        AvailabilityRule::Enabled => Availability::enabled(),
        AvailabilityRule::RequireContextProperty { key } => {
            if context
                .value
                .property(key)
                .is_some_and(|value| value == "true")
            {
                Availability::enabled()
            } else {
                Availability::hidden()
            }
        }
        AvailabilityRule::Hidden => Availability::hidden(),
        AvailabilityRule::Disabled { reason } => Availability::disabled(reason),
        AvailabilityRule::HideIfFactMatchesContext { fact } => {
            if facts.matches_context(fact, context) {
                Availability::hidden()
            } else {
                Availability::enabled()
            }
        }
        AvailabilityRule::RequireFlag { fact, reason } => match facts.flag(fact) {
            Some(true) => Availability::enabled(),
            Some(false) => Availability::disabled(reason),
            None => Availability::disabled(missing_flag_reason(fact)),
        },
    }
}

/// Contexts whose value is an object the user explicitly provided (a pasted URL, JSON
/// document or path) rather than ambient environment.
pub fn is_user_object(context: &ResolvedContext) -> bool {
    context.source == crate::core::context::ContextSource::UserInput
        && !matches!(context.value, ContextValue::Text(_))
}

fn missing_flag_reason(fact: &str) -> String {
    if fact == GIT {
        GIT_UNCONFIRMED.into()
    } else {
        format!("无法确认 {fact} 是否可用")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::{ContextSource, ContextValue};
    use crate::core::session::facts::{FactValue, CLIPBOARD, GIT};
    use anywhere_protocol::{AvailabilityState, TEXT};

    fn text(value: &str) -> ResolvedContext {
        ResolvedContext::new(
            TEXT,
            ContextValue::Text(value.into()),
            ContextSource::UserInput,
            "test",
        )
    }

    #[test]
    fn hide_if_matches_text_and_ignores_missing_clipboard() {
        let rule = AvailabilityRule::hide_if(CLIPBOARD);
        let context = text("/tmp");
        assert_eq!(
            evaluate(&rule, &context, &SessionFacts::default()).state,
            AvailabilityState::Enabled
        );
        let facts = SessionFacts::default().with(CLIPBOARD, FactValue::Text("/tmp".into()));
        assert_eq!(
            evaluate(&rule, &context, &facts).state,
            AvailabilityState::Hidden
        );
    }

    #[test]
    fn require_flag_distinguishes_false_from_missing() {
        let rule = AvailabilityRule::require(GIT, GIT_UNAVAILABLE);
        let context = text("n/a");
        let missing = evaluate(&rule, &context, &SessionFacts::default());
        assert_eq!(missing.state, AvailabilityState::Disabled);
        assert_eq!(missing.reason.as_deref(), Some(GIT_UNCONFIRMED));
        let off = evaluate(
            &rule,
            &context,
            &SessionFacts::default().with(GIT, FactValue::Flag(false)),
        );
        assert_eq!(off.reason.as_deref(), Some(GIT_UNAVAILABLE));
        let on = evaluate(
            &rule,
            &context,
            &SessionFacts::default().with(GIT, FactValue::Flag(true)),
        );
        assert_eq!(on.state, AvailabilityState::Enabled);
    }

    #[test]
    fn rules_reject_unknown_and_mistyped_facts() {
        assert!(AvailabilityRule::hide_if("nope")
            .validate()
            .unwrap_err()
            .to_string()
            .contains("unknown"));
        assert!(AvailabilityRule::hide_if(GIT).validate().is_err());
        assert!(AvailabilityRule::require(CLIPBOARD, "x")
            .validate()
            .is_err());
        assert!(AvailabilityRule::require(GIT, "x").validate().is_ok());
        assert!(AvailabilityRule::hide_if(CLIPBOARD).validate().is_ok());
        assert!(AvailabilityRule::Enabled.validate().is_ok());
        assert!(AvailabilityRule::Disabled { reason: "x".into() }
            .validate()
            .is_ok());
    }

    #[test]
    fn disabled_rule_is_visible_and_not_executable() {
        let availability = evaluate(
            &AvailabilityRule::Disabled {
                reason: "菜单项当前不可用".into(),
            },
            &text("n/a"),
            &SessionFacts::default(),
        );
        assert_eq!(availability.state, AvailabilityState::Disabled);
        assert_eq!(availability.reason.as_deref(), Some("菜单项当前不可用"));
        assert!(availability.is_visible());
        assert!(!availability.is_executable());
    }
}
