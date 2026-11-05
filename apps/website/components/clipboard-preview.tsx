"use client";

import { useState } from "react";
import { clipboardItems } from "@/lib/examples";
import { Icon } from "./icons";

export function ClipboardPreview() {
  const [query, setQuery] = useState("");
  const items = clipboardItems.filter((item) =>
    `${item.search} ${item.content}`.toLowerCase().includes(query.trim().toLowerCase()),
  );
  return (
    <div className="clipboard-art feature-art">
      <div className="clipboard-window">
        <div className="clipboard-search">
          <Icon name="search" />
          <label className="visually-hidden" htmlFor="clipboard-search">
            Search example clipboard items
          </label>
          <input
            id="clipboard-search"
            type="search"
            placeholder="Find that thing you copied…"
            autoComplete="off"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <kbd>⌘ K</kbd>
        </div>
        <div className="clipboard-meta">
          A FEW THINGS WORTH KEEPING<span>EXAMPLE HISTORY</span>
        </div>
        <div className="clipboard-items">
          {items.map((item) => (
            <div className="clip-item" key={item.id}>
              <span className={`clip-kind ${item.id}-kind`}>
                {item.icon ? <Icon name={item.icon} /> : item.symbol}
              </span>
              <div>
                <strong>{item.content}</strong>
                <small>
                  {item.kind} · {item.time}
                </small>
              </div>
              {item.id === "text" && <Icon name="star" />}
              {item.id === "color" && (
                <span className="color-swatch-label">a rather good blue</span>
              )}
            </div>
          ))}
          {items.length === 0 && (
            <p className="clipboard-empty">No matches. Try “color”, “link”, or “design”.</p>
          )}
        </div>
        <div className="clipboard-footer">
          <output>
            {items.length} {items.length === 1 ? "thing" : "things"}, right where you left them.
          </output>
          <span>↵ paste</span>
        </div>
      </div>
      <span className="art-caption">Your second thought, saved.</span>
    </div>
  );
}
