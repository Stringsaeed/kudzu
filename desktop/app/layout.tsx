import type { Metadata, Viewport } from "next"
import brand from "@/design/brand.json"
// Figtree is only the non-macOS fallback; @font-face loads lazily, so Macs never download it.
import "@fontsource-variable/figtree"
import "./globals.css"

export const metadata: Metadata = {
  title: "Kudzu",
  description: brand.description,
}

export const viewport: Viewport = {
  colorScheme: "dark light",
}

// Runs before first paint so there's never a light flash in a dark-first app.
const themeScript = `(function(){try{var t=localStorage.getItem('kudzu-theme')||'dark';var d=t==='dark'||(t==='system'&&matchMedia('(prefers-color-scheme: dark)').matches);var c=document.documentElement.classList;c.toggle('dark',d);c.toggle('light',!d);document.documentElement.style.colorScheme=d?'dark':'light';var p=location.pathname;if(p.indexOf('/palette')===0||p.indexOf('/form')===0||p.indexOf('/clipboard')===0||p.indexOf('/hud')===0||p.indexOf('/permissions')===0)c.add('transparent');if(/Mac/.test(navigator.platform)){c.add('mac');if('__TAURI_INTERNALS__' in window)c.add('vibrant')}}catch(e){}})()`

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
      </head>
      <body>{children}</body>
    </html>
  )
}
