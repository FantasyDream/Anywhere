//! Minimal deterministic query parsing and field matching.
//!
//! Deliberately excludes NLP, CJK word segmentation and pinyin conversion.

/// Trim, lowercase and split on ASCII whitespace into non-empty tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub raw_input: String,
    pub normalized_query: String,
    pub tokens: Vec<String>,
    compact: String,
}

impl Query {
    pub fn parse(raw_input: &str) -> Self {
        let normalized_query = raw_input.trim().to_lowercase();
        let tokens = normalized_query
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect();
        let compact = normalized_query
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        Self {
            raw_input: raw_input.to_owned(),
            normalized_query,
            tokens,
            compact,
        }
    }

    /// The normalized query with separators removed.
    ///
    /// Lets a user type a name without its separators: `abdown` → ABDownloadManager.
    ///
    /// Only separators are dropped. **All** letters and digits survive, including
    /// non-ASCII ones: filtering to ASCII deleted a Chinese token outright, so
    /// `google 不存在` compacted to `google` and matched Google Chrome even though the
    /// second token matched nothing. That is discarding query information, which is
    /// not the same as "no Chinese segmentation".
    pub fn compact(&self) -> &str {
        &self.compact
    }

    /// A query with no tokens is Context Home, not a search.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

/// How well a candidate's searchable fields matched the query.
///
/// Ordered best-first; a higher score always outranks a lower one regardless of
/// usage history, so the query tier dominates every other ranking term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchLevel {
    /// Every token matched inside some field.
    Substring,
    /// Every token matched the start of a word in some field.
    WordPrefix,
    /// Every token matched the start of one field.
    Prefix,
    /// The whole normalized query equals one field.
    Exact,
}

impl MatchLevel {
    /// Query-match score; `None` means the query did not match at all.
    pub fn score(self, applied: bool) -> i32 {
        if !applied {
            return 0;
        }
        match self {
            Self::Exact => 4000,
            Self::Prefix => 3000,
            Self::WordPrefix => 2000,
            Self::Substring => 1000,
        }
    }
}

/// Per-candidate searchable text, precomputed once when the candidate is registered.
///
/// The two sets are matched differently on purpose. A **visible name** also matches as
/// a substring, so `vscode` finds Visual Studio Code. An **identifier** — a bundle id
/// or a file stem, which the user never sees — only matches exactly or at a word
/// start; otherwise `co` would surface every app whose bundle id contains those
/// letters, which is exactly what was reported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFields {
    name: Vec<String>,
    metadata: Vec<String>,
}

impl SearchFields {
    /// Searchable text: the visible title and aliases, then identifier keywords.
    pub fn new(title: &str, aliases: &[String], keywords: &[String]) -> Self {
        let mut name = Vec::with_capacity(1 + aliases.len());
        name.push(title.to_lowercase());
        name.extend(aliases.iter().map(|alias| alias.to_lowercase()));
        name.retain(|field| !field.is_empty());
        let mut fields = Self {
            name,
            metadata: Vec::new(),
        };
        fields
            .metadata
            .extend(keywords.iter().map(|entry| entry.to_lowercase()));
        fields.metadata.retain(|field| !field.is_empty());
        fields
    }

    /// Add identifiers that must never be matched as substrings.
    pub fn with_metadata(mut self, metadata: &[&str]) -> Self {
        self.metadata
            .extend(metadata.iter().map(|entry| entry.to_lowercase()));
        self.metadata.retain(|field| !field.is_empty());
        self
    }

    pub fn names(&self) -> &[String] {
        &self.name
    }

    pub fn metadata(&self) -> &[String] {
        &self.metadata
    }

    /// The best match level across all fields, or `None` when some token is missing.
    pub fn matched(&self, query: &Query) -> Option<MatchLevel> {
        if query.is_empty() {
            return None;
        }
        let names = self
            .name
            .iter()
            .map(|field| field_match(field, query, false));
        let metadata = self
            .metadata
            .iter()
            .map(|field| field_match(field, query, true));
        names.chain(metadata).flatten().max()
    }
}

fn field_match(field: &str, query: &Query, identifier_only: bool) -> Option<MatchLevel> {
    if field == query.normalized_query {
        return Some(MatchLevel::Exact);
    }
    // An identifier is matched as a whole, never segment by segment: `co` must not
    // reach the `com` segment of a bundle id. Pasting the full id still finds it.
    if identifier_only {
        return (field.starts_with(query.normalized_query.as_str())
            && !query.normalized_query.is_empty())
        .then_some(MatchLevel::Prefix);
    }
    if query.tokens.iter().all(|token| {
        field
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| !word.is_empty() && word.starts_with(token.as_str()))
    }) {
        return Some(MatchLevel::WordPrefix);
    }
    if !field.contains(' ')
        && field.starts_with(query.normalized_query.as_str())
        && !query.normalized_query.is_empty()
    {
        return Some(MatchLevel::Prefix);
    }
    // Separator-insensitive prefix of a visible name: `abdown` → ABDownloadManager.
    if compact_prefix(field, query.compact()) {
        return Some(MatchLevel::Substring);
    }
    if query
        .tokens
        .iter()
        .all(|token| field.contains(token.as_str()))
    {
        return Some(MatchLevel::Substring);
    }
    None
}

/// Compare only as far as the query needs, without allocating a compact copy of
/// every candidate on every keystroke. Non-ASCII UTF-8 bytes cannot pass the ASCII
/// filter, exactly like the previous char-based materialization.
fn compact_prefix(field: &str, wanted: &str) -> bool {
    let mut compact = field.bytes().filter(u8::is_ascii_alphanumeric);
    !wanted.is_empty() && wanted.bytes().all(|byte| compact.next() == Some(byte))
}

/// Guards against pathological manifests without allocating per lookup.
pub fn valid_search_field(value: &str) -> bool {
    value.len() <= 200
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_compact_prefix_preserves_materialized_unicode_and_separator_behavior() {
        for field in [
            "",
            "google chrome",
            "ab-download_manager",
            "café",
            "工具abc",
            "İstanbul",
            "😀x",
            "^a!b$1",
        ] {
            let compact: String = field
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            for wanted in [
                "", "g", "googlec", "abdown", "ab1", "caf", "café", "工具", "abc", "istan", "x",
                "a", "z",
            ] {
                assert_eq!(
                    compact_prefix(field, wanted),
                    !wanted.is_empty() && compact.starts_with(wanted),
                    "{field:?}, {wanted:?}"
                );
            }
        }
    }

    fn fields(title: &str, aliases: &[&str], keywords: &[&str]) -> SearchFields {
        let aliases = aliases.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let keywords = keywords.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        SearchFields::new(title, &aliases, &keywords)
    }

    #[test]
    fn parsing_trims_lowercases_and_splits_on_whitespace() {
        let query = Query::parse("  Git   STA  ");
        assert_eq!(query.raw_input, "  Git   STA  ");
        assert_eq!(query.normalized_query, "git   sta");
        assert_eq!(query.tokens, ["git", "sta"]);
        assert!(!query.is_empty());
        for blank in ["", "   ", "\t\n"] {
            assert!(Query::parse(blank).is_empty());
        }
    }

    #[test]
    fn match_levels_rank_exact_over_prefix_over_word_over_substring() {
        let chrome = fields("Google Chrome", &["chrome", "browser"], &["web"]);
        assert_eq!(
            chrome.matched(&Query::parse("google chrome")),
            Some(MatchLevel::Exact)
        );
        assert_eq!(
            chrome.matched(&Query::parse("chrome")),
            Some(MatchLevel::Exact)
        );
        assert_eq!(
            chrome.matched(&Query::parse("google c")),
            Some(MatchLevel::WordPrefix)
        );
        assert_eq!(
            chrome.matched(&Query::parse("web")),
            Some(MatchLevel::Exact)
        );
        assert_eq!(
            chrome.matched(&Query::parse("hrome")),
            Some(MatchLevel::Substring)
        );
        assert_eq!(chrome.matched(&Query::parse("firefox")), None);
        assert_eq!(chrome.matched(&Query::parse("   ")), None);
    }

    /// Reported defect: typing `co` listed apps whose *bundle id* contains "co"
    /// while their visible name does not. Identifiers must not match substrings.
    #[test]
    fn hidden_identifiers_do_not_match_on_substrings() {
        // Identifiers arrive already reduced to their name-shaped part, so neither a
        // leading `com.` nor a trailing `.com` can match a two-letter query.
        let hiddify = fields("Hiddify", &[], &[]).with_metadata(&["hiddify"]);
        let loop_app = fields("Loop", &[], &[]).with_metadata(&["mrkai77.loop"]);
        let abdown =
            fields("ABDownloadManager", &[], &[]).with_metadata(&["abdownloadmanager.desktop"]);

        for application in [&hiddify, &loop_app, &abdown] {
            assert_eq!(
                application.matched(&Query::parse("co")),
                None,
                "a bundle id must not produce a substring hit"
            );
        }
        // A visible name still matches, including through separators and word starts.
        let chrome = fields("Google Chrome", &["chrome"], &[]);
        assert_eq!(
            chrome.matched(&Query::parse("co")),
            None,
            "'co' is not a word start"
        );
        assert_eq!(
            chrome.matched(&Query::parse("chr")),
            Some(MatchLevel::WordPrefix)
        );
        let visual = fields("Visual Studio Code", &["vscode"], &[]);
        assert_eq!(
            visual.matched(&Query::parse("co")),
            Some(MatchLevel::WordPrefix)
        );
        assert_eq!(
            visual.matched(&Query::parse("vscode")),
            Some(MatchLevel::Exact)
        );
        // The whole query covers the front of the separator-free name, so it is a
        // prefix of that name rather than a mere substring.
        assert_eq!(
            abdown.matched(&Query::parse("abdown")),
            Some(MatchLevel::Prefix),
            "a name typed without separators still matches"
        );
    }

    #[test]
    fn partial_word_prefix_matches_only_whole_words() {
        let git_status = fields("Git Status", &[], &["repository"]);
        assert_eq!(
            git_status.matched(&Query::parse("git sta")),
            Some(MatchLevel::WordPrefix)
        );
        // A token starting mid-word only matches as a substring, not a word prefix.
        assert_eq!(
            git_status.matched(&Query::parse("git tatus")),
            Some(MatchLevel::Substring)
        );
    }

    /// A non-matching token must not be silently dropped by the compact form.
    #[test]
    fn dropping_separators_keeps_every_character_that_carries_meaning() {
        let chrome = fields("Google Chrome", &["google"], &[]);
        assert_eq!(
            chrome.matched(&Query::parse("google 不存在")),
            None,
            "the unmatched Chinese token must not be discarded"
        );
        // The same query without the extra token still matches.
        assert!(chrome.matched(&Query::parse("google")).is_some());
        // Separators are still ignored, so a name can be typed without them.
        assert_eq!(
            Query::parse("visual studio code").compact(),
            "visualstudiocode"
        );
        assert_eq!(Query::parse("google 不存在").compact(), "google不存在");
    }

    #[test]
    fn every_token_must_match_somewhere() {
        let search = fields("Web Search", &[], &[]);
        // "web" is the first word of the title, so it is a word prefix, not an exact field.
        assert_eq!(
            search.matched(&Query::parse("web")),
            Some(MatchLevel::WordPrefix)
        );
        assert_eq!(search.matched(&Query::parse("web missing")), None);
        // A configurable exact match only happens against a whole field.
        assert_eq!(
            search.matched(&Query::parse("web search")),
            Some(MatchLevel::Exact)
        );
    }

    #[test]
    fn query_match_score_tiers_exceed_every_other_ranking_term() {
        assert_eq!(MatchLevel::Exact.score(true), 4000);
        assert_eq!(MatchLevel::Substring.score(true), 1000);
        assert_eq!(MatchLevel::Exact.score(false), 0);
        assert!(
            MatchLevel::Substring.score(true) > crate::core::ranking::score::NON_QUERY_TERM_CEILING
        );
    }

    #[test]
    fn search_fields_skip_empty_entries_and_are_capped_by_length_check() {
        let fields = fields("Copy Path", &["", "Copy"], &[]);
        assert_eq!(fields.names(), ["copy path", "copy"]);
        assert!(valid_search_field("a".repeat(200).as_str()));
        assert!(!valid_search_field("a".repeat(201).as_str()));
    }
}
