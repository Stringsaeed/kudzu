export function IconDefinitions() {
  return (
    <svg className="icon-definitions" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <symbol id="i-menu" viewBox="0 0 24 24">
        <path d="M4 6h16M4 12h16M4 18h16" />
      </symbol>
      <symbol id="i-close" viewBox="0 0 24 24">
        <path d="m6 6 12 12M6 18 18 6" />
      </symbol>
      <symbol id="i-arrow" viewBox="0 0 24 24">
        <path d="M5 12h14m-6-6 6 6-6 6" />
      </symbol>
      <symbol id="i-download" viewBox="0 0 24 24">
        <path d="M12 3v12m-5-5 5 5 5-5M5 16v4h14v-4" />
      </symbol>
      <symbol id="i-copy" viewBox="0 0 24 24">
        <rect x="8" y="8" width="12" height="12" rx="2" />
        <path d="M16 8V4H4v12h4" />
      </symbol>
      <symbol id="i-search" viewBox="0 0 24 24">
        <circle cx="10.5" cy="10.5" r="6.5" />
        <path d="m16 16 5 5" />
      </symbol>
      <symbol id="i-check" viewBox="0 0 24 24">
        <path d="m5 12 4 4L19 6" />
      </symbol>
      <symbol id="i-star" viewBox="0 0 24 24">
        <path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3l-5.6 2.9 1.1-6.2L3 9.6l6.2-.9Z" />
      </symbol>
      <symbol id="i-grid" viewBox="0 0 24 24">
        <rect x="4" y="4" width="6" height="6" rx="1" />
        <rect x="14" y="4" width="6" height="6" rx="1" />
        <rect x="4" y="14" width="6" height="6" rx="1" />
        <rect x="14" y="14" width="6" height="6" rx="1" />
      </symbol>
      <symbol id="i-folder" viewBox="0 0 24 24">
        <path d="M3 7V5h7l2 3h9v12H3Z" />
      </symbol>
      <symbol id="i-lock" viewBox="0 0 24 24">
        <rect x="5" y="10" width="14" height="11" rx="2" />
        <path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" />
      </symbol>
      <symbol id="i-arrow-down" viewBox="0 0 24 24">
        <path d="M12 5v14m-6-6 6 6 6-6" />
      </symbol>
      <symbol id="i-link" viewBox="0 0 24 24">
        <path d="M7 17 17 7M9 7h8v8" />
      </symbol>
      <symbol id="i-message" viewBox="0 0 24 24">
        <path d="M4 5h16v11H9l-5 4Z" />
      </symbol>
      <symbol id="i-pen" viewBox="0 0 24 24">
        <path d="m4 20 1-5L16 4l4 4L9 19Zm9.5-13.5 4 4" />
      </symbol>
      <symbol id="i-clock" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="8.5" />
        <path d="M12 7v5l3.5 2" />
      </symbol>
      <symbol id="i-heart" viewBox="0 0 24 24">
        <path d="M12 20S4 15 4 9.2A4.2 4.2 0 0 1 12 7.4a4.2 4.2 0 0 1 8 1.8C20 15 12 20 12 20Z" />
      </symbol>
      <symbol id="i-reset" viewBox="0 0 24 24">
        <path d="M5 12a7 7 0 1 0 2.2-5.1L4.5 9.5M4.5 4.5v5h5" />
      </symbol>
      <symbol id="i-plus" viewBox="0 0 24 24">
        <path d="M12 5v14M5 12h14" />
      </symbol>
      <symbol id="i-sparkle" viewBox="0 0 24 24">
        <path d="M12 4v16M4 12h16M6.5 6.5l11 11m0-11-11 11" />
      </symbol>
      <symbol id="i-sun" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="4" />
        <path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" />
      </symbol>
      <symbol id="i-moon" viewBox="0 0 24 24">
        <path d="M20.5 13a8.5 8.5 0 0 1-9.5-9.5A8.5 8.5 0 1 0 20.5 13Z" />
      </symbol>
      <symbol id="i-monitor" viewBox="0 0 24 24">
        <rect x="3" y="4" width="18" height="13" rx="2" />
        <path d="M8 21h8m-4-4v4" />
      </symbol>
      <symbol id="i-play" viewBox="0 0 24 24">
        <path d="M7 4.5v15l12-7.5Z" />
      </symbol>
      <symbol id="i-send" viewBox="0 0 24 24">
        <path d="M12 15V3m-5 5 5-5 5 5M5 13v8h14v-8" />
      </symbol>
      <symbol id="i-command" viewBox="0 0 24 24">
        <path d="M8 8H5a3 3 0 1 1 3-3v14a3 3 0 1 1-3-3h14a3 3 0 1 1-3 3V5a3 3 0 1 1 3 3H8Z" />
      </symbol>
    </svg>
  );
}

export function Icon({ name, className = "" }: { name: string; className?: string }) {
  return (
    <svg className={`icon ${className}`} aria-hidden="true">
      <use href={`#i-${name}`} />
    </svg>
  );
}
