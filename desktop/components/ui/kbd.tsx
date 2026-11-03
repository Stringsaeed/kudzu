import { cn } from "cn"

function Kbd({ className, ...props }: React.ComponentProps<"kbd">) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "pointer-events-none inline-flex h-5 w-fit min-w-5 items-center justify-center gap-1 rounded-[5px] bg-foreground/[0.06] px-1 font-sans text-[11px] leading-none font-medium text-muted-foreground tabular shadow-key select-none",
        // Inside tooltips (inverted surface) and filled buttons, keycaps go translucent so they read on any fill.
        "in-data-[slot=tooltip-content]:bg-background/20 in-data-[slot=tooltip-content]:text-background in-data-[slot=tooltip-content]:shadow-none dark:in-data-[slot=tooltip-content]:bg-background/10",
        "[[data-slot=button][data-variant=default]_&]:bg-black/15 [[data-slot=button][data-variant=default]_&]:text-current [[data-slot=button][data-variant=default]_&]:shadow-[inset_0_1px_1px_oklch(0_0_0/0.15)]",
        "[&_svg:not([class*='size-'])]:size-3",
        className
      )}
      {...props}
    />
  )
}

function KbdGroup({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <kbd
      data-slot="kbd-group"
      className={cn("inline-flex items-center gap-1", className)}
      {...props}
    />
  )
}

export { Kbd, KbdGroup }
