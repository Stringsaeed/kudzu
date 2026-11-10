"use client"

import { useSyncExternalStore } from "react"
import { Toaster as Sonner, type ToasterProps } from "sonner"
import { CircleCheckIcon, InfoIcon, TriangleAlertIcon, OctagonXIcon, Loader2Icon } from "lucide-react"

/*
 * The app paints its theme as a class on <html> (lib/theme.ts), which can differ from the OS
 * appearance. Sonner picks its description, cancel-button and close-button colours from its own
 * theme attribute, so it follows that class rather than the system setting.
 */
function subscribeToTheme(onChange: () => void) {
  const observer = new MutationObserver(onChange)
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] })
  return () => observer.disconnect()
}
const currentTheme = () => (document.documentElement.classList.contains("dark") ? "dark" : "light")
const serverTheme = () => "dark" as const

const Toaster = ({ ...props }: ToasterProps) => {
  const theme = useSyncExternalStore(subscribeToTheme, currentTheme, serverTheme)

  return (
    <Sonner
      theme={theme}
      className="toaster group font-sans!"
      icons={{
        success: (
          <CircleCheckIcon className="size-4" />
        ),
        info: (
          <InfoIcon className="size-4" />
        ),
        warning: (
          <TriangleAlertIcon className="size-4" />
        ),
        error: (
          <OctagonXIcon className="size-4" />
        ),
        loading: (
          <Loader2Icon className="size-4 animate-spin" />
        ),
      }}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          // The edge is the hairline inside shadow-float, like every other floating surface.
          "--normal-border": "transparent",
          "--border-radius": "calc(var(--radius) * 1.4)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          // Sonner's own styles are injected at runtime, hence the `!`. Motion: the app's slow speed
          // and travel curve instead of Sonner's 400ms ease, for entering, stacking and leaving.
          toast:
            "cn-toast shadow-float! duration-(--motion-slow)! ease-spring! focus-visible:ring-2! focus-visible:ring-ring!",
          description: "text-muted-foreground!",
          actionButton: "rounded-md! focus-visible:ring-2! focus-visible:ring-ring! focus-visible:ring-offset-1! focus-visible:ring-offset-popover!",
          cancelButton: "rounded-md! bg-foreground/[0.07]! text-foreground! focus-visible:ring-2! focus-visible:ring-ring!",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
