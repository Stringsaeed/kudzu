import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
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
  title: site.title,
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

export const viewport: Viewport = { themeColor: "#f7f7f2" };

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
      <body>{children}</body>
    </html>
  );
}
