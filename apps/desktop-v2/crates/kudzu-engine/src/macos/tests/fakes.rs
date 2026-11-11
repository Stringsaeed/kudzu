//! The three seams, implemented on the fake desktop.
//!
//! - [`FakeEvents`]: records every press and held event (decoded), applies it to the target
//!   document, and sends a replayed event back through the listener as macOS would. A press
//!   fails exactly when the real one would: no posting permission, or Secure Input on.
//! - [`FakeBoard`]: an in-memory pasteboard with a change count.
//! - [`FakeObserve`]: a virtual clock (sleeps return at once and advance it), a scripted
//!   frontmost app, permissions, Secure Input and a caret read from the document.

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::desk::{
    observed_from_tap, plain, At, BoardOp, Desk, Ev, Key, ListenerOp, Out, Slot, Tick, PLAIN,
};
use super::keyboard::{apply_event, apply_key, decode, label, unicode};
use crate::macos::events::{Events, Listener};
use crate::macos::ffi::{kCGEventKeyDown, kCGEventKeyUp, CGEventFlags, CGKeyCode};
use crate::macos::keys::{self, KeyError, REPLAY_MARKER};
use crate::macos::observe::{CaretMark, Clock, Observe};
use crate::macos::pasteboard::{Board, Item, Items};
use crate::macos::tap::{self, Handler, HeldEvent};
use crate::{AppInfo, PermissionStatus, Rect, TapMode};

pub(super) struct FakeEvents(pub Desk);

impl Events for FakeEvents {
    /// The real poster's two-half logic ([`keys::press_with`]) on the fake session: a half
    /// that can't be created fails as `CGEventCreateKeyboardEvent` would, and a key-down that
    /// goes out reaches the listener and the document at once.
    fn press(&self, key: CGKeyCode, flags: CGEventFlags, unicode: &[u16]) -> Result<(), KeyError> {
        let text = String::from_utf16_lossy(unicode);
        let out = decode(key, flags, &text);
        self.0.with(|s| {
            if !s.post_event || s.secure.0 {
                return Err(KeyError::NotAllowed);
            }
            Ok(())
        })?;
        keys::press_with(
            |down| self.0.with(|s| s.can_create(&out, down)).then_some(down),
            |down| {
                if down {
                    self.0.with(|s| {
                        s.log.push(Ev::Out(out.clone()));
                        s.press_times.push(Instant::now());
                    });
                }
                let swallowed = self.0.synthetic(key, flags, &text, down);
                if down && !swallowed {
                    self.0.with(|s| apply_key(s, key, flags, &text));
                }
            },
        )?;
        let n = self.0.with(|s| {
            s.presses += 1;
            s.presses - 1
        });
        self.0.fire(At::Press(n));
        Ok(())
    }

    fn post_held(&self, held: HeldEvent, marker: i64) {
        let event = held.raw();
        let down = held.is_key_down();
        let key = Key {
            label: label(tap::key_code(event) as CGKeyCode, &unicode(event)),
            down,
        };
        held.mark(marker);
        let replay = marker == REPLAY_MARKER;
        self.0.with(|s| {
            s.log.push(Ev::Out(if replay {
                Out::Replay(key)
            } else {
                Out::Release(key)
            }))
        });
        // A replayed event comes back through the listener; a released one is let through.
        let kind = if down { kCGEventKeyDown } else { kCGEventKeyUp };
        let reaches = !replay || !self.0.deliver(kind, event);
        if reaches && down {
            self.0.with(|s| apply_event(s, event));
        }
        drop(held);
        let n = self.0.with(|s| {
            s.helds += 1;
            s.helds - 1
        });
        self.0.fire(At::Held(n));
    }

    fn listen(&self, mode: TapMode, handler: Handler) -> Option<Box<dyn Listener>> {
        let id = self.0.with(|s| {
            if s.refuse.contains(&mode) {
                s.log.push(Ev::Listener(ListenerOp::Refused(mode)));
                return None;
            }
            let id = s.next_listener;
            s.next_listener += 1;
            s.listeners.push(Slot {
                id,
                mode,
                enabled: false,
                handler: Arc::new(handler),
            });
            s.log.push(Ev::Listener(ListenerOp::Created(mode)));
            Some(id)
        })?;
        Some(Box::new(FakeListener {
            desk: self.0.clone(),
            id,
            mode,
        }))
    }
}

struct FakeListener {
    desk: Desk,
    id: usize,
    mode: TapMode,
}

impl Listener for FakeListener {
    fn mode(&self) -> TapMode {
        self.mode
    }

    fn enable(&self) {
        self.desk.with(|s| {
            if let Some(slot) = s.listeners.iter_mut().find(|l| l.id == self.id) {
                slot.enabled = true;
            }
            let enabled = s.listeners.iter().filter(|l| l.enabled).count();
            s.peak_enabled = s.peak_enabled.max(enabled);
            s.log.push(Ev::Listener(ListenerOp::Enabled(self.mode)));
        });
    }
}

impl Drop for FakeListener {
    fn drop(&mut self) {
        self.desk.with(|s| {
            s.listeners.retain(|l| l.id != self.id);
            s.log.push(Ev::Listener(ListenerOp::Stopped(self.mode)));
        });
    }
}

pub(super) struct FakeBoard(pub Desk);

impl Board for FakeBoard {
    fn change_count(&self) -> isize {
        self.0.with(|s| s.count)
    }

    fn snapshot(&self) -> Option<Items> {
        let (items, n) = self.0.with(|s| {
            s.board_ops[0] += 1;
            ((!s.unreadable).then(|| s.items.clone()), s.board_ops[0] - 1)
        });
        self.0.fire(At::Snapshot(n));
        items
    }

    fn read_items(&self) -> Items {
        self.0.with(|s| s.items.clone())
    }

    fn write_items(&self, items: &[Item]) {
        let n = self.0.with(|s| {
            s.board_ops[2] += 1;
            s.board_ops[2] - 1
        });
        self.0.fire(At::Writing(n));
        self.0.with(|s| {
            s.items = items.to_vec();
            s.count += 1;
            s.log.push(Ev::Board(BoardOp::Wrote(plain(&s.items))));
        });
    }

    fn write_transient(&self, text: &str, html: Option<&str>) -> bool {
        let written = self.0.with(|s| {
            if s.transient_fails {
                return None;
            }
            let mut item: Item = vec![
                (PLAIN.into(), text.as_bytes().to_vec()),
                ("org.nspasteboard.TransientType".into(), Vec::new()),
                ("org.nspasteboard.ConcealedType".into(), Vec::new()),
            ];
            if let Some(html) = html {
                item.push(("public.html".into(), html.as_bytes().to_vec()));
            }
            s.items = vec![item];
            s.count += 1;
            s.log.push(Ev::Board(BoardOp::Transient {
                text: text.into(),
                html: html.is_some(),
            }));
            s.board_ops[1] += 1;
            Some(s.board_ops[1] - 1)
        });
        let Some(n) = written else { return false };
        self.0.fire(At::Transient(n));
        true
    }

    fn read_string(&self) -> Option<String> {
        let n = self.0.with(|s| {
            s.board_ops[3] += 1;
            s.board_ops[3] - 1
        });
        self.0.fire(At::ReadClipboard(n));
        self.0.with(|s| plain(&s.items))
    }

    fn write_string(&self, text: &str) -> bool {
        self.0.with(|s| {
            s.items = vec![vec![(PLAIN.into(), text.as_bytes().to_vec())]];
            s.count += 1;
            s.log.push(Ev::Board(BoardOp::Wrote(Some(text.into()))));
            true
        })
    }
}

pub(super) struct FakeObserve(pub Desk);

impl Clock for FakeObserve {
    fn now(&self) -> Instant {
        self.0.now()
    }

    fn sleep(&self, duration: Duration) {
        let n = self.0.with(|s| {
            let n = s.sleeps.entry(duration).or_default();
            *n += 1;
            *n - 1
        });
        self.0.fire(At::Sleep(duration, n));
        self.0.with(|s| {
            s.elapsed += duration;
            s.log.push(Ev::Slept(duration));
        });
    }

    fn every(&self, _name: &str, _period: Duration, tick: Tick) -> io::Result<()> {
        self.0.add_tick(tick);
        Ok(())
    }
}

impl Observe for FakeObserve {
    fn frontmost_pid(&self) -> Option<i32> {
        self.0.with(|s| {
            observed_from_tap(s);
            s.frontmost.as_ref().and_then(|a| a.pid)
        })
    }

    fn frontmost_app(&self) -> Option<AppInfo> {
        self.0.with(|s| {
            observed_from_tap(s);
            s.frontmost.clone()
        })
    }

    fn app_of(&self, pid: i32) -> Option<AppInfo> {
        self.0.with(|s| {
            observed_from_tap(s);
            s.running.iter().find(|a| a.pid == Some(pid)).cloned()
        })
    }

    fn bundle_id_of(&self, pid: i32) -> Option<String> {
        self.app_of(pid).map(|a| a.bundle_id)
    }

    fn name_of(&self, pid: i32) -> Option<String> {
        self.app_of(pid).map(|a| a.name)
    }

    fn resolve_pid(&self, app: &AppInfo) -> Option<i32> {
        self.0.with(|s| {
            s.running
                .iter()
                .find(|a| a.pid.is_some() && (a.pid == app.pid || a.bundle_id == app.bundle_id))
                .and_then(|a| a.pid)
        })
    }

    fn activate_and_wait(&self, app: &AppInfo, _timeout: Duration) -> bool {
        let pid = self.resolve_pid(app);
        let (activated, n) = self.0.with(|s| {
            s.log.push(Ev::Activated(app.bundle_id.clone()));
            s.activations += 1;
            let found = s.running.iter().find(|a| a.pid.is_some() && a.pid == pid);
            let activated = match found.cloned().filter(|_| s.activation_works) {
                Some(running) => {
                    s.frontmost = Some(running);
                    true
                }
                None => false,
            };
            (activated, s.activations - 1)
        });
        self.0.fire(At::Activate(n));
        activated
    }

    fn permissions(&self) -> PermissionStatus {
        self.0.with(|s| s.permissions)
    }

    fn post_event_allowed(&self) -> bool {
        self.0.with(|s| s.post_event)
    }

    fn secure_input(&self) -> (bool, Option<i32>) {
        self.0.with(|s| s.secure)
    }

    fn caret_rect(&self) -> Option<Rect> {
        Some(Rect {
            x: 120.0,
            y: 340.0,
            width: 1.0,
            height: 17.0,
        })
    }

    fn caret_snapshot(&self) -> Option<Box<dyn CaretMark>> {
        let start = self
            .0
            .with(|s| s.doc.reports_caret.then(|| s.doc.caret_utf16()))?;
        Some(Box::new(FakeCaret {
            desk: self.0.clone(),
            start,
        }))
    }
}

struct FakeCaret {
    desk: Desk,
    start: usize,
}

impl CaretMark for FakeCaret {
    fn inserted_utf16(&self) -> Option<usize> {
        self.desk.with(|s| {
            s.doc
                .reports_caret
                .then(|| s.doc.caret_utf16().checked_sub(self.start))
                .flatten()
        })
    }
}
