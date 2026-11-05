"use client";

import { useEffect, useRef } from "react";
import { Icon } from "./icons";
import { GitHubLogo } from "./github-logo";
import { NavLinks } from "./nav-links";

export function MobileNavigation() {
  const menu = useRef<HTMLDetailsElement>(null);
  const close = () => {
    if (menu.current) menu.current.open = false;
  };

  useEffect(() => {
    const closeOutside = (event: Event) => {
      if (
        menu.current?.open &&
        event.target instanceof Node &&
        !menu.current.contains(event.target)
      ) {
        menu.current.open = false;
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && menu.current?.open) {
        event.preventDefault();
        menu.current.open = false;
        menu.current.querySelector("summary")?.focus();
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("focusin", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("focusin", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, []);

  return (
    <details className="mobile-navigation" ref={menu}>
      <summary aria-label="Navigation menu">
        <Icon name="menu" className="menu-open-icon" />
        <Icon name="close" className="menu-close-icon" />
      </summary>
      <nav aria-label="Mobile navigation">
        <NavLinks onNavigate={close} />
        <a
          className="github-star"
          href="https://github.com/Stringsaeed/kudzu"
          target="_blank"
          rel="noopener noreferrer"
          onClick={close}
        >
          <GitHubLogo />
          Star on GitHub
          <Icon name="star" />
        </a>
      </nav>
    </details>
  );
}
