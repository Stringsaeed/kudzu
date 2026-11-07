import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { IconDefinitions } from "@/components/icons";
import { SiteFooter } from "@/components/site-footer";
import { ScrollMotion } from "@/components/scroll-motion";
import { SiteHeader } from "@/components/site-header";
import { WorstCaseToggle } from "@/components/worst-case-toggle";
import { site } from "@/lib/site";
import { themeInitialization } from "@/lib/theme";
import "./globals.css";

const figtree = localFont({
  src: "../public/assets/figtree.woff2",
  variable: "--font-figtree",
  display: "swap",
});
// Paper Mono only sets labels and code, so it isn't preloaded ahead of the Figtree headline.
const paperMono = localFont({
  src: "../public/assets/paper-mono.woff2",
  variable: "--font-paper",
  display: "swap",
  preload: false,
  adjustFontFallback: false,
});

export const metadata: Metadata = {
  metadataBase: new URL(site.url),
  title: { default: site.title, template: "%s — Kudzu" },
  description: site.description,
  alternates: { canonical: "/" },
  icons: { icon: "/assets/kudzu-icon.png", apple: "/assets/kudzu-icon.png" },
  openGraph: {
    type: "website",
    url: site.url,
    title: site.title,
    description: site.description,
    siteName: site.name,
  },
  twitter: { card: "summary_large_image", title: site.title, description: site.description },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f7f7f2" },
    { media: "(prefers-color-scheme: dark)", color: "#191918" },
  ],
};

// The layout reads no data, so pages without release details (docs, compare, privacy) stay
// fully static. The home page carries the app's structured data, with its version.
export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html
      lang="en"
      className={`${figtree.variable} ${paperMono.variable}`}
      suppressHydrationWarning
    >
      <head>
        <script id="kudzu-theme" dangerouslySetInnerHTML={{ __html: themeInitialization }} />
      </head>
      <body>
        <a className="skip-link" href="#main">
          Skip to content
        </a>
        <IconDefinitions />
        <SiteHeader />
        {children}
        <SiteFooter />
        <ScrollMotion />
        {process.env.NODE_ENV === "development" && <WorstCaseToggle />}
      </body>
    </html>
  );
}
