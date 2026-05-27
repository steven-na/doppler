use strsim::normalized_damerau_levenshtein;

pub fn search<T>(
    query: &str,
    possible: impl Iterator<Item = (String, T)>,
    match_count: usize,
) -> Vec<(f64, T)> {
    let mut c: Vec<(f64, T)> = possible
        .map(|(m, id)| {
            let c = normalized_damerau_levenshtein(query, &m);
            (c, id)
        })
        .collect();
    c.sort_unstable_by(|this, that| that.0.total_cmp(&this.0));
    c.into_iter().take(match_count).rev().collect()
}
