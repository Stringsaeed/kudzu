export const site = {
  name: "Kudzu",
  url: "https://kudzu.site",
  title: "Kudzu — Free Text Expander & Clipboard Manager for Mac",
  description:
    "Your best words, a few keystrokes away. Kudzu is a free, open-source text expander and clipboard manager for Mac. No account. No subscription.",
  github: "https://github.com/Stringsaeed/kudzu",
  issues: "https://github.com/Stringsaeed/kudzu/issues",
  license: "https://github.com/Stringsaeed/kudzu/blob/main/LICENSE",
  engineSource:
    "https://github.com/Stringsaeed/kudzu/tree/main/apps/desktop/src-tauri/crates/kudzu-engine",
  keyBufferSource:
    "https://github.com/Stringsaeed/kudzu/blob/main/apps/desktop/src-tauri/crates/kudzu-core/src/matching/key_buffer.rs",
  templateSyntax:
    "https://github.com/Stringsaeed/kudzu/blob/main/apps/desktop/docs/TEMPLATE_SYNTAX.md",
  releases: "https://github.com/Stringsaeed/kudzu/releases/latest",
  // Resolves to the newest stable DMG. See app/download/route.ts.
  download: "/download",
  brew: "brew install --cask stringsaeed/tap/kudzu",
};

export type NavItem = { href: string; label: string; section?: string };

export const navItems: NavItem[] = [
  { href: "/#features", label: "Features", section: "features" },
  { href: "/#playground", label: "Try it", section: "playground" },
  { href: "/#privacy", label: "Privacy", section: "privacy" },
  { href: "/#faq", label: "FAQ", section: "faq" },
  { href: "/docs", label: "Docs" },
];
