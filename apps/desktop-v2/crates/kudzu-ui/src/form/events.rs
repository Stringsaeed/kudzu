//! NSApp.currentEvent as a form `InputEvent`. Missing events stay `None`.
use kudzu_app::ports::{EventStamp, InputEvent};
use kudzu_appkit::current_event::CurrentEvent;

pub fn live() -> Option<InputEvent> {
    kudzu_appkit::current_event::current_event().map(from_appkit)
}

pub fn from_appkit(event: CurrentEvent) -> InputEvent {
    InputEvent {
        stamp: EventStamp::from_cg_event_timestamp(event.timestamp),
        key_code: event.key_code,
    }
}

/// Fake stamps for tests. Never used on the live path.
pub fn stamped(timestamp: u64, key_code: Option<u16>) -> InputEvent {
    InputEvent {
        stamp: EventStamp::from_cg_event_timestamp(timestamp),
        key_code,
    }
}

/// Injectable `NSApp.currentEvent` for tests. The live path never reads this.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        static EVENT: Cell<Option<(u64, Option<u16>)>> = const { Cell::new(None) };
    }

    pub fn set(timestamp: u64, key_code: Option<u16>) {
        EVENT.with(|event| event.set(Some((timestamp, key_code))));
    }

    pub fn clear() {
        EVENT.with(|event| event.set(None));
    }

    pub fn current() -> Option<InputEvent> {
        EVENT.with(|event| event.get().map(|(stamp, key)| stamped(stamp, key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_native_event_stays_none() {
        assert!(Option::<CurrentEvent>::None.map(from_appkit).is_none());
    }

    #[test]
    fn a_real_stamp_keeps_its_cg_timestamp_and_key() {
        let event = from_appkit(CurrentEvent {
            timestamp: 9_001,
            key_code: Some(36),
        });
        assert_eq!(event.stamp.cg_event_timestamp(), 9_001);
        assert_eq!(event.key_code, Some(36));
    }

    #[test]
    fn a_click_has_a_stamp_and_no_key_code() {
        let event = stamped(12, None);
        assert_eq!(event.stamp.cg_event_timestamp(), 12);
        assert_eq!(event.key_code, None);
    }
}
