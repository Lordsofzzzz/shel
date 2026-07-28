use nucleo::{Matcher, Utf32Str};

use crate::models::Entry;

/// Fuzzy-match entries against query, return (index, score) sorted desc by score.
/// Empty query returns all entries with score 0 (original order).
pub fn fuzzy_search(entries: &[Entry], query: &str) -> Vec<(usize, u16)> {
    if query.is_empty() {
        return entries.iter().enumerate().map(|(i, _)| (i, 0)).collect();
    }

    let needle_lower: Vec<u8> = query.to_lowercase().into_bytes();
    let needle = Utf32Str::Ascii(&needle_lower);
    let mut matcher = Matcher::default();

    let mut results: Vec<(usize, u16)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let cmd_lower = e.command.to_lowercase();
            let haystack = Utf32Str::Ascii(cmd_lower.as_bytes());
            matcher.fuzzy_match(haystack, needle).map(|s| (i, s))
        })
        .collect();

    results.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| entries[b.0].timestamp.cmp(&entries[a.0].timestamp))
    });
    results
}
