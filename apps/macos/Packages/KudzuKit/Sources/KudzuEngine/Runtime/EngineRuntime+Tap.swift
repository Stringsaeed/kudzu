import CoreGraphics
import Foundation
import KudzuCore

// The tap callback: matching under the core lock, then effects after it is released.
// Port of `Imp::receive` / `Imp::receive_locked` in `macos/engine.rs`.

extension EngineRuntime {
    func receive(_ kind: CGEventType, _ event: CGEvent, mode: TapMode) -> Bool {
        var effects = Effects()
        let swallow = core.withLock { core in
            receiveLocked(&core, kind, event, mode: mode, effects: &effects)
        }
        for held in effects.postUnmatched {
            held.post(Keys.syntheticMarker)
        }
        if let job = effects.job {
            send(job)
        }
        if effects.replay {
            send(.replay)
        }
        return swallow
    }

    private func receiveLocked(_ core: inout Core, _ kind: CGEventType, _ event: CGEvent, mode: TapMode, effects: inout Effects) -> Bool {
        if kind == .tapDisabledByTimeout || kind == .tapDisabledByUserInput {
            effects.postUnmatched = core.invalidate()
            return false
        }
        let marker = TapEvents.marker(event)
        if marker == Keys.syntheticMarker {
            return false
        }
        if kind != .keyDown && kind != .keyUp {
            // A click moves the caret or focus: the typed context is gone. Clicks in our own
            // focused windows (form, palette) don't affect the target document.
            if !flags.withLock({ $0.suppressed }) {
                effects.postUnmatched = core.invalidate()
            }
            return false
        }
        let replay = marker == Keys.replayMarker
        let keyDown = kind == .keyDown
        if core.holdExpired {
            effects.postUnmatched = core.invalidate()
        }
        // Holding comes first: typing during an insertion is replayed after it rather than
        // landing inside the text we're about to delete or insert.
        if !replay && mode == .active && core.held.state == .inserting {
            if let copy = HeldEvent.copy(event, keyDown: keyDown), core.held.enqueue(copy) {
                return true
            }
        }
        if core.preparingForm {
            core.preparingInterrupted = core.preparingInterrupted || keyDown
            effects.replay = replay
            return false
        }
        let suppressed = flags.withLock { $0.suppressed }
        // Typing that reaches another app while a form is open changes the document the
        // trigger sits in. Typing into our own windows (the form itself) doesn't.
        if keyDown && !suppressed && core.pendingForm != nil && Apps.frontmostPid() != ownPid {
            core.pendingForm?.interrupted = true
        }
        // Listen-only can't withhold keys. Anything typed before our backspaces go out would
        // land inside the deletion range, so the pending insertion is abandoned instead.
        if !replay && mode == .listenOnly && core.busy {
            if keyDown {
                core.listenOnlyInterrupted = true
            }
            return false
        }
        if !typedAllowed() {
            core.resetMatcher()
            effects.replay = replay
            return false
        }
        if !keyDown {
            effects.replay = replay
            return false
        }
        guard let input = TapEvents.classify(event) else {
            effects.replay = replay
            return false
        }
        let pid = Apps.frontmostPid()
        if pid != core.frontmostPid {
            core.frontmostPid = pid
            core.frontmostBundle = pid.flatMap(Apps.bundleId(of:))
            core.resetMatcher()
        }
        if pid == ownPid {
            core.resetMatcher()
            effects.replay = replay
            return false
        }
        let swallowed: String? = if case .character(let text) = input { text } else { nil }
        let undoCaret = core.undoCaretLeft
        core.undoCaretLeft = 0
        let bundle = core.frontmostBundle
        guard let matched = core.matcher.handle(input, bundleID: bundle) else {
            effects.replay = replay
            return false
        }
        if core.busy {
            core.resetMatcher()
            return false
        }
        core.busy = true
        core.listenOnlyInterrupted = false
        if mode == .active {
            core.startHold()
        }
        core.generation &+= 1
        let planMode: PlanMode = mode == .active ? .active : .listenOnly
        effects.job = .matched(Matched(
            generation: core.generation,
            event: matched,
            mode: planMode,
            undoCaret: undoCaret,
            swallowed: swallowed,
            pid: pid
        ))
        return mode == .active
    }
}
