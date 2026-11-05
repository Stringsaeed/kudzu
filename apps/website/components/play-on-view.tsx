"use client";

import { useEffect, useRef, type ReactNode } from "react";

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
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = ref.current;
    if (!element || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const { top, bottom } = element.getBoundingClientRect();
    if (top < window.innerHeight * 0.85 && bottom > 0) return;
    element.dataset.play = "waiting";
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
    <div ref={ref} className={className} aria-labelledby={labelledBy}>
      {children}
    </div>
  );
}
