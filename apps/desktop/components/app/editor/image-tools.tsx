"use client"

import { isolateHistory } from "@codemirror/commands"
import type { EditorView } from "@codemirror/view"
import { ImagePlus } from "lucide-react"
import { useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { formattingField, selectedRange, setFormatting } from "@/lib/editor-formatting"

async function compactImage(file: File): Promise<{ dataUrl: string; width: number }> {
  if (!/^image\/(png|jpeg|webp)$/.test(file.type)) throw new Error("Choose a PNG, JPEG, or WebP image.")
  if (file.size > 20 * 1024 * 1024) throw new Error("Choose an image smaller than 20 MB.")
  const image = new Image()
  const imageUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => typeof reader.result === "string" ? resolve(reader.result) : reject(new Error("This image couldn't be read."))
    reader.onerror = () => reject(new Error("This image couldn't be read."))
    reader.readAsDataURL(file)
  })
  try {
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve()
      image.onerror = () => reject(new Error("This image couldn't be opened."))
      image.src = imageUrl
    })
    const canvas = document.createElement("canvas")
    const context = canvas.getContext("2d")
    if (!context) throw new Error("Image resizing isn't available.")
    let scale = Math.min(1, 512 / Math.max(image.naturalWidth, image.naturalHeight))
    for (let attempt = 0; attempt < 8; attempt++) {
      canvas.width = Math.max(1, Math.round(image.naturalWidth * scale))
      canvas.height = Math.max(1, Math.round(image.naturalHeight * scale))
      context.drawImage(image, 0, 0, canvas.width, canvas.height)
      const dataUrl = canvas.toDataURL("image/png")
      if (dataUrl.length <= 196608) return { dataUrl, width: Math.max(16, Math.min(canvas.width, 240)) }
      scale *= 0.75
    }
    throw new Error("This image is too detailed to embed. Try a smaller image.")
  } finally { image.src = "" }
}

export function ImageTools({ view }: { view: () => EditorView | null }) {
  const [open, setOpen] = useState(false)
  const [image, setImage] = useState<{ dataUrl: string; width: number } | null>(null)
  const [label, setLabel] = useState("")
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(false)
  const request = useRef(0)

  return <Popover open={open} onOpenChange={(value) => {
    request.current++
    setOpen(value)
    setImage(null)
    setError("")
    setLoading(false)
  }}>
    <PopoverTrigger asChild><Button variant="ghost" size="sm" aria-label="Insert image" className="text-muted-foreground" onMouseDown={(e) => e.preventDefault()}><ImagePlus /> Image</Button></PopoverTrigger>
    <PopoverContent className="w-80" align="start" onCloseAutoFocus={(e) => { e.preventDefault(); view()?.focus() }}>
      <form className="flex flex-col gap-3" onSubmit={(e) => {
        e.preventDefault()
        const editor = view()
        if (!editor || !image || loading || editor.composing) return
        const { from, to } = selectedRange(editor.state)
        const text = `[${label.trim().replace(/[{}\r\n]/g, " ") || "Image"}]`
        const changes = { from, to, insert: text }
        const mapped = editor.state.update({ changes }).state.field(formattingField) ?? []
        const spans = mapped.filter((s) => !(s.style.kind === "image" && s.start < from + text.length && s.end > from))
        const size = spans.reduce((sum, s) => sum + (s.style.kind === "image" ? s.style.dataUrl.length : 0), image.dataUrl.length)
        if (size > 524288) { setError("This snippet has too many images. Remove an image or use a smaller one."); return }
        spans.push({ start: from, end: from + text.length, style: { kind: "image", ...image } })
        editor.dispatch({ changes, effects: setFormatting.of(spans), selection: { anchor: from + text.length }, annotations: isolateHistory.of("full"), userEvent: "input.image" })
        setOpen(false)
        editor.focus()
      }}>
        <span className="text-sm font-medium">Embed an image</span>
        <p className="text-xs text-muted-foreground">Saved with your snippet. Large images are resized automatically.</p>
        <Input type="file" accept="image/png,image/jpeg,image/webp" aria-label="Choose image" onChange={async (e) => {
          const file = e.target.files?.[0]
          if (!file) return
          const token = ++request.current
          setLoading(true)
          setError("")
          setImage(null)
          try {
            const resized = await compactImage(file)
            if (token !== request.current) return
            setImage(resized)
            setLabel(file.name.replace(/\.[^.]+$/, ""))
          } catch (reason) { if (token === request.current) setError(String(reason instanceof Error ? reason.message : reason)) }
          finally { if (token === request.current) setLoading(false) }
        }} />
        {image && <>
          {/* oxlint-disable-next-line nextjs/no-img-element -- preview of an embedded raster image */}
          <img src={image.dataUrl} width={image.width} alt={label} className="max-h-40 max-w-full self-center object-contain" />
          <label className="flex flex-col gap-1 text-xs">Text alternative<Input value={label} onChange={(e) => setLabel(e.target.value)} maxLength={200} /></label>
          <label className="flex flex-col gap-1 text-xs">Width in pixels<Input type="number" min={16} max={512} value={image.width} onChange={(e) => setImage({ ...image, width: Math.min(512, Math.max(16, Number(e.target.value) || 16)) })} /></label>
        </>}
        {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
        <Button type="submit" size="sm" disabled={!image || loading}>{loading ? "Preparing image…" : "Insert image"}</Button>
      </form>
    </PopoverContent>
  </Popover>
}
