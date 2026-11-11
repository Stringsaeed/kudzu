//! FIFO of physical input withheld while an insertion (or its replay) is in flight.

use std::collections::VecDeque;

/// Whether incoming input should be withheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HoldState {
    Idle,
    Inserting,
}

/// Holds keystrokes typed during an insertion so they land after it, in order.
#[derive(Debug)]
pub(crate) struct HoldReplayQueue<T> {
    state: HoldState,
    elements: VecDeque<T>,
}

impl<T> Default for HoldReplayQueue<T> {
    fn default() -> Self {
        Self {
            state: HoldState::Idle,
            elements: VecDeque::new(),
        }
    }
}

impl<T> HoldReplayQueue<T> {
    /// Current hold state.
    pub fn state(&self) -> HoldState {
        self.state
    }

    /// Starts holding, preserving any input still waiting for replay.
    pub fn begin(&mut self) {
        self.state = HoldState::Inserting;
    }

    /// Accepts `element` while holding. Returns it back when idle (it must pass through).
    pub fn enqueue(&mut self, element: T) -> Result<(), T> {
        if self.state != HoldState::Inserting {
            return Err(element);
        }
        self.elements.push_back(element);
        Ok(())
    }

    /// Takes one element to replay; stays holding until a read finds the queue empty.
    pub fn next(&mut self) -> Option<T> {
        let next = self.elements.pop_front();
        if next.is_none() {
            self.state = HoldState::Idle;
        }
        next
    }

    /// Releases everything in order and returns to idle.
    pub fn drain(&mut self) -> Vec<T> {
        self.state = HoldState::Idle;
        self.elements.drain(..).collect()
    }

    /// Number of held elements.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.elements.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_queue_passes_input_through() {
        let mut q = HoldReplayQueue::default();
        assert_eq!(q.enqueue(1), Err(1));
        assert_eq!(q.state(), HoldState::Idle);
    }

    #[test]
    fn holds_and_replays_in_order_then_goes_idle() {
        let mut q = HoldReplayQueue::default();
        q.begin();
        assert!(q.enqueue(1).is_ok());
        assert!(q.enqueue(2).is_ok());
        assert_eq!(q.next(), Some(1));
        // Still holding: a key typed during replay queues behind the rest.
        assert!(q.enqueue(3).is_ok());
        assert_eq!(q.next(), Some(2));
        assert_eq!(q.next(), Some(3));
        assert_eq!(q.state(), HoldState::Inserting);
        assert_eq!(q.next(), None);
        assert_eq!(q.state(), HoldState::Idle);
    }

    #[test]
    fn begin_during_replay_keeps_remaining_input() {
        let mut q = HoldReplayQueue::default();
        q.begin();
        q.enqueue('a').unwrap();
        q.enqueue('b').unwrap();
        assert_eq!(q.next(), Some('a'));
        // 'a' completed another trigger: a new insertion starts with 'b' still queued.
        q.begin();
        q.enqueue('c').unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q.drain(), vec!['b', 'c']);
        assert_eq!(q.state(), HoldState::Idle);
        assert_eq!(q.len(), 0);
    }
}
