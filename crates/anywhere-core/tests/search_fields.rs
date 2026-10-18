//! Product contracts retained after the discarded matcher experiments were removed.
use anywhere_core::core::search::query::{MatchLevel, Query, SearchFields};

#[test]
fn visible_names_aliases_and_identifiers_keep_their_distinct_matching_rules() {
    let chrome = SearchFields::new("Google Chrome", &["chrome".into()], &[])
        .with_metadata(&["google.chrome"]);
    for (raw, expected) in [
        ("google chrome", Some(MatchLevel::Exact)),
        ("chrome", Some(MatchLevel::Exact)),
        ("google c", Some(MatchLevel::WordPrefix)),
        ("hrome", Some(MatchLevel::Substring)),
        ("googlechrome", Some(MatchLevel::Substring)),
        ("co", None),
        ("gchr", None),
        ("google 不存在", None),
        ("不存在 google", None),
        ("google.chrome", Some(MatchLevel::Exact)),
    ] {
        assert_eq!(chrome.matched(&Query::parse(raw)), expected, "{raw}");
    }
    let fields = SearchFields::new(
        "Unrelated Tool",
        &["Git Status".into(), "Branch Name".into()],
        &["vendor.workspace".into()],
    );
    for raw in ["workspace", "vwrk", "git branch", "git 不存在", ""] {
        assert_eq!(fields.matched(&Query::parse(raw)), None, "{raw}");
    }
    assert_eq!(
        fields.matched(&Query::parse("vendor.")),
        Some(MatchLevel::Prefix)
    );
}

#[test]
fn unicode_substrings_remain_literal_including_combining_marks_and_emoji() {
    for title in [
        "Cafe\u{301}",
        "Café",
        "İstanbul",
        "Straße",
        "👩\u{200d}💻工具",
        "a\r\nb",
        "a^$!'b",
    ] {
        let fields = SearchFields::new(title, &[], &[]);
        let normalized: Vec<_> = title.to_lowercase().chars().collect();
        for start in 0..normalized.len() {
            for end in start + 1..=normalized.len() {
                let raw: String = normalized[start..end].iter().collect();
                let query = Query::parse(&raw);
                assert_eq!(
                    fields.matched(&query).is_some(),
                    !query.is_empty(),
                    "{title:?}, {raw:?}"
                );
            }
        }
    }
    // Matching lowercases but does not invent transliteration or normalize accents.
    assert_eq!(
        SearchFields::new("Café", &[], &[]).matched(&Query::parse("cafe\u{301}")),
        None
    );
    assert_eq!(
        SearchFields::new("Straße", &[], &[]).matched(&Query::parse("strasse")),
        None
    );
}

#[test]
fn registered_actions_keep_exact_matches_for_full_visible_names_and_aliases() {
    let registry = anywhere_core::application::builtin_registry();
    let mut checked = 0;
    for command in registry.commands() {
        for name in command.search().names() {
            if name.trim().is_empty() || name.trim() != name {
                continue;
            }
            assert_eq!(
                command.search().matched(&Query::parse(name)),
                Some(MatchLevel::Exact),
                "{}: {name}",
                command.id
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "the registered action catalog must be exercised"
    );
}
