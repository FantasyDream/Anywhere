//! R3 experiment only; no alternate engine, settings, threads or production API.
use super::query::{MatchLevel, Query, SearchFields};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Native,
    Substring,
    Fuzzy,
}

// Preserve codepoints literally. Utf32Str::new's grapheme preprocessing would
// collapse combining marks, changing the existing UTF-8 substring contract.
enum Text {
    Ascii(Vec<u8>),
    Unicode(Vec<char>),
}
impl Text {
    fn new(text: &str) -> Self {
        if text.is_ascii() {
            Self::Ascii(text.as_bytes().to_vec())
        } else {
            Self::Unicode(text.chars().collect())
        }
    }
    fn as_ref(&self) -> Utf32Str<'_> {
        match self {
            Self::Ascii(s) => Utf32Str::Ascii(s),
            Self::Unicode(s) => Utf32Str::Unicode(s),
        }
    }
}

struct Name<'a> {
    text: &'a str,
    compact: String,
    encoded: Option<Text>,
}
struct Prepared<'a> {
    names: Vec<Name<'a>>,
    fields: &'a SearchFields,
}
impl<'a> Prepared<'a> {
    fn new(fields: &'a SearchFields, mode: Mode) -> Self {
        Self {
            fields,
            names: fields
                .names()
                .iter()
                .map(|text| Name {
                    text,
                    compact: text.chars().filter(|c| c.is_ascii_alphanumeric()).collect(),
                    encoded: (mode == Mode::Fuzzy || (mode == Mode::Substring && text.is_ascii()))
                        .then(|| Text::new(text)),
                })
                .collect(),
        }
    }

    fn matched(
        &self,
        query: &Query,
        tokens: &[Text],
        matcher: &mut Option<Matcher>,
        mode: Mode,
    ) -> Option<MatchLevel> {
        if query.is_empty() {
            return None;
        }
        let names = self.names.iter().filter_map(|name| {
            if name.text == query.normalized_query {
                return Some(MatchLevel::Exact);
            }
            if query.tokens.iter().all(|token| {
                name.text
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|word| !word.is_empty() && word.starts_with(token.as_str()))
            }) {
                return Some(MatchLevel::WordPrefix);
            }
            if !name.text.contains(' ') && name.text.starts_with(query.normalized_query.as_str()) {
                return Some(MatchLevel::Prefix);
            }
            if !query.compact().is_empty() && name.compact.starts_with(query.compact()) {
                return Some(MatchLevel::Substring);
            }
            let found = query.tokens.iter().enumerate().all(|(i, token)| {
                // Pinned 0.3.1's Unicode substring path misses valid suffixes;
                // the regression below records it. Never change Unicode membership.
                if mode == Mode::Native || !name.text.is_ascii() || !token.is_ascii() {
                    name.text.contains(token.as_str())
                } else {
                    matcher
                        .as_mut()
                        .unwrap()
                        .substring_match(
                            name.encoded.as_ref().unwrap().as_ref(),
                            tokens[i].as_ref(),
                        )
                        .is_some()
                }
            });
            found.then_some(MatchLevel::Substring)
        });
        let metadata = self.fields.metadata().iter().filter_map(|field| {
            if field == &query.normalized_query {
                Some(MatchLevel::Exact)
            } else {
                field
                    .starts_with(query.normalized_query.as_str())
                    .then_some(MatchLevel::Prefix)
            }
        });
        let strict = names.chain(metadata).max();
        if strict.is_some() || mode != Mode::Fuzzy {
            return strict;
        }
        self.names
            .iter()
            .any(|name| {
                tokens.iter().all(|token| {
                    matcher
                        .as_mut()
                        .unwrap()
                        .fuzzy_match(name.encoded.as_ref().unwrap().as_ref(), token.as_ref())
                        .is_some()
                })
            })
            .then_some(MatchLevel::Substring)
    }
}

fn matcher() -> Option<Matcher> {
    let mut config = Config::DEFAULT;
    config.ignore_case = false;
    config.normalize = false;
    Some(Matcher::new(config))
}

fn tokens(query: &Query, mode: Mode) -> Vec<Text> {
    if mode == Mode::Native {
        Vec::new()
    } else {
        query.tokens.iter().map(|token| Text::new(token)).collect()
    }
}

mod benchmark;
// Frozen 4198a40 field matcher for differential tests and the allocation baseline.
// Test-only: production has exactly one matcher, not a configurable backend.
fn legacy_matched(fields: &SearchFields, query: &Query) -> Option<MatchLevel> {
    if query.is_empty() {
        return None;
    }
    let names = fields.names().iter().filter_map(|field| {
        if field == &query.normalized_query {
            return Some(MatchLevel::Exact);
        }
        if query.tokens.iter().all(|token| {
            field
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| !word.is_empty() && word.starts_with(token.as_str()))
        }) {
            return Some(MatchLevel::WordPrefix);
        }
        if !field.contains(' ') && field.starts_with(query.normalized_query.as_str()) {
            return Some(MatchLevel::Prefix);
        }
        let compact: String = field
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        if (!query.compact().is_empty() && compact.starts_with(query.compact()))
            || query
                .tokens
                .iter()
                .all(|token| field.contains(token.as_str()))
        {
            return Some(MatchLevel::Substring);
        }
        None
    });
    let metadata = fields.metadata().iter().filter_map(|field| {
        if field == &query.normalized_query {
            Some(MatchLevel::Exact)
        } else {
            field
                .starts_with(query.normalized_query.as_str())
                .then_some(MatchLevel::Prefix)
        }
    });
    names.chain(metadata).max()
}

#[cfg(test)]
mod tests;
