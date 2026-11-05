"use client";

import { useEffect, useRef, useState } from "react";
import { site } from "@/lib/site";
import { Icon } from "./icons";

export function CopyCommand() {
  const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return (
    <>
      <div className="brew-install">
        <code>{site.brew}</code>
        <button
          type="button"
          aria-label="Copy Homebrew install command"
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(site.brew);
              setStatus("copied");
              window.clearTimeout(timer.current);
              timer.current = window.setTimeout(() => setStatus("idle"), 2500);
            } catch {
              setStatus("failed");
            }
          }}
        >
          <Icon key={status} name={status === "copied" ? "check" : "copy"} />
        </button>
      </div>
      <output className="copy-status">
        {status === "copied"
          ? "Copied. Your Terminal is ready for it."
          : status === "failed"
            ? "Select the command above and copy it with ⌘C."
            : ""}
      </output>
    </>
  );
}
