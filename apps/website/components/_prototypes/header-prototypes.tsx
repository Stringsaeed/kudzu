"use client";

// Three header layouts on the real homepage, selected by ?__variant= on this throwaway branch.
import Image from "next/image";
import { useCallback, useEffect, useState } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { SiteHeader } from "../site-header";
import { ThemeToggle } from "../theme-toggle";
import { Icon } from "../icons";
import "./header-prototypes.css";

const variants = [
  {
    id: "dock",
    name: "Dock",
    position: "One floating bar",
    cost: "Compact, with more visual weight around navigation.",
  },
  {
    id: "centered",
    name: "Centered",
    position: "Brand in the middle",
    cost: "Balanced, with less room for additional navigation links.",
  },
  {
    id: "editorial",
    name: "Editorial",
    position: "Masthead above navigation",
    cost: "More room for the brand, with a taller opening section.",
  },
];

function StarLink() {
  return (
    <a
      className="github-star"
      aria-label="Star on GitHub"
      href="https://github.com/Stringsaeed/kudzu"
      target="_blank"
      rel="noopener noreferrer"
    >
      <Icon name="github" />
      <span>Star on GitHub</span>
      <Icon name="star" />
    </a>
  );
}

function Wordmark() {
  return (
    <a href="#main" className="prototype-wordmark" aria-label="Kudzu home">
      <span aria-hidden="true">kudzu</span>
    </a>
  );
}

function VariantDock() {
  return (
    <header className="prototype-dock wrap">
      <a href="#main" className="brand" aria-label="Kudzu home">
        <Image src="/assets/kudzu-icon.png" width={38} height={38} alt="" />
        <span>kudzu</span>
      </a>
      <nav aria-label="Main navigation">
        <a href="#features">Features</a>
        <a href="#playground">Try it out</a>
        <StarLink />
      </nav>
      <div className="prototype-actions">
        <ThemeToggle />
        <a className="button button-small button-primary" href="#download">
          Get Kudzu
        </a>
      </div>
      <details className="prototype-mobile-menu">
        <summary>
          Explore <span aria-hidden="true">+</span>
        </summary>
        <nav aria-label="Mobile navigation">
          <a href="#features">Features</a>
          <a href="#playground">Try it out</a>
          <StarLink />
        </nav>
      </details>
    </header>
  );
}

function VariantCentered() {
  return (
    <header className="prototype-centered wrap">
      <nav aria-label="Main navigation">
        <a href="#features">The app</a>
        <a href="#playground">Try a shortcut</a>
      </nav>
      <Wordmark />
      <div className="prototype-actions">
        <StarLink />
        <ThemeToggle />
        <a href="#download" className="prototype-download-link">
          Download <Icon name="download" />
        </a>
      </div>
    </header>
  );
}

function VariantEditorial() {
  return (
    <header className="prototype-editorial wrap">
      <div className="prototype-masthead">
        <Wordmark />
        <p>
          Clipboard history and
          <br />
          text shortcuts for your Mac.
        </p>
        <div className="prototype-actions">
          <span>FREE & OPEN SOURCE</span>
          <ThemeToggle />
        </div>
      </div>
      <div className="prototype-editorial-nav">
        <nav aria-label="Main navigation">
          <a href="#features">Explore Kudzu</a>
          <a href="#playground">Try a shortcut</a>
          <StarLink />
        </nav>
        <a href="#download" className="button button-small button-dark">
          Download for Mac
        </a>
      </div>
    </header>
  );
}

export function HeaderPrototypes() {
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const [hidden, setHidden] = useState(false);
  const active = variants.find((item) => item.id === params.get("__variant"));
  const choose = useCallback(
    (id: string) => {
      const query = new URLSearchParams(params.toString());
      query.set("__variant", id);
      router.replace(`${pathname}?${query.toString()}`, { scroll: false });
    },
    [params, pathname, router],
  );
  const step = useCallback(
    (direction: number) => {
      const index = variants.findIndex((item) => item.id === active?.id);
      choose(variants[(index + direction + variants.length) % variants.length].id);
    },
    [active?.id, choose],
  );

  useEffect(() => {
    if (!active) return;
    const handleKey = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey
      )
        return;
      if (
        event.target instanceof Element &&
        event.target.closest("input,textarea,select,[contenteditable]")
      )
        return;
      if (event.key.toLowerCase() === "h") {
        setHidden((value) => !value);
        return;
      }
      if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
      event.preventDefault();
      step(event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1);
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [active, step]);

  if (!active) return <SiteHeader />;

  return (
    <div className="header-prototype">
      <div className="variant-picker-select" hidden={hidden}>
        <label htmlFor="header-variant">HEADER PROTOTYPE · STRUCTURE</label>
        <select
          id="header-variant"
          value={active.id}
          onChange={(event) => choose(event.target.value)}
        >
          <optgroup label="Structure">
            {variants.map((variant) => (
              <option value={variant.id} key={variant.id}>
                {variant.name} — {variant.position}
              </option>
            ))}
          </optgroup>
        </select>
        <span>{active.cost}</span>
      </div>
      <nav className="variant-picker" aria-label="Header variants" hidden={hidden}>
        <fieldset aria-labelledby="header-structure">
          <span className="variant-picker-heading" id="header-structure">
            <Icon name="arrow" />
            Structure
          </span>
          {variants.map((variant) => (
            <button
              type="button"
              key={variant.id}
              aria-pressed={variant.id === active.id}
              onClick={() => choose(variant.id)}
            >
              {variant.name}
              <small>{variant.position}</small>
            </button>
          ))}
        </fieldset>
      </nav>
      <div className="variant-stage">
        {active.id === "dock" ? (
          <VariantDock />
        ) : active.id === "centered" ? (
          <VariantCentered />
        ) : (
          <VariantEditorial />
        )}
      </div>
      <div className="prototype-controls" aria-label="Prototype controls" hidden={hidden}>
        <button type="button" onClick={() => step(-1)} aria-label="Previous header variant">
          ←
        </button>
        <output>
          {active.name} <span>· {variants.indexOf(active) + 1} / 3</span>
        </output>
        <button type="button" onClick={() => step(1)} aria-label="Next header variant">
          →
        </button>
        <button type="button" className="prototype-hide" onClick={() => setHidden(true)}>
          Hide <kbd>H</kbd>
        </button>
      </div>
      {hidden && (
        <button className="prototype-show" type="button" onClick={() => setHidden(false)}>
          Show header variants <kbd>H</kbd>
        </button>
      )}
    </div>
  );
}
