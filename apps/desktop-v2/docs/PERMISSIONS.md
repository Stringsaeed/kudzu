# Permission setup

Onboarding and Settings → General open the same permission companion. It stays visible while System Settings is active, and closes independently of the library window.

Accessibility is the first step (labelled Device Control and Data Access on macOS 27 and later). The engine attempts an active keyboard tap when it is granted. Input Monitoring remains available for additional listening access. The UI does not treat Input Monitoring alone as permission to insert text.

## Native handoff

`src-tauri/src/permissions/` owns the companion window and a small AppKit bridge. The bridge is compiled with the Xcode command line tools from `build.rs`; it adds no package dependencies.

The icon's transparent native hit area receives the actual mouse-down/drag events. Its drag pasteboard contains `NSBundle.mainBundle.bundleURL`, so it always represents the running application, including Kudzu Dev. A loose development executable has no application bundle to drag; use `pnpm tauri:build:dev` to test the complete handoff.

Opening Settings positions the companion beside the visible System Settings window using window bounds metadata. It does not capture the screen, move System Settings, or require Accessibility to guide the user toward granting Accessibility. If bounds are unavailable, the companion keeps its position and its written instructions remain usable.

A click-through overlay outlines the Settings window and demonstrates the drag twice. It stops on a real drag, permission detection, a Settings window move, switching apps, or closing the companion. Reduce Motion disables the overlay. If the screen cannot fit both windows side by side, the animation is skipped. The Finder button and the system + button provide keyboard-accessible alternatives.

The app never flips a privacy switch or approves authentication. macOS owns those actions.

## Live state

The existing engine poll reads Accessibility, Input Monitoring, posting access, and keyboard tap health. Its status now includes `canPostEvents` and `canExpand`.

- `canPostEvents`: macOS allows synthetic keyboard events.
- `canExpand`: a listener is attached, posting is allowed, and neither pause nor Secure Input blocks expansion.
- Setup completion requires Accessibility, posting access, and an attached listener. Pause and Secure Input are reported separately and never silently changed.

The companion subscribes to status events, rechecks on focus, and polls while its document is visible. A drop or successful Settings launch never counts as a grant. Revocation updates the same panel. If the grant is detected but the keyboard connection is still unavailable after five seconds, the companion offers Restart. An app-local marker reopens the companion after that explicit restart; the marker never stands in for an OS grant. Help also exposes the restart action.

## Verification

Browser preview: `pnpm dev`, then `/permissions` (or `/permissions?light`). The preview controls exercise missing, allowed, paused, Secure Input, restart-required, and revoked states without changing macOS permissions.

For native verification, build and launch Kudzu Dev. Check:

1. Onboarding and General settings both open the companion.
2. Open Settings lands on the selected pane; the helper stays visible.
3. The ghost icon and outline follow the actual window placement; they never intercept clicks.
4. Drag the icon into Settings and verify the item is **Kudzu Dev**. Cancel the authentication prompt to test cancellation without granting access.
5. With the user's approval, enable access and observe Allowed without clicking Refresh. Confirm a real expansion in another app after explicitly resuming the dev engine.
6. Check revocation, relaunch, the Finder fallback, Escape/close, light/dark, Reduce Motion, multiple displays, and a display too narrow for side-by-side placement.

The in-app practice field is explicitly a preview; it cannot prove system-wide insertion because Kudzu suppresses its engine inside its own windows.
