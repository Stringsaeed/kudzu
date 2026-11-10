import { useState } from "react"

/**
 * How many times `value` has changed since this component mounted; 0 on the first render.
 * Use it as a `key` and gate the animation class on it being above 0, so an entrance replays for
 * a real change and stays quiet on mount and remount.
 */
export function useChanges<T>(value: T): number {
  const [seen, setSeen] = useState(value)
  const [changes, setChanges] = useState(0)
  if (!Object.is(seen, value)) {
    setSeen(value)
    setChanges(changes + 1)
  }
  return changes
}
