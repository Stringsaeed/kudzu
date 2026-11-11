//! Nearest-rank percentiles for the latency batches.

#[derive(Clone, Debug, PartialEq)]
pub struct Spread {
    pub n: usize,
    pub min: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}

pub fn spread(values: &[f64]) -> Option<Spread> {
    if values.is_empty() || values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    Some(Spread {
        n: sorted.len(),
        min: sorted[0],
        median: median(&sorted),
        p95: nearest_rank(&sorted, 0.95),
        max: *sorted.last().expect("nonempty"),
    })
}

fn median(sorted: &[f64]) -> f64 {
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Nearest rank: index `ceil(p * n) - 1` into a sorted sample.
pub fn nearest_rank(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let rank = (p * n as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(n - 1)]
}

pub fn median_of(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.retain(|v| v.is_finite());
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    Some(median(&sorted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p95_of_twenty_is_the_nineteenth_sorted_sample() {
        let values: Vec<f64> = (1..=20).map(|n| n as f64).collect();
        let spread = spread(&values).expect("spread");
        assert_eq!(spread.min, 1.0);
        assert_eq!(spread.max, 20.0);
        assert_eq!(spread.p95, 19.0);
        assert_eq!(spread.median, 10.5);
    }
}
