import { cn } from "@/lib/utils"

export function Wordmark({ variant = "full", className }: { variant?: "full" | "small"; className?: string }) {
  const source = variant === "small" ? "/brand/kudzu-wordmark-echo-small.svg" : "/brand/kudzu-wordmark-echo.svg"
  return <span role="img" aria-label="Kudzu" className={cn("inline-block shrink-0 bg-current", className)}
    style={{ aspectRatio: variant === "small" ? "741 / 207" : "735 / 201", maskImage: `url(${source})`, maskSize: "contain", maskRepeat: "no-repeat", maskPosition: "center" }} />
}
