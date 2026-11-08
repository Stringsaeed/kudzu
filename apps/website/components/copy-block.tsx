"use client";

import { useEffect, useRef, useState } from "react";
import { Icon } from "./icons";

/** A command or config file with a copy button, for the docs. */
export function CopyBlock({ label, value }: { label: string; value: string }) {
  const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const settle = (next: "copied" | "failed") => {
    setStatus(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setStatus("idle"), next === "copied" ? 2500 : 6000);
  };
  return (
    <div className="copy-block">
      <pre>
        <code>{value}</code>
      </pre>
      <button
        type="button"
        aria-label={`Copy ${label}`}
        onClick={() => navigator.clipboard.writeText(value).then(() => settle("copied"), () => settle("failed"))}
      >
        <Icon key={status} name={status === "copied" ? "check" : "copy"} />
      </button>
      <output className="copy-status">
        {status === "copied" ? "Copied." : status === "failed" ? "Select the text and copy it with ⌘C." : ""}
      </output>
    </div>
  );
}
