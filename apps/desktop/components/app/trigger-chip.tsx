import { cn } from "@/lib/utils"

/** A trigger rendered as a mono token. */
export function TriggerChip({ trigger, className, muted }: { trigger: string; className?: string; muted?: boolean }) {
  return (
    <span
      className={cn(
        "inline-flex h-5 max-w-full items-center truncate rounded-[5px] px-1.5 font-mono text-[11.5px] leading-none tracking-tight",
        muted ? "bg-foreground/[0.05] text-muted-foreground" : "bg-iris-soft text-iris-foreground",
        className,
      )}
    >
      {trigger.replace(/ /g, "␣")}
    </span>
  )
}
