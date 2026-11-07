"use client";

import Image from "next/image";
import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";

const screens = [
  {
    id: "library",
    label: "Library",
    name: "library",
    // Phones show a zoomed crop of the capture. This is where it sits, from 0 (left) to 1 (right).
    focus: 0.34,
    caption: "Groups, favorites, a live preview, and rules for every snippet.",
    alt: "The Kudzu library, with a snippet list, an editor for the email signature snippet, and its preview.",
  },
  {
    id: "search",
    label: "Quick search",
    name: "palette",
    focus: 0.36,
    caption: "Press ⌃⌥Space in any app to find a snippet and insert it.",
    alt: "The Kudzu quick search window over the library, with suggested snippets and a preview of the selected one.",
  },
  {
    id: "clipboard",
    label: "Clipboard",
    name: "clipboard",
    focus: 0.34,
    caption: "Text, links, code, colors, images, and files. All searchable.",
    alt: "Kudzu clipboard history, listing pinned and recent copies with a preview of a pinned email.",
  },
  {
    id: "picker",
    label: "Clipboard panel",
    name: "picker",
    focus: 0.345,
    caption: "Press ⌃⌥V in any app to search what you copied and paste it right where you are.",
    alt: "The Kudzu clipboard panel over the library, with recent copies numbered for quick pasting and a preview of the selected one.",
  },
  {
    id: "insights",
    label: "Insights",
    name: "insights",
    focus: 0.42,
    caption: "See your expansions, characters, and time saved.",
    alt: "Kudzu Insights, showing expansions, characters saved, time saved, and most-used snippets.",
  },
];

export function AppGallery() {
  const [active, setActive] = useState(0);
  // Only screens someone has asked for are mounted, so one frame doesn't download all five.
  // Hovering or focusing a tab mounts its screen early, so it's usually loaded by the click.
  const [wanted, setWanted] = useState<ReadonlySet<number>>(() => new Set([0]));
  const want = (index: number) =>
    setWanted((current) => (current.has(index) ? current : new Set(current).add(index)));
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const tabList = useRef<HTMLDivElement>(null);
  const screen = screens[active];

  // The sliding indicator follows the selected tab through layout and font changes.
  useEffect(() => {
    const list = tabList.current;
    if (!list) return;
    const place = () => {
      const tab = tabs.current[active];
      if (!tab) return;
      list.style.setProperty("--tab-x", `${tab.offsetLeft}px`);
      list.style.setProperty("--tab-y", `${tab.offsetTop}px`);
      list.style.setProperty("--tab-width", `${tab.offsetWidth}px`);
      list.style.setProperty("--tab-height", `${tab.offsetHeight}px`);
      list.dataset.indicator = "true";
      // On phones the tabs scroll sideways. Keep the selected one in view, without moving the page.
      if (list.scrollWidth > list.clientWidth) {
        list.scrollTo({ left: tab.offsetLeft - (list.clientWidth - tab.offsetWidth) / 2 });
      }
    };
    place();
    const observer = new ResizeObserver(place);
    observer.observe(list);
    return () => observer.disconnect();
  }, [active]);

  function navigate(event: KeyboardEvent<HTMLButtonElement>) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? screens.length - 1
          : (active + (event.key === "ArrowRight" ? 1 : -1) + screens.length) % screens.length;
    want(next);
    setActive(next);
    tabs.current[next]?.focus();
  }

  return (
    <section className="gallery-section wrap" id="app" aria-labelledby="gallery-title">
      <div className="gallery-heading">
        <span className="eyebrow">THE REAL THING</span>
        <h2 id="gallery-title">Right at home on your Mac.</h2>
      </div>
      <div className="gallery-tabs" role="tablist" aria-label="Kudzu screens" ref={tabList}>
        {screens.map((item, index) => (
          <button
            key={item.id}
            ref={(node) => {
              tabs.current[index] = node;
            }}
            type="button"
            role="tab"
            id={`gallery-tab-${item.id}`}
            aria-selected={index === active}
            aria-controls="gallery-panel"
            tabIndex={index === active ? 0 : -1}
            onPointerEnter={() => want(index)}
            onFocus={() => want(index)}
            onClick={() => {
              want(index);
              setActive(index);
            }}
            onKeyDown={navigate}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div
        className="gallery-panel"
        id="gallery-panel"
        role="tabpanel"
        aria-labelledby={`gallery-tab-${screen.id}`}
      >
        <div className="gallery-frame">
          {/* Light and dark captures of each screen. CSS shows the one for the current theme,
              and the hidden one is display: none, so the browser never fetches it. */}
          {screens.map((item, index) =>
            !wanted.has(index)
              ? null
              : (["light", "dark"] as const).map((theme) => (
                  <Image
                    key={`${item.id}-${theme}`}
                    className={`shot-${theme}`}
                    src={`/assets/app/${item.name}-${theme}.png`}
                    width={2880}
                    height={1800}
                    alt={index === active ? item.alt : ""}
                    data-active={index === active || undefined}
                    style={{ "--focus": item.focus } as CSSProperties}
                    sizes="(max-width: 600px) 270vw, (max-width: 1240px) 100vw, 1160px"
                  />
                )),
          )}
        </div>
        <p>
          {screen.caption} <span>Shown with sample data.</span>
        </p>
      </div>
    </section>
  );
}
