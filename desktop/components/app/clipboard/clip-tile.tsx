"use client"

import { AlignLeft, Braces, File, Files, Globe, Image as ImageIcon, Package } from "lucide-react"
import { memo, useState } from "react"

import { clipMediaUrl } from "@/lib/backend"
import type { ClipEntry, ClipKind } from "@/lib/types"
import { cn } from "@/lib/utils"

import { AppIcon } from "../app-icon"

export const KIND_ICON: Record<ClipKind, React.ComponentType<{ className?: string }>> = {
  text: AlignLeft,
  code: Braces,
  link: Globe,
  color: AlignLeft,
  image: ImageIcon,
  file: File,
  other: Package,
}

/** Small image from the backend's media protocol; falls back to `fallback` when it can't load. */
export function ClipImage({
  src,
  className,
  alt = "",
  fallback,
  fit = "cover",
}: {
  src: string | null
  className?: string
  alt?: string
  fallback?: React.ReactNode
  /** cover/contain fill the box; natural keeps the image's own box (so its background only shows behind it). */
  fit?: "cover" | "contain" | "natural"
}) {
  const [failed, setFailed] = useState<string | null>(null)
  if (!src || failed === src) return <>{fallback ?? null}</>
  return (
    // eslint-disable-next-line @next/next/no-img-element -- static export; custom-protocol URL
    <img
      src={src}
      alt={alt}
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => setFailed(src)}
      className={cn(fit === "natural" ? "max-h-full max-w-full object-contain" : "size-full", fit === "cover" && "object-cover", fit === "contain" && "object-contain", className)}
    />
  )
}

/** Checkerboard behind transparent images (two hairline-tinted squares, scales with the theme). */
export const checkerboard =
  "bg-[conic-gradient(color-mix(in_oklch,var(--foreground)_7%,transparent)_25%,transparent_0_50%,color-mix(in_oklch,var(--foreground)_7%,transparent)_0_75%,transparent_0)] bg-[length:14px_14px]"

/**
 * The leading visual of a row: a thumbnail for images, a swatch for colours, a kind glyph for
 * everything else. The source app's icon sits in the corner like a Finder badge.
 */
export const ClipTile = memo(function ClipTile({ entry, active, size = 36, badge = true }: { entry: ClipEntry; active?: boolean; size?: number; badge?: boolean }) {
  const e = entry
  const radius = Math.round(size * 0.26)
  const icon = (Icon: React.ComponentType<{ className?: string }>) => (
    <span
      className={cn(
        "flex size-full items-center justify-center transition-colors duration-100",
        active ? "bg-iris text-iris-contrast shadow-accent sheen-strong" : "bg-foreground/[0.055] text-muted-foreground shadow-[inset_0_0_0_1px_var(--hairline)]",
      )}
      style={{ borderRadius: radius }}
    >
      <Icon className={size >= 32 ? "size-4" : "size-3.5"} />
    </span>
  )

  let body: React.ReactNode
  if (e.kind === "color" && e.color) {
    body = <span className="block size-full shadow-[inset_0_0_0_1px_oklch(0_0_0/0.12),inset_0_1px_0_oklch(1_0_0/0.18)]" style={{ background: e.color, borderRadius: radius }} />
  } else if (e.kind === "image" || (e.kind === "file" && e.image)) {
    body = (
      <span className={cn("block size-full overflow-hidden shadow-[inset_0_0_0_1px_var(--hairline)]", checkerboard)} style={{ borderRadius: radius }}>
        <ClipImage src={clipMediaUrl("thumb", e.id)} fallback={icon(ImageIcon)} />
      </span>
    )
  } else if (e.kind === "file") {
    const many = (e.files?.length ?? 0) > 1
    body = (
      <span className="flex size-full items-center justify-center" style={{ borderRadius: radius }}>
        <ClipImage src={clipMediaUrl("icon", e.id, 0)} fit="contain" fallback={icon(many ? Files : File)} />
      </span>
    )
  } else {
    body = icon(KIND_ICON[e.kind])
  }

  return (
    <span className={cn("relative shrink-0", active && (e.kind === "image" || e.kind === "color") && "rounded-[inherit] ring-2 ring-iris/70 ring-offset-1 ring-offset-background")} style={{ width: size, height: size, borderRadius: radius }}>
      {body}
      {badge && e.source && (
        <span className="absolute -right-1 -bottom-1 rounded-[5px] bg-background p-px shadow-[0_0_0_1px_var(--hairline)]">
          <AppIcon path={e.source.path} name={e.source.name} size={14} className="rounded-[3px]" />
        </span>
      )}
    </span>
  )
})
