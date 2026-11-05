"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { navItems } from "@/lib/site";

/** Tracks which home page section is under the header, for the navigation's current item. */
function useActiveSection(enabled: boolean) {
  const [active, setActive] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled) return;
    const sections = navItems.flatMap((item) => {
      const element = item.section ? document.getElementById(item.section) : null;
      return element ? [element] : [];
    });
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) setActive(entry.target.id);
          else setActive((current) => (current === entry.target.id ? null : current));
        }
      },
      { rootMargin: "-20% 0px -70% 0px" },
    );
    sections.forEach((section) => observer.observe(section));
    return () => observer.disconnect();
  }, [enabled]);
  return enabled ? active : null;
}

export function NavLinks({ onNavigate }: { onNavigate?: () => void }) {
  const pathname = usePathname();
  const active = useActiveSection(pathname === "/");
  return navItems.map((item) => {
    const current = item.section
      ? active === item.section
      : pathname === item.href || pathname.startsWith(`${item.href}/`);
    return (
      <Link
        key={item.href}
        href={item.href}
        aria-current={current ? (item.section ? "location" : "page") : undefined}
        onClick={onNavigate}
      >
        {item.label}
      </Link>
    );
  });
}
