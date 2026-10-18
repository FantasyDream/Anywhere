use super::*;
use crate::core::ranking::score::{score, UsageStats};
use anywhere_protocol::ContextSource;

#[cfg(test)]
fn fields(title: &str, aliases: &[&str], metadata: &[&str]) -> SearchFields {
    SearchFields::new(
        title,
        &aliases.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &metadata.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
}

#[test]
fn compatible_backends_preserve_levels_on_curated_and_registered_actions() {
    let mut fields = vec![
        fields("Google Chrome", &["chrome", "browser"], &["google.chrome"]),
        fields("Visual Studio Code", &["vscode"], &["microsoft.vscode"]),
        fields("AB Download Manager", &[], &[]),
        fields("Git Status", &["Branch Name"], &["repository"]),
        fields("复制 分支 名称", &[], &[]),
        fields("Café", &["Cafe\u{301}"], &[]),
        fields("İstanbul Straße 😀 工具", &[], &[]),
        fields("node.js ^Open $Close !Restore", &[], &[]),
    ];
    fields.extend(
        crate::application::builtin_registry()
            .commands()
            .map(|c| c.search().clone()),
    );
    let queries = [
        "",
        "  ",
        "google chrome",
        "chr",
        "hrome",
        "co",
        "vscode",
        "vsco",
        "vsc",
        "code",
        "abdown",
        "git sta",
        "git branch",
        "git missing",
        "google 不存在",
        "复制 名称",
        "café",
        "cafe\u{301}",
        "istanbul",
        "i\u{307}",
        "straße",
        "strasse",
        "😀 工具",
        "^open",
        "!restore",
        "node.js",
        "^",
        "$",
        "!",
        "git    sta",
    ];
    let mut matcher = matcher();
    for field in &fields {
        for mode in [Mode::Native, Mode::Substring] {
            let prepared = Prepared::new(field, mode);
            for raw in queries {
                let query = Query::parse(raw);
                assert_eq!(field.matched(&query), legacy_matched(field, &query));
                assert_eq!(
                    prepared.matched(&query, &tokens(&query, mode), &mut matcher, mode),
                    field.matched(&query),
                    "{mode:?}: {:?}, {raw:?}",
                    field.names()
                );
            }
        }
    }
}

#[test]
fn pinned_raw_unicode_substring_mismatch_requires_native_fallback() {
    let haystack = Text::new("Cafe\u{301}");
    let needle = Text::new("afe\u{301}");
    assert!("Cafe\u{301}".contains("afe\u{301}"));
    assert!(matcher()
        .as_mut()
        .unwrap()
        .substring_match(haystack.as_ref(), needle.as_ref())
        .is_none());
}

#[test]
fn unicode_substrings_and_literal_punctuation_do_not_gain_normalization_or_dsl() {
    let mut matcher = matcher();
    for title in [
        "Cafe\u{301}",
        "Café",
        "İstanbul",
        "Straße",
        "👩\u{200d}💻工具",
        "a\r\nb",
        "a^$!'b",
    ] {
        let fields = fields(title, &[], &[]);
        let prepared = Prepared::new(&fields, Mode::Substring);
        // Every contiguous Unicode substring, including an isolated combining mark.
        let chars: Vec<char> = title.chars().collect();
        for a in 0..chars.len() {
            for b in a + 1..=chars.len() {
                let raw: String = chars[a..b].iter().collect();
                let query = Query::parse(&raw);
                assert_eq!(fields.matched(&query), legacy_matched(&fields, &query));
                assert_eq!(
                    prepared.matched(
                        &query,
                        &tokens(&query, Mode::Substring),
                        &mut matcher,
                        Mode::Substring
                    ),
                    fields.matched(&query),
                    "{title:?}, {raw:?}"
                );
            }
        }
    }
}

#[test]
fn hidden_metadata_and_separate_aliases_cannot_satisfy_missing_tokens() {
    let fields = fields(
        "Unrelated Tool",
        &["Git Status", "Branch Name"],
        &["vendor.workspace"],
    );
    let mut matcher = matcher();
    for mode in [Mode::Native, Mode::Substring, Mode::Fuzzy] {
        let prepared = Prepared::new(&fields, mode);
        for raw in ["workspace", "vwrk", "git branch", "git 不存在"] {
            let query = Query::parse(raw);
            assert_eq!(
                prepared.matched(&query, &tokens(&query, mode), &mut matcher, mode),
                None,
                "{mode:?} {raw}"
            );
        }
    }
}

#[test]
fn fuzzy_expansion_exposes_both_useful_abbreviations_and_short_query_noise() {
    let fields = fields("Google Chrome", &[], &[]);
    let prepared = Prepared::new(&fields, Mode::Fuzzy);
    let mut matcher = matcher();
    for raw in ["gchr", "co"] {
        let query = Query::parse(raw);
        assert_eq!(fields.matched(&query), None);
        assert_eq!(
            prepared.matched(
                &query,
                &tokens(&query, Mode::Fuzzy),
                &mut matcher,
                Mode::Fuzzy
            ),
            Some(MatchLevel::Substring)
        );
    }
    // R3 records the behavior difference; it does not silently approve this product rule.
}

#[test]
fn tiers_and_frecency_remain_owned_by_the_existing_ranker() {
    let make_score = |level, count| {
        score(
            level,
            true,
            ContextSource::ApplicationContext,
            90,
            10,
            UsageStats {
                execute_count: count,
                last_executed_at: 1000,
            },
            1000,
        )
    };
    let fields = fields("Google Chrome", &["chrome"], &[]);
    let mut matcher = matcher();
    let prepared = Prepared::new(&fields, Mode::Substring);
    for raw in ["google chrome", "chr", "hrome"] {
        let query = Query::parse(raw);
        let level = prepared.matched(
            &query,
            &tokens(&query, Mode::Substring),
            &mut matcher,
            Mode::Substring,
        );
        for count in [0, 1, 20, u32::MAX] {
            assert_eq!(
                make_score(level, count),
                make_score(fields.matched(&query), count)
            );
        }
    }
    assert!(
        make_score(Some(MatchLevel::Exact), 0) > make_score(Some(MatchLevel::Substring), u32::MAX)
    );
    assert!(
        make_score(Some(MatchLevel::WordPrefix), 8) > make_score(Some(MatchLevel::WordPrefix), 1)
    );
}

#[test]
fn long_missing_queries_and_empty_fields_are_rejected() {
    let mut matcher = matcher();
    for fields in [SearchFields::default(), fields("Short Name", &[], &[])] {
        for mode in [Mode::Native, Mode::Substring, Mode::Fuzzy] {
            let prepared = Prepared::new(&fields, mode);
            for raw in ["".to_string(), "x".repeat(1025), "不存在".repeat(100)] {
                let query = Query::parse(&raw);
                assert_eq!(
                    prepared.matched(&query, &tokens(&query, mode), &mut matcher, mode),
                    None
                );
            }
        }
    }
}
