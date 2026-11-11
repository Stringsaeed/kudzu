//! Contiguous summary matches indexed by their first two UTF-16 units.
use super::fuzzy::{find, is_boundary};

#[derive(Debug, Clone)]
pub(super) struct Content {
    text: Vec<u16>,
    starts: Option<PairStarts>,
}

/// Dense Latin pairs make the common lookup one load. Other scripts use a compact sorted
/// table. Both remember only the first occurrence; full UTF-16 comparison decides the match.
#[derive(Debug, Clone)]
struct PairStarts {
    latin: Box<[u16; 26 * 26]>,
    other: Vec<(u32, u16)>,
}
impl PairStarts {
    fn new(text: &[u16]) -> Self {
        let mut latin = Box::new([u16::MAX; 26 * 26]);
        let mut other = Vec::new();
        for at in 0..text.len().saturating_sub(1) {
            let key = pair(text, at);
            if let Some(index) = latin_pair(key) {
                if latin[index] == u16::MAX {
                    latin[index] = at as u16;
                }
            } else {
                other.push((key, at as u16));
            }
        }
        other.sort_unstable();
        other.dedup_by_key(|entry| entry.0);
        other.shrink_to_fit();
        Self { latin, other }
    }
    fn first(&self, key: u32) -> Option<usize> {
        if let Some(index) = latin_pair(key) {
            let at = self.latin[index];
            (at != u16::MAX).then_some(usize::from(at))
        } else {
            self.other
                .binary_search_by_key(&key, |p| p.0)
                .ok()
                .map(|i| usize::from(self.other[i].1))
        }
    }
}
fn latin_pair(key: u32) -> Option<usize> {
    let a = key >> 16;
    let b = key & 0xffff;
    ((97..=122).contains(&a) && (97..=122).contains(&b)).then(|| ((a - 97) * 26 + b - 97) as usize)
}
impl Content {
    pub fn new(text: Vec<u16>) -> Self {
        let starts = u16::try_from(text.len())
            .ok()
            .map(|_| PairStarts::new(&text));
        Self { text, starts }
    }

    pub fn match_boundary(&self, term: &[u16]) -> Option<bool> {
        self.find(term).map(|at| is_boundary(&self.text, at))
    }

    fn find(&self, term: &[u16]) -> Option<usize> {
        let Some(starts) = self.starts.as_ref().filter(|_| term.len() >= 2) else {
            return find(&self.text, term);
        };
        let at = starts.first(pair(term, 0))?;
        if self.text[at..].starts_with(term) {
            return Some(at);
        }
        if term
            .windows(2)
            .skip(1)
            .any(|p| starts.first(pair(p, 0)).is_none())
        {
            return None;
        }
        find(&self.text[at + 1..], term).map(|found| at + 1 + found)
    }
}
fn pair(text: &[u16], at: usize) -> u32 {
    (u32::from(text[at]) << 16) | u32::from(text[at + 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_substrings_keep_the_first_utf16_match() {
        for text in [
            "abababba ab café 😀 العربية",
            "é e\u{301} 👩\u{200d}💻 \r\n",
            "",
            "x",
        ] {
            let text: Vec<u16> = text.encode_utf16().collect();
            let index = Content::new(text.clone());
            for start in 0..text.len() {
                for end in start + 1..=text.len() {
                    let needle = &text[start..end];
                    assert_eq!(index.find(needle), find(&text, needle));
                }
            }
            assert_eq!(index.find(&[0, 1, 2]), None);
        }
        let long = Content::new(vec![65; 70_000]);
        assert_eq!(long.find(&[65, 65]), Some(0));
    }
}
