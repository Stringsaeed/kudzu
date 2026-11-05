import Link from "next/link";
import Image from "next/image";
import { site } from "@/lib/site";

const columns = [
  {
    title: "Product",
    links: [
      { href: "/#features", label: "Features" },
      { href: "/#playground", label: "Try it" },
      { href: "/#download", label: "Download" },
      { href: "/changelog", label: "Changelog" },
    ],
  },
  {
    title: "Learn",
    links: [
      { href: "/docs", label: "Getting started" },
      { href: "/docs/templates", label: "Template guide" },
      { href: "/compare", label: "Compare Kudzu" },
      { href: "/#faq", label: "FAQ" },
    ],
  },
  {
    title: "Project",
    links: [
      { href: site.github, label: "Source code" },
      { href: site.issues, label: "Report an issue" },
      { href: site.license, label: "MIT License" },
      { href: "/privacy", label: "Privacy" },
    ],
  },
];

export function SiteFooter() {
  return (
    <footer className="footer-section">
      <div className="site-footer wrap">
        <div className="footer-brand">
          <Link className="brand" href="/" aria-label="Kudzu home">
            <Image src="/assets/kudzu-icon.png" width="32" height="32" alt="" />
            <span>kudzu</span>
          </Link>
          <p>For things worth repeating.</p>
          <p>
            Made by <a href="https://thisissaeed.com">Saeed</a>
          </p>
        </div>
        <nav className="footer-columns" aria-label="Footer">
          {columns.map((column) => (
            <div key={column.title}>
              <h2>{column.title}</h2>
              <ul>
                {column.links.map((link) => (
                  <li key={link.href}>
                    <a href={link.href}>{link.label}</a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>
      </div>
      <div className="footer-wordmark wrap" aria-hidden="true" />
    </footer>
  );
}
