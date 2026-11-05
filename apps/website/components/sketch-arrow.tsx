export function SketchArrow({
  variant,
  className = "",
}: {
  variant: "up" | "left" | "right";
  className?: string;
}) {
  return (
    <svg className={`sketch-arrow ${className}`} viewBox="0 0 80 80" fill="none" aria-hidden="true">
      {variant === "up" ? (
        <>
          <path d="M13 68C23 63 19 55 29 47S35 35 46 30 51 18 66 12" />
          <path d="M48 13c8 1 12-1 18-1-2 6-2 12-5 18" />
        </>
      ) : variant === "left" ? (
        <>
          <path d="M69 10C54 5 51 16 53 25S45 35 42 44 30 57 13 54" />
          <path d="M23 46c-3 4-7 5-10 8 4 3 6 8 10 12" />
        </>
      ) : (
        <>
          <path d="M10 9c13-5 20 4 18 14S35 35 39 43s14 14 29 10" />
          <path d="M57 45c4 3 7 5 11 8-3 5-8 8-11 12" />
        </>
      )}
    </svg>
  );
}
