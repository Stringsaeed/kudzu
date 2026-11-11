//! Kudzu Lucide glyphs as [`rune::Icon`] statics.
//!
//! One static per unique `lucide-react` import used by the Tauri UI (`components/`
//! and `app/`). `FooIcon` is folded onto `Foo`. Bodies are copied from lucide-react
//! 1.66.0. Lucide's default SVG attributes live on a wrapping `<g>` so Rune inherits
//! stroke paint the way the SVG root does.
//!
//! Folded aliases: CheckIcon, ChevronDownIcon, ChevronRightIcon, ChevronUpIcon, CircleCheckIcon, InfoIcon, Loader2Icon, OctagonXIcon, SearchIcon, TriangleAlertIcon, XIcon
//!
//! ## lucide-react imports
//!
//! ```imports
//! AlignLeft
//! ArrowDownUp
//! ArrowLeft
//! ArrowRight
//! ArrowUpRight
//! BarChart3
//! Bold
//! Braces
//! CalendarDays
//! Check
//! ChevronDown
//! ChevronRight
//! ChevronUp
//! CircleCheck
//! CircleHelp
//! CircleOff
//! Clipboard
//! ClipboardList
//! Clock
//! Clock3
//! Copy
//! CornerDownLeft
//! CornerDownRight
//! Dices
//! Download
//! ExternalLink
//! EyeOff
//! File
//! FileText
//! Files
//! Fingerprint
//! Folder
//! FolderInput
//! FolderOpen
//! FolderPlus
//! Globe
//! History
//! Image
//! ImagePlus
//! Info
//! Italic
//! KeyRound
//! Keyboard
//! Layers
//! Link
//! List
//! ListChecks
//! ListOrdered
//! ListX
//! Loader2
//! Lock
//! Moon
//! MoreHorizontal
//! MousePointer2
//! OctagonX
//! Package
//! Palette
//! PanelLeft
//! Pause
//! Pencil
//! Pin
//! PinOff
//! Play
//! Plus
//! RefreshCw
//! RemoveFormatting
//! RotateCcw
//! Search
//! SearchX
//! Settings
//! Settings2
//! ShieldAlert
//! ShieldCheck
//! ShieldOff
//! SlidersHorizontal
//! Sparkles
//! Star
//! Sun
//! TextCursor
//! TextCursorInput
//! TextSelect
//! Timer
//! Trash2
//! TriangleAlert
//! Type
//! Underline
//! Upload
//! X
//! ```
//!
//! ## Lucide licence (ISC)
//!
//! ISC License
//!
//! Copyright (c) 2026 Lucide Icons and Contributors
//!
//! Permission to use, copy, modify, and/or distribute this software for any
//! purpose with or without fee is hereby granted, provided that the above
//! copyright notice and this permission notice appear in all copies.
//!
//! THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
//! WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
//! ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
//! WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
//! ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
//! OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//!
//! Some Lucide glyphs are derived from Feather. lucide-react 1.66.0's LICENSE
//! reproduces that notice, kept here with the copied glyph data:
//!
//! The MIT License (MIT)
//!
//! Copyright (c) 2013-present Cole Bemis
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use rune::Icon;

/// Wrap a Lucide inner SVG with the default root paint attributes.
macro_rules! lucide {
    ($name:literal, $body:literal) => {
        Icon {
            name: $name,
            view: 24.0,
            body: concat!(
                r#"<g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
                $body,
                "</g>"
            ),
        }
    };
}

pub static ALIGN_LEFT: Icon = lucide!(
    "align-left",
    r#"<path d="M21 5H3"/><path d="M15 12H3"/><path d="M17 19H3"/>"#
);

pub static ARROW_DOWN_UP: Icon = lucide!(
    "arrow-down-up",
    r#"<path d="m3 16 4 4 4-4"/><path d="M7 20V4"/><path d="m21 8-4-4-4 4"/><path d="M17 4v16"/>"#
);

pub static ARROW_LEFT: Icon = lucide!(
    "arrow-left",
    r#"<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>"#
);

pub static ARROW_RIGHT: Icon = lucide!(
    "arrow-right",
    r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#
);

pub static ARROW_UP_RIGHT: Icon = lucide!(
    "arrow-up-right",
    r#"<path d="M7 7h10v10"/><path d="M7 17 17 7"/>"#
);

pub static BAR_CHART_3: Icon = lucide!(
    "bar-chart-3",
    r#"<path d="M3 3v16a2 2 0 0 0 2 2h16"/><path d="M18 17V9"/><path d="M13 17V5"/><path d="M8 17v-3"/>"#
);

pub static BOLD: Icon = lucide!(
    "bold",
    r#"<path d="M6 12h9a4 4 0 010 8H7a1 1 0 01-1-1V5a1 1 0 011-1h7a4 4 0 010 8"/>"#
);

pub static BRACES: Icon = lucide!(
    "braces",
    r#"<path d="M8 3H7a2 2 0 0 0-2 2v5a2 2 0 0 1-2 2 2 2 0 0 1 2 2v5c0 1.1.9 2 2 2h1"/><path d="M16 21h1a2 2 0 0 0 2-2v-5c0-1.1.9-2 2-2a2 2 0 0 1-2-2V5a2 2 0 0 0-2-2h-1"/>"#
);

pub static CALENDAR_DAYS: Icon = lucide!(
    "calendar-days",
    r#"<path d="M8 2v3"/><path d="M16 2v3"/><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18"/><path d="M8 13h.01"/><path d="M12 13h.01"/><path d="M16 13h.01"/><path d="M8 17h.01"/><path d="M12 17h.01"/><path d="M16 17h.01"/>"#
);

pub static CHECK: Icon = lucide!("check", r#"<path d="M20 6 9 17l-5-5"/>"#);

pub static CHEVRON_DOWN: Icon = lucide!("chevron-down", r#"<path d="m6 9 6 6 6-6"/>"#);

pub static CHEVRON_RIGHT: Icon = lucide!("chevron-right", r#"<path d="m9 18 6-6-6-6"/>"#);

pub static CHEVRON_UP: Icon = lucide!("chevron-up", r#"<path d="m18 15-6-6-6 6"/>"#);

pub static CIRCLE_CHECK: Icon = lucide!(
    "circle-check",
    r#"<circle cx="12" cy="12" r="10"/><path d="m16 9-5.5 5.5L8 12"/>"#
);

pub static CIRCLE_HELP: Icon = lucide!(
    "circle-help",
    r#"<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>"#
);

pub static CIRCLE_OFF: Icon = lucide!(
    "circle-off",
    r#"<path d="m2 2 20 20"/><path d="M8.35 2.69A10 10 0 0 1 21.3 15.65"/><path d="M19.08 19.08A10 10 0 1 1 4.92 4.92"/>"#
);

pub static CLIPBOARD: Icon = lucide!(
    "clipboard",
    r#"<rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>"#
);

pub static CLIPBOARD_LIST: Icon = lucide!(
    "clipboard-list",
    r#"<rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><path d="M12 11h4"/><path d="M12 16h4"/><path d="M8 11h.01"/><path d="M8 16h.01"/>"#
);

pub static CLOCK: Icon = lucide!(
    "clock",
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#
);

pub static CLOCK_3: Icon = lucide!(
    "clock-3",
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6h4"/>"#
);

pub static COPY: Icon = lucide!(
    "copy",
    r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#
);

pub static CORNER_DOWN_LEFT: Icon = lucide!(
    "corner-down-left",
    r#"<path d="M20 4v7a4 4 0 0 1-4 4H4"/><path d="m9 10-5 5 5 5"/>"#
);

pub static CORNER_DOWN_RIGHT: Icon = lucide!(
    "corner-down-right",
    r#"<path d="m15 10 5 5-5 5"/><path d="M4 4v7a4 4 0 0 0 4 4h12"/>"#
);

pub static DICES: Icon = lucide!(
    "dices",
    r#"<rect width="12" height="12" x="2" y="10" rx="2" ry="2"/><path d="m17.92 14 3.5-3.5a2.24 2.24 0 0 0 0-3l-5-4.92a2.24 2.24 0 0 0-3 0L10 6"/><path d="M6 18h.01"/><path d="M10 14h.01"/><path d="M15 6h.01"/><path d="M18 9h.01"/>"#
);

pub static DOWNLOAD: Icon = lucide!(
    "download",
    r#"<path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>"#
);

pub static EXTERNAL_LINK: Icon = lucide!(
    "external-link",
    r#"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"#
);

pub static EYE_OFF: Icon = lucide!(
    "eye-off",
    r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>"#
);

pub static FILE: Icon = lucide!(
    "file",
    r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/>"#
);

pub static FILE_TEXT: Icon = lucide!(
    "file-text",
    r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>"#
);

pub static FILES: Icon = lucide!(
    "files",
    r#"<path d="M15 2h-4a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V8"/><path d="M16.706 2.706A2.4 2.4 0 0 0 15 2v5a1 1 0 0 0 1 1h5a2.4 2.4 0 0 0-.706-1.706z"/><path d="M5 7a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 1.732-1"/>"#
);

pub static FINGERPRINT: Icon = lucide!(
    "fingerprint",
    r#"<path d="M12 10a2 2 0 0 0-2 2c0 1.02-.1 2.51-.26 4"/><path d="M14 13.12c0 2.38 0 6.38-1 8.88"/><path d="M17.29 21.02c.12-.6.43-2.3.5-3.02"/><path d="M2 12a10 10 0 0 1 18-6"/><path d="M2 16h.01"/><path d="M21.8 16c.2-2 .131-5.354 0-6"/><path d="M5 19.5C5.5 18 6 15 6 12a6 6 0 0 1 .34-2"/><path d="M8.65 22c.21-.66.45-1.32.57-2"/><path d="M9 6.8a6 6 0 0 1 9 5.2v2"/>"#
);

pub static FOLDER: Icon = lucide!(
    "folder",
    r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#
);

pub static FOLDER_INPUT: Icon = lucide!(
    "folder-input",
    r#"<path d="M2 9V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-1"/><path d="M2 13h10"/><path d="m9 16 3-3-3-3"/>"#
);

pub static FOLDER_OPEN: Icon = lucide!(
    "folder-open",
    r#"<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>"#
);

pub static FOLDER_PLUS: Icon = lucide!(
    "folder-plus",
    r#"<path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#
);

pub static GLOBE: Icon = lucide!(
    "globe",
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"#
);

pub static HISTORY: Icon = lucide!(
    "history",
    r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>"#
);

pub static IMAGE: Icon = lucide!(
    "image",
    r#"<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>"#
);

pub static IMAGE_PLUS: Icon = lucide!(
    "image-plus",
    r#"<path d="M16 5h6"/><path d="M19 2v6"/><path d="M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/><circle cx="9" cy="9" r="2"/>"#
);

pub static INFO: Icon = lucide!(
    "info",
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>"#
);

pub static ITALIC: Icon = lucide!(
    "italic",
    r#"<line x1="19" x2="10" y1="4" y2="4"/><line x1="14" x2="5" y1="20" y2="20"/><line x1="15" x2="9" y1="4" y2="20"/>"#
);

pub static KEY_ROUND: Icon = lucide!(
    "key-round",
    r#"<path d="M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z"/><circle cx="16.5" cy="7.5" r=".5" fill="currentColor"/>"#
);

pub static KEYBOARD: Icon = lucide!(
    "keyboard",
    r#"<path d="M10 8h.01"/><path d="M12 12h.01"/><path d="M14 8h.01"/><path d="M16 12h.01"/><path d="M18 8h.01"/><path d="M6 8h.01"/><path d="M7 16h10"/><path d="M8 12h.01"/><rect width="20" height="16" x="2" y="4" rx="2"/>"#
);

pub static LAYERS: Icon = lucide!(
    "layers",
    r#"<path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z"/><path d="M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12"/><path d="M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17"/>"#
);

pub static LINK: Icon = lucide!(
    "link",
    r#"<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>"#
);

pub static LIST: Icon = lucide!(
    "list",
    r#"<path d="M3 5h.01"/><path d="M3 12h.01"/><path d="M3 19h.01"/><path d="M8 5h13"/><path d="M8 12h13"/><path d="M8 19h13"/>"#
);

pub static LIST_CHECKS: Icon = lucide!(
    "list-checks",
    r#"<path d="M13 5h8"/><path d="M13 12h8"/><path d="M13 19h8"/><path d="m3 17 2 2 4-4"/><path d="m3 7 2 2 4-4"/>"#
);

pub static LIST_ORDERED: Icon = lucide!(
    "list-ordered",
    r#"<path d="M11 5h10"/><path d="M11 12h10"/><path d="M11 19h10"/><path d="M4 4h1v5"/><path d="M4 9h2"/><path d="M6.5 20H3.4c0-1 2.6-1.925 2.6-3.5a1.5 1.5 0 0 0-2.6-1.02"/>"#
);

pub static LIST_X: Icon = lucide!(
    "list-x",
    r#"<path d="M16 5H3"/><path d="M11 12H3"/><path d="M16 19H3"/><path d="m15.5 9.5 5 5"/><path d="m20.5 9.5-5 5"/>"#
);

pub static LOADER_2: Icon = lucide!("loader-2", r#"<path d="M21 12a9 9 0 1 1-6.219-8.56"/>"#);

pub static LOCK: Icon = lucide!(
    "lock",
    r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"#
);

pub static MOON: Icon = lucide!(
    "moon",
    r#"<path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/>"#
);

pub static MORE_HORIZONTAL: Icon = lucide!(
    "more-horizontal",
    r#"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"#
);

pub static MOUSE_POINTER_2: Icon = lucide!(
    "mouse-pointer-2",
    r#"<path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/>"#
);

pub static OCTAGON_X: Icon = lucide!(
    "octagon-x",
    r#"<path d="m15 9-6 6"/><path d="M2.586 16.726A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2h6.624a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586z"/><path d="m9 9 6 6"/>"#
);

pub static PACKAGE: Icon = lucide!(
    "package",
    r#"<path d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z"/><path d="M12 22V12"/><polyline points="3.29 7 12 12 20.71 7"/><path d="m7.5 4.27 9 5.15"/>"#
);

pub static PALETTE: Icon = lucide!(
    "palette",
    r#"<path d="M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.25a1.75 1.75 0 0 0-1.4 2.8l.3.4a1.75 1.75 0 0 1-1.4 2.8z"/><circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/><circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/>"#
);

pub static PANEL_LEFT: Icon = lucide!(
    "panel-left",
    r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>"#
);

pub static PAUSE: Icon = lucide!(
    "pause",
    r#"<rect x="14" y="3" width="5" height="18" rx="1"/><rect x="5" y="3" width="5" height="18" rx="1"/>"#
);

pub static PENCIL: Icon = lucide!(
    "pencil",
    r#"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/>"#
);

pub static PIN: Icon = lucide!(
    "pin",
    r#"<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>"#
);

pub static PIN_OFF: Icon = lucide!(
    "pin-off",
    r#"<path d="M12 17v5"/><path d="M15 9.34V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H7.89"/><path d="m2 2 20 20"/><path d="M9 9v1.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h11"/>"#
);

pub static PLAY: Icon = lucide!(
    "play",
    r#"<path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"/>"#
);

pub static PLUS: Icon = lucide!("plus", r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#);

pub static REFRESH_CW: Icon = lucide!(
    "refresh-cw",
    r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"#
);

pub static REMOVE_FORMATTING: Icon = lucide!(
    "remove-formatting",
    r#"<path d="M4 7V4h16v3"/><path d="M5 20h6"/><path d="M13 4 8 20"/><path d="m15 15 5 5"/><path d="m20 15-5 5"/>"#
);

pub static ROTATE_CCW: Icon = lucide!(
    "rotate-ccw",
    r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>"#
);

pub static SEARCH: Icon = lucide!(
    "search",
    r#"<path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>"#
);

pub static SEARCH_X: Icon = lucide!(
    "search-x",
    r#"<path d="m13.5 8.5-5 5"/><path d="m8.5 8.5 5 5"/><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#
);

pub static SETTINGS: Icon = lucide!(
    "settings",
    r#"<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>"#
);

pub static SETTINGS_2: Icon = lucide!(
    "settings-2",
    r#"<path d="M14 17H5"/><path d="M19 7h-9"/><circle cx="17" cy="17" r="3"/><circle cx="7" cy="7" r="3"/>"#
);

pub static SHIELD_ALERT: Icon = lucide!(
    "shield-alert",
    r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="M12 8v4"/><path d="M12 16h.01"/>"#
);

pub static SHIELD_CHECK: Icon = lucide!(
    "shield-check",
    r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>"#
);

pub static SHIELD_OFF: Icon = lucide!(
    "shield-off",
    r#"<path d="m2 2 20 20"/><path d="M5 5a1 1 0 0 0-1 1v7c0 5 3.5 7.5 7.67 8.94a1 1 0 0 0 .67.01c2.35-.82 4.48-1.97 5.9-3.71"/><path d="M9.309 3.652A12.252 12.252 0 0 0 11.24 2.28a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1v7a9.784 9.784 0 0 1-.08 1.264"/>"#
);

pub static SLIDERS_HORIZONTAL: Icon = lucide!(
    "sliders-horizontal",
    r#"<path d="M10 5H3"/><path d="M12 19H3"/><path d="M14 3v4"/><path d="M16 17v4"/><path d="M21 12h-9"/><path d="M21 19h-5"/><path d="M21 5h-7"/><path d="M8 10v4"/><path d="M8 12H3"/>"#
);

pub static SPARKLES: Icon = lucide!(
    "sparkles",
    r#"<path d="M11.017 2.814a1 1 0 0 1 1.966 0l1.051 5.558a2 2 0 0 0 1.594 1.594l5.558 1.051a1 1 0 0 1 0 1.966l-5.558 1.051a2 2 0 0 0-1.594 1.594l-1.051 5.558a1 1 0 0 1-1.966 0l-1.051-5.558a2 2 0 0 0-1.594-1.594l-5.558-1.051a1 1 0 0 1 0-1.966l5.558-1.051a2 2 0 0 0 1.594-1.594z"/><path d="M20 2v4"/><path d="M22 4h-4"/><circle cx="4" cy="20" r="2"/>"#
);

pub static STAR: Icon = lucide!(
    "star",
    r#"<path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/>"#
);

pub static SUN: Icon = lucide!(
    "sun",
    r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>"#
);

pub static TEXT_CURSOR: Icon = lucide!(
    "text-cursor",
    r#"<path d="M17 22h-1a4 4 0 0 1-4-4V6a4 4 0 0 1 4-4h1"/><path d="M7 22h1a4 4 0 0 0 4-4"/><path d="M7 2h1a4 4 0 0 1 4 4"/>"#
);

pub static TEXT_CURSOR_INPUT: Icon = lucide!(
    "text-cursor-input",
    r#"<path d="M12 20h-1a2 2 0 0 1-2-2 2 2 0 0 1-2 2H6"/><path d="M13 8h7a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2h-7"/><path d="M5 16H4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2h1"/><path d="M6 4h1a2 2 0 0 1 2 2 2 2 0 0 1 2-2h1"/><path d="M9 6v12"/>"#
);

pub static TEXT_SELECT: Icon = lucide!(
    "text-select",
    r#"<path d="M14 21h1"/><path d="M14 3h1"/><path d="M19 3a2 2 0 0 1 2 2"/><path d="M21 14v1"/><path d="M21 19a2 2 0 0 1-2 2"/><path d="M21 9v1"/><path d="M3 14v1"/><path d="M3 9v1"/><path d="M5 21a2 2 0 0 1-2-2"/><path d="M5 3a2 2 0 0 0-2 2"/><path d="M7 12h10"/><path d="M7 16h6"/><path d="M7 8h8"/><path d="M9 21h1"/><path d="M9 3h1"/>"#
);

pub static TIMER: Icon = lucide!(
    "timer",
    r#"<line x1="10" x2="14" y1="2" y2="2"/><line x1="12" x2="15" y1="14" y2="11"/><circle cx="12" cy="14" r="8"/>"#
);

pub static TRASH_2: Icon = lucide!(
    "trash-2",
    r#"<path d="M10 11v6"/><path d="M14 11v6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"#
);

pub static TRIANGLE_ALERT: Icon = lucide!(
    "triangle-alert",
    r#"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#
);

pub static TYPE: Icon = lucide!(
    "type",
    r#"<path d="M12 4v16"/><path d="M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2"/><path d="M9 20h6"/>"#
);

pub static UNDERLINE: Icon = lucide!(
    "underline",
    r#"<path d="M6 4v6a6 6 0 0 0 12 0V4"/><line x1="4" x2="20" y1="20" y2="20"/>"#
);

pub static UPLOAD: Icon = lucide!(
    "upload",
    r#"<path d="M12 3v12"/><path d="m17 8-5-5-5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>"#
);

pub static X: Icon = lucide!("x", r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#);

pub const ALL: &[&Icon] = &[
    &ALIGN_LEFT,
    &ARROW_DOWN_UP,
    &ARROW_LEFT,
    &ARROW_RIGHT,
    &ARROW_UP_RIGHT,
    &BAR_CHART_3,
    &BOLD,
    &BRACES,
    &CALENDAR_DAYS,
    &CHECK,
    &CHEVRON_DOWN,
    &CHEVRON_RIGHT,
    &CHEVRON_UP,
    &CIRCLE_CHECK,
    &CIRCLE_HELP,
    &CIRCLE_OFF,
    &CLIPBOARD,
    &CLIPBOARD_LIST,
    &CLOCK,
    &CLOCK_3,
    &COPY,
    &CORNER_DOWN_LEFT,
    &CORNER_DOWN_RIGHT,
    &DICES,
    &DOWNLOAD,
    &EXTERNAL_LINK,
    &EYE_OFF,
    &FILE,
    &FILE_TEXT,
    &FILES,
    &FINGERPRINT,
    &FOLDER,
    &FOLDER_INPUT,
    &FOLDER_OPEN,
    &FOLDER_PLUS,
    &GLOBE,
    &HISTORY,
    &IMAGE,
    &IMAGE_PLUS,
    &INFO,
    &ITALIC,
    &KEY_ROUND,
    &KEYBOARD,
    &LAYERS,
    &LINK,
    &LIST,
    &LIST_CHECKS,
    &LIST_ORDERED,
    &LIST_X,
    &LOADER_2,
    &LOCK,
    &MOON,
    &MORE_HORIZONTAL,
    &MOUSE_POINTER_2,
    &OCTAGON_X,
    &PACKAGE,
    &PALETTE,
    &PANEL_LEFT,
    &PAUSE,
    &PENCIL,
    &PIN,
    &PIN_OFF,
    &PLAY,
    &PLUS,
    &REFRESH_CW,
    &REMOVE_FORMATTING,
    &ROTATE_CCW,
    &SEARCH,
    &SEARCH_X,
    &SETTINGS,
    &SETTINGS_2,
    &SHIELD_ALERT,
    &SHIELD_CHECK,
    &SHIELD_OFF,
    &SLIDERS_HORIZONTAL,
    &SPARKLES,
    &STAR,
    &SUN,
    &TEXT_CURSOR,
    &TEXT_CURSOR_INPUT,
    &TEXT_SELECT,
    &TIMER,
    &TRASH_2,
    &TRIANGLE_ALERT,
    &TYPE,
    &UNDERLINE,
    &UPLOAD,
    &X,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded_imports(src: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut in_block = false;
        for line in src.lines() {
            let Some(rest) = line.strip_prefix("//!") else {
                if in_block {
                    break;
                }
                continue;
            };
            let rest = rest.trim();
            if rest == "```imports" {
                in_block = true;
                continue;
            }
            if in_block && rest == "```" {
                break;
            }
            if in_block && !rest.is_empty() {
                out.push(rest);
            }
        }
        out
    }

    fn pascal_to_kebab(name: &str) -> String {
        let chars: Vec<char> = name.chars().collect();
        let mut out = String::new();
        for (i, &c) in chars.iter().enumerate() {
            if i > 0 {
                let prev = chars[i - 1];
                let next = chars.get(i + 1).copied();
                let before_upper = c.is_ascii_uppercase()
                    && (prev.is_ascii_lowercase()
                        || prev.is_ascii_digit()
                        || (prev.is_ascii_uppercase()
                            && next.is_some_and(|n| n.is_ascii_lowercase())));
                let before_digit = c.is_ascii_digit() && prev.is_ascii_alphabetic();
                if before_upper || before_digit {
                    out.push('-');
                }
            }
            out.push(c.to_ascii_lowercase());
        }
        out
    }

    #[test]
    fn statics_match_recorded_imports() {
        let src = include_str!("icons.rs");
        let from_header: Vec<String> = recorded_imports(src)
            .into_iter()
            .map(pascal_to_kebab)
            .collect();
        let from_statics: Vec<String> = ALL.iter().map(|icon| icon.name.to_string()).collect();
        assert_eq!(
            from_header, from_statics,
            "header import list must match ALL"
        );
        let mut unique = from_statics.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            from_statics.len(),
            "icon names must be unique"
        );
    }

    #[test]
    fn every_icon_rasterises_non_blank_at_16pt() {
        // 16 pt at 1× is 16 device pixels; this is the stricter size the gallery draws.
        for icon in ALL {
            let cov = rune::text::icon_coverage(icon, 16);
            assert_eq!(cov.len(), 16 * 16, "{} coverage size", icon.name);
            assert!(
                cov.iter().any(|&c| c != 0),
                "{} rasterised blank at 16 px",
                icon.name
            );
        }
    }
}
