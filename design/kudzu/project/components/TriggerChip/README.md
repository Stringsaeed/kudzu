# TriggerChip

The monospace chip that shows a trigger: Kudzu's signature element, used everywhere a trigger appears.

Consumer provides the trigger text as children, optional `removable` and `onRemove`. A leading symbol (`:` or `;`) is set in `vine-ink` so the prefix reads as syntax; the rest is `ink` on `chip`, in the `trigger` style.

- Show triggers exactly as typed; never title-case or truncate them.
- Use `removable` only inside TriggerInput.
