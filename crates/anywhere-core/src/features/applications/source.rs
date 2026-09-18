//! Installed applications as a candidate source.
//!
//! Applications are matched, ranked and presented through exactly the same pipeline as
//! actions; the search engine has no application branch. The only thing this source owns
//! is what an application looks like and how it is matched.
use crate::core::context::{ContextSource, ResolvedContext};
use crate::core::ranking::score::{self, UsageStats};
use crate::core::search::candidate::Candidate;
use crate::core::search::candidate_source::{CandidateSource, SearchContext};
use crate::features::applications::index::IndexedApplication;
use anywhere_protocol::*;

/// Longest query still matched against the application index.
///
/// Application names are one or two words, so a query longer than this cannot be one in any
/// useful sense — it is pasted content, and the typed-object actions for it are already in
/// the list. Matching it against every installed application costs an exact scan of the
/// whole query per name: 1 MiB of pasted text against ~1000 installed applications took
/// 1.2s on the reference machine, on the path that builds the first snapshot. The bound
/// keeps that cost at a few hundred microseconds.
const MAX_QUERY: usize = 1024;

pub struct ApplicationSource<'a> {
    pub applications: &'a [IndexedApplication],
    /// `(application usage id, application context type)` usage, over every app.
    pub app_usage: &'a dyn Fn(&str, &str) -> UsageStats,
    pub now: i64,
}

impl CandidateSource for ApplicationSource<'_> {
    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        let query = search.query;
        let searching = !query.is_empty();
        // Context Home lists context actions only; applications exist for search, and only
        // for a query that can still be an application name.
        if !searching || query.raw_input.len() > MAX_QUERY {
            return Vec::new();
        }
        let mut candidates = Vec::new();
        for application in self.applications {
            // The bundle id is an identifier, not a name: a two-letter query must not
            // surface every app whose id contains those letters. Reverse-DNS noise is
            // stripped so matching happens on the part that identifies the app.
            let identifier = searchable_bundle_id(&application.bundle_id);
            let fields = crate::core::search::query::SearchFields::new(
                &application.name,
                &application.aliases,
                &application.keywords,
            )
            .with_metadata(&[identifier.as_str()]);
            let match_level = fields.matched(query);
            if match_level.is_none() {
                continue;
            }
            let usage = (self.app_usage)(
                &application.usage_id(),
                crate::core::ranking::usage::APPLICATION_CONTEXT_TYPE,
            );
            let score = score::score(
                match_level,
                searching,
                ContextSource::ApplicationContext,
                crate::core::ranking::usage::APPLICATION_CONTEXT_TYPE,
                10,
                usage,
                self.now,
            );
            candidates.push(Candidate {
                presentation: CandidatePresentation {
                    id: application_candidate_id(&application.bundle_id, &application.path),
                    kind: CandidateKind::Application,
                    title: application.name.clone(),
                    subtitle: Some(application.path.clone()),
                    icon: IconDescriptor {
                        key: "application".into(),
                    },
                    score,
                    group_id: crate::core::action::group::groups::APP.into(),
                    availability: Availability::enabled(),
                    command_id: None,
                    bundle_id: Some(application.bundle_id.clone()),
                    owning_app: None,
                },
                target: crate::core::search::candidate::CandidateTarget::LaunchApplication {
                    path: application.path.clone(),
                    icon_source: application.icon_source().to_owned(),
                },
            });
        }
        candidates
    }
}

/// The searchable form of a bundle id: the part that identifies the app, without the
/// reversed-DNS and TLD noise that is identical across vendors.
///
/// Both ends matter. `app.hiddify.com` contains `com` in the middle, which made a
/// two-letter query match it from either side; `com.jetbrains.intellij` both starts and
/// ends with the noise. The result is name-shaped, so a *whole or prefix* match on it is
/// meaningful: `co` matches nothing, while `jetbrains` or the full id still work.
pub fn searchable_bundle_id(bundle_id: &str) -> String {
    let lowered = bundle_id.to_lowercase();
    // A component is "noise" when it is a DNS TLD, in either position.
    let is_noise = |part: &str| {
        matches!(
            part,
            "com" | "org" | "net" | "io" | "dev" | "app" | "cn" | "me" | "co" | "xyz"
        )
    };
    let parts = lowered.split('.').collect::<Vec<_>>();
    let start = usize::from(parts.len() > 1 && is_noise(parts[0]));
    let mut end = parts.len();
    if end > start + 1 && is_noise(parts[end - 1]) {
        end -= 1;
    }
    let kept = parts[start..end].join(".");
    if kept.is_empty() {
        lowered
    } else {
        kept
    }
}

/// Stable identity of a launch candidate, namespaced under the same family as its usage row
/// so the two can never disagree about which application they describe.
pub fn application_candidate_id(bundle_id: &str, path: &str) -> String {
    format!(
        "{}:{path}",
        crate::core::ranking::usage::application_usage_id(bundle_id)
    )
}

/// Convenience for callers that only need the identifier a query should match.
pub fn search_fields<'a>(
    application: &'a IndexedApplication,
    identifier: &'a str,
) -> crate::core::search::query::SearchFields {
    crate::core::search::query::SearchFields::new(
        &application.name,
        &application.aliases,
        &application.keywords,
    )
    .with_metadata(&[identifier])
}

/// Re-exported so a feature can describe its own contexts without importing protocol types.
pub type FeatureContext = ResolvedContext;
