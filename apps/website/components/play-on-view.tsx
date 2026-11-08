"use client";

import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";

/**
 * Plays a CSS entrance when the element scrolls into view. Content stays visible without
 * JavaScript, with reduced motion, and if the element is already on screen when the page loads.
 */
export function PlayOnView({
  className,
  labelledBy,
  children,
}: {
  className?: string;
  labelledBy?: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLElement>(null);
  // Set `waiting` in a layout effect so the pre-hidden state lands before the browser paints
  // the hydrated DOM; doing it in a passive effect flashed the finished art first.
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const { top, bottom } = element.getBoundingClientRect();
    if (top < window.innerHeight * 0.85 && bottom > 0) return;
    element.dataset.play = "waiting";
  }, []);
  useEffect(() => {
    const element = ref.current;
    if (!element || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (element.dataset.play !== "waiting") return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry.isIntersecting) return;
        observer.disconnect();
        element.dataset.play = "run";
      },
      { threshold: 0.4 },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return (
    <figure ref={ref} className={className} aria-labelledby={labelledBy}>
      {children}
    </figure>
  );
}
