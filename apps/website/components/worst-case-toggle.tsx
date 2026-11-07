"use client";

import { useSyncExternalStore } from "react";

const cookie = "kudzu-worst-case";

function read() {
  return document.cookie.split("; ").includes(`${cookie}=1`);
}

/** Development only (see lib/worst-case.ts). Swaps the demo data for worst-case data. */
export function WorstCaseToggle() {
  const on = useSyncExternalStore(
    () => () => {},
    read,
    () => false,
  );
  return (
    <fieldset className="worst-case-toggle">
      <legend className="visually-hidden">Preview data</legend>
      {[false, true].map((value) => (
        <button
          key={String(value)}
          type="button"
          aria-pressed={on === value}
          onClick={() => {
            document.cookie = `${cookie}=${value ? 1 : 0}; path=/; SameSite=Lax`;
            window.location.reload();
          }}
        >
          {value ? "Worst case" : "Demo data"}
        </button>
      ))}
    </fieldset>
  );
}
