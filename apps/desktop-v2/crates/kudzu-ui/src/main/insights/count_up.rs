//! Arrival count-up: only the leading number moves, in the final value's unit and precision.
//!
//! Matches `useCountUp` in `components/app/main/insights.tsx`: 560 ms, quartic
//! `1 - (1 - p) ** 4`, parsing `/^(\d+(?:\.(\d+))?)(.*)$/` by hand (no regex crate).

pub const DURATION: f64 = 0.560;

/// The string shown at `progress` in `0..=1` toward `value` (`"471"`, `"27k"`, `"2.3 h"`).
pub fn interpolate(value: &str, progress: f64) -> String {
    if progress >= 1.0 {
        return value.to_owned();
    }
    let Some((number, suffix, digits)) = split_numeric_prefix(value) else {
        return value.to_owned();
    };
    let t = progress.clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t).powi(4);
    format!(
        "{}{suffix}",
        kudzu_core::fmt::to_fixed(number * eased, digits)
    )
}

/// Leading number, trailing unit, and the number of digits after the decimal in the source.
fn split_numeric_prefix(value: &str) -> Option<(f64, &str, usize)> {
    let bytes = value.as_bytes();
    if bytes.first().is_none_or(|b| !b.is_ascii_digit()) {
        return None;
    }
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        let frac = i + 1;
        if frac < bytes.len() && bytes[frac].is_ascii_digit() {
            i = frac;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    let head = &value[..i];
    let digits = head.find('.').map(|dot| head.len() - dot - 1).unwrap_or(0);
    let number = head.parse().ok()?;
    Some((number, &value[i..], digits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_progress_is_the_formatted_string() {
        assert_eq!(interpolate("471", 1.0), "471");
        assert_eq!(interpolate("27k", 1.0), "27k");
        assert_eq!(interpolate("2.3 h", 1.0), "2.3 h");
    }

    #[test]
    fn only_the_leading_number_moves() {
        assert_eq!(interpolate("471", 0.0), "0");
        assert_eq!(interpolate("27k", 0.0), "0k");
        assert_eq!(interpolate("2.3 h", 0.0), "0.0 h");
        let half = interpolate("471", 0.5);
        assert_eq!(half, "442");
        assert_eq!(interpolate("2.3 h", 0.5), "2.2 h");
    }

    #[test]
    fn a_non_numeric_string_stays_put() {
        assert_eq!(interpolate("Ready", 0.3), "Ready");
    }
}
