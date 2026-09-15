//! Unified candidate construction: applicability, availability, query matching,
//! ranking, grouping and scope membership.
use crate::command::{CommandDescriptor, CommandRegistry};
use crate::context::{ContextValue, ResolvedContext};
use crate::query::Query;
use crate::ranking::{self, UsageStats};
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::*;
use std::collections::BTreeMap;
use std::time::Instant;

pub const HOME_LIMIT: usize = 7;

/// Longest query that is still matched against the application index.
///
/// Application names are one or two words, so a query longer than this cannot be one in
/// any useful sense — it is pasted content, and the typed-object actions for it are
/// already in the list. Matching it against every installed application costs an exact
/// scan of the whole query per name: 1 MiB of pasted text against ~1000 installed
/// applications took 1.2s on the reference machine, on the path that builds the first
/// snapshot. The bound keeps that cost at a few hundred microseconds.
const MAX_QUERY_FOR_APPLICATIONS: usize = 1024;

/// An installed application available for direct launch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexedApplication {
    pub bundle_id: String,
    pub name: String,
    /// Canonical bundle path: both the launch target and the icon source.
    pub path: String,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
}

impl IndexedApplication {
    /// Icon source for this application.
    ///
    /// The bytes are fetched lazily for visible rows only; holding them all in the
    /// index would cost far more memory than the index itself.
    pub fn icon_source(&self) -> &str {
        &self.path
    }

    /// Usage row this application's history is recorded and read under.
    ///
    /// Derived from `bundle_id` so recording and ranking can never drift apart.
    pub fn usage_id(&self) -> String {
        crate::history::application_usage_id(&self.bundle_id)
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub presentation: CandidatePresentation,
    pub command: Option<CommandDescriptor>,
    pub context: Option<ResolvedContext>,
    pub application: Option<IndexedApplication>,
}

/// Read-only session facts an availability rule may inspect. No IO is allowed.
#[derive(Debug, Clone, Default)]
pub struct SessionFacts {
    pub clipboard_text: Option<String>,
    pub git_available: bool,
    pub frontmost: Option<ApplicationInfo>,
}

impl SessionFacts {
    fn clipboard_matches(&self, value: &str) -> bool {
        self.clipboard_text.as_deref() == Some(value)
    }
}

pub struct Matcher<'a> {
    pub registry: &'a CommandRegistry,
    pub applications: &'a [IndexedApplication],
    /// `(command_id, context_type)` usage, scoped to the current foreground app.
    pub usage: &'a dyn Fn(&str, &str) -> UsageStats,
    /// `(application usage id, application context type)` usage, over every app.
    pub app_usage: &'a dyn Fn(&str, &str) -> UsageStats,
    pub facts: &'a SessionFacts,
    pub now: i64,
}

pub struct CandidateSet {
    pub candidates: Vec<Candidate>,
    pub scopes: Vec<ScopeEntry>,
    /// Usage total per dynamic scope, used to order the scope bar.
    pub scope_use: BTreeMap<GroupId, u32>,
    /// Action matching: applicability, availability and query match per context.
    pub matcher_us: u64,
    /// Application-index matching, reported separately so it cannot be mistaken for
    /// sorting cost.
    pub apps_us: u64,
    /// Scoring and ordering of the candidates that were matched.
    pub ranking_us: u64,
}

impl<'a> Matcher<'a> {
    pub fn build(&self, query: &Query, contexts: &[ResolvedContext]) -> CandidateSet {
        let matcher_started = Instant::now();
        let searching = !query.is_empty();
        let mut action_candidates = Vec::new();

        for context in contexts {
            // Ownership is per context, never carried over: an input-derived object
            // must not inherit the ambient application just because an ambient context
            // was visited earlier in this loop.
            let owned_by = match self.context_ownership(context) {
                Ownership::App(app) => Some(app),
                Ownership::Ambient => None,
            };
            for command in self.registry.matching(context) {
                let match_level = command.search.matched(query);
                if searching && match_level.is_none() && !Self::is_typed_object_action(context) {
                    continue;
                }
                let availability = self.availability(command, context);
                if !availability.is_visible() {
                    continue;
                }
                let stats = (self.usage)(&command.id, &context.type_id);
                let mut score = ranking::score(
                    match_level,
                    searching,
                    context.source,
                    &context.type_id,
                    command.base_priority,
                    stats,
                    self.now,
                );
                if !searching {
                    score += self.home_bonus(command, context);
                }
                action_candidates.push(Candidate {
                    presentation: CandidatePresentation {
                        id: action_candidate_id(&command.id, context),
                        kind: CandidateKind::Action,
                        title: command.title.clone(),
                        subtitle: Some(self.subtitle(command, context)),
                        icon: IconDescriptor {
                            key: command.icon.clone(),
                        },
                        score,
                        group_id: command.group_id.clone(),
                        availability,
                        command_id: Some(command.id.clone()),
                        bundle_id: None,
                        owning_app: owned_by.clone().or_else(|| command.owning_app.clone()),
                    },
                    command: Some(command.clone()),
                    context: Some(context.clone()),
                    application: None,
                });
            }
        }
        let matcher_us = matcher_started.elapsed().as_micros() as u64;
        // How much the user actually uses each group: the bar should lead with the
        // scope they lean on instead of a fixed order.
        let mut scope_use: BTreeMap<GroupId, u32> = BTreeMap::new();
        for candidate in &action_candidates {
            let (Some(command), Some(context)) = (
                candidate.presentation.command_id.as_ref(),
                candidate.context.as_ref(),
            ) else {
                continue;
            };
            let count = (self.usage)(command, &context.type_id).execute_count;
            *scope_use
                .entry(candidate.presentation.group_id.clone())
                .or_default() += count;
        }

        // Application matching is *matching*, not ranking: it builds search fields and
        // matches every installed application against the query. Charging it to
        // `ranking_us` made that number unattributable and hid a 1.2s regression behind a
        // metric that was supposed to describe sorting.
        let apps_started = Instant::now();
        let mut applications = Vec::new();
        // Context Home lists context actions only; applications exist for search, and only
        // for a query that can still be an application name.
        if searching && query.raw_input.len() <= MAX_QUERY_FOR_APPLICATIONS {
            applications = self.application_candidates(query);
        }
        let apps_us = apps_started.elapsed().as_micros() as u64;

        let ranking_started = Instant::now();
        action_candidates.sort_by(compare_candidates);
        deduplicate_actions(&mut action_candidates);

        let mut candidates = action_candidates;
        if !applications.is_empty() {
            applications.sort_by(compare_candidates);
            candidates.extend(applications);
            candidates.sort_by(compare_candidates);
        }
        // Text fallbacks are appended last, after sorting, so they never displace a
        // real query match or a typed-object action.
        if searching {
            candidates.extend(self.text_fallbacks(query, &candidates));
        }
        // Scopes are built from every visible candidate, before the Home limit, so a
        // group never looks empty just because its rows fall outside the top seven.
        // Which application the Current App scope stands for: the ambient owner, taken
        // from the ambient contexts only.
        let ambient_app = contexts
            .iter()
            .filter(|context| context.source != ContextSource::UserInput)
            .find_map(|context| match self.context_ownership(context) {
                Ownership::App(app) => Some(app),
                Ownership::Ambient => None,
            });
        let scopes = self.scopes(&candidates, &ambient_app, &scope_use);
        // Every candidate stays in the state. Home is a *view* limit, applied by the
        // host to the scope it displays: truncating the set here left each scope's id
        // list pointing at entries that no longer existed, so a group could look
        // populated and then render empty, and a row pushed past the limit lost the
        // identity the selection was holding on to.
        let ranking_us = ranking_started.elapsed().as_micros() as u64;
        CandidateSet {
            candidates,
            scopes,
            scope_use,
            matcher_us,
            apps_us,
            ranking_us,
        }
    }

    /// Typed-object actions stay reachable while searching even without a title match,
    /// so pasting a URL, JSON document or path still offers its actions.
    fn is_typed_object_action(context: &ResolvedContext) -> bool {
        context.source == ContextSource::UserInput
            && !matches!(context.value, ContextValue::Text(_))
    }

    /// Home-only weighting: the ambient object leads, so its actions come first.
    ///
    /// There is deliberately no per-group term here: Git actions only ever accept a
    /// `git.repository` context, so a rule keyed on a directory could never apply.
    fn home_bonus(&self, _command: &CommandDescriptor, context: &ResolvedContext) -> f32 {
        match context.source {
            ContextSource::ApplicationContext | ContextSource::ForegroundApplication => 200.0,
            ContextSource::UserInput => 0.0,
        }
    }

    fn subtitle(&self, command: &CommandDescriptor, context: &ResolvedContext) -> String {
        let detail = display_detail(context);
        match &command.subtitle {
            Some(subtitle) => format!("{subtitle} · {detail}"),
            None => detail,
        }
    }

    fn availability(&self, command: &CommandDescriptor, context: &ResolvedContext) -> Availability {
        let path = context.value.path();
        match command.id.as_str() {
            // The clipboard already holds this exact value, so copying is a no-op.
            "core.directory.copy" | "core.file.copy" => {
                if let Some(path) = path {
                    if self.facts.clipboard_matches(&path.to_string_lossy()) {
                        return Availability::hidden();
                    }
                }
            }
            // Availability must use the same source of truth as execution dispatch.
            //
            // Only in-memory facts are consulted here. The spec forbids availability
            // from doing IO of its own, so whether the repository still exists is left
            // to execution, which reports it as a normal failure.
            id if crate::command::is_git_action(id) && !self.facts.git_available => {
                return Availability::disabled("git is not installed");
            }
            _ => {}
        }
        Availability::enabled()
    }

    /// Text actions offered when the query did not match their searchable fields.
    ///
    /// This preserves the V0.1 text behaviour: pasting a URL, JSON document or path
    /// still offers Web Search and text plugins, but they sit below every candidate
    /// that actually matched the query.
    fn text_fallbacks(&self, query: &Query, existing: &[Candidate]) -> Vec<Candidate> {
        let context = ResolvedContext::new(
            TEXT,
            ContextValue::Text(query.raw_input.clone()),
            ContextSource::UserInput,
            "core.text_fallback",
        );
        let mut fallbacks = self
            .registry
            .matching(&context)
            .map(|command| {
                let level = command.search.matched(query);
                Candidate {
                    presentation: CandidatePresentation {
                        id: action_candidate_id(&command.id, &context),
                        kind: CandidateKind::Action,
                        title: command.title.clone(),
                        subtitle: Some(display_detail(&context)),
                        icon: IconDescriptor {
                            key: command.icon.clone(),
                        },
                        score: ranking::fallback_score(level, command.base_priority),
                        group_id: command.group_id.clone(),
                        availability: Availability::enabled(),
                        command_id: Some(command.id.clone()),
                        bundle_id: None,
                        owning_app: None,
                    },
                    command: Some(command.clone()),
                    context: Some(context.clone()),
                    application: None,
                }
            })
            .filter(|candidate| {
                !existing.iter().any(|existing| {
                    existing.presentation.command_id == candidate.presentation.command_id
                })
            })
            .collect::<Vec<_>>();
        fallbacks.sort_by(compare_candidates);
        fallbacks
    }

    fn application_candidates(&self, query: &Query) -> Vec<Candidate> {
        // Every application name is scanned against each query token, so cost grows with
        // the query too; a pasted document is not an application search (see the bound and
        // its measurement in `MAX_QUERY_FOR_APPLICATIONS`).
        if query.raw_input.len() > MAX_QUERY_FOR_APPLICATIONS {
            return Vec::new();
        }
        let searching = !query.is_empty();
        let mut candidates = Vec::new();
        for application in self.applications {
            // The bundle id is an identifier, not a name: a two-letter query must not
            // surface every app whose id contains those letters. Reverse-DNS noise is
            // stripped so matching happens on the part that identifies the app.
            let identifier = searchable_bundle_id(&application.bundle_id);
            let search = crate::query::SearchFields::new(
                &application.name,
                &application.aliases,
                &application.keywords,
            )
            .with_metadata(&[identifier.as_str()]);
            let match_level = search.matched(query);
            if searching && match_level.is_none() {
                continue;
            }
            // Must match the id recorded when the application is launched.
            let usage = (self.app_usage)(
                &application.usage_id(),
                crate::history::APPLICATION_CONTEXT_TYPE,
            );
            let score = ranking::score(
                match_level,
                searching,
                ContextSource::ApplicationContext,
                crate::history::APPLICATION_CONTEXT_TYPE,
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
                    group_id: crate::command::groups::APP.into(),
                    availability: Availability::enabled(),
                    command_id: None,
                    bundle_id: Some(application.bundle_id.clone()),
                    owning_app: None,
                },
                command: None,
                context: None,
                application: Some(application.clone()),
            });
        }
        candidates
    }

    /// Membership follows the selected Current App semantics: ambient provenance
    /// plus explicitly app-owned actions. It never includes launch candidates.
    fn context_ownership(&self, context: &ResolvedContext) -> Ownership {
        match context.source {
            ContextSource::ForegroundApplication | ContextSource::ApplicationContext => {
                match self
                    .facts
                    .frontmost
                    .as_ref()
                    .filter(|app| !app.bundle_id.is_empty())
                {
                    Some(application) => Ownership::App(application.bundle_id.clone()),
                    None => Ownership::Ambient,
                }
            }
            ContextSource::UserInput => Ownership::Ambient,
        }
    }

    fn scopes(
        &self,
        candidates: &[Candidate],
        current_app_group: &Option<String>,
        scope_use: &BTreeMap<GroupId, u32>,
    ) -> Vec<ScopeEntry> {
        let all_ids = candidates
            .iter()
            .map(|candidate| candidate.presentation.id.clone())
            .collect::<Vec<_>>();
        let app_label = self.app_group_label();
        let mut scopes = vec![
            ScopeEntry {
                id: SCOPE_ALL.into(),
                kind: ScopeKind::All,
                label: "All".into(),
                candidate_ids: all_ids,
            },
            ScopeEntry {
                id: SCOPE_APP.into(),
                kind: ScopeKind::App,
                label: app_label,
                candidate_ids: Vec::new(),
            },
        ];
        let mut groups: BTreeMap<GroupId, Vec<ActionId>> = BTreeMap::new();
        for candidate in candidates {
            if candidate.presentation.kind != CandidateKind::Action {
                continue;
            }
            let owns_current_app = match (
                &candidate.presentation.owning_app,
                current_app_group.as_deref(),
            ) {
                (Some(owning), Some(current)) => owning == current,
                _ => false,
            };
            if owns_current_app {
                scopes[1]
                    .candidate_ids
                    .push(candidate.presentation.id.clone());
            }
            groups
                .entry(candidate.presentation.group_id.clone())
                .or_default()
                .push(candidate.presentation.id.clone());
        }
        // Presentation order is fixed for the built-in groups so the bar reads the
        // same as the documented examples (`Git` before `Folder`); anything else
        // follows in id order.
        let mut ordered = groups.into_iter().collect::<Vec<_>>();
        // Most-used scope first. `group_rank` then the id break ties, so a fresh
        // install still reads like the documented example and the order never depends
        // on the luck of iteration.
        let use_of = |group: &str| scope_use.get(group).copied().unwrap_or_default();
        ordered.sort_by(|(a, _), (b, _)| {
            use_of(b)
                .cmp(&use_of(a))
                .then_with(|| group_rank(a).cmp(&group_rank(b)))
                .then_with(|| a.cmp(b))
        });
        for (group, ids) in ordered {
            // The Current App scope already merges app-owned groups into slot two.
            if group == crate::command::groups::APP {
                continue;
            }
            scopes.push(ScopeEntry {
                id: group.clone(),
                kind: ScopeKind::Group,
                label: group_label(&group),
                candidate_ids: ids,
            });
        }
        scopes
    }

    fn app_group_label(&self) -> String {
        self.facts
            .frontmost
            .as_ref()
            .map(|app| app.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Current App".into())
    }
}

enum Ownership {
    App(String),
    Ambient,
}

/// Scope ids reserved by the host: `All` first, the current application second.
pub const SCOPE_ALL: &str = "all";
pub const SCOPE_APP: &str = "app";

/// Display order of the built-in dynamic scopes.
fn group_rank(group: &str) -> u8 {
    match group {
        crate::command::groups::GIT => 0,
        crate::command::groups::FOLDER => 1,
        crate::command::groups::FILE => 2,
        crate::command::groups::WEB => 3,
        crate::command::groups::JSON => 4,
        _ => 5,
    }
}

/// Built-in display order for known groups; unknown groups fall back to an id-derived label.
fn group_label(group: &str) -> String {
    match group {
        crate::command::groups::GIT => "Git".into(),
        crate::command::groups::FOLDER => "Folder".into(),
        crate::command::groups::FILE => "File".into(),
        crate::command::groups::WEB => "Web".into(),
        crate::command::groups::JSON => "JSON".into(),
        crate::command::groups::APP => "Current App".into(),
        other => match other.strip_prefix("plugin.") {
            Some(plugin) => plugin
                .split(['.', '_', '-'])
                .map(capitalize)
                .collect::<Vec<_>>()
                .join(" "),
            None => capitalize(other),
        },
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Stable business identity: survives re-ranking, renumbering and late enrichment.
pub fn action_candidate_id(command: &str, context: &ResolvedContext) -> String {
    format!(
        "action:{command}:{}:{}",
        context.type_id,
        context.value.text()
    )
}

/// The searchable form of a bundle id: the part that identifies the app, without the
/// reversed-DNS and TLD noise that is identical across vendors.
///
/// Both ends matter. `app.hiddify.com` contains `com` in the middle, which made a
/// two-letter query match it from either side; `com.jetbrains.intellij` both starts
/// and ends with the noise. The result is name-shaped, so a *whole or prefix* match on
/// it is meaningful: `co` matches nothing, while `jetbrains` or the full id still work.
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

/// Stable identity of a launch candidate, namespaced under the same family as its
/// usage row so the two can never disagree about which application they describe.
pub fn application_candidate_id(bundle_id: &str, path: &str) -> String {
    format!("{}:{path}", crate::history::application_usage_id(bundle_id))
}

fn candidate_sort_key(candidate: &Candidate) -> (&str, &str) {
    (
        candidate
            .presentation
            .command_id
            .as_deref()
            .unwrap_or_else(|| candidate.presentation.bundle_id.as_deref().unwrap_or("")),
        candidate.presentation.id.as_str(),
    )
}

fn compare_candidates(a: &Candidate, b: &Candidate) -> std::cmp::Ordering {
    b.presentation
        .score
        .total_cmp(&a.presentation.score)
        .then_with(|| {
            let (a_command, a_id) = candidate_sort_key(a);
            let (b_command, b_id) = candidate_sort_key(b);
            a_command.cmp(b_command).then_with(|| a_id.cmp(b_id))
        })
}

/// Keep one row per command and canonical object, preferring the highest-ranked
/// occurrence. Scope membership is unioned so ownership survives deduplication.
fn deduplicate_actions(candidates: &mut Vec<Candidate>) {
    let mut merged: Vec<Candidate> = Vec::with_capacity(candidates.len());
    let mut index: BTreeMap<(CommandId, ContextTypeId, String), usize> = BTreeMap::new();
    for candidate in candidates.drain(..) {
        let Some(command) = candidate.presentation.command_id.clone() else {
            merged.push(candidate);
            continue;
        };
        let context = candidate
            .context
            .as_ref()
            .map(|context| (context.type_id.clone(), context.value.text()))
            .unwrap_or_default();
        let key = (command, context.0, context.1);
        match index.get(&key) {
            Some(position) => {
                let existing = &mut merged[*position];
                if existing.presentation.owning_app.is_none() {
                    existing.presentation.owning_app = candidate.presentation.owning_app;
                }
            }
            None => {
                index.insert(key, merged.len());
                merged.push(candidate);
            }
        }
    }
    *candidates = merged;
}

/// Short subtitle text; execution still uses the full transport value.
fn display_detail(context: &ResolvedContext) -> String {
    const MAX: usize = 120;
    let text = context.value.text();
    if text.chars().count() <= MAX {
        return text;
    }
    let mut shortened = text.chars().take(MAX).collect::<String>();
    shortened.push('…');
    shortened
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRegistry;
    use crate::input::text_context;

    fn contexts(input: &str) -> Vec<ResolvedContext> {
        let mut contexts = crate::input::InputEngine.resolve(input);
        if let Some(text) = text_context(input) {
            contexts.insert(0, text);
        }
        contexts
    }

    fn facts() -> SessionFacts {
        SessionFacts {
            git_available: true,
            ..Default::default()
        }
    }

    fn no_usage(_: &str, _: &str) -> UsageStats {
        UsageStats::default()
    }

    fn builtin_matcher<'a>(
        registry: &'a CommandRegistry,
        applications: &'a [IndexedApplication],
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
        facts: &'a SessionFacts,
    ) -> Matcher<'a> {
        Matcher {
            registry,
            applications,
            usage,
            app_usage: usage,
            facts,
            now: 1000,
        }
    }

    #[test]
    fn directory_context_orders_the_documented_examples() {
        let registry = CommandRegistry::builtin();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        std::fs::create_dir(path.join(".git")).unwrap();
        let mut contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(path.clone()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        contexts.push(ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository(path.clone()),
            ContextSource::ApplicationContext,
            "git",
        ));
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let titles = set
            .candidates
            .iter()
            .map(|c| c.presentation.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            &titles[..3],
            ["Git Status", "Open Terminal Here", "Git Log"],
            "git directory example"
        );

        contexts.retain(|context| context.type_id != GIT_REPOSITORY);
        let set = matcher.build(&Query::parse(""), &contexts);
        let titles = set
            .candidates
            .iter()
            .map(|c| c.presentation.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            &titles[..3],
            ["Open Terminal Here", "Copy Path", "Open Directory"]
        );
    }

    /// Home is a view limit, not a reason to drop candidates.
    ///
    /// Truncating the set left scope id lists referring to entries that no longer
    /// existed, so a populated group could render empty.
    #[test]
    fn every_scope_id_resolves_to_a_candidate() {
        let registry = CommandRegistry::builtin();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let mut contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.clone()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(path),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        // Several distinct directories, so the set comfortably exceeds the Home limit.
        let extra = tempfile::tempdir().unwrap();
        for index in 0..4 {
            let nested = extra.path().join(format!("d{index}"));
            std::fs::create_dir_all(&nested).unwrap();
            contexts.push(ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(nested.canonicalize().unwrap()),
                ContextSource::ApplicationContext,
                "finder",
            ));
        }
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        assert!(
            set.candidates.len() > HOME_LIMIT,
            "this fixture must exceed the Home limit to be meaningful: {} candidates",
            set.candidates.len()
        );
        for scope in &set.scopes {
            for id in &scope.candidate_ids {
                assert!(
                    set.candidates
                        .iter()
                        .any(|candidate| &candidate.presentation.id == id),
                    "scope {} refers to a dropped candidate {id}",
                    scope.id
                );
            }
        }
    }

    #[test]
    fn home_is_bounded_while_search_stays_scrollable() {
        let registry = CommandRegistry::builtin();
        let mut contexts = contexts(&"a".repeat(60));
        contexts.push(ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        ));
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        // Home keeps every candidate; the host limits what it displays.
        let home = matcher.build(&Query::parse(""), &contexts);
        assert!(!home.candidates.is_empty());
        let search = matcher.build(&Query::parse("copy"), &contexts);
        // Search results stay scrollable; only Home is capped at seven rows.
        assert!(!search.candidates.is_empty(), "{:?}", search.candidates);
        assert!(search.candidates.iter().all(|c| c
            .presentation
            .title
            .to_lowercase()
            .contains("copy")
            || c.presentation.title == "Web Search"));
    }

    #[test]
    fn groups_are_visible_before_scope_filtering_and_home_truncation() {
        let registry = CommandRegistry::builtin();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.clone()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(path),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let scope_ids = set.scopes.iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
        assert_eq!(scope_ids[0], "all");
        assert_eq!(scope_ids[1], "app");
        assert!(scope_ids.contains(&"git"));
        assert!(scope_ids.contains(&"folder"));
    }

    /// The scope bar leads with what the user actually uses, not a fixed order.
    #[test]
    fn scopes_are_ordered_by_usage_with_a_deterministic_tie_break() {
        let registry = CommandRegistry::builtin();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.clone()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(path),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        let applications = Vec::new();
        let facts = facts();

        // Heavy use of the folder actions; the Git group has never been used.
        let folder_heavy = |command: &str, kind: &str| {
            if kind == DIRECTORY {
                UsageStats {
                    execute_count: 8,
                    last_executed_at: 1000,
                }
            } else {
                let _ = command;
                UsageStats::default()
            }
        };
        let matcher = builtin_matcher(&registry, &applications, &folder_heavy, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["all", "app", "folder", "git"],
            "folder is used, git is not"
        );
        assert_eq!(
            set.scope_use.get("folder"),
            Some(&24),
            "8 uses of 3 folder actions"
        );

        // With the same usage everywhere, the documented order decides.
        let none_used = |_: &str, _: &str| UsageStats::default();
        let matcher = builtin_matcher(&registry, &applications, &none_used, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["all", "app", "git", "folder"],
            "fresh install keeps the docs order"
        );

        // Using only the Git actions flips the two.
        let git_heavy = |_: &str, kind: &str| {
            if kind == GIT_REPOSITORY {
                UsageStats {
                    execute_count: 9,
                    last_executed_at: 1000,
                }
            } else {
                UsageStats::default()
            }
        };
        let matcher = builtin_matcher(&registry, &applications, &git_heavy, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["all", "app", "git", "folder"],
            "git use puts git first"
        );
        assert!(set.scope_use.get("git").copied().unwrap_or_default() > 0);
    }

    #[test]
    fn copy_path_is_hidden_when_the_clipboard_already_holds_it() {
        let registry = CommandRegistry::builtin();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(path.clone()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        let applications = Vec::new();
        let usage = no_usage;
        let mut session = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &session);
        assert!(matcher
            .build(&Query::parse(""), &contexts)
            .candidates
            .iter()
            .any(|c| c.presentation.title == "Copy Path"));

        session.clipboard_text = Some(path.to_string_lossy().into_owned());
        let matcher = builtin_matcher(&registry, &applications, &usage, &session);
        assert!(!matcher
            .build(&Query::parse(""), &contexts)
            .candidates
            .iter()
            .any(|c| c.presentation.title == "Copy Path"));
    }

    #[test]
    fn git_actions_are_disabled_rather_than_hidden_without_git() {
        let registry = CommandRegistry::builtin();
        let contexts = vec![ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository("/tmp".into()),
            ContextSource::ApplicationContext,
            "git",
        )];
        let applications = Vec::new();
        let usage = no_usage;
        let facts = SessionFacts::default();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let status = set
            .candidates
            .iter()
            .find(|c| c.presentation.title == "Git Status")
            .expect("git status stays visible so the group is discoverable");
        assert_eq!(
            status.presentation.availability.state,
            AvailabilityState::Disabled
        );
        assert!(status.presentation.availability.reason.is_some());
    }

    #[test]
    fn application_search_matches_names_aliases_and_keywords() {
        let registry = CommandRegistry::builtin();
        let applications = vec![IndexedApplication {
            bundle_id: "com.google.Chrome".into(),
            name: "Google Chrome".into(),
            path: "/Applications/Google Chrome.app".into(),
            aliases: vec!["chrome".into()],
            keywords: vec!["browser".into()],
        }];
        let chrome_contexts = contexts("chrome");
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse("chrome"), &chrome_contexts);
        assert_eq!(set.candidates[0].presentation.title, "Google Chrome");
        assert_eq!(
            set.candidates[0].presentation.kind,
            CandidateKind::Application
        );
        let by_keyword = matcher.build(&Query::parse("browser"), &self::contexts("browser"));
        assert_eq!(by_keyword.candidates[0].presentation.title, "Google Chrome");
        // Applications never appear in Context Home.
        let home = matcher.build(&Query::parse(""), &chrome_contexts);
        assert!(home
            .candidates
            .iter()
            .all(|c| c.presentation.kind != CandidateKind::Application));
    }

    #[test]
    fn action_search_finds_git_status_by_partial_words() {
        let registry = CommandRegistry::builtin();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory("/tmp".into()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository("/tmp".into()),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse("git sta"), &contexts);
        assert_eq!(set.candidates[0].presentation.title, "Git Status");
    }

    #[test]
    fn typed_object_actions_survive_a_query_without_a_title_match() {
        let registry = CommandRegistry::builtin();
        let url_contexts = contexts("https://github.com");
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse("https://github.com"), &url_contexts);
        let titles = set
            .candidates
            .iter()
            .map(|c| c.presentation.title.as_str())
            .collect::<Vec<_>>();
        assert!(titles.contains(&"Open in Default Browser"), "{titles:?}");
        assert!(titles.contains(&"Copy URL"), "{titles:?}");
        // No text fallback ships with the host: a built-in web search could only
        // hardcode one provider, so it is a plugin decision instead.
        assert!(
            !titles.iter().any(|title| title.contains("Web Search")),
            "the host must not ship a hardcoded web search: {titles:?}"
        );

        // An unrelated sentence must not surface URL actions for a URL context.
        let unrelated = contexts("hello world");
        let set = matcher.build(&Query::parse("hello"), &unrelated);
        assert!(set
            .candidates
            .iter()
            .all(|c| c.presentation.title != "Open in Default Browser"));
    }

    #[test]
    fn candidate_ids_are_stable_business_identity() {
        let context = ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        );
        let id = action_candidate_id("core.directory.copy", &context);
        assert_eq!(id, "action:core.directory.copy:core.directory:/tmp");
        assert_eq!(
            application_candidate_id("com.google.Chrome", "/Applications/Chrome.app"),
            "app:com.google.Chrome:/Applications/Chrome.app"
        );
    }

    #[test]
    fn current_app_scope_uses_ambient_provenance_and_excludes_launch_candidates() {
        let registry = CommandRegistry::builtin();
        let contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        let applications = vec![IndexedApplication {
            bundle_id: "com.apple.finder".into(),
            name: "Finder".into(),
            path: "/System/Library/CoreServices/Finder.app".into(),
            ..Default::default()
        }];
        let usage = no_usage;
        let facts = SessionFacts {
            git_available: true,
            frontmost: Some(ApplicationInfo {
                bundle_id: "com.apple.finder".into(),
                name: "Finder".into(),
            }),
            ..Default::default()
        };
        let matcher = builtin_matcher(&registry, &applications, &usage, &facts);
        let set = matcher.build(&Query::parse(""), &contexts);
        let app_scope = set.scopes.iter().find(|s| s.id == "app").unwrap();
        assert_eq!(app_scope.label, "Finder");
        assert!(!app_scope.candidate_ids.is_empty());
        assert!(app_scope
            .candidate_ids
            .iter()
            .all(|id| !id.starts_with("app:")));
    }
}
