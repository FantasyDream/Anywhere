//! Deterministic scoring. No ML, no randomness, no clock other than the caller's `now`.
pub use super::usage::UsageStats;
use crate::core::search::query::MatchLevel;
use anywhere_protocol::*;

/// Every term other than `query_match` must stay below the lowest query tier
/// (`Substring` = 1000), so usage history can never outrank a real query match.
pub const NON_QUERY_TERM_CEILING: i32 = 950;

/// Frequency weight per execution, capped at [`FREQUENCY_CAP`] executions.
///
/// Repetition has to be able to reorder a list, or the usage history is decorative:
/// at the original weight of 2, a seven-times-used action still lost to a
/// once-used action with 30 more `base_priority`, which is not what "frequency"
/// means to a user.
const FREQUENCY_WEIGHT: i32 = 8;
const FREQUENCY_CAP: u32 = 20;
/// Hard bounds so one term cannot dominate the others.
const FREQUENCY_CEILING: i32 = (FREQUENCY_WEIGHT as u32 * FREQUENCY_CAP) as i32;
const RECENCY_CEILING: i32 = 30;

/// Floor of the text-fallback band: the score a text action gets when the query
/// did not match its own searchable fields.
pub const FALLBACK_FLOOR: f32 = 100.0;
/// Ceiling of the text-fallback band, kept below the lowest query-match tier so a
/// fallback can never outrank a candidate that actually matched the query.
pub const FALLBACK_CEILING: f32 = 600.0;

/// Score for a text action offered as a query fallback.
///
/// The built-in Web Search and text plugins share this band, so the old V0.1
/// behaviour is preserved: both stay reachable, a matched field raises the score
/// within the band, and `base_priority` decides between otherwise equal fallbacks.
pub fn fallback_score(match_level: Option<MatchLevel>, base_priority: i32) -> f32 {
    let matched = match_level
        .map(|level| level.score(true) as f32 / 4000.0)
        .unwrap_or(0.0);
    let base = (FALLBACK_CEILING - FALLBACK_FLOOR) * matched;
    (FALLBACK_FLOOR + base + base_priority.clamp(-50, 50) as f32).clamp(0.0, FALLBACK_CEILING)
}

/// Upper bound of `context_relevance + base_priority + frequency + recency`.
pub fn non_query_terms(
    context_score: i32,
    base_priority: i32,
    frequency: i32,
    recency: i32,
) -> i32 {
    let total = context_score
        + base_priority.clamp(-50, 50)
        + frequency.clamp(0, FREQUENCY_CEILING)
        + recency.clamp(0, RECENCY_CEILING);
    total.min(NON_QUERY_TERM_CEILING)
}

/// Context relevance differs between Context Home and an explicit query.
///
/// Home: the ambient object leads, so context dominates usage.
/// Search: the query dominates, so context only breaks ties.
pub fn context_relevance(source: ContextSource, searching: bool, semantic: i32) -> i32 {
    let base = match source {
        ContextSource::UserInput => 200,
        ContextSource::FinderSelection => 190,
        ContextSource::SelectedText => 175,
        ContextSource::BrowserCurrentPage => 160,
        ContextSource::ApplicationContext => 120,
        ContextSource::Clipboard => 80,
        ContextSource::ForegroundApplication => 30,
    };
    if searching {
        base / 2 + semantic
    } else {
        base + semantic
    }
}

/// `frequency` and `recency` are bounded so repeated use cannot dominate context.
pub fn usage_terms(execute_count: u32, last_executed_at: i64, now: i64) -> (i32, i32) {
    let frequency = execute_count.min(FREQUENCY_CAP) as i32 * FREQUENCY_WEIGHT;
    let age = now.saturating_sub(last_executed_at).max(0);
    let recency = if execute_count == 0 {
        0
    } else if age < 3600 {
        30
    } else if age < 86400 {
        15
    } else if age < 604800 {
        8
    } else {
        0
    };
    (frequency, recency)
}

pub fn score(
    match_level: Option<MatchLevel>,
    searching: bool,
    source: ContextSource,
    semantic: i32,
    base_priority: i32,
    usage: UsageStats,
    now: i64,
) -> f32 {
    let query_match = match_level.map_or(0, |level| level.score(true));
    let (frequency, recency) = usage_terms(usage.execute_count, usage.last_executed_at, now);
    let terms = non_query_terms(
        context_relevance(source, searching, semantic),
        base_priority,
        frequency,
        recency,
    );
    (query_match + terms) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_fallbacks_stay_below_every_query_tier() {
        for level in [
            None,
            Some(MatchLevel::Substring),
            Some(MatchLevel::WordPrefix),
            Some(MatchLevel::Prefix),
            Some(MatchLevel::Exact),
        ] {
            let score = fallback_score(level, 50);
            assert!(
                score <= FALLBACK_CEILING,
                "{level:?} escaped the fallback band: {score}"
            );
            assert!(score < MatchLevel::Substring.score(true) as f32);
        }
    }

    #[test]
    fn a_higher_base_priority_wins_between_equal_fallbacks() {
        let plugin = fallback_score(None, 20);
        let builtin = fallback_score(None, 0);
        assert!(
            plugin > builtin,
            "the plugin must lead the plain text search"
        );
        // A matched field raises the score inside the band, still capped by the ceiling.
        let matched = fallback_score(Some(MatchLevel::Substring), 0);
        assert!(matched > builtin && matched <= FALLBACK_CEILING);
        assert_eq!(
            fallback_score(Some(MatchLevel::Exact), 50),
            FALLBACK_CEILING
        );
    }

    #[test]
    fn no_query_term_can_ever_reach_a_query_match_tier() {
        let worst = non_query_terms(
            context_relevance(ContextSource::UserInput, false, 90),
            50,
            80,
            40,
        );
        assert!(worst <= NON_QUERY_TERM_CEILING);
        assert!(worst < MatchLevel::Substring.score(true));
    }

    #[test]
    fn home_weights_context_more_than_search_does() {
        let home = context_relevance(ContextSource::ApplicationContext, false, 90);
        let search = context_relevance(ContextSource::ApplicationContext, true, 90);
        assert!(home > search);
        assert!(
            context_relevance(ContextSource::UserInput, false, 90)
                > context_relevance(ContextSource::ForegroundApplication, false, 90)
        );
    }

    #[test]
    fn usage_terms_are_bounded_and_decay() {
        assert_eq!(usage_terms(0, 0, 1000), (0, 0));
        assert_eq!(usage_terms(u32::MAX, 1000, 1000), (160, 30));
        assert_eq!(usage_terms(1, 0, 10_000).1, 15);
        assert_eq!(usage_terms(1, 0, 700_000).1, 0);
        assert_eq!(usage_terms(1, 0, 600_000).1, 8);
        assert_eq!(usage_terms(1, 0, 8_000_000).1, 0);
        // Repetition keeps growing until the cap, so a habit can outrank a default.
        assert!(usage_terms(8, 0, 1000).0 > usage_terms(2, 0, 1000).0);
        assert_eq!(usage_terms(50, 0, 1000).0, usage_terms(20, 0, 1000).0);
    }

    #[test]
    fn non_query_terms_are_bounded_and_saturate_at_the_ceiling() {
        let realistic = non_query_terms(
            context_relevance(ContextSource::UserInput, false, 90),
            50,
            80,
            40,
        );
        assert!(realistic <= NON_QUERY_TERM_CEILING);
        let saturated = non_query_terms(1000, 1000, 1000, 1000);
        assert_eq!(saturated, NON_QUERY_TERM_CEILING);
    }

    /// The reported defect: seven uses of `Copy Path` lost to one use of
    /// `Git Status`, because `base_priority` outweighed the whole frequency term.
    #[test]
    fn repeated_use_outranks_a_higher_base_priority_default() {
        let now = 1_000_000;
        let usage = |count: u32| UsageStats {
            execute_count: count,
            // Used moments ago, so recency is equal and cannot mask the comparison.
            last_executed_at: now,
        };
        let git_status_once = score(
            None,
            false,
            ContextSource::ApplicationContext,
            90,
            50,
            usage(1),
            now,
        );
        let copy_path_seven = score(
            None,
            false,
            ContextSource::ApplicationContext,
            90,
            20,
            usage(7),
            now,
        );
        assert!(
            copy_path_seven > git_status_once,
            "seven uses ({copy_path_seven}) must beat one use of a higher-priority \
             default ({git_status_once})"
        );

        // The default still wins while the habit has not formed.
        let copy_path_once = score(
            None,
            false,
            ContextSource::ApplicationContext,
            90,
            20,
            usage(1),
            now,
        );
        assert!(git_status_once > copy_path_once);
    }

    #[test]
    fn a_frequent_low_tier_match_never_beats_a_prefix_match() {
        let frequent = score(
            Some(MatchLevel::Substring),
            true,
            ContextSource::UserInput,
            90,
            50,
            UsageStats {
                execute_count: u32::MAX,
                last_executed_at: 1000,
            },
            1000,
        );
        let rare = score(
            Some(MatchLevel::Prefix),
            true,
            ContextSource::ForegroundApplication,
            90,
            -50,
            UsageStats::default(),
            1000,
        );
        assert!(rare > frequent);
    }
}
