use super::*;
use crate::core::ranking::score::UsageStats;
use crate::core::search::candidate::{Candidate, CandidateTarget};
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone)]
pub(super) struct Offers {
    pub actions: Arc<ActionRegistry>,
    pub diagnostics: Vec<String>,
    pub candidates: Vec<Candidate>,
    pub scopes: Vec<ScopeEntry>,
    pub shortcuts: Vec<super::super::shortcuts::ShortcutAction>,
}

pub(crate) struct OfferCache {
    registry: Arc<ActionRegistry>,
    contexts: Vec<ResolvedContext>,
    facts: SessionFacts,
    usage: Arc<UsageSnapshot>,
    now: i64,
    offers: Offers,
}

pub(super) fn prepare(
    input: &SnapshotInput<'_>,
    cache: &Mutex<Option<OfferCache>>,
    usage_of: &dyn Fn(&str, &str) -> UsageStats,
) -> Offers {
    let contexts: Vec<_> = input
        .contexts
        .iter()
        .cloned()
        .map(|mut context| {
            // A query revision changes the request stamp, not the captured value.
            context.revision = 0;
            context.session_id = 0;
            context
        })
        .collect();
    let cached = {
        let guard = crate::core::sync::lock_cache(cache, "offer_cache");
        guard
            .as_ref()
            .filter(|cache| {
                Arc::ptr_eq(&cache.registry, input.actions)
                    && Arc::ptr_eq(&cache.usage, input.usage)
                    && cache.facts == *input.facts
                    && cache.contexts == contexts
                    && cache.now.div_euclid(60) == input.now.div_euclid(60)
            })
            .map(|cache| cache.offers.clone())
    };
    let mut offers = cached.unwrap_or_else(|| {
        let (expanded, diagnostics) = input.actions.for_contexts(input.contexts);
        let actions = match expanded {
            std::borrow::Cow::Borrowed(_) => input.actions.clone(),
            std::borrow::Cow::Owned(actions) => Arc::new(actions),
        };
        let search = ActionSearch::new(
            &actions,
            usage_of,
            input.facts,
            input.now,
            crate::features::context_semantic,
        );
        let candidates = search
            .search(&Query::parse(""), input.contexts, &[])
            .candidates;
        let scopes = crate::core::search::scope::build(
            &candidates,
            &actions,
            input.usage,
            input.facts.frontmost.as_ref(),
            input.now,
        );
        let shortcuts = super::super::shortcuts::catalog_options(&actions);
        let offers = Offers {
            actions,
            diagnostics,
            candidates,
            scopes,
            shortcuts,
        };
        *crate::core::sync::lock_cache(cache, "offer_cache") = Some(OfferCache {
            registry: input.actions.clone(),
            contexts,
            facts: input.facts.clone(),
            usage: input.usage.clone(),
            now: input.now,
            offers: offers.clone(),
        });
        offers
    });
    let current: HashMap<_, _> = input
        .contexts
        .iter()
        .map(|context| (&context.id, context))
        .collect();
    for candidate in &mut offers.candidates {
        if let CandidateTarget::Action { context, .. } = &mut candidate.target {
            // Cached offers carry no authority across revisions. Rebind their
            // execution payload to this request's exact captured context.
            if let Some(latest) = current.get(&context.id) {
                *context = Arc::new((*latest).clone());
            }
        }
    }
    offers
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_offers_rebind_revision_and_discard_changed_facts() {
        let registry = crate::application::builtin_registry();
        let registry = Arc::new(registry.for_contexts(&[]).0.into_owned());
        let usage = Arc::new(HashMap::new());
        let app_usage = HashMap::new();
        let catalog = ApplicationCatalog::empty();
        let mut contexts = vec![ResolvedContext::new(
            URL,
            crate::core::context::ContextValue::Url(
                url::Url::parse("https://example.org/").unwrap(),
            ),
            ContextSource::UserInput,
            "test",
        )];
        contexts[0].id = "c0".into();
        contexts[0].session_id = 7;
        contexts[0].revision = 1;
        let cache = Mutex::new(None);
        let facts = SessionFacts::default();
        macro_rules! input {
            ($contexts:expr, $facts:expr) => {
                SnapshotInput {
                    action_catalog_version: 1,
                    input: "query",
                    input_mode: InputMode::Search,
                    selected_scope: None,
                    facts: $facts,
                    usage: &usage,
                    app_usage: &app_usage,
                    actions: &registry,
                    contexts: $contexts,
                    incoming_diagnostics: &[],
                    prior_diagnostics: &[],
                    catalog: &catalog,
                    sources: &[],
                    search_candidates: &[],
                    object_actions: &[],
                    now: 1,
                }
            };
        }
        let first = prepare(&input!(&contexts, &facts), &cache, &|_, _| {
            UsageStats::default()
        });
        assert!(!first.candidates.is_empty());
        contexts[0].revision = 2;
        let second = prepare(&input!(&contexts, &facts), &cache, &|_, _| {
            UsageStats::default()
        });
        assert!(second.candidates.iter().all(|candidate| candidate
            .context()
            .is_none_or(|context| context.revision == 2)));
        let changed = facts.with(
            crate::core::session::facts::CLIPBOARD,
            crate::core::session::facts::FactValue::Text("https://example.org/".into()),
        );
        let third = prepare(&input!(&contexts, &changed), &cache, &|_, _| {
            UsageStats::default()
        });
        assert!(third.candidates.iter().all(|candidate| candidate
            .presentation
            .command_id
            .as_deref()
            != Some("core.url.copy")));
    }
}
