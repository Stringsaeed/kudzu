use std::collections::BTreeSet;

#[derive(Default)]
pub struct Selection {
    pub ids: BTreeSet<usize>,
    anchor: Option<usize>,
    pub cursor: Option<usize>,
}
impl Selection {
    pub fn select(&mut self, id: usize, visible: &[usize], shift: bool, command: bool) {
        self.cursor = Some(id);
        if shift {
            let a = self
                .anchor
                .and_then(|a| visible.iter().position(|&v| v == a));
            let b = visible.iter().position(|&v| v == id);
            if let (Some(a), Some(b)) = (a, b) {
                self.ids = visible[a.min(b)..=a.max(b)].iter().copied().collect();
            }
        } else if command {
            if !self.ids.remove(&id) {
                self.ids.insert(id);
            }
            self.anchor = Some(id);
        } else {
            self.ids = [id].into();
            self.anchor = Some(id);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_uses_visible_order_and_command_toggles() {
        let mut s = Selection::default();
        let ids = [8, 4, 9, 2];
        s.select(4, &ids, false, false);
        s.select(2, &ids, true, false);
        assert_eq!(s.ids, [2, 4, 9].into());
        s.select(4, &ids, false, true);
        assert_eq!(s.ids, [2, 9].into());
    }
}
