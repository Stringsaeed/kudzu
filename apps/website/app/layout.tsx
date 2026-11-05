import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { IconDefinitions } from "@/components/icons";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { getLatestRelease } from "@/lib/releases";
import { site } from "@/lib/site";
import { themeInitialization } from "@/lib/theme";
import "./globals.css";

const figtree = localFont({
  src: "../public/assets/figtree.woff2",
  variable: "--font-figtree",
  display: "swap",
});
const paperMono = localFont({
  src: "../public/assets/paper-mono.woff2",
  variable: "--font-paper",
  display: "swap",
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

export default async function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  const release = await getLatestRelease();
  const softwareSchema = JSON.stringify({
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: site.name,
    url: site.url,
    description: site.description,
    applicationCategory: "UtilitiesApplication",
    operatingSystem: "macOS 14 or later",
    license: site.license,
    downloadUrl: `${site.url}${site.download}`,
    ...(release ? { softwareVersion: release.version } : {}),
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  }).replace(/</g, "\\u003c");
  return (
    <html
      lang="en"
      className={`${figtree.variable} ${paperMono.variable}`}
      suppressHydrationWarning
    >
      <head>
        <script id="kudzu-theme" dangerouslySetInnerHTML={{ __html: themeInitialization }} />
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: softwareSchema }} />
      </head>
      <body>
        <a className="skip-link" href="#main">
          Skip to content
        </a>
        <IconDefinitions />
        <SiteHeader />
        {children}
        <SiteFooter />
      </body>
    </html>
  );
}
