"use client";

import Image from "next/image";
import { AppleMark } from "./apple-mark";
import { MacOSDock } from "./macos-dock";
import { StageCallouts } from "./stage-callouts";
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { snippets as demoSnippets } from "@/lib/examples";
import { Icon } from "./icons";

type Snippet = (typeof demoSnippets)[number];
type Filter = "all" | "favorites" | "everyday" | "work";
type Expansion = { previous: string; caret: number; expanded: string; end: number; saved: number };

const idleMessage = "A few keys. All the right words.";
const typingDelay = 110;
const filters: { id: Filter; label: string; heading: string; icon: string }[] = [
  { id: "all", label: "All snippets", heading: "All snippets", icon: "grid" },
  { id: "favorites", label: "Favorites", heading: "Favorites", icon: "star" },
  { id: "everyday", label: "Everyday", heading: "Everyday words", icon: "folder" },
  { id: "work", label: "Work stuff", heading: "Work stuff", icon: "folder" },
];

function inFilter(snippet: Snippet, filter: Filter) {
  if (filter === "all") return true;
  if (filter === "favorites") return snippet.favorite;
  return snippet.group === filter;
}

function prefersReducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function hasKeyboard() {
  return window.matchMedia("(hover: hover) and (pointer: fine)").matches;
}

function result(snippet: Snippet, ending: string) {
  return `${snippet.trigger.length} keys → ${snippet.text.length} characters. ${ending}`;
}

export function SnippetPlayground({ snippets = demoSnippets }: { snippets?: Snippet[] }) {
  const [selected, setSelected] = useState(snippets[0]);
  const [filter, setFilter] = useState<Filter>("all");
  const [value, setValue] = useState("");
  const [expansion, setExpansion] = useState<Expansion | null>(null);
  const [message, setMessage] = useState(idleMessage);
  const [saved, setSaved] = useState(0);
  const [playing, setPlaying] = useState(false);
  // Autoplay runs as the visitor scrolls, so its result isn't announced to screen readers.
  const [quiet, setQuiet] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const screenRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const timers = useRef<number[]>([]);
  const touched = useRef(false);
  const autoplayed = useRef(false);

  const visible = snippets.filter((snippet) => inFilter(snippet, filter));
  const heading = filters.find((item) => item.id === filter)?.heading;

  function clearTimers() {
    timers.current.forEach((timer) => window.clearTimeout(timer));
    timers.current = [];
  }

  function reset() {
    clearTimers();
    autoplayed.current = false;
    setPlaying(false);
    setQuiet(false);
    setValue("");
    setExpansion(null);
    setMessage(idleMessage);
  }

  function select(snippet: Snippet) {
    touched.current = true;
    setSelected(snippet);
    reset();
  }

  function chooseFilter(next: Filter) {
    touched.current = true;
    const nextVisible = snippets.filter((snippet) => inFilter(snippet, next));
    setFilter(next);
    if (!nextVisible.some((snippet) => snippet.id === selected.id)) setSelected(nextVisible[0]);
    reset();
  }

  function navigateTabs(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    // The list is a column on wide screens and a 2×2 grid on phones, so both axes move.
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[event.key];
    if (step === undefined && event.key !== "Home" && event.key !== "End") return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? visible.length - 1
          : (index + (step ?? 0) + visible.length) % visible.length;
    select(visible[next]);
    tabRefs.current[next]?.focus();
  }

  function expand(element: HTMLTextAreaElement) {
    const text = element.value;
    const caret = element.selectionStart;
    const before = text.slice(0, caret);
    setValue(text);
    setExpansion(null);
    setMessage(idleMessage);
    if (!before.endsWith(selected.trigger) || element.selectionEnd !== caret) return;
    const start = caret - selected.trigger.length;
    const expanded = text.slice(0, start) + selected.text + text.slice(caret);
    const end = start + selected.text.length;
    const gained = selected.text.length - selected.trigger.length;
    setValue(expanded);
    setExpansion({ previous: text, caret, expanded, end, saved: gained });
    setSaved((total) => total + gained);
    setMessage(result(selected, "Backspace to undo."));
    requestAnimationFrame(() => inputRef.current?.setSelectionRange(end, end));
  }

  function play(snippet: Snippet, auto: boolean) {
    reset();
    const characters = [...snippet.trigger];
    const finish = () => {
      timers.current = [];
      const gained = snippet.text.length - snippet.trigger.length;
      setValue(snippet.text);
      setExpansion({
        previous: snippet.trigger,
        caret: snippet.trigger.length,
        expanded: snippet.text,
        end: snippet.text.length,
        saved: gained,
      });
      setSaved((total) => total + gained);
      setPlaying(false);
      setQuiet(auto);
      autoplayed.current = auto;
      const canUndo = !auto && hasKeyboard();
      setMessage(result(snippet, auto ? "Your turn." : canUndo ? "Backspace to undo." : ""));
      if (canUndo) {
        inputRef.current?.focus();
        requestAnimationFrame(() =>
          inputRef.current?.setSelectionRange(snippet.text.length, snippet.text.length),
        );
      }
    };
    if (prefersReducedMotion()) {
      finish();
      return;
    }
    setPlaying(true);
    characters.forEach((_, index) => {
      timers.current.push(
        window.setTimeout(
          () => setValue(characters.slice(0, index + 1).join("")),
          typingDelay * (index + 1),
        ),
      );
    });
    timers.current.push(window.setTimeout(finish, typingDelay * (characters.length + 3)));
  }

  const startAutoplay = useEffectEvent(() => {
    if (!touched.current && !prefersReducedMotion()) play(selected, true);
  });

  // The browser paints the server-rendered HTML before any of this runs, so the CSS default in
  // globals.css carries that first paint. This sets the exact scale synchronously on mount,
  // before the hydrated DOM paints, so there is no 1 → value snap; the observer then handles
  // resizes.
  useLayoutEffect(() => {
    const screen = screenRef.current;
    if (!screen) return;
    const apply = () =>
      screen.style.setProperty("--device-scale", String(screen.getBoundingClientRect().width / 960));
    apply();
    const observer = new ResizeObserver(apply);
    observer.observe(screen);
    return () => observer.disconnect();
  }, []);

  const stopOffscreen = useEffectEvent(() => {
    if (timers.current.length === 0) return;
    clearTimers();
    setPlaying(false);
    setValue("");
  });

  useEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;
    const state = { started: false };
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.intersectionRatio >= 0.5 && !state.started) {
          state.started = true;
          startAutoplay();
        } else if (!entry.isIntersecting && state.started) {
          // Scrolled away mid-demo: stop the timers rather than type to nobody.
          stopOffscreen();
        }
      },
      { threshold: [0, 0.5] },
    );
    observer.observe(panel);
    return () => observer.disconnect();
  }, []);

  useEffect(
    () => () => {
      timers.current.forEach((timer) => window.clearTimeout(timer));
    },
    [],
  );

  return (
    <div className="product-stage" id="playground">
      <div className="device-frame">
        <StageCallouts />
        <Image
          className="device-bezel"
          src="/assets/macbook-neo-citrus.png"
          width={3220}
          height={2100}
          alt="MacBook Neo in Citrus"
          sizes="(max-width: 1100px) 1px, 1160px"
          preload
          fetchPriority="high"
        />
        <div className="device-screen" ref={screenRef}>
          <div className="device-workspace">
            <div className="desktop-menubar" aria-hidden="true">
              <AppleMark />
              <strong>Kudzu</strong>
              <span>File</span>
              <span>Edit</span>
              <span>View</span>
              <span>Window</span>
              <span>Help</span>
              <span className="desktop-clock">9:41 AM</span>
            </div>
            <div className="app-window">
              <div className="app-toolbar">
                <div className="traffic-lights" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                </div>
                <span>Kudzu</span>
                <span className="toolbar-status" key={saved}>
                  <span className="status-dot" />
                  {saved > 0 ? `${saved.toLocaleString("en-US")} keystrokes saved` : "Ready when you are"}
                </span>
              </div>
              <div className="app-layout">
                <aside className="app-sidebar" aria-label="Filter example snippets">
                  <div className="sidebar-brand">
                    <Image src="/assets/kudzu-icon.png" width={29} height={29} alt="" />
                    Your words, organized.
                  </div>
                  {filters.slice(0, 2).map((item) => (
                    <SidebarButton
                      key={item.id}
                      item={item}
                      count={snippets.filter((snippet) => inFilter(snippet, item.id)).length}
                      active={filter}
                      onChoose={chooseFilter}
                    />
                  ))}
                  <div className="sidebar-label">YOUR COLLECTIONS</div>
                  {filters.slice(2).map((item) => (
                    <SidebarButton
                      key={item.id}
                      item={item}
                      count={snippets.filter((snippet) => inFilter(snippet, item.id)).length}
                      active={filter}
                      onChoose={chooseFilter}
                    />
                  ))}
                  <div className="sidebar-label">ALWAYS WITH YOU</div>
                  <a className="sidebar-item" href="#clipboard">
                    <Icon name="copy" />
                    Clipboard history
                  </a>
                  <div className="sidebar-bottom">
                    <span className="status-dot" />
                    All good. Keep creating.
                  </div>
                </aside>
                <div className="snippet-list">
                  <div className="list-heading">
                    {heading}
                    <span>
                      {visible.length} {visible.length === 1 ? "snippet" : "snippets"}
                    </span>
                  </div>
                  <div
                    className="snippet-tabs"
                    role="tablist"
                    aria-label="Example snippets"
                    aria-orientation="vertical"
                  >
                    {visible.map((snippet, index) => (
                      <button
                        key={snippet.id}
                        ref={(node) => {
                          tabRefs.current[index] = node;
                        }}
                        className={`snippet-tab ${selected.id === snippet.id ? "active" : ""}`}
                        role="tab"
                        id={`tab-${snippet.id}`}
                        aria-selected={selected.id === snippet.id}
                        aria-controls="snippet-panel"
                        tabIndex={selected.id === snippet.id ? 0 : -1}
                        onClick={() => select(snippet)}
                        onKeyDown={(event) => navigateTabs(event, index)}
                      >
                        <span className="snippet-emoji" aria-hidden="true">
                          <Icon name={snippet.icon} />
                        </span>
                        <span>
                          <strong>{snippet.name}</strong>
                          <small>{snippet.description}</small>
                        </span>
                        <code>{snippet.trigger}</code>
                      </button>
                    ))}
                  </div>
                  <div className="list-footer">
                    <Icon name="command" />
                    Good words. Always within reach.
                  </div>
                </div>
                <div
                  className="snippet-editor"
                  id="snippet-panel"
                  role="tabpanel"
                  aria-labelledby={`tab-${selected.id}`}
                  ref={panelRef}
                >
                  <div className="editor-heading">
                    <span>TRY IT</span>
                    <Icon name="star" className={selected.favorite ? "is-favorite" : ""} />
                  </div>
                  <p className="editor-title">{selected.name}</p>
                  <div className="trigger-line">
                    Type
                    <button
                      type="button"
                      className="trigger-chip"
                      aria-label={`Watch ${selected.trigger} expand`}
                      aria-disabled={playing}
                      onClick={() => {
                        touched.current = true;
                        if (playing) return;
                        play(selected, false);
                      }}
                    >
                      <code>{selected.trigger}</code>
                      <Icon name="play" />
                    </button>
                    or press it to watch
                  </div>
                  <div className="demo-input-wrap">
                    <label className="visually-hidden" htmlFor="demo-input">
                      Try the {selected.trigger} shortcut
                    </label>
                    <div className={`demo-field ${expansion ? "expanded" : ""}`}>
                      <textarea
                      ref={inputRef}
                      id="demo-input"
                      rows={5}
                      placeholder={`Type ${selected.trigger} here…`}
                      spellCheck={false}
                      aria-describedby="demo-status"
                      value={value}
                      className={expansion ? "expanded" : playing ? "playing" : ""}
                      onFocus={() => {
                        touched.current = true;
                        // Reads the ref because this also fires during finish(), before React
                        // has re-rendered, when the playing state would still be stale.
                        if (timers.current.length > 0 || autoplayed.current) reset();
                      }}
                      onChange={(event) => {
                        touched.current = true;
                        clearTimers();
                        setPlaying(false);
                        autoplayed.current = false;
                        if (
                          event.nativeEvent instanceof InputEvent &&
                          event.nativeEvent.isComposing
                        ) {
                          setValue(event.target.value);
                          return;
                        }
                        expand(event.currentTarget);
                      }}
                      onCompositionEnd={(event) => expand(event.currentTarget)}
                      onKeyDown={(event) => {
                        if (
                          event.key !== "Backspace" ||
                          !expansion ||
                          value !== expansion.expanded ||
                          event.currentTarget.selectionStart !== expansion.end ||
                          event.currentTarget.selectionEnd !== expansion.end
                        )
                          return;
                        event.preventDefault();
                        setValue(expansion.previous);
                        setSaved((total) => Math.max(0, total - expansion.saved));
                        const caret = expansion.caret;
                        requestAnimationFrame(() =>
                          inputRef.current?.setSelectionRange(caret, caret),
                        );
                        setExpansion(null);
                        setMessage("Undone. Your shortcut is back.");
                      }}
                    />
                    </div>
                    <div className="demo-bottom">
                      <output id="demo-status" aria-live={quiet ? "off" : "polite"}>
                        {message}
                      </output>
                      <button
                        type="button"
                        aria-label="Reset snippet demo"
                        onClick={() => {
                          touched.current = true;
                          reset();
                          if (hasKeyboard()) inputRef.current?.focus();
                        }}
                      >
                        Reset
                        <Icon name="reset" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <MacOSDock />
          </div>
        </div>
      </div>
      <p className="demo-caption">
        <Icon name="sparkle" className="tiny-spark" />
        Just a practice run. On your Mac, this works in every app that accepts text.
      </p>
    </div>
  );
}

function SidebarButton({
  item,
  count,
  active,
  onChoose,
}: {
  item: (typeof filters)[number];
  count: number;
  active: Filter;
  onChoose: (filter: Filter) => void;
}) {
  return (
    <button
      type="button"
      className={`sidebar-item ${active === item.id ? "selected" : ""}`}
      aria-pressed={active === item.id}
      onClick={() => onChoose(item.id)}
    >
      <Icon name={item.icon} />
      {item.label} <span>{count}</span>
    </button>
  );
}
