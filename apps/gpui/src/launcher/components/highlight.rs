use gpui_kit::{
    AnyElement, FontWeight, HighlightStyle, Hsla, IntoElement, SharedString, StyledText,
};
use std::ops::Range;

/// Compute UTF-8 byte ranges in `text` that match `query` tokens or its
/// separator-insensitive compact prefix (e.g. `abdown` -> `ABDownloadManager`).
pub fn compute_match_highlights(text: &str, query: &str) -> Vec<Range<usize>> {
    let trimmed = query.trim();
    if text.is_empty() || trimmed.is_empty() {
        return Vec::new();
    }
    let chars: Vec<(usize, usize, char)> = text
        .char_indices()
        .map(|(start, ch)| (start, start + ch.len_utf8(), ch))
        .collect();
    let mut lowered = String::with_capacity(text.len());
    let mut byte_map: Vec<(usize, usize)> = Vec::with_capacity(text.len());
    for &(orig_start, orig_end, ch) in &chars {
        for lower in ch.to_lowercase() {
            let len = lower.len_utf8();
            lowered.push(lower);
            for _ in 0..len {
                byte_map.push((orig_start, orig_end));
            }
        }
    }
    let query_lower = trimmed.to_lowercase();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut matched_any = false;
    for token in query_lower.split_whitespace().filter(|t| !t.is_empty()) {
        if let Some(span) = find_token_span(&lowered, token, &byte_map) {
            ranges.push(span);
            matched_any = true;
        }
    }
    if !matched_any {
        let compact: Vec<char> = query_lower
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        if !compact.is_empty() {
            if let Some(compact_ranges) = find_compact_prefix_spans(&chars, &compact) {
                ranges.extend(compact_ranges);
            }
        }
    }
    merge_ranges(ranges)
}

/// Render `text` with semibold highlights for characters matching `query`.
pub fn highlighted_text(text: SharedString, query: &str, highlight_color: Hsla) -> AnyElement {
    let ranges = compute_match_highlights(text.as_ref(), query);
    if ranges.is_empty() {
        return text.into_any_element();
    }
    let style = HighlightStyle {
        color: Some(highlight_color),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(text)
        .with_highlights(ranges.into_iter().map(move |range| (range, style)))
        .into_any_element()
}

fn find_token_span(
    lowered: &str,
    token: &str,
    byte_map: &[(usize, usize)],
) -> Option<Range<usize>> {
    let mut first_substring = None;
    let mut search_from = 0;
    while let Some(offset) = lowered[search_from..].find(token) {
        let start = search_from + offset;
        let end = start + token.len();
        let is_word_start = start == 0
            || lowered[..start]
                .chars()
                .next_back()
                .is_some_and(|prev| !prev.is_alphanumeric());
        let mapped = map_lowered_range(start, end, byte_map)?;
        if is_word_start {
            return Some(mapped);
        }
        if first_substring.is_none() {
            first_substring = Some(mapped);
        }
        let step = lowered[start..].chars().next().map_or(1, char::len_utf8);
        search_from = start + step;
    }
    first_substring
}

fn map_lowered_range(
    start: usize,
    end: usize,
    byte_map: &[(usize, usize)],
) -> Option<Range<usize>> {
    if start >= end || end > byte_map.len() {
        return None;
    }
    let orig_start = byte_map[start].0;
    let orig_end = byte_map[end - 1].1;
    (orig_start < orig_end).then_some(orig_start..orig_end)
}

fn find_compact_prefix_spans(
    chars: &[(usize, usize, char)],
    compact_query: &[char],
) -> Option<Vec<Range<usize>>> {
    let mut matched = Vec::with_capacity(compact_query.len());
    let mut query_idx = 0;
    for &(start, end, ch) in chars {
        if !ch.is_alphanumeric() {
            continue;
        }
        let mut lower_iter = ch.to_lowercase();
        let Some(first_lower) = lower_iter.next() else {
            continue;
        };
        if lower_iter.next().is_some() || first_lower != compact_query[query_idx] {
            return None;
        }
        matched.push(start..end);
        query_idx += 1;
        if query_idx == compact_query.len() {
            return Some(matched);
        }
    }
    None
}

fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if ranges.len() <= 1 {
        return ranges;
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end {
                last.end = last.end.max(range.end);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_word_prefixes_and_merges_overlapping_tokens() {
        let title = "Git Status";
        let spans = compute_match_highlights(title, "git sta");
        assert_eq!(spans, vec![0..3, 4..7]);
        assert_eq!(&title[spans[0].clone()], "Git");
        assert_eq!(&title[spans[1].clone()], "Sta");
    }

    #[test]
    fn highlights_compact_prefix_across_separators() {
        let title = "AB-Download_Manager";
        let spans = compute_match_highlights(title, "abdown");
        assert_eq!(spans, vec![0..2, 3..7]);
        assert_eq!(&title[spans[0].clone()], "AB");
        assert_eq!(&title[spans[1].clone()], "Down");
    }

    #[test]
    fn highlights_unicode_and_chinese_on_valid_utf8_boundaries() {
        let title = "微信开发工具 🚀 Notes";
        let spans = compute_match_highlights(title, "开发 notes");
        assert_eq!(spans.len(), 2);
        assert_eq!(&title[spans[0].clone()], "开发");
        assert_eq!(&title[spans[1].clone()], "Notes");
    }
}
