use std::cmp::Ordering;

use strsim::normalized_damerau_levenshtein;

// Thanks ai
fn partial_ratio(query: &str, target: &str) -> f64 {
    let qc: Vec<char> = query.chars().collect();
    let tc: Vec<char> = target.chars().collect();

    if qc.is_empty() || tc.is_empty() {
        return 0.0;
    }

    if tc.len() <= qc.len() {
        return normalized_damerau_levenshtein(query, target);
    }

    (0..=(tc.len() - qc.len()))
        .map(|i| {
            let window: String = tc[i..i + qc.len()].iter().collect();
            normalized_damerau_levenshtein(query, &window)
        })
        .fold(0.0_f64, f64::max)
}

fn combined_score(query: &str, target: &str) -> f64 {
    let full = normalized_damerau_levenshtein(query, target);
    let partial = partial_ratio(query, target);
    let contains = if target.contains(query) { 0.05 } else { 0.0 };
    // Weight partial heavily so substrings surface, but full match still wins
    f64::max(full, partial * 0.95) + contains
}

pub fn search<T>(
    query: &str,
    possible: impl Iterator<Item = (String, T)>,
    match_count: usize,
) -> Vec<(f64, T)> {
    let mut c: Vec<(f64, T)> = possible
        .map(|(m, id)| {
            let c = combined_score(
                query.to_lowercase().as_str(),
                m.to_lowercase()
                    .chars()
                    .filter(|c| !c.is_ascii_punctuation())
                    .collect::<String>()
                    .as_str(),
            );
            (c, id)
        })
        .collect();
    c.sort_unstable_by(|this, that| that.0.total_cmp(&this.0));
    c.into_iter()
        .take(match_count)
        .filter(|(s, _)| matches!(s.total_cmp(&0.25), Ordering::Greater))
        .rev()
        .collect()
}
