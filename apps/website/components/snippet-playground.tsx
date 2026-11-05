"use client";

import Image from "next/image";
import { AppleMark } from "./apple-mark";
import { MacOSDock } from "./macos-dock";
import { SketchArrow } from "./sketch-arrow";
import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { snippets } from "@/lib/examples";
import { Icon } from "./icons";

type Expansion = { previous: string; caret: number; expanded: string; end: number };

export function SnippetPlayground() {
  const [selected, setSelected] = useState(snippets[0]);
  const [value, setValue] = useState("");
  const [expansion, setExpansion] = useState<Expansion | null>(null);
  const [message, setMessage] = useState("A few keys. All the right words.");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const screenRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const screen = screenRef.current;
    if (!screen) return;
    const observer = new ResizeObserver(([entry]) => {
      screen.style.setProperty("--device-scale", String(entry.contentRect.width / 960));
    });
    observer.observe(screen);
    return () => observer.disconnect();
  }, []);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  function reset() {
    setValue("");
    setExpansion(null);
    setMessage("A few keys. All the right words.");
  }

  function select(index: number) {
    setSelected(snippets[index]);
    reset();
  }

  function navigateTabs(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? snippets.length - 1
          : (index + (event.key === "ArrowDown" ? 1 : -1) + snippets.length) % snippets.length;
    select(next);
    tabRefs.current[next]?.focus();
  }

  function expand(element: HTMLTextAreaElement) {
    const text = element.value;
    const caret = element.selectionStart;
    const before = text.slice(0, caret);
    setValue(text);
    setExpansion(null);
    setMessage("A few keys. All the right words.");
    if (!before.endsWith(selected.trigger) || element.selectionEnd !== caret) return;
    const start = caret - selected.trigger.length;
    const expanded = text.slice(0, start) + selected.text + text.slice(caret);
    const end = start + selected.text.length;
    setValue(expanded);
    setExpansion({ previous: text, caret, expanded, end });
    setMessage(
      `${selected.trigger.length} keys → ${selected.text.length} characters. Backspace to undo.`,
    );
    requestAnimationFrame(() => inputRef.current?.setSelectionRange(end, end));
  }

  return (
    <div className="product-stage" id="playground">
      <div className="stage-note note-left">
        <span>Pick a snippet</span>
        <SketchArrow variant="right" />
      </div>
      <div className="stage-note note-right">
        <SketchArrow variant="left" />
        <span>Type its shortcut</span>
      </div>
      <div className="device-frame">
        <Image
          className="device-bezel"
          src="/assets/macbook-neo-citrus.png"
          width={3220}
          height={2100}
          alt="MacBook Neo in Citrus"
          sizes="(max-width: 600px) 1px, (max-width: 1240px) 100vw, 1160px"
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
                <span className="toolbar-status">
                  <span className="status-dot" />
                  Ready when you are
                </span>
              </div>
              <div className="app-layout">
                <aside className="app-sidebar" aria-label="Illustrative library sidebar">
                  <div className="sidebar-brand">
                    <Image src="/assets/kudzu-icon.png" width={29} height={29} alt="" />
                    Your words, organized.
                  </div>
                  <div className="sidebar-item selected">
                    <Icon name="grid" />
                    All snippets <span>4</span>
                  </div>
                  <div className="sidebar-item">
                    <Icon name="star" />
                    Favorites <span>2</span>
                  </div>
                  <div className="sidebar-label">YOUR COLLECTIONS</div>
                  <div className="sidebar-item">
                    <Icon name="folder" />
                    Everyday
                  </div>
                  <div className="sidebar-item">
                    <Icon name="folder" />
                    Work stuff
                  </div>
                  <div className="sidebar-label">ALWAYS WITH YOU</div>
                  <div className="sidebar-item">
                    <Icon name="copy" />
                    Clipboard history
                  </div>
                  <div className="sidebar-bottom">
                    <span className="status-dot" />
                    All good. Keep creating.
                  </div>
                </aside>
                <div className="snippet-list">
                  <div className="list-heading">
                    Everyday words<span>4 snippets</span>
                  </div>
                  <div
                    className="snippet-tabs"
                    role="tablist"
                    aria-label="Example snippets"
                    aria-orientation="vertical"
                  >
                    {snippets.map((snippet, index) => (
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
                        onClick={() => select(index)}
                        onKeyDown={(event) => navigateTabs(event, index)}
                      >
                        <span className="snippet-emoji" aria-hidden="true">
                          {snippet.symbol}
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
                >
                  <div className="editor-heading">
                    <span>SNIPPET PREVIEW</span>
                    <Icon name="star" />
                  </div>
                  <h2>{selected.name}</h2>
                  <div className="trigger-line">
                    When I type <code>{selected.trigger}</code>
                    <span>↵</span>
                  </div>
                  <div className="snippet-content">{selected.text}</div>
                  <div className="demo-input-wrap">
                    <label htmlFor="demo-input">
                      TRY IT HERE <span>Type {selected.trigger} below</span>
                    </label>
                    <textarea
                      ref={inputRef}
                      id="demo-input"
                      rows={2}
                      placeholder={selected.trigger}
                      spellCheck={false}
                      aria-describedby="demo-status"
                      value={value}
                      className={expansion ? "expanded" : ""}
                      onChange={(event) => {
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
                        const caret = expansion.caret;
                        requestAnimationFrame(() =>
                          inputRef.current?.setSelectionRange(caret, caret),
                        );
                        setExpansion(null);
                        setMessage("Undone. Your shortcut is back.");
                      }}
                    />
                    <div className="demo-bottom">
                      <output id="demo-status">{message}</output>
                      <button
                        type="button"
                        aria-label="Reset snippet demo"
                        onClick={() => {
                          reset();
                          inputRef.current?.focus();
                        }}
                      >
                        Reset ↺
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
        <span className="tiny-spark" aria-hidden="true">
          ✳
        </span>
        A little interactive preview. Pick a snippet, then type its shortcut.
      </p>
    </div>
  );
}
