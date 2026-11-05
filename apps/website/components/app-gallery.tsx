"use client";

import Image from "next/image";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";

const screens = [
  {
    id: "library",
    label: "Library",
    src: "/assets/app/library.png",
    width: 2560,
    height: 1600,
    caption: "Groups, favorites, a live preview, and rules for every snippet.",
    alt: "The Kudzu library, with a snippet list, an editor for the email signature snippet, and its preview.",
  },
  {
    id: "clipboard",
    label: "Clipboard",
    src: "/assets/app/clipboard.png",
    width: 2560,
    height: 1600,
    caption: "Text, links, code, colors, images, and files. All searchable.",
    alt: "Kudzu clipboard history, listing pinned and recent copies with a preview of a pinned email.",
  },
  {
    id: "search",
    label: "Quick search",
    src: "/assets/app/palette.png",
    width: 1520,
    height: 1040,
    caption: "Press ⌃⌥Space in any app to find a snippet and insert it.",
    alt: "The Kudzu quick search window, with suggested snippets and a preview of the selected one.",
  },
  {
    id: "insights",
    label: "Insights",
    src: "/assets/app/insights.png",
    width: 2560,
    height: 1600,
    caption: "See your expansions, characters, and time saved.",
    alt: "Kudzu Insights, showing expansions, characters saved, time saved, and most-used snippets.",
  },
];

export function AppGallery() {
  const [active, setActive] = useState(0);
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
            onClick={() => setActive(index)}
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
          {screens.map((item, index) => (
            <Image
              key={item.id}
              src={item.src}
              width={item.width}
              height={item.height}
              alt={index === active ? item.alt : ""}
              data-active={index === active || undefined}
              sizes="(max-width: 1240px) 100vw, 1160px"
            />
          ))}
        </div>
        <p>
          {screen.caption} <span>Shown with sample data.</span>
        </p>
      </div>
    </section>
  );
}
