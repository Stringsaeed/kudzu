import { ClipboardPicker } from "@/components/app/clipboard/clipboard-picker"
import { TooltipProvider } from "@/components/ui/tooltip"

export default function Page() {
  return (
    <TooltipProvider delayDuration={500} skipDelayDuration={200}>
      <ClipboardPicker />
    </TooltipProvider>
  )
}
