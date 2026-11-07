"use client";

import Link from "next/link";
import { useState, type ReactNode } from "react";
import { site } from "@/lib/site";
import { AppleMark } from "./apple-mark";
import { Icon } from "./icons";

type Status = "idle" | "started" | "shared" | "copied" | "failed";

/**
 * The Mac download, or on touch devices (which can't open a DMG) a way to send the page to a Mac.
 * Both buttons are rendered and CSS shows the right one, so nothing shifts during hydration.
 */
export function DownloadAction({
  label = "Download for Mac",
  secondary,
}: {
  label?: string;
  secondary?: ReactNode;
}) {
  const [status, setStatus] = useState<Status>("idle");
  const link = `${site.url}/#download`;

  async function send() {
    try {
      if (typeof navigator.share === "function") {
        await navigator.share({
          title: "Kudzu for Mac",
          text: "A free text expander and clipboard manager for Mac.",
          url: link,
        });
        setStatus("shared");
        return;
      }
      await navigator.clipboard.writeText(link);
      setStatus("copied");
    } catch (error) {
      if (error instanceof DOMException && error.name === "AbortError") return;
      setStatus("failed");
    }
  }

  return (
    <div className="download-action">
      <div className="hero-actions">
        <a
          className="button button-primary download-mac"
          href={site.download}
          onClick={() => setStatus("started")}
        >
          <AppleMark />
          {label}
        </a>
        <button type="button" className="button button-primary download-send" onClick={send}>
          <Icon name="send" />
          Share the Mac link
        </button>
        {secondary}
      </div>
      <output className="download-status">
        {status === "started" && (
          <>
            Getting the latest DMG from GitHub. If a release page opens instead, pick the DMG under
            Assets. <Link href="/#install">Here’s what happens next</Link>
          </>
        )}
        {status === "shared" && "Link shared. Open it on your Mac to install Kudzu."}
        {status === "copied" && "Link copied. Open it on your Mac."}
        {status === "failed" && "Kudzu runs on Mac. Visit kudzu.site on yours to download it."}
      </output>
    </div>
  );
}
