// Coordinates are in a 1000 × 652 space that matches the laptop image, so each leader line keeps
// landing on its target as the laptop scales. The left line crosses the empty band of the sidebar
// to reach the snippet list, and the right line crosses the bezel to reach the input.
const leaders = [
  { line: "M95 432H276", dot: { cx: 280, cy: 432 } },
  { line: "M905 350H844", dot: { cx: 840, cy: 350 } },
];

export function StageCallouts() {
  return (
    <>
      <p className="stage-callout stage-callout-left">Pick a snippet</p>
      <p className="stage-callout stage-callout-right">Type its shortcut</p>
      <svg className="stage-leaders" viewBox="0 0 1000 652" fill="none" aria-hidden="true">
        {leaders.map(({ line, dot }) => (
          <g key={line}>
            <path className="stage-leader-halo" d={line} />
            <path className="stage-leader-line" d={line} />
            <circle className="stage-leader-dot" cx={dot.cx} cy={dot.cy} r={3.2} />
          </g>
        ))}
      </svg>
    </>
  );
}
