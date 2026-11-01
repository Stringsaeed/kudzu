# M0 platform spike

M0 implements the platform probes in both targets. A successful build and launch do **not** establish that expansion works with TCC enabled. The permission-dependent compatibility results remain unmeasured until a person runs the tests below.

`Kudzu/Engine` and `Kudzu/Panels` contain the reusable adapters. The disposable spike orchestration was removed; reference it with `git show d7e45e1:Kudzu/Spike/SpikeController.swift` and `git show d7e45e1:Kudzu/Spike/SpikeMatcher.swift`. The real engine now uses KudzuCore matching and templates.

## Platform requirements and distribution risks

| API or operation | TCC requirement | Direct build | Sandboxed build and review risk |
| --- | --- | --- | --- |
| Session `CGEventTap`, `.listenOnly` | Input Monitoring | Supported mechanism; grant required | Apple DTS explicitly supports this mechanism in sandboxed apps. This establishes API support, not App Review approval. [1] |
| `CGEvent.post`, Unicode keys, backspaces, simulated ⌘V | Post Event preflight; exposed through Accessibility settings | Must receive user approval; delivery has no acknowledgement | Treat cross-app posting as unproven. A successful preflight or void-returning post call does not prove delivery. [2] |
| `AXUIElement`, selected range and bounds | Accessibility | Focused app must expose the attributes | Apple's sandbox documentation excludes assistive Accessibility APIs, and DTS confirms the restriction. Expect caret lookup to fall back. There is no supported `com.apple.security.accessibility` entitlement. [3] |
| `NSPasteboard` snapshot and temporary text | No AX grant for the pasteboard write itself; simulated paste still needs Post Event | Copies all readable items and representations into memory | The clipboard does not bypass restrictions on delivering ⌘V. Test any clipboard privacy UI on the OS version being shipped. [4] |
| `IsSecureEventInputEnabled` | No permission request | Reports secure input; event interception can stop | Detection is retained in both targets. Owner lookup is advisory and may return nothing. [5] |
| Carbon `RegisterEventHotKey`, ⌃⌥Space | No global event monitor permission request | Registration can fail if another app owns the chord | Registration is attempted independently of AX/Input Monitoring. Verify invocation in the sandbox. [6] |
| `NSPanel`, `.nonactivatingPanel` | None to show the panel | Does not activate Kudzu | Panel display remains available when AX positioning is unavailable. |

References checked 2026-11-01:

1. [Apple DTS: sandboxed global monitoring](https://developer.apple.com/forums/thread/811443) and [working listen-only session tap example](https://developer.apple.com/forums/thread/724608). Both distinguish Input Monitoring from Accessibility. [Listen preflight API](https://developer.apple.com/documentation/coregraphics/cgpreflightlisteneventaccess()).
2. [Post Event preflight](https://developer.apple.com/documentation/coregraphics/cgpreflightposteventaccess()), [posting permission discussion](https://developer.apple.com/forums/thread/103992), and [Unicode event API](https://developer.apple.com/documentation/coregraphics/cgevent/keyboardsetunicodestring(stringlength:unicodestring:)).
3. [Protecting user data with App Sandbox](https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox), [DTS: Accessibility and sandbox restrictions](https://developer.apple.com/forums/thread/805556), [AX trust request](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions), and [range bounds attribute](https://developer.apple.com/documentation/applicationservices/kaxboundsforrangeparameterizedattribute).
4. [NSPasteboardItem](https://developer.apple.com/documentation/appkit/nspasteboarditem). Transient/concealed type names are clipboard-manager conventions, not a confidentiality guarantee. Other apps can still read the temporary text.
5. [Apple TN2150: Using Secure Event Input Fairly](https://developer.apple.com/library/archive/technotes/tn2150/_index.html). `kCGSSessionSecureInputPID` is an undocumented dictionary entry, accessed without private symbol linkage. It can be absent, stale, or unavailable in the sandbox; remove owner attribution before submission if Review objects. Boolean detection does not depend on it.
6. [Carbon event manager reference](https://developer.apple.com/documentation/carbon/carbon_event_manager). Registration status is shown in the menu when nonzero.

[App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/) require appropriate sandboxing for new Mac App Store apps under 2.4.5 and govern privacy under 5.1. User consent alone does not establish review eligibility. Explain global monitoring, clipboard use, and the absence of text logging in review notes. Do not add undocumented entitlements or an unsandboxed helper to make the test pass.

**Distribution decision remains open:** full M0 success in the App Store target is not established. AX caret positioning conflicts with the documented sandbox restrictions. Confirm a supported posting path with Apple DTS before relying on sandboxed automatic expansion. A mouse-positioned panel is a fallback, not proof of caret placement.

## Historical M0 implementation limits

These findings describe the removed spike. The real engine replaces its abort-on-typing heuristic with active-tap hold and replay, renders stored snippets, and leaves the panel/hotkey adapters unused until M3. Its current checks are in [Engine manual test](compat.md#engine-manual-test).

- Permission status refreshes every second, including after grants, so revocations are also detected. Requests occur only from the menu. System Settings deep links are best-effort and can change across OS releases. Accessibility and Post Event are shown separately even though both link to Accessibility settings.
- The tap runs on the main run loop. Its callback translates text through `NSEvent(cgEvent:).characters` using the active layout. It ignores marked synthetic events and resets for navigation, command/control shortcuts, clicks, app activation, and tap disablement. Timeout disablement re-enables the tap.
- Layout-aware key characters are not committed IME text. Dead keys, composition, autocorrect, app-side transformations, selection changes without key events, and mouse drags can invalidate the rolling buffer. Test non-US layouts explicitly. This matcher is intentionally disposable.
- A listen-only tap cannot suppress the trigger. The spike waits 30 ms before deleting it and aborts if another observed input or activation occurs first. This is a timing heuristic, not a target-app acknowledgement. Stop typing briefly at the end of a trigger. Late target processing can still reorder deletion and insertion.
- Posting reports only that events were submitted. Unicode uses at most 20 UTF-16 units per event and never splits a surrogate pair. Grapheme sequences may span events. Applications can ignore Unicode events or interpret them differently.
- Automatic insertion uses typing below 100 Swift characters with fewer than two newline characters; otherwise it uses paste. The long spike always forces paste. The reusable inserter also supports explicit overrides and left-arrow movement.
- Paste snapshots every item/type as data before changing the clipboard. An unreadable representation aborts the operation. Lazy providers and file promises may not survive conversion into eager data. Huge clipboard contents can consume memory or block the main thread while reading.
- Restore defaults to 250 ms, configurable through `Inserter.restoreDelay`. There is no public acknowledgement that another app has consumed the paste. Slow targets may require a longer delay. New clipboard contents win over restore via `changeCount`; cancellation and normal termination restore a pending clipboard. Crashes or forced termination can leave temporary text behind.
- Insertions are serialized by the spike. Focus can still move after event posting, and external input can interleave with synthetic events. If insertion fails after trigger deletion, there is no rollback in M0. Use disposable documents and never test at a shell prompt that can execute commands.
- The spike excludes Kudzu itself, common password-manager bundle IDs, Keychain Access, and loginwindow. Secure Input prevents expansion. These safeguards are not a complete sensitive-field detector.
- Caret lookup falls back from AX selection bounds to focused element bounds, then the mouse. The panel labels the source so a fallback cannot be mistaken for success. AX queries have a 200 ms timeout. AX coordinates flip around the primary display's top edge, preserving negative display coordinates. Placement uses the destination display's visible frame and moves above the caret if needed. The task's mouse fallback supersedes the plan's earlier screen-center fallback for this spike.
- The panel is a display probe, not the future interactive form editor. It does not reposition continuously after opening.
- Both targets currently share `sh.saeed.kudzu`. Different product names do not isolate TCC identity. Run only one at a time and use separate fresh VM snapshots for trustworthy sandbox comparisons. Local signing is ad hoc; signed release behavior still needs testing.

## Run the manual tests

Follow [Engine manual test](compat.md#engine-manual-test). Create the test snippets in
the main window; the old hard-coded `;kz`, `;now` and `;long` probes no longer exist.
Use separate fresh-account/VM snapshots for the two targets because they share a
bundle identifier. Record permissions, signing identity, OS, keyboard layout and
destination-app versions in the compatibility matrix. Repeat signed-release testing
on macOS 15 and the current OS before claiming platform compatibility.

The caret panel and global hotkey measurements described above remain historical:
these adapters are retained for M3 but have no menu action or hotkey in the real engine.

## Local verification

Verified on 2026-11-01 with Xcode 27.1 (27A9275), Swift 6.4, arm64:

- `xcodegen generate` succeeded.
- Both `Kudzu` and `KudzuAppStore` schemes built successfully with zero source errors or warnings. Xcode's tool emitted `Metadata extraction skipped, no AppIntents.framework dependency found`; it is not a Swift source diagnostic.
- `codesign -d --entitlements -` confirmed `com.apple.security.app-sandbox = true` on KudzuAppStore and no sandbox entitlement on Kudzu. Debug signatures are ad hoc. Xcode notes that hardened runtime is disabled with ad-hoc signing; this is not Developer ID/notarization proof.
- Each built app was launched through `open`, remained running after three seconds, then accepted a normal `NSRunningApplication.terminate()` quit request and exited. No TCC grants were attempted.

Rerun the launch check with `swift docs/scripts/launch-smoke.swift "$PWD/build/DerivedData/Build/Products/Debug"` after building both schemes. Quit existing instances first. The script opens each bundle in turn, verifies its executable is running, and requests normal termination.

TCC grants, cross-app insertion, AX placement, and App Review acceptance remain unverified. The compatibility matrix intentionally starts empty.
