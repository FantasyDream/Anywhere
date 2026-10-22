//! Pinyin is a name-matching fallback, never an identity or an execution locator.
//! The open-source dictionary owns readings. A bounded state set matches those
//! readings without generating the Cartesian product of polyphonic characters.
use pinyin::ToPinyinMulti;

const MAX_TOKEN_BYTES: usize = 256;

/// Inputs use the same lowercase normalization as the caller's literal fields.
pub fn matches(name: &str, tokens: &[String]) -> bool {
    if name.is_ascii() || tokens.is_empty() {
        return false;
    }
    tokens.iter().all(|token| {
        !token.is_empty()
            && token.len() <= MAX_TOKEN_BYTES
            && (name.contains(token)
                || (token.bytes().any(|b| b.is_ascii_alphabetic())
                    && (matches_token(name, token, false) || matches_token(name, token, true))))
    })
}

fn matches_token(name: &str, token: &str, initials: bool) -> bool {
    let mut active = vec![false; token.len() + 1];
    let mut next = active.clone();
    active[0] = true;
    for ch in name.chars() {
        next.fill(false);
        next[0] = true; // A substring may start at any original character boundary.
        let mut literal = [0; 4];
        let literal = ch.encode_utf8(&mut literal);
        let readings = ch.to_pinyin_multi();
        for (offset, enabled) in active.iter().enumerate().take(token.len()) {
            if !enabled || !token.is_char_boundary(offset) {
                continue;
            }
            let remaining = &token[offset..];
            let mut advance = |spelling: &str| {
                if spelling.starts_with(remaining) {
                    return true; // The last syllable may be partially typed.
                }
                if remaining.starts_with(spelling) {
                    next[offset + spelling.len()] = true;
                }
                false
            };
            if advance(literal) {
                return true;
            }
            if let Some(readings) = readings {
                for reading in readings {
                    let spelling = reading.plain();
                    let spelling = if initials {
                        &spelling[..spelling.chars().next().map_or(0, char::len_utf8)]
                    } else {
                        spelling
                    };
                    if advance(spelling)
                        || (spelling.contains('ü') && advance(&spelling.replace('ü', "v")))
                    {
                        return true;
                    }
                }
            } else if !ch.is_alphanumeric() {
                next[offset] = true; // Match separator-free names, retaining all letters.
            }
        }
        std::mem::swap(&mut active, &mut next);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn find(name: &str, query: &str) -> bool {
        matches(
            &name.to_lowercase(),
            &query
                .to_lowercase()
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
    }
    #[test]
    fn full_initials_partial_and_mixed_names() {
        for query in ["weixin", "wx", "weix", "xin", "wei 信"] {
            assert!(find("微信", query), "{query}");
        }
        assert!(find("QQ音乐2026", "qqyinyue2026"));
        assert!(find("QQ音乐2026", "qqyy2026"));
        assert!(find("开发文档.md", "kaifawendang.md"));
        assert!(find("开发文档.md", "kfwd"));
        assert!(find("微信开发工具", "weixin gongju"));
        assert!(find("女绿", "nvlv"));
        assert!(find("女绿", "nl"));
        // Dictionary readings may begin with non-ASCII ê; initials are UTF-8 safe.
        assert!(!find("欸", "zz"));
    }
    #[test]
    fn polyphonic_readings_do_not_expand_exponentially() {
        assert!(find("重庆银行", "chongqingyinhang"));
        assert!(find("重庆银行", "cqyh"));
        assert!(find(&"重行长乐".repeat(100), "chonghangzhangyue"));
    }
    #[test]
    fn missing_tokens_and_unknown_characters_are_not_dropped() {
        assert!(!find("微信", "weixin missing"));
        assert!(!find("微信", "wx不存在"));
        assert!(!find("微龘信", "weixin"));
        assert!(!find("微信", ""));
        assert!(!find("微信", &"x".repeat(257)));
        assert!(!find("Google Chrome", "google不存在"));
    }
}
