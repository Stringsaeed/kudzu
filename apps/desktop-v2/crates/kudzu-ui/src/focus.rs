//! Which pane of a window has the keyboard. RUNE-GAP(#173): Rune has no `Tx::focus(key)` for
//! arbitrary elements, so `on_key` sends keys to the pane this model marks active and the pane
//! draws its own ring. Text fields still take focus through `TextEdit::focused`.

/// The active pane among a window's panes, in Tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus<P: Copy + Eq + 'static> {
    order: &'static [P],
    active: P,
}

impl<P: Copy + Eq + 'static> Focus<P> {
    /// Starts on the first pane. `order` must not be empty.
    pub fn new(order: &'static [P]) -> Self {
        Self {
            order,
            active: order[0],
        }
    }

    pub fn active(&self) -> P {
        self.active
    }

    pub fn is_active(&self, pane: P) -> bool {
        self.active == pane
    }

    /// ⌘L, ⌘E and friends. A pane the window does not have is ignored.
    pub fn move_to(&mut self, pane: P) {
        if self.order.contains(&pane) {
            self.active = pane;
        }
    }

    /// Tab: the next pane, wrapping.
    pub fn next(&mut self) {
        self.step(1);
    }

    /// Shift-Tab: the previous pane, wrapping.
    pub fn previous(&mut self) {
        self.step(self.order.len() - 1);
    }

    fn step(&mut self, by: usize) {
        let at = self
            .order
            .iter()
            .position(|pane| *pane == self.active)
            .unwrap_or(0);
        self.active = self.order[(at + by) % self.order.len()];
    }
}

#[cfg(test)]
mod tests {
    use super::Focus;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Pane {
        Sidebar,
        List,
        Editor,
        Elsewhere,
    }

    const ORDER: &[Pane] = &[Pane::Sidebar, Pane::List, Pane::Editor];

    #[test]
    fn one_pane_is_active_and_tab_wraps_both_ways() {
        let mut focus = Focus::new(ORDER);
        assert!(focus.is_active(Pane::Sidebar));
        focus.next();
        assert_eq!(focus.active(), Pane::List);
        focus.move_to(Pane::Editor);
        focus.next();
        assert_eq!(focus.active(), Pane::Sidebar);
        focus.previous();
        assert_eq!(focus.active(), Pane::Editor);
        focus.move_to(Pane::Elsewhere);
        assert_eq!(focus.active(), Pane::Editor, "an unknown pane is ignored");
    }
}
