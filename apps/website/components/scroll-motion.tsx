"use client";

import { usePathname } from "next/navigation";
import { useEffect } from "react";

/**
 * The site's scroll motion, built on GSAP ScrollTrigger. Every effect is scrubbed, so it follows
 * the scroll position both ways rather than playing once.
 *
 * - GSAP loads after the page has rendered, so it never delays the first paint. Without
 *   JavaScript, or before it arrives, the page is simply static.
 * - Everything sits inside `gsap.matchMedia`, so reduced motion gets no movement and phone-only
 *   rules follow the viewport. The whole setup is reverted and rebuilt on every route change.
 * - Reveals tween the CSS `translate` property rather than `transform`, so they never fight the
 *   hover lifts and press scales that cards and buttons already use.
 */
export function ScrollMotion() {
  const pathname = usePathname();

  useEffect(() => {
    // Mutable state shared between the async setup and the cleanup.
    const run: { cancelled: boolean; revert?: () => void } = { cancelled: false };

    void (async () => {
      const [{ gsap }, { ScrollTrigger }] = await Promise.all([
        import("gsap"),
        import("gsap/ScrollTrigger"),
      ]);
      if (run.cancelled) return;
      gsap.registerPlugin(ScrollTrigger);
      const all = <T extends Element = HTMLElement>(selector: string) =>
        gsap.utils.toArray<T>(selector);

      const mm = gsap.matchMedia();
      run.revert = () => mm.revert();

      // The header gains depth once the page has moved. A class plus a CSS transition, so it
      // works with reduced motion too.
      mm.add("all", () => {
        const header = document.querySelector(".site-header");
        if (!header) return;
        ScrollTrigger.create({
          start: 1,
          end: "max",
          toggleClass: { targets: header, className: "is-scrolled" },
        });
      });

      mm.add(
        {
          motion: "(prefers-reduced-motion: no-preference)",
          wide: "(min-width: 1101px)",
          phone: "(max-width: 600px)",
        },
        (context) => {
          const { motion, wide, phone } = context.conditions as Record<string, boolean>;
          if (!motion) return;

          // Copy and headings rise in over their first stretch through the viewport.
          const copy = [
            ".intro-section > *",
            ".feature-copy > *",
            ".section-heading",
            ".gallery-heading",
            ".privacy-copy > *",
            ".trust-heading",
            ".faq-section > div:first-child > *",
            ".download-card",
            ".clipboard-art",
            ".choose-grid section",
            ".page-cta",
          ].join(", ");
          for (const element of all(copy)) {
            gsap.fromTo(
              element,
              { opacity: 0, translate: "0px 28px" },
              {
                opacity: 1,
                translate: "0px 0px",
                ease: "none",
                scrollTrigger: { trigger: element, start: "top bottom", end: "top 76%", scrub: true },
              },
            );
          }

          // The muted half of a two-tone headline types itself in from the left.
          for (const element of all(".intro-section h2 > span, .privacy-copy h2 em")) {
            gsap.set(element, { display: "inline-block" });
            gsap.fromTo(
              element,
              { clipPath: "inset(-0.2em 100% -0.2em 0%)" },
              {
                clipPath: "inset(-0.2em 0% -0.2em 0%)",
                ease: "none",
                scrollTrigger: { trigger: element, start: "top 88%", end: "top 62%", scrub: true },
              },
            );
          }

          // Card rows stagger left to right. On phones two of the rows scroll sideways and hold
          // their cards still.
          const rows = [
            ".template-grid",
            ".more-features",
            ".switch-grid",
            ".install-steps",
            ".trust-grid",
            ".privacy-notes",
          ];
          for (const row of all(rows.join(", "))) {
            if (phone && row.matches(".template-grid, .more-features")) continue;
            const cards = Array.from(row.children).filter((child) => child.tagName !== "H2");
            gsap.fromTo(
              cards,
              { opacity: 0, translate: "0px 32px" },
              {
                opacity: 1,
                translate: "0px 0px",
                ease: "none",
                stagger: 0.12,
                scrollTrigger: { trigger: row, start: "top bottom", end: "top 66%", scrub: true },
              },
            );
          }

          // The real app settles into place.
          for (const frame of all(".gallery-frame")) {
            gsap.fromTo(
              frame,
              { opacity: 0.4, scale: 0.94, translate: "0px 40px" },
              {
                opacity: 1,
                scale: 1,
                translate: "0px 0px",
                ease: "none",
                scrollTrigger: { trigger: frame, start: "top bottom", end: "top 55%", scrub: true },
              },
            );
          }

          if (wide) {
            // The laptop lid opens toward you as you scroll into the demo.
            const device = document.querySelector(".device-frame");
            if (device) {
              gsap.fromTo(
                device,
                { rotationX: 16, scale: 0.93, transformPerspective: 2600, transformOrigin: "50% 100%" },
                {
                  rotationX: 0,
                  scale: 1,
                  ease: "none",
                  scrollTrigger: { trigger: device, start: "top bottom", end: "bottom bottom", scrub: true },
                },
              );
            }
            // The headline drifts up and away as the demo takes over.
            for (const element of all(".hero > h1, .hero > .hero-description")) {
              gsap.to(element, {
                opacity: 0.25,
                translate: "0px -48px",
                ease: "none",
                scrollTrigger: { trigger: element, start: "top top", end: "bottom top", scrub: true },
              });
            }
          }

          // The footer wordmark rises out of the bottom edge, then repeats itself: its four echoes
          // start stacked under the letters and fan out, 7 units per step, into the trail. One
          // timeline over the footer's arrival, finishing exactly as the page runs out.
          const footer = document.querySelector(".footer-section");
          const wordmark = footer?.querySelector(".footer-wordmark");
          if (footer && wordmark) {
            const echoes = all<SVGPathElement>(".footer-wordmark .wordmark-echo");
            const timeline = gsap.timeline({
              defaults: { ease: "none" },
              scrollTrigger: { trigger: footer, start: "top bottom", end: "bottom bottom", scrub: 0.6 },
            });
            timeline
              .fromTo(
                wordmark,
                { clipPath: "inset(100% 0% 0% 0%)", y: 24 },
                { clipPath: "inset(0% 0% 0% 0%)", y: 0, duration: 0.55 },
                0.05,
              )
              .fromTo(
                echoes,
                {
                  opacity: 0,
                  x: (_, echo: SVGPathElement) => -7 * Number(echo.style.getPropertyValue("--step")),
                  y: (_, echo: SVGPathElement) => -7 * Number(echo.style.getPropertyValue("--step")),
                },
                { opacity: 1, x: 0, y: 0, duration: 0.6, stagger: { each: 0.05, from: "end" } },
                0.4,
              );
          }
        },
      );

      // Fonts and lazy images can change section heights after the first measurement.
      void document.fonts?.ready.then(() => {
        if (!run.cancelled) ScrollTrigger.refresh();
      });
    })();

    return () => {
      run.cancelled = true;
      run.revert?.();
    };
  }, [pathname]);

  return null;
}
