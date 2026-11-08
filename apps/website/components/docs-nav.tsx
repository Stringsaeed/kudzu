import Link from "next/link";

const pages = [
  { href: "/docs", label: "Getting started" },
  { href: "/docs/templates", label: "Template guide" },
  { href: "/docs/ai-agents", label: "AI agents" },
] as const;

type Section = { id: string; label: string };

/**
 * The docs sidebar on wide screens. Below 850px it becomes a row of page links with an
 * "On this page" disclosure, so phone readers keep the table of contents.
 */
export function DocsNav({
  current,
  sections = [],
}: {
  current: (typeof pages)[number]["href"];
  sections?: Section[];
}) {
  const list = (
    <ul>
      {sections.map((section) => (
        <li key={section.id}>
          <a href={`#${section.id}`}>{section.label}</a>
        </li>
      ))}
    </ul>
  );
  return (
    <nav className="docs-nav" aria-label="Documentation">
      <div className="docs-pages">
        {pages.map((page) => (
          <Link
            key={page.href}
            href={page.href}
            aria-current={page.href === current ? "page" : undefined}
          >
            {page.label}
          </Link>
        ))}
      </div>
      {sections.length > 0 && (
        <>
          <div className="docs-sections">{list}</div>
          <details className="docs-toc">
            <summary>On this page</summary>
            {list}
          </details>
        </>
      )}
    </nav>
  );
}
